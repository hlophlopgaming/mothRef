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
        self.fit_bounds(scene.bounds());
    }
    pub(super) fn fit_selection(&mut self, scene: &Scene, selection: &HashSet<Uuid>) {
        let bounds = selection.iter().filter_map(|id| scene.get(*id)).fold(
            egui::Rect::NOTHING,
            |bounds, item| {
                let e = item.rect.rotated_envelope(item.transform.rotation);
                bounds.union(egui::Rect::from_min_max(
                    egui::pos2(e.lower()[0], e.lower()[1]),
                    egui::pos2(e.upper()[0], e.upper()[1]),
                ))
            },
        );
        self.fit_bounds(if bounds.is_positive() {
            Some(Rect {
                x: bounds.min.x,
                y: bounds.min.y,
                width: bounds.width(),
                height: bounds.height(),
            })
        } else {
            scene.bounds()
        });
    }
    pub(super) fn fit_bounds(&mut self, bounds: Option<Rect>) {
        self.needs_fit = true;
        if !self.viewport.is_finite() || !self.viewport.is_positive() {
            return;
        }
        if let Some(bounds) = bounds {
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
fn request_priority(
    a: &(moth_io::Request, f32, f32),
    b: &(moth_io::Request, f32, f32),
) -> std::cmp::Ordering {
    b.1.total_cmp(&a.1)
        .then_with(|| a.2.total_cmp(&b.2))
        .then(a.0.handle.0.cmp(&b.0.handle.0))
        .then(a.0.tile.cmp(&b.0.tile))
        .then(a.0.side.cmp(&b.0.side))
}
pub(super) struct Document {
    pub(super) view: View,
    pub(super) motion: super::item_motion::Items,
    ui_id: Uuid,
    storage: Arc<moth_platform::DesktopStorage>,
    decode: moth_io::DecodePool,
    clipboard: moth_platform::ClipboardWorker,
    pub(super) scene: Scene,
    pub(super) image_count: usize,
    boards: moth_platform::boards::Worker,
    pub(super) in_flight: usize,
    pub(super) loading: bool,
    pub(super) loading_bounds: Option<Rect>,
    pub(super) pasting: usize,
    pub(super) path: String,
    pub(super) dirty: bool,
    pub(super) close_after_save: bool,
    pub(super) locked: bool,
    pub(super) grayscale: bool,
    pub(super) selection: HashSet<Uuid>,
    pub(super) sources: HashMap<BlobId, Handle>,
    /// Board state of the last whole-board bake offer, so unchanged boards offer nothing.
    board_bake: Option<(usize, moth_io::BakeMode, bool)>,
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
    /// Opening a scene recounts once; normal edits maintain the count incrementally.
    pub(super) fn replace_scene(&mut self, scene: Scene) {
        self.image_count = scene.items().filter(|item| item.blob().is_some()).count();
        self.scene = scene;
    }
    /// Called only for commands acknowledged by the journal worker.
    pub(super) fn apply_visual(
        &mut self,
        command: Command,
    ) -> Result<Command, moth_scene::SceneError> {
        let id = match &command {
            Command::Insert(item) => Some(item.id),
            Command::Remove(id) | Command::SetRect { id, .. } => Some(*id),
            _ => None,
        };
        let image_delta: isize = match &command {
            Command::Insert(item) if item.blob().is_some() => 1,
            Command::Remove(id)
                if self
                    .scene
                    .get(*id)
                    .is_some_and(|item| item.blob().is_some()) =>
            {
                -1
            }
            _ => 0,
        };
        let now = Instant::now();
        let before = id.and_then(|id| {
            self.motion
                .get(id, now)
                .or_else(|| self.scene.get(id).cloned())
        });
        let inverse = self.scene.apply(command)?;
        self.image_count = self.image_count.saturating_add_signed(image_delta);
        if let Some(id) = id {
            self.motion.record(before, self.scene.get(id).cloned(), now);
        }
        Ok(inverse)
    }
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

    pub(super) fn visible(&self, library: bool) -> Vec<&Item> {
        let p = self
            .view
            .camera
            .world([self.view.viewport.min.x, self.view.viewport.min.y]);
        self.scene.query(Rect {
            x: p[0],
            y: p[1],
            width: if library {
                0.
            } else {
                self.view.viewport.width() / self.view.camera.zoom
            },
            height: self.view.viewport.height() / self.view.camera.zoom,
        })
    }
    /// Queues the board's originals for background tile baking when they or the bake
    /// preference change; the pool ignores it unless whole-board baking is selected.
    pub(super) fn offer_board_bakes(&mut self, format: moth_io::ImageFormat) {
        let cache = self.decode.cache();
        let state = (self.sources.len(), cache.bake_mode(), cache.bakes());
        if self.board_bake == Some(state) {
            return;
        }
        self.board_bake = Some(state);
        self.decode.offer_board(
            format,
            self.sources.iter().map(|(&hash, &handle)| (handle, hash)),
        );
    }
    /// Images of this board waiting for a background tile bake.
    pub(super) fn bake_pending(&self) -> usize {
        self.decode.bake_pending()
    }
    pub(super) fn request_images(
        &self,
        registry: &mut moth_render::Registry,
        visible: &[&Item],
        library: bool,
        pixels_per_point: f32,
        target: Camera,
    ) {
        if library {
            self.decode.requests.schedule([]);
            registry.retain_tiles(&HashSet::new());
            return;
        }
        let viewport = self.view.viewport;
        registry.tile_detail = self.decode.cache().mode() != moth_io::CacheMode::MemoryLowQuality;
        let screen = [viewport.max.x.max(1.), viewport.max.y.max(1.)];
        let clip = [
            viewport.min.x,
            viewport.min.y,
            viewport.max.x,
            viewport.max.y,
        ];
        let mut wanted_tiles = HashSet::new();
        let mut candidates =
            HashMap::<(BlobId, Option<moth_io::tiles::Tile>), (moth_io::Request, f32, f32)>::new();
        let mut scheduling = visible.to_vec();
        if !library
            && (target.zoom != self.view.camera.zoom || target.origin != self.view.camera.origin)
        {
            let ids: HashSet<_> = visible.iter().map(|item| item.id).collect();
            let min = target.world([viewport.min.x, viewport.min.y]);
            scheduling.extend(
                self.scene
                    .query(Rect {
                        x: min[0],
                        y: min[1],
                        width: viewport.width() / target.zoom,
                        height: viewport.height() / target.zoom,
                    })
                    .into_iter()
                    .filter(|item| !ids.contains(&item.id)),
            );
        }
        registry.fit_tile_budget(
            &scheduling,
            [self.view.camera, target],
            screen,
            clip,
            pixels_per_point,
        );
        for item in &scheduling {
            let Some(blob) = item.blob() else { continue };
            let Some(&handle) = self.sources.get(&blob) else {
                continue;
            };
            if self.failed.contains(&handle) {
                continue;
            }
            let mut side = moth_render::preview_side(
                item.rect.width.max(item.rect.height) * target.zoom * pixels_per_point,
            )
            .min(self.decode.cache().max_side());
            let original = registry.original_dimensions(blob);
            if original.is_none()
                || (registry.tile_detail
                    && original.is_some_and(|dims| dims[0].max(dims[1]) > moth_io::MAX_SIDE))
            {
                side = side.min(moth_io::tiles::SIDE);
            }
            let envelope = item.rect.rotated_envelope(item.transform.rotation);
            let min = target.screen(envelope.lower());
            let max = target.screen(envelope.upper());
            let rect =
                egui::Rect::from_min_max(egui::pos2(min[0], min[1]), egui::pos2(max[0], max[1]));
            let area = rect.intersect(viewport).area().max(0.);
            let distance = rect.center().distance_sq(viewport.center());
            let request = moth_io::Request {
                tile: None,
                handle,
                side,
                format: registry.image_format(),
                blob: Some(blob),
            };
            if !registry.resident(blob, side) {
                candidates
                    .entry((blob, None))
                    .and_modify(|entry| {
                        entry.0.side = entry.0.side.max(side);
                        entry.1 = entry.1.max(area);
                        entry.2 = entry.2.min(distance);
                    })
                    .or_insert((request, area, distance));
            }
            for camera in [self.view.camera, target] {
                for tile in registry.visible_tiles(item, camera, screen, clip, pixels_per_point) {
                    let key = tile.key(blob);
                    wanted_tiles.insert(key);
                    // Coarser tiles stay resident to stand in while this one loads.
                    for levels in 1..=moth_render::PARENT_LEVELS {
                        if let Some(parent) = tile.parent(levels) {
                            wanted_tiles.insert(parent.key(blob));
                        }
                    }
                    if camera.origin != target.origin
                        || camera.zoom != target.zoom
                        || registry.resident(key, moth_io::tiles::SIDE)
                    {
                        continue;
                    }
                    let Some([x, y, right, bottom]) =
                        registry.tile_screen_bounds(item, tile, target, screen)
                    else {
                        continue;
                    };
                    let rect =
                        egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(right, bottom));
                    let area = rect.intersect(viewport).area().max(0.);
                    let distance = rect.center().distance_sq(viewport.center());
                    candidates.entry((blob, Some(tile))).or_insert((
                        moth_io::Request {
                            tile: Some(tile),
                            side: moth_io::tiles::SIDE,
                            ..request
                        },
                        area,
                        distance,
                    ));
                }
            }
        }
        registry.retain_tiles(&wanted_tiles);
        let mut candidates: Vec<_> = candidates.into_values().collect();
        candidates.sort_by(request_priority);
        self.decode
            .requests
            .schedule(candidates.into_iter().map(|entry| entry.0));
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
        let decode = moth_io::DecodePool::with_cache(
            storage.clone(),
            wake.clone(),
            moth_io::LodCache::shared(),
        )?;
        let clipboard = moth_platform::ClipboardWorker::new(storage.clone(), wake.clone())?;
        let boards = if discover {
            moth_platform::boards::Worker::new(storage.clone(), wake)?
        } else {
            moth_platform::boards::Worker::new_tab(storage.clone(), wake)?
        };
        Ok(Self {
            view: View::default(),
            motion: super::item_motion::Items::default(),
            ui_id: Uuid::now_v7(),
            storage,
            decode,
            clipboard,
            scene: Scene::default(),
            image_count: 0,
            boards,
            in_flight: 0,
            loading: false,
            loading_bounds: None,
            pasting: 0,
            path: String::new(),
            dirty: false,
            close_after_save: false,
            locked: false,
            grayscale: false,
            selection: HashSet::new(),
            sources: HashMap::new(),
            board_bake: None,
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
        if self.loading_bounds.is_some() {
            self.view.fit_bounds(self.loading_bounds);
        } else {
            self.view.fit(&self.scene);
        }
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
        if self.loading {
            self.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
            return;
        }
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
    #[test]
    fn request_priority_prefers_visible_area_then_center_with_stable_ties() {
        let request = |handle| moth_io::Request {
            tile: None,
            handle: Handle(handle),
            side: 2048,
            format: moth_io::ImageFormat::Rgba8,
            blob: Some(BlobId::from_bytes([1; 32])),
        };
        let mut candidates = [
            (request(1), 10., 0.),
            (request(2), 20., 100.),
            (request(4), 20., 25.),
            (request(3), 20., 25.),
        ];
        candidates.sort_by(request_priority);
        assert_eq!(candidates.map(|entry| entry.0.handle.0), [3, 4, 2, 1]);
    }
    #[test]
    fn selection_fit_uses_rotated_bounds_and_empty_selection_fits_board() {
        let mut scene = super::Scene::default();
        let mut selected = moth_scene::Item::text(
            "selected",
            super::Rect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 50.,
            },
            None,
        );
        selected.transform.rotation = std::f32::consts::FRAC_PI_2;
        let id = selected.id;
        scene.apply(super::Command::Insert(selected)).unwrap();
        scene
            .apply(super::Command::Insert(moth_scene::Item::text(
                "other",
                super::Rect {
                    x: 500.,
                    y: 500.,
                    width: 200.,
                    height: 200.,
                },
                None,
            )))
            .unwrap();
        let mut view = super::View {
            viewport: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
            ..super::View::default()
        };
        view.fit_selection(&scene, &std::collections::HashSet::from([id]));
        let center = view.camera.screen([50., 25.]);
        assert!((center[0] - 400.).abs() < 0.001 && (center[1] - 300.).abs() < 0.001);
        assert!((view.camera.zoom - 5.52).abs() < 0.001);
        view.fit_selection(&scene, &std::collections::HashSet::new());
        let camera = view.camera;
        view.fit(&scene);
        assert_eq!(view.camera.origin, camera.origin);
        assert_eq!(view.camera.zoom, camera.zoom);
    }
    use super::*;
    #[test]
    fn batch_edits_show_affected_items_and_undo_together() {
        let root = std::env::temp_dir().join(format!("mothref-batch-{}", Uuid::now_v7()));
        let board = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let mut workspace = workspace::Workspace::new(board, Arc::new(|| {}));
        let items: Vec<_> = (0..3)
            .map(|n| {
                Item::text(
                    "note",
                    Rect {
                        x: n as f32 * 30.,
                        y: 0.,
                        width: 20.,
                        height: 10.,
                    },
                    None,
                )
            })
            .collect();
        let drain = |workspace: &mut workspace::Workspace| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while workspace.board.in_flight > 0 {
                workspace.pump(workspace::Pump {
                    format: moth_io::ImageFormat::default(),
                    cursor: None,
                    fallback: [0.; 2],
                    auto_arrange: false,
                    threshold: 10,
                    upload: &mut |_| {},
                    events: Vec::new(),
                });
                assert!(Instant::now() < deadline, "batch must be acknowledged");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        workspace.persist_batch(
            items
                .iter()
                .cloned()
                .map(|i| (Command::Insert(i), None))
                .collect(),
        );
        drain(&mut workspace);
        workspace.board.selection.clear();
        workspace.persist_batch(
            items
                .iter()
                .take(2)
                .map(|i| {
                    (
                        Command::SetRect {
                            id: i.id,
                            rect: Rect { y: 50., ..i.rect },
                        },
                        None,
                    )
                })
                .collect(),
        );
        drain(&mut workspace);
        assert_eq!(
            workspace.board.selection,
            items.iter().take(2).map(|i| i.id).collect()
        );
        assert_eq!(workspace.board.scene.get(items[2].id), Some(&items[2]));
        workspace.persist(moth_platform::boards::Request::Undo);
        drain(&mut workspace);
        for item in &items {
            assert_eq!(workspace.board.scene.get(item.id), Some(item));
        }
        workspace.persist(moth_platform::boards::Request::Redo);
        drain(&mut workspace);
        for item in &items[..2] {
            assert_eq!(workspace.board.scene.get(item.id).unwrap().rect.y, 50.);
        }
        workspace.board.boards.finish().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn image_count_tracks_successful_commands_undo_and_opened_scenes() {
        let root = std::env::temp_dir().join(format!("mothref-image-count-{}", Uuid::now_v7()));
        let mut board = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 40.,
            height: 60.,
        };
        let image = Item::image(BlobId::from_bytes([1; 32]), rect, None);
        let remove = board.apply_visual(Command::Insert(image.clone())).unwrap();
        assert_eq!(board.image_count, 1);
        board
            .apply_visual(Command::Insert(Item::text("note", rect, None)))
            .unwrap();
        assert_eq!(board.image_count, 1);
        assert!(board.apply_visual(Command::Insert(image.clone())).is_err());
        assert_eq!(board.image_count, 1);
        let insert = board.apply_visual(remove).unwrap();
        assert_eq!(board.image_count, 0);
        board.apply_visual(insert).unwrap();
        assert_eq!(board.image_count, 1);
        let mut opened = Scene::default();
        opened.apply(Command::Insert(image)).unwrap();
        board.replace_scene(opened);
        assert_eq!(board.image_count, 1);
        board.replace_scene(Scene::default());
        assert_eq!(board.image_count, 0);
        drop(board);
        let _ = std::fs::remove_dir_all(root);
    }
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
        workspace.library = false;
        assert!(
            matches!(workspace.activate(0), workspace::TabChange::None),
            "closing the active tab must not reset its image residency before confirmation"
        );
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
