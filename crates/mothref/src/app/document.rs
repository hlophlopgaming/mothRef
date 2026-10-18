// SPDX-License-Identifier: GPL-3.0-or-later
//! A tab owns its journal and workers; switching tabs never swaps persistence identities.
use super::*;
pub(super) const MAX_TABS: usize = 4;
/// Per-tab camera and viewport state, independent of scene and journal identity.
#[derive(Clone, Copy)]
pub(super) struct View {
    pub camera: Camera,
    pub focus_return: Option<(Uuid, Camera)>,
    pub needs_fit: bool,
    pub pan: bool,
    pub viewport: egui::Rect,
}
impl Default for View {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            focus_return: None,
            needs_fit: false,
            pan: true,
            viewport: egui::Rect::NOTHING,
        }
    }
}
impl View {
    pub(super) fn fit(&mut self, scene: &Scene) {
        self.needs_fit = true;
        if !self.viewport.is_finite() || !self.viewport.is_positive() {
            return;
        }
        if let Some(bounds) = scene.bounds() {
            self.camera.zoom = ((self.viewport.width() - 48.) / bounds.width)
                .min((self.viewport.height() - 48.) / bounds.height)
                .clamp(0.01, 64.);
            self.camera.origin = [
                bounds.x + bounds.width / 2. - self.viewport.center().x / self.camera.zoom,
                bounds.y + bounds.height / 2. - self.viewport.center().y / self.camera.zoom,
            ];
        }
        self.needs_fit = false;
    }
}
pub(super) struct Document {
    pub(super) view: View,
    ui_id: Uuid,
    storage: Arc<moth_platform::DesktopStorage>,
    decode: moth_io::DecodePool,
    clipboard: moth_platform::ClipboardWorker,
    pub(super) scene: Scene,
    boards: moth_platform::boards::Worker,
    pub(super) in_flight: usize,
    pub(super) pasting: usize,
    pub(super) path: String,
    pub(super) dirty: bool,
    pub(super) close_after_save: bool,
    pub(super) locked: bool,
    pub(super) grayscale: bool,
    pub(super) selection: HashSet<Uuid>,
    pub(super) sources: HashMap<BlobId, Handle>,
    pub(super) pending: HashMap<Handle, Pending>,
    pub(super) failed: HashSet<Handle>,
    pub(super) waiting: Vec<Handle>,
    pub(super) message: String,
    pub(super) discarding: bool,
    pub(super) discarded: bool,
    pub(super) batching: bool,
    /// Item ids inserted in the current import/paste burst, cleared when it drains.
    pub(super) burst: Vec<Uuid>,
    /// Ids of a finished burst awaiting an arrangement choice while the prompt is shown.
    pub(super) arrange_prompt: Option<Vec<Uuid>>,
}
impl Document {
    pub(super) fn boards(&self) -> &moth_platform::boards::Worker {
        &self.boards
    }

    pub(super) fn clipboard(&self) -> &moth_platform::ClipboardWorker {
        &self.clipboard
    }

    pub(super) fn decode(&self) -> &moth_io::DecodePool {
        &self.decode
    }

    pub(super) fn storage(&self) -> &Arc<moth_platform::DesktopStorage> {
        &self.storage
    }

    pub(super) fn ui_id(&self) -> Uuid {
        self.ui_id
    }

    pub(super) fn visible(
        &mut self,
        registry: &moth_render::Registry,
        library: bool,
        pixels_per_point: f32,
    ) -> Vec<&Item> {
        let p = self
            .view
            .camera
            .world([self.view.viewport.min.x, self.view.viewport.min.y]);
        let visible = self.scene.query(Rect {
            x: p[0],
            y: p[1],
            width: if library {
                0.
            } else {
                self.view.viewport.width() / self.view.camera.zoom
            },
            height: self.view.viewport.height() / self.view.camera.zoom,
        });
        for item in &visible {
            let Some(blob) = item.blob() else {
                continue;
            };
            let side = moth_render::preview_side(
                item.rect.width.max(item.rect.height) * self.view.camera.zoom * pixels_per_point,
            );
            if !registry.resident(blob, side)
                && let Some(&handle) = self.sources.get(&blob)
                && !self.pending.contains_key(&handle)
                && !self.failed.contains(&handle)
                && self
                    .decode
                    .requests
                    .try_send(moth_io::Request {
                        handle,
                        side,
                        format: registry.image_format(),
                    })
                    .is_ok()
            {
                self.pending.insert(handle, Pending::Preview(blob));
            }
        }
        visible
    }
    pub(super) fn cursor_point(&self, cursor: Option<[f32; 2]>, fallback: [f32; 2]) -> [f32; 2] {
        let point = cursor
            .filter(|p| {
                self.view.viewport.is_finite()
                    && self.view.viewport.contains(egui::pos2(p[0], p[1]))
            })
            .unwrap_or_else(|| {
                if self.view.viewport.is_finite() {
                    [self.view.viewport.center().x, self.view.viewport.center().y]
                } else {
                    fallback
                }
            });
        self.view.camera.world(point)
    }

