// SPDX-License-Identifier: GPL-3.0-or-later
//! A tab owns its journal and workers; switching tabs never swaps persistence identities.
use super::*;
pub(super) const MAX_TABS: usize = 4;
/// Per-tab camera and viewport state, independent of scene and journal identity.
#[derive(Clone, Copy)]
pub(super) struct View {
    pub compact: bool,
    pub camera: Camera,
    pub focus_return: Option<(Uuid, Camera)>,
    pub needs_fit: bool,
    pub viewport: egui::Rect,
    pub safe_area: egui::Rect,
    pub selection_bar_height: f32,
    pub selection_bar_visible: bool,
}
impl Default for View {
    fn default() -> Self {
        Self {
            compact: false,
            camera: Camera::default(),
            focus_return: None,
            needs_fit: false,
            viewport: egui::Rect::NOTHING,
            safe_area: egui::Rect::NOTHING,
            selection_bar_height: 0.,
            selection_bar_visible: false,
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
        let visible = self.selection_bar_visible;
        self.selection_bar_visible = !selection.is_empty();
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
        self.selection_bar_visible = visible;
    }
    pub(super) fn fit_bounds(&mut self, bounds: Option<Rect>) {
        self.needs_fit = true;
        if !self.viewport.is_finite() || !self.viewport.is_positive() {
            return;
        }
        let mut area = if self.safe_area.is_positive() {
            self.safe_area
        } else {
            self.viewport
        };
        if self.selection_bar_visible {
            area.min.y += self.selection_bar_height;
        }
        if let Some(bounds) = bounds {
            self.camera.zoom = ((area.width() - 48.) / bounds.width)
                .min((area.height() - 48.) / bounds.height)
                .clamp(0.01, 64.);
            self.camera.origin = [
                bounds.x + bounds.width / 2. - area.center().x / self.camera.zoom,
                bounds.y + bounds.height / 2. - area.center().y / self.camera.zoom,
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
pub(super) struct Added {
    pub ids: Vec<Uuid>,
    pub mode: super::arrange::Arrange,
    pub needs_fit: bool,
}

pub(super) struct Document {
    pub(super) view: View,
    pub(super) appearance: super::window_mode::Appearance,
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
    pub(super) conversion: Option<(std::path::PathBuf, usize)>,
    pub(super) conversion_prompt: bool,
    pub(super) restoring: Option<std::path::PathBuf>,
    pub(super) dirty: bool,
    pub(super) close_after_save: bool,
    pub(super) locked: bool,
    pub(super) grayscale: bool,
    pub(super) selection: HashSet<Uuid>,
    pub(super) sources: HashMap<BlobId, Handle>,
    /// Board state of the last whole-board bake offer, so unchanged boards offer nothing.
    board_bake: Option<(usize, moth_io::BakeMode, bool)>,
    /// Originals offered to the full-resolution preload; `None` until the first offer.
    preload_offered: Option<usize>,
    /// The board exceeded the video memory limit: the rest loads on demand.
    pub(super) preload_overflow: bool,
    /// The initial preload finished; later additions load without covering the board.
    pub(super) preload_ready: bool,
    pub(super) pending: HashMap<Handle, Pending>,
    pub(super) failed: HashSet<Handle>,
    pub(super) waiting: Vec<Handle>,
    pub(super) message: moth_ui::Message,
    pub(super) export_message: Option<moth_ui::Message>,
    pub(super) exporting: bool,
    pub(super) discarding: bool,
    pub(super) discarded: bool,
    pub(super) batching: bool,
    /// Item ids inserted in the current import/paste burst, cleared when it drains.
    pub(super) burst: Vec<Uuid>,
    /// Ids of a finished burst awaiting an arrangement choice while the prompt is shown.
    pub(super) arrange_prompt: Option<Vec<Uuid>>,
    /// One import notification; its layouts share the insertion undo group until dismissed.
    pub(super) added: Option<Added>,
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

    pub(super) fn presented(&self, item: &Item) -> bool {
        self.appearance.presents(self.view.compact, item)
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
    /// Offers the board's originals to the full-resolution preload whenever new ones
    /// arrive; already offered images are skipped by the pool.
    pub(super) fn offer_preload(&mut self, format: moth_io::ImageFormat) {
        if self.preload_overflow || self.preload_offered == Some(self.sources.len()) {
            return;
        }
        self.preload_offered = Some(self.sources.len());
        if let Err(error) = self.decode.preload(
            format,
            self.sources.iter().map(|(&hash, &handle)| (handle, hash)),
        ) {
            self.message = error.to_string().into();
        }
    }
    /// Whether a full-resolution preload was offered since the last reset.
    pub(super) fn preload_started(&self) -> bool {
        self.preload_offered.is_some()
    }
    /// Starts preloading from scratch after video memory was cleared or the mode changed.
    pub(super) fn reset_preload(&mut self) {
        self.preload_offered = None;
        self.preload_overflow = false;
        self.preload_ready = false;
        self.decode.restart_preload();
    }
    /// Marks the initial preload done once the board finished loading and every
    /// offered image was prepared.
    pub(super) fn settle_preload(&mut self) {
        if !self.loading
            && self.preload_offered.is_some()
            && !self.preload_progress().active()
            && self.decode.preloaded().is_empty()
        {
            self.preload_ready = true;
        }
    }
    /// Stops preloading because the next image would exceed the video memory limit.
    pub(super) fn overflow_preload(&mut self) {
        self.preload_overflow = true;
        self.preload_ready = true;
        self.decode.cancel_preload();
        self.message = moth_ui::Text::PreloadOverflow.into();
    }
    /// Images of the current preload pass prepared so far.
    pub(super) fn preload_progress(&self) -> moth_io::PreloadProgress {
        self.decode.preload_progress()
    }
    /// The next prepared image, if any.
    pub(super) fn next_preloaded(&self) -> Option<moth_io::Decoded> {
        self.decode.preloaded().try_recv().ok()
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
        floating: &[(&Item, Camera, [f32; 2])],
    ) {
        if library && floating.is_empty() {
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
                    && original.is_some_and(|dims| dims[0].max(dims[1]) > registry.tile_threshold))
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
        for &(item, camera, screen) in floating {
            let Some(blob) = item.blob() else { continue };
            let Some(&handle) = self.sources.get(&blob) else {
                continue;
            };
            if self.failed.contains(&handle) {
                continue;
            }
            let mut side =
                moth_render::preview_side(item.rect.width.max(item.rect.height) * camera.zoom)
                    .min(self.decode.cache().max_side());
            if registry.original_dimensions(blob).is_none() || registry.tile_detail {
                side = side.min(moth_io::tiles::SIDE);
            }
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
                        entry.1 = entry.1.max(screen[0] * screen[1]);
                        entry.2 = 0.;
                    })
                    .or_insert((request, screen[0] * screen[1], 0.));
            }
            for tile in
                registry.visible_tiles(item, camera, screen, [0., 0., screen[0], screen[1]], 1.)
            {
                let key = tile.key(blob);
                wanted_tiles.insert(key);
                for levels in 1..=moth_render::PARENT_LEVELS {
                    if let Some(parent) = tile.parent(levels) {
                        wanted_tiles.insert(parent.key(blob));
                    }
                }
                if !registry.resident(key, moth_io::tiles::SIDE) {
                    candidates.entry((blob, Some(tile))).or_insert((
                        moth_io::Request {
                            tile: Some(tile),
                            side: moth_io::tiles::SIDE,
                            ..request
                        },
                        screen[0] * screen[1],
                        0.,
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
            appearance: super::window_mode::Appearance::default(),
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
            conversion: None,
            conversion_prompt: false,
            restoring: None,
            dirty: false,
            close_after_save: false,
            locked: false,
            grayscale: false,
            selection: HashSet::new(),
            sources: HashMap::new(),
            board_bake: None,
            preload_offered: None,
            preload_overflow: false,
            preload_ready: false,
            pending: HashMap::new(),
            failed: HashSet::new(),
            waiting: Vec::new(),
            message: moth_ui::Message::default(),
            export_message: None,
            exporting: false,
            discarding: false,
            discarded: false,
            batching: false,
            burst: Vec::new(),
            arrange_prompt: None,
            added: None,
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
        let path = if self.path.is_empty() {
            self.conversion
                .as_ref()
                .map(|(path, _)| path.as_path())
                .unwrap_or(std::path::Path::new(&self.path))
        } else {
            std::path::Path::new(&self.path)
        };
        path.file_stem()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or(moth_ui::text(moth_ui::Text::Untitled))
    }
    /// Enqueue a journaled save; the caller resolves the path first.
    pub(super) fn save(&mut self) {
        if self.loading {
            self.message = moth_ui::Text::StorageBusy.into();
            return;
        }
        match self
            .boards
            .requests
            .try_send(moth_platform::boards::Request::Save(
                std::path::PathBuf::from(&self.path),
            )) {
            Ok(()) => self.in_flight += 1,
            Err(_) => self.message = moth_ui::Text::StorageBusy.into(),
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
            self.message = moth_ui::Text::StorageBusy.into();
            return;
        }
        if !self.discarding {
            match self
                .clipboard
                .requests
                .try_send(moth_platform::ClipboardRequest::Discard)
            {
                Ok(()) => self.discarding = true,
                Err(_) => self.message = moth_ui::Text::ClipboardBusy.into(),
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
    fn settings_columns_stack_their_controls_after_resize() {
        use super::super::settings::{Settings, SettingsWindow};
        let root = std::env::temp_dir().join(format!("mothref-settings-{}", Uuid::now_v7()));
        let board = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::LIGHT);
        let mut window = SettingsWindow::default();
        window.open = true;
        let mut settings = Settings::default();
        let mut tools = super::super::tools::Tools::default();
        let enabled = vec![true; super::super::bindings::ACTIONS.len()];
        for size in [egui::vec2(1440., 900.), egui::vec2(900., 600.)] {
            for frame in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        window.show(ui, &mut settings, &mut tools, &board, "", &enabled);
                    },
                );
                output.textures_delta.clear();
                // egui measures a newly opened window before painting its labels.
                if frame == 0 {
                    continue;
                }
                let position = |label| {
                    output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(t) if t.galley.text() == moth_ui::text(label) => {
                                Some(t.pos)
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| {
                            panic!("settings label {label:?} is missing at {size:?}, frame {frame}")
                        })
                };
                let general = position(moth_ui::Text::GeneralSettings);
                let appearance = position(moth_ui::Text::AppearanceSettings);
                assert!((general.x - appearance.x).abs() < 1.);
                assert!(appearance.y > general.y);
                let language = position(moth_ui::Text::Language);
                let reset = position(moth_ui::Text::ResetSection);
                assert!(reset.y > language.y);
                output.textures_delta.clear();
            }
        }
        board.boards.finish().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frames_inside_chrome() {
        let mut view = super::View {
            viewport: egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(900., 600.)),
            safe_area: egui::Rect::from_min_max(egui::pos2(0., 60.), egui::pos2(900., 520.)),
            selection_bar_height: 48.,
            selection_bar_visible: true,
            ..super::View::default()
        };
        view.fit_bounds(Some(super::Rect {
            x: 0.,
            y: 0.,
            width: 800.,
            height: 600.,
        }));
        let top = view.camera.screen([0., 0.]);
        let bottom = view.camera.screen([800., 600.]);
        assert!(top[1] >= 108. && bottom[1] <= 520.);
        assert!((view.camera.screen([400., 300.])[1] - 314.).abs() < 0.001);
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
    fn import_layout_choices_share_one_undo_and_fit_only_after_acknowledgment() {
        let root = std::env::temp_dir().join(format!("mothref-import-feedback-{}", Uuid::now_v7()));
        let board = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let mut workspace = workspace::Workspace::new(board, Arc::new(|| {}));
        workspace.board.view.viewport =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900., 600.));
        fn drain(workspace: &mut workspace::Workspace) {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                workspace.pump(workspace::Pump {
                    format: moth_io::ImageFormat::Rgba8,
                    cursor: None,
                    fallback: [0.; 2],
                    upload: &mut |_| {},
                    events: Vec::new(),
                });
                if workspace.board.idle() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "journal acknowledgment must arrive"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let base = Item::text(
            "existing",
            Rect {
                x: -100.,
                y: -100.,
                width: 50.,
                height: 50.,
            },
            None,
        );
        workspace.persist_batch(vec![(Command::Insert(base.clone()), None)]);
        drain(&mut workspace);
        let a = Item::text(
            "a",
            Rect {
                x: 200.,
                y: 200.,
                width: 200.,
                height: 100.,
            },
            Some(&base.z),
        );
        let b = Item::text(
            "b",
            Rect {
                x: 220.,
                y: 220.,
                width: 100.,
                height: 200.,
            },
            Some(&a.z),
        );
        let ids = vec![a.id, b.id];
        workspace.begin_batch();
        workspace.persist_batch(vec![
            (Command::Insert(a.clone()), None),
            (Command::Insert(b.clone()), None),
        ]);
        workspace.board.added = Some(Added {
            ids: ids.clone(),
            mode: arrange::Arrange::Horizontal,
            needs_fit: true,
        });
        drain(&mut workspace);
        for mode in [arrange::Arrange::Horizontal, arrange::Arrange::Vertical] {
            let items: Vec<_> = ids
                .iter()
                .map(|id| workspace.board.scene.get(*id).unwrap())
                .collect();
            let commands = arrange::counted(&items, mode, 1, 24.);
            let camera = workspace.board.view.camera;
            workspace.board.added = None;
            workspace.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
            workspace.board.added = Some(Added {
                ids: ids.clone(),
                mode,
                needs_fit: true,
            });
            assert!(workspace.board.added.as_ref().unwrap().needs_fit);
            assert_eq!(workspace.board.view.camera.origin, camera.origin);
            drain(&mut workspace);
            assert!(!workspace.board.added.as_ref().unwrap().needs_fit);
            let mut expected = workspace.board.view;
            expected.fit_selection(&workspace.board.scene, &ids.iter().copied().collect());
            assert_eq!(workspace.board.view.camera.origin, expected.camera.origin);
            assert_eq!(workspace.board.view.camera.zoom, expected.camera.zoom);
        }
        let final_a = workspace.board.scene.get(a.id).unwrap().clone();
        let moved = Rect {
            x: -150.,
            ..base.rect
        };
        workspace.persist_batch(vec![(
            Command::SetRect {
                id: base.id,
                rect: moved,
            },
            None,
        )]);
        assert!(workspace.board.added.is_none());
        drain(&mut workspace);
        workspace.persist(moth_platform::boards::Request::Undo);
        drain(&mut workspace);
        assert_eq!(workspace.board.scene.get(base.id), Some(&base));
        assert!(workspace.board.scene.get(a.id).is_some());
        workspace.persist(moth_platform::boards::Request::Undo);
        drain(&mut workspace);
        assert_eq!(workspace.board.scene.len(), 1);
        assert_eq!(workspace.board.scene.get(base.id), Some(&base));
        workspace.persist(moth_platform::boards::Request::Redo);
        drain(&mut workspace);
        assert_eq!(workspace.board.scene.get(a.id), Some(&final_a));
        assert_eq!(workspace.board.scene.len(), 3);
        workspace.board.boards.finish().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_edits_and_layout_preserve_camera() {
        let root = std::env::temp_dir().join(format!("mothref-batch-{}", Uuid::now_v7()));
        let board = Document::with_storage(
            Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
            Arc::new(|| {}),
            false,
        )
        .unwrap();
        let mut workspace = workspace::Workspace::new(board, Arc::new(|| {}));
        workspace.board.view.viewport =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
        workspace.board.view.camera = Camera {
            origin: [-123., 45.],
            zoom: 0.75,
        };
        let camera = workspace.board.view.camera;
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
        assert_eq!(workspace.board.view.camera.origin, camera.origin);
        assert_eq!(workspace.board.view.camera.zoom, camera.zoom);
        assert_eq!(workspace.board.scene.get(items[2].id), Some(&items[2]));
        workspace.persist(moth_platform::boards::Request::Undo);
        drain(&mut workspace);
        assert_eq!(workspace.board.view.camera.origin, camera.origin);
        assert_eq!(workspace.board.view.camera.zoom, camera.zoom);
        for item in &items {
            assert_eq!(workspace.board.scene.get(item.id), Some(item));
        }
        workspace.persist(moth_platform::boards::Request::Redo);
        drain(&mut workspace);
        assert_eq!(workspace.board.view.camera.origin, camera.origin);
        assert_eq!(workspace.board.view.camera.zoom, camera.zoom);
        for item in &items[..2] {
            assert_eq!(workspace.board.scene.get(item.id).unwrap().rect.y, 50.);
        }
        let arranged: Vec<_> = workspace.board.scene.items().collect();
        let commands = arrange::counted(&arranged, arrange::Arrange::Horizontal, 1, 12.);
        assert!(!commands.is_empty());
        workspace.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
        drain(&mut workspace);
        assert_eq!(workspace.board.view.camera.zoom, camera.zoom);
        assert_eq!(workspace.board.view.camera.origin, camera.origin);
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
    fn foreign_conversion_offers_native_save_and_requires_close_confirmation() {
        for fixture in ["bee/tiny.bee", "pur/v2.1-redbar.pur"] {
            let root = std::env::temp_dir().join(format!("mothref-conversion-{}", Uuid::now_v7()));
            let board = Document::with_storage(
                Arc::new(moth_platform::DesktopStorage::in_directory(root.clone())),
                Arc::new(|| {}),
                false,
            )
            .unwrap();
            let mut workspace = workspace::Workspace::new(board, Arc::new(|| {}));
            let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus")
                .join(fixture);
            workspace.import_foreign(source.clone());
            let deadline = Instant::now() + Duration::from_secs(10);
            while workspace.board.loading {
                workspace.pump(workspace::Pump {
                    format: moth_io::ImageFormat::default(),
                    cursor: None,
                    fallback: [0., 0.],
                    upload: &mut |_| {},
                    events: Vec::new(),
                });
                assert!(Instant::now() < deadline, "conversion must finish");
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(workspace.board.conversion_prompt);
            assert!(workspace.board.dirty);
            assert!(workspace.board.path.is_empty());
            assert_eq!(workspace.board.conversion.as_ref().unwrap().0, source);
            assert_eq!(
                workspace.board.title(),
                source.file_stem().unwrap().to_str().unwrap()
            );
            // Continuing without saving retains the unsaved close guard.
            workspace.board.conversion_prompt = false;
            workspace.request_close(false);
            assert!(workspace.confirm_close);
            assert!(!workspace.board.discarding);
            workspace.board.boards.finish().unwrap();
            std::fs::remove_dir_all(root).unwrap();
        }
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
        assert!(!session.message.render(moth_ui::locale()).is_empty());

        // Route the actual save acknowledgment through the workspace, including discard.
        let mut workspace = workspace::Workspace::new(session, Arc::new(|| {}));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !workspace.board.discarded {
            workspace.pump(workspace::Pump {
                format: moth_io::ImageFormat::default(),
                cursor: None,
                fallback: [0., 0.],
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
