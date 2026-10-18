// SPDX-License-Identifier: GPL-3.0-or-later
//! A tab owns its journal and workers; switching tabs never swaps persistence identities.
use super::*;
pub(super) const MAX_TABS: usize = 4;
pub(super) struct Session {
    pub(super) ui_id: Uuid,
    pub(super) storage: Arc<moth_platform::DesktopStorage>,
    pub(super) decode: moth_io::DecodePool,
    pub(super) clipboard: moth_platform::ClipboardWorker,
    pub(super) scene: Scene,
    pub(super) boards: moth_platform::boards::Worker,
    pub(super) in_flight: usize,
    pub(super) pasting: usize,
    pub(super) path: String,
    pub(super) dirty: bool,
    pub(super) close_after_save: bool,
    pub(super) camera: Camera,
    pub(super) focus_return: Option<(Uuid, Camera)>,
    pub(super) needs_fit: bool,
    pub(super) pan: bool,
    pub(super) locked: bool,
    pub(super) grayscale: bool,
    pub(super) selection: HashSet<Uuid>,
    pub(super) sources: HashMap<BlobId, Handle>,
    pub(super) pending: HashMap<Handle, Pending>,
    pub(super) failed: HashSet<Handle>,
    pub(super) waiting: Vec<Handle>,
    pub(super) viewport: egui::Rect,
    pub(super) message: String,
    pub(super) discarding: bool,
    pub(super) discarded: bool,
    pub(super) batching: bool,
    /// Item ids inserted in the current import/paste burst, cleared when it drains.
    pub(super) burst: Vec<Uuid>,
    /// Ids of a finished burst awaiting an arrangement choice while the prompt is shown.
    pub(super) arrange_prompt: Option<Vec<Uuid>>,
}
impl Session {
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
            camera: Camera::default(),
            focus_return: None,
            needs_fit: false,
            pan: true,
            locked: false,
            grayscale: false,
            selection: HashSet::new(),
            sources: HashMap::new(),
            pending: HashMap::new(),
            failed: HashSet::new(),
            waiting: Vec::new(),
            viewport: egui::Rect::NOTHING,
            message: String::new(),
            discarding: false,
            discarded: false,
            batching: false,
            burst: Vec::new(),
            arrange_prompt: None,
        })
    }
    pub(super) fn fit(&mut self) {
        self.needs_fit = true;
        if !self.viewport.is_finite() || !self.viewport.is_positive() {
            return;
        }
        if let Some(bounds) = self.scene.bounds() {
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
        let mut first = Session::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let mut second = Session::with_storage(
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
        assert!(first.needs_fit);
        assert!(first.camera.origin.iter().all(|v| v.is_finite()));
        first.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300., 200.));
        first.fit();
        assert!(!first.needs_fit);
        let center = first.camera.screen([25., 40.]);
        assert!((center[0] - 150.).abs() < 0.01 && (center[1] - 100.).abs() < 0.01);
        std::mem::swap(&mut first, &mut second);
        assert!(first.scene.is_empty());
        assert!(second.scene.get(id).is_some());
        second
            .boards
            .requests
            .send(moth_platform::boards::Request::Undo)
            .unwrap();
        match second
            .boards
            .results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        {
            moth_platform::boards::Completion::Applied(command, _) => {
                second.scene.apply(command).unwrap();
            }
            _ => panic!("undo must target the parked tab's journal"),
        }
        assert!(second.scene.is_empty());
        assert!(first.scene.is_empty());
        first.boards.finish().unwrap();
        second.boards.finish().unwrap();
        // Decode/clipboard handles keep no files open after their owning sessions are dropped.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_and_close_never_discards_before_the_save_completes() {
        let root = std::env::temp_dir().join(format!("mothref-close-{}", Uuid::now_v7()));
        let save_path = root.join("board.moth");
        let mut session = Session::with_storage(
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

        // Once the save completes and in_flight drains, close_after_save discards.
        match session
            .boards
            .results
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
        {
            moth_platform::boards::Completion::Saved => {}
            _ => panic!("expected Saved completion"),
        }
        session.in_flight = session.in_flight.saturating_sub(1);
        session.discard();
        assert!(session.discarding);

        session.boards.finish().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