    pub(super) fn new(wake: Arc<dyn Fn() + Send + Sync>, discover: bool) -> anyhow::Result<Self> {
        let storage = Arc::new(moth_platform::DesktopStorage::default());
        Self::with_storage(storage, wake, discover)
    }
    fn with_storage(
        storage: Arc<moth_platform::DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
        discover: bool,
    ) -> anyhow::Result<Self> {
        let decode = moth_io::DecodePool::new(storage.clone(), wake.clone())?;
        let clipboard = moth_platform::ClipboardWorker::new(storage.clone(), wake.clone())?;
        let boards = if discover {
            moth_platform::boards::Worker::new(storage.clone(), wake)?
        } else {
            moth_platform::boards::Worker::new_tab(storage.clone(), wake)?
        };
        Ok(Self {
            view: View::default(),
            ui_id: Uuid::now_v7(),
            storage,
            decode,
            clipboard,
            scene: Scene::default(),
            boards,
            in_flight: 0,
            pasting: 0,
            path: String::new(),
            dirty: false,
            close_after_save: false,
            locked: false,
            grayscale: false,
            selection: HashSet::new(),
            sources: HashMap::new(),
            pending: HashMap::new(),
            failed: HashSet::new(),
            waiting: Vec::new(),
            message: String::new(),
            discarding: false,
            discarded: false,
            batching: false,
            burst: Vec::new(),
            arrange_prompt: None,
        })
    }
    pub(super) fn fit(&mut self) {
        self.view.fit(&self.scene);
    }
    pub(super) fn idle(&self) -> bool {
        self.pasting == 0
            && self.in_flight == 0
            && self.pending.is_empty()
            && self.waiting.is_empty()
            && !self.discarding
    }
    pub(super) fn title(&self) -> &str {
        std::path::Path::new(&self.path)
            .file_stem()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or(moth_ui::text(moth_ui::Text::Untitled))
    }
    /// Enqueue a journaled save; the caller resolves the path first.
    pub(super) fn save(&mut self) {
        match self
            .boards
            .requests
            .try_send(moth_platform::boards::Request::Save(
                std::path::PathBuf::from(&self.path),
            )) {
            Ok(()) => self.in_flight += 1,
            Err(_) => self.message = moth_ui::text(moth_ui::Text::StorageBusy).into(),
        }
    }
    /// Begin discarding staged originals and the journal; guarded by in-flight
    /// work so a close never loses journaled but unsaved edits.
    pub(super) fn discard(&mut self) {
        if self.in_flight > 0
            || self.pasting > 0
            || !self.pending.is_empty()
            || !self.waiting.is_empty()
        {
            self.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
            return;
        }
        if !self.discarding {
            match self
                .clipboard
                .requests
                .try_send(moth_platform::ClipboardRequest::Discard)
            {
                Ok(()) => self.discarding = true,
                Err(_) => self.message = moth_ui::text(moth_ui::Text::ClipboardBusy).into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parked_tabs_keep_separate_journals_and_resume_their_own_scene() {
        let root = std::env::temp_dir().join(format!("mothref-tabs-{}", Uuid::now_v7()));
        let mut first = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let second = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let (source, image) = first
            .storage
            .stage_preview(1, 1, &[12, 34, 56, 255])
            .unwrap();
        let item = Item::image(
            image.hash,
            Rect {
                x: 10.,
                y: 20.,
                width: 30.,
                height: 40.,
            },
            None,
        );
        let id = item.id;
        first
            .boards
            .requests
            .send(moth_platform::boards::Request::Apply(
                Command::Insert(item),
                Some(source),
            ))
            .unwrap();
        match first
            .boards
            .results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        {
            moth_platform::boards::Completion::Applied(command, _) => {
                first.scene.apply(command).unwrap();
            }
            _ => panic!("insert must be acknowledged after journaling"),
        }
        assert!(second.scene.is_empty());
        first.fit();
        assert!(first.view.needs_fit);
        assert!(first.view.camera.origin.iter().all(|v| v.is_finite()));
        first.view.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300., 200.));
        first.fit();
        assert!(!first.view.needs_fit);
        let center = first.view.camera.screen([25., 40.]);
        assert!((center[0] - 150.).abs() < 0.01 && (center[1] - 100.).abs() < 0.01);
        let original_identity = first.ui_id;
        let mut workspace = workspace::Workspace::new(first, Arc::new(|| {}));
        workspace.tabs.push(second);
        assert!(matches!(
            workspace.activate(1),
            workspace::TabChange::Switched
        ));
        assert!(workspace.board.scene.is_empty());
        assert!(workspace.tabs[0].scene.get(id).is_some());
        assert_eq!(workspace.tabs[0].ui_id, original_identity);
        workspace.tabs[0]
            .boards()
            .requests
            .send(moth_platform::boards::Request::Undo)
            .unwrap();
        workspace.tabs[0].in_flight += 1;
        let deadline = Instant::now() + Duration::from_secs(10);
        while workspace.tabs[0].scene.get(id).is_some() {
            workspace.pump(workspace::Pump {
                format: moth_io::ImageFormat::default(),
                cursor: None,
                fallback: [0., 0.],
                auto_arrange: false,
                threshold: 10,
                upload: &mut |_| panic!("parked tab must not upload"),
                events: Vec::new(),
            });
            assert!(Instant::now() < deadline, "parked undo must complete");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(workspace.board.scene.is_empty());
        assert!(matches!(
            workspace.activate(1),
            workspace::TabChange::Switched
        ));
        assert_eq!(workspace.board.ui_id, original_identity);
        assert!(workspace.board.scene.is_empty());
        workspace.board.boards.finish().unwrap();
        workspace.tabs.pop().unwrap().boards.finish().unwrap();
        // Decode/clipboard handles keep no files open after their owning sessions are dropped.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn request_close_preserves_an_unsaved_board_until_confirmed() {
        let root = std::env::temp_dir().join(format!("mothref-close-request-{}", Uuid::now_v7()));
        let mut board = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let item = Item::image(
            BlobId::from([7; 32]),
            Rect {
                x: 0.,
                y: 0.,
                width: 30.,
                height: 40.,
            },
            None,
        );
        let id = item.id;
        board.scene.apply(Command::Insert(item)).unwrap();
        board.dirty = true;
        let mut workspace = workspace::Workspace::new(board, Arc::new(|| {}));
        let identity = workspace.board.ui_id;
        assert!(matches!(
            workspace.request_close(true),
            workspace::TabChange::None
        ));
        assert!(workspace.confirm_close && workspace.closing_window);
        assert!(!workspace.board.discarding && !workspace.board.discarded);
        assert!(workspace.board.scene.get(id).is_some());
        assert_eq!(workspace.board.ui_id, identity);
        assert!(matches!(workspace.activate(0), workspace::TabChange::None));
        workspace.board.boards.finish().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_and_close_never_discards_before_the_save_completes() {
        let root = std::env::temp_dir().join(format!("mothref-close-{}", Uuid::now_v7()));
        let save_path = root.join("board.moth");
        let mut session = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();

        // SaveAndClose on a board with a path enqueues the save and marks the close.
        session.path = save_path.to_string_lossy().into_owned();
        session.close_after_save = true;
        session.save();
        assert_eq!(session.in_flight, 1);

        // Discard must not drop the in-flight save: the guard loses nothing.
        session.discard();
        assert!(!session.discarding);
        assert!(!session.discarded);
        assert!(!session.message.is_empty());

        // Route the actual save acknowledgment through the workspace, including discard.
        let mut workspace = workspace::Workspace::new(session, Arc::new(|| {}));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !workspace.board.discarded {
            workspace.pump(workspace::Pump {
                format: moth_io::ImageFormat::default(),
                cursor: None,
                fallback: [0., 0.],
                auto_arrange: false,
                threshold: 10,
                upload: &mut |_| panic!("empty board has no images"),
                events: Vec::new(),
            });
            assert!(Instant::now() < deadline, "save and discard must complete");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(workspace.board.in_flight, 0);
        assert!(!workspace.board.dirty);
        assert!(save_path.is_file());
        workspace.board.boards.finish().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
