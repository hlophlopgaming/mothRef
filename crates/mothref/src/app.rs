// SPDX-License-Identifier: GPL-3.0-or-later
//! Coordinates native events, intents and the independent application subsystems.
use crate::gpu::Gpu;
use input::InputState;
use moth_format::Handle;
use moth_platform::Platform;
use moth_platform::input::{Input, NativeEvent, Phase};
use moth_scene::{BlobId, Camera, Command, Item, Rect, Scene, Transform, Uuid};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};
#[derive(Debug)]
pub enum Wake {
    Shutdown,
    Global(moth_platform::shortcuts::Shortcut),
    Ready,
    Repaint(Duration),
}
enum Pending {
    Import,
    Paste,
}
/// Inline note editor state; the draft is committed (or discarded) on focus loss.
struct Editing {
    id: Uuid,
    /// Text before editing began, so an unchanged note is not rewritten.
    original: String,
    /// Live text as the user types.
    draft: String,
    /// World anchor where the editor floats, independent of scene timing.
    anchor: [f32; 2],
    font_size: f32,
    width: f32,
    wrap: bool,
    focus: bool,
}
mod actions;
mod apply;
mod arrange;
mod bindings;
mod camera_motion;
mod canvas;
mod chrome;
mod document;
mod floating;
mod gesture;
mod groups;
mod input;
mod intent;
mod item_motion;
mod library;
mod modal;
mod motion;
mod overlay;
mod panes;
mod resize;
mod selection;
mod session;
mod session_ui;
mod session_window;
mod settings;
mod shell;
mod shortcuts;
mod snap;
mod surfaces;
mod tools;
mod window_mode;
mod workspace;
use document::Document;
use intent::Intent;

pub struct App {
    memory: moth_platform::memory::Monitor,
    /// Detected adapter memory; `None` until the detection worker reports.
    video: Option<moth_platform::gpu::VideoMemory>,
    video_detection: Option<crossbeam_channel::Receiver<moth_platform::gpu::VideoMemory>>,
    cache_maintenance: moth_platform::lod_store::Maintenance,
    workspace: workspace::Workspace,
    tools: tools::Tools,
    /// Snapshots for Settings › For developers.
    diagnostics: settings::Diagnostics,
    settings: settings::Settings,
    settings_window: settings::SettingsWindow,
    shortcuts: shortcuts::Shortcuts,
    global_shortcuts: Option<moth_platform::shortcuts::Shortcuts>,
    window_preferences: std::collections::BTreeMap<String, String>,
    window_preferences_dirty: bool,
    library_screen: library::LibraryScreen,
    modal: modal::Modal,
    camera_motion: camera_motion::CameraMotion,
    camera_active: bool,
    items_active: bool,
    motion: motion::DragMotion,
    editing: Option<Editing>,
    pub startup_error: Option<anyhow::Error>,
    shell: shell::Shell,
    /// This main window's mode, pin and auxiliary windows; see `panes.rs`.
    window: panes::WindowState,
    input: InputState,
    /// What a press on the canvas does; see `gesture.rs`.
    tool: gesture::Tool,
    /// What the current press is doing, until release.
    gesture: gesture::Gesture,
    /// What the last gesture left on screen until the journal acknowledges it.
    settling: gesture::Settling,
    startup: Vec<std::path::PathBuf>,
    /// The other main windows; see `panes.rs`.
    panes: Vec<panes::Pane>,
    /// Tab moves between windows, run after the frame that requested them.
    window_ops: Vec<(WindowId, panes::WindowOp)>,
}
impl App {
    /// Canvas texture format for decode requests; RGBA8 until the GPU exists.
    fn image_format(&self) -> moth_io::ImageFormat {
        self.shell
            .gpu
            .as_ref()
            .map(|g| g.registry.image_format())
            .unwrap_or_default()
    }
    pub fn new(proxy: EventLoopProxy<Wake>) -> anyhow::Result<Self> {
        let ready = proxy.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = ready.send_event(Wake::Ready);
        });
        // Resolving the cache folder reads environment variables only; workers do the I/O.
        let disk = Arc::new(moth_platform::lod_store::DiskStore::new());
        moth_io::LodCache::shared().set_store(disk.clone());
        let cache_maintenance = moth_platform::lod_store::Maintenance::new(disk, wake.clone())?;
        let board = Document::new(wake.clone(), true)?;
        let memory = moth_platform::memory::Monitor::new(wake.clone())?;
        let library_worker = moth_platform::library::Worker::new(
            Arc::new(moth_platform::DesktopStorage::default()),
            wake.clone(),
        )?;
        let shell = shell::Shell::new(proxy, wake, std::env::args().any(|a| a == "--software"));
        Ok(Self {
            memory,
            video: None,
            video_detection: None,
            cache_maintenance,
            workspace: workspace::Workspace::new(board, shell.wake()),
            tools: tools::Tools::default(),
            diagnostics: settings::Diagnostics::default(),
            settings: settings::Settings::default(),
            settings_window: settings::SettingsWindow::default(),
            shortcuts: shortcuts::Shortcuts::default(),
            global_shortcuts: None,
            window_preferences: Default::default(),
            window_preferences_dirty: false,
            library_screen: library::LibraryScreen::new(library_worker),
            modal: modal::Modal::default(),
            camera_motion: camera_motion::CameraMotion::default(),
            camera_active: false,
            items_active: false,
            motion: motion::DragMotion::default(),
            editing: None,
            startup_error: None,
            window: panes::WindowState::new(shell.wake()),
            shell,
            input: InputState::default(),
            tool: gesture::Tool::default(),
            gesture: gesture::Gesture::default(),
            settling: gesture::Settling::default(),
            startup: std::env::args_os()
                .skip(1)
                .filter(|a| a != "--software")
                .map(std::path::PathBuf::from)
                .collect(),
            panes: Vec::new(),
            window_ops: Vec::new(),
        })
    }
    fn pick_window(&mut self) {
        self.window.applied_background = None;
        let Some(window) = &self.shell.window else {
            return;
        };
        if !moth_platform::Desktop
            .window_capabilities(window)
            .above_window
        {
            return;
        }
        let result = moth_platform::Desktop
            .pin_target(&mut self.window.pin, window, false)
            .and_then(|()| moth_platform::Desktop.window_effects(window, false, false, false));
        match result {
            Ok(()) => {
                self.cancel_drag();
                self.window.moving_window = false;
                self.window.picking_window = true;
            }
            Err(error) => self.workspace.board.notices.message = error.into(),
        }
    }
    fn cancel_window_mode(&mut self) {
        self.window.moving_window = false;
        if self.window.picking_window {
            self.window.picking_window = false;
            self.apply_pin(self.window.pinned);
        }
    }
    fn apply_pin(&mut self, enabled: bool) {
        if enabled {
            self.window.bottom = false;
        }
        self.window.applied_background = None;
        let Some(window) = &self.shell.window else {
            return;
        };
        let enabled = enabled && moth_platform::Desktop.window_capabilities(window).on_top;
        // Release the target pin first: leaving it may reset stacking that
        // above-all mode is about to set.
        let result = (|| {
            moth_platform::Desktop.pin_target(
                &mut self.window.pin,
                window,
                enabled && self.window.target_pin,
            )?;
            moth_platform::Desktop.window_effects(
                window,
                self.window.overlay.is_some(),
                enabled && !self.window.target_pin,
                false,
            )
        })();
        match result {
            Ok(()) => {
                self.window.pinned = enabled;
                if self.window.bottom {
                    moth_platform::Desktop.window_bottom(window, true);
                }
            }
            Err(error) => {
                self.window.pinned = false;
                self.workspace.board.notices.message = error.into();
            }
        }
    }
    fn toggle_overlay(&mut self, event_loop: &ActiveEventLoop) {
        self.window.applied_background = None;
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        self.cancel_window_mode();
        if self.window.overlay.is_some() {
            if let Err(error) = moth_platform::Desktop.window_effects(
                &window,
                false,
                self.window.pinned && !self.window.target_pin,
                false,
            ) {
                self.workspace.board.notices.message = error.into();
            } else {
                self.window.overlay = None;
                self.workspace.board.appearance.over = false;
            }
        } else if self.shell.gpu.as_ref().is_some_and(|gpu| gpu.transparent) {
            self.window.bottom = false;
            match overlay::Overlay::new(
                event_loop,
                self.shell.wake(),
                self.shell.software(),
                moth_platform::Desktop.window_capabilities(&window).on_top,
            ) {
                Ok(mut overlay) => {
                    if self.settings.over_hint {
                        overlay.hint = Some(format!(
                            "{} {}",
                            moth_ui::text(moth_ui::Text::OverHint),
                            self.shortcuts.registry.label(bindings::Action::Overlay)
                        ));
                    }
                    if self.workspace.board.appearance.background == 0 {
                        self.workspace.board.appearance.background = 2;
                    }
                    self.workspace.board.appearance.over = true;
                    overlay.appearance = self.workspace.board.appearance;
                    if let Err(error) =
                        moth_platform::Desktop.pin_target(&mut self.window.pin, &window, false)
                    {
                        self.workspace.board.notices.message = error.into();
                    }
                    if let Err(error) = moth_platform::Desktop
                        .window_effects(&window, true, overlay.on_top, false)
                        .and_then(|()| overlay.restack())
                    {
                        self.workspace.board.notices.message = error.into();
                    }
                    self.window.overlay = Some(overlay);
                    self.workspace.library = false;
                    self.library_screen.cards.clear();
                    self.library_screen.catalog.dismiss_editor();
                }
                Err(error) => self.workspace.board.notices.message = error.to_string().into(),
            }
        } else {
            self.workspace.board.notices.message = moth_ui::Text::OverlayUnsupported.into();
        }
        self.remember_appearance();
        if self.window.overlay.is_none() {
            self.apply_pin(self.window.pinned);
        }
        self.redraw();
    }

    fn reset_board_ui(&mut self) {
        self.window.floating.clear();
        if !self.workspace.board.intake.loading {
            self.remember_appearance();
        }
        self.tools = tools::Tools::default();
        self.library_screen.cards.clear();
        self.library_screen.catalog.dismiss_editor();
        self.editing = None;
        self.motion = motion::DragMotion::default();
        self.shell.refinements.clear();
        self.camera_motion.stop();
        self.gesture = gesture::Gesture::Idle;
        self.settling.resize = None;
        self.input.pending_focus = None;
        self.input.held.clear();
        self.input.pan_time = None;
        self.input.pointer_action = None;
        self.input.gestures.cancel();
        if let Some(gpu) = &mut self.shell.gpu {
            gpu.registry.clear();
        }
        // Video memory was cleared: the board loads again.
        self.workspace.board.reset_preload();
    }
    fn tab_change(&mut self, change: workspace::TabChange) {
        if matches!(
            change,
            workspace::TabChange::Switched | workspace::TabChange::Closed
        ) {
            self.reset_board_ui();
        }
        self.redraw();
    }
    fn activate(&mut self, index: usize) {
        let change = self.workspace.activate(index);
        self.tab_change(change);
    }
    fn new_board(&mut self) {
        let change = self.workspace.new_board();
        self.tab_change(change);
    }
    fn open_in_tab(&mut self, path: std::path::PathBuf) {
        let change = self.workspace.open_in_tab(path);
        self.tab_change(change);
    }
    fn import(&mut self, path: std::path::PathBuf) {
        let change = self.workspace.import(path, self.image_format());
        self.tab_change(change);
    }
    fn request_close(&mut self, window: bool) {
        let change = self.workspace.request_close(window);
        self.tab_change(change);
    }
    fn finish_close(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let change = self.workspace.finish_close()?;
        if matches!(change, workspace::TabChange::WindowClosed) {
            // Other windows keep running; `about_to_wait` drops this one.
            self.window.window_closed = true;
            if self.panes.is_empty() {
                event_loop.exit();
            }
        }
        self.tab_change(change);
        Ok(())
    }
    /// Whether the board is hidden behind the full-resolution preload progress.
    fn covered_by_preload(&self) -> bool {
        self.settings.performance_mode == moth_io::CacheMode::Full
            && !self.workspace.library
            && !self.workspace.board.intake.preload_ready
    }
    fn canvas_keys_blocked(&self) -> bool {
        self.modal.dialog.is_some()
            || self.workspace.board.notices.conversion_prompt
            || self.workspace.confirm_close
            || self.workspace.board.notices.arrange_prompt.is_some()
            || self.shortcuts.blocked()
            || self.editing.is_some()
            || self.shell.context().text_edit_focused()
            || egui::Popup::is_any_open(self.shell.context())
    }
    fn redraw(&self) {
        for image in &self.window.floating {
            image.window.request_redraw();
        }
        if let Some(w) = &self.shell.window {
            w.request_redraw();
        }
    }
    fn reset_renderer_ui(&mut self) {
        for image in &mut self.window.floating {
            image.surface = None;
        }
        self.input.native_pressed = None;
        // A recreated device starts with empty video memory.
        self.workspace.board.reset_preload();
        self.library_screen.cards.clear();
        self.library_screen.catalog.dismiss_editor();
        self.workspace.refresh_library = self.workspace.library;
    }
    fn recover(&mut self) -> anyhow::Result<()> {
        self.reset_renderer_ui();
        self.shell.recover()?;
        self.rebuild_window_surfaces();
        Ok(())
    }
    fn paste(&mut self) {
        if self.workspace.board.locked || self.workspace.board.intake.loading {
            return;
        }
        if self.workspace.board.file.discarding {
            return;
        }
        if !self.workspace.finish_added() {
            return;
        }
        // A new paste supersedes a pending arrangement prompt.
        self.workspace.board.notices.arrange_prompt = None;
        match self
            .workspace
            .board
            .clipboard()
            .requests
            .try_send(moth_platform::ClipboardRequest::Paste)
        {
            Ok(()) => self.workspace.board.intake.pasting += 1,
            Err(_) => self.workspace.board.notices.message = moth_ui::Text::ClipboardBusy.into(),
        }
    }

    fn open_files(&mut self, images: bool) {
        if self.workspace.confirm_close {
            return;
        }
        let dialog = rfd::FileDialog::new().set_title(moth_ui::text(if images {
            moth_ui::Text::InsertImages
        } else {
            moth_ui::Text::Open
        }));
        let dialog = if images {
            dialog.add_filter(
                moth_ui::text(moth_ui::Text::Images),
                &["png", "jpg", "jpeg", "webp", "gif", "bmp"],
            )
        } else {
            dialog
        };
        let files = dialog.pick_files().unwrap_or_default();
        for path in files {
            if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
            {
                self.open_in_tab(path);
            } else {
                if self.workspace.library {
                    self.new_board();
                }
                self.import(path);
            }
        }
    }

    /// Creates an empty note at `world` and opens the inline editor for it.
    fn add_note(&mut self, world: [f32; 2]) {
        if self.workspace.board.locked || self.workspace.board.intake.loading {
            return;
        }
        if self.workspace.board.file.discarding || !self.workspace.board.idle() {
            return;
        }
        let rect = canvas::new_note_rect(world, self.workspace.board.view.camera.zoom);
        let font_size = canvas::note_font_size(rect, "");
        let width = rect.width;
        let mut item = Item::text("", rect, self.workspace.board.scene.top());
        item.transform.note_font_size = font_size;
        let id = item.id;
        self.workspace
            .persist(moth_platform::boards::Request::Apply(
                Command::Insert(item),
                None,
            ));
        self.editing = Some(Editing {
            id,
            original: String::new(),
            draft: String::new(),
            anchor: world,
            font_size,
            width,
            wrap: false,
            focus: true,
        });
    }
    /// Opens the inline editor over an existing note.
    fn edit_note(&mut self, id: Uuid) {
        if self.workspace.board.locked || self.workspace.board.intake.loading {
            return;
        }
        let Some(item) = self.workspace.board.scene.get(id) else {
            return;
        };
        let Some(text) = item.note() else {
            return;
        };
        self.editing = Some(Editing {
            id,
            original: text.to_owned(),
            draft: text.to_owned(),
            anchor: [item.rect.x, item.rect.y],
            font_size: item.note_font_size(),
            width: item.rect.width,
            wrap: item.transform.note_wrap,
            focus: true,
        });
    }
    /// Commits or discards the inline editor's draft once it loses focus.
    fn finish_editing(&mut self) {
        let Some(editing) = self.editing.take() else {
            return;
        };
        if editing.draft == editing.original {
            return;
        }
        let mut commands = Vec::new();
        if editing.draft.trim().is_empty() {
            commands.push(Command::Remove(editing.id));
        } else {
            let [width, height] = canvas::measure_note_frame(
                self.shell.context(),
                &editing.draft,
                editing.font_size,
                editing.wrap.then_some(editing.width),
            );
            if let Some(item) = self.workspace.board.scene.get(editing.id)
                && item.transform.note_font_size == 0.
            {
                commands.push(Command::SetTransform {
                    id: editing.id,
                    transform: Transform {
                        note_font_size: editing.font_size,
                        ..item.transform
                    },
                });
            }
            commands.push(Command::SetText {
                id: editing.id,
                text: editing.draft,
            });
            let rect = Rect {
                x: editing.anchor[0],
                y: editing.anchor[1],
                width,
                height,
            };
            if self
                .workspace
                .board
                .scene
                .get(editing.id)
                .is_none_or(|i| i.rect != rect)
            {
                commands.push(Command::SetRect {
                    id: editing.id,
                    rect,
                });
            }
        }
        self.workspace
            .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
    }

    fn adapt_gpu_budget(&mut self) {
        if self
            .cache_maintenance
            .update(self.settings_window.open, self.diagnostics.clear_disk_cache)
        {
            self.diagnostics.clear_disk_cache = false;
        }
        self.diagnostics.disk_cache_usage = self.cache_maintenance.usage.clone();
        self.diagnostics.disk_cache_busy = self.cache_maintenance.busy;
        let available = self.memory.available();
        let cache = moth_io::LodCache::shared();
        let mode = self.settings.performance_mode;
        if cache.mode() != mode {
            cache.set_mode(mode);
        }
        cache.set_memory_budget(moth_platform::memory::lod_cache_budget(
            available,
            cache.memory_bytes(),
            mode.uses_memory(),
        ));
        cache.set_bake_mode(self.settings.bake_mode);
        if let Some(gpu) = &self.shell.gpu {
            self.workspace
                .board
                .offer_board_bakes(gpu.registry.image_format());
        }
        self.diagnostics.bake_pending = self.workspace.board.bake_pending();
        self.detect_video_memory();
        let full = mode == moth_io::CacheMode::Full;
        if !full && self.workspace.board.preload_started() {
            self.workspace.board.reset_preload();
        }
        let windows = self.window_count();
        if let Some(gpu) = &mut self.shell.gpu {
            // Leave room for decoded imports awaiting their journal acknowledgement.
            let active = self.workspace.board.image_count > 0 || !self.workspace.board.idle();
            // Main windows share one device, so they share its limit.
            let limit = self
                .diagnostics
                .vram_limit(&self.settings)
                .unwrap_or(moth_render::TEXTURE_BUDGET)
                / windows;
            gpu.registry.set_hard_limit(limit);
            let ceiling = if gpu.kind == moth_platform::gpu::AdapterKind::Discrete {
                limit
            } else {
                // Shared-memory adapters take textures from RAM; stay below what is free.
                moth_platform::memory::shared_ceiling(available, gpu.registry.image_bytes(), limit)
            };
            gpu.registry.adapt_budget(active, ceiling);
            gpu.registry.tile_threshold = mode.tile_threshold();
            gpu.registry.keep_all = full && !self.workspace.board.intake.preload_overflow;
            // Preloading waits for the detected limit, so it never overflows a placeholder.
            if full && self.video.is_some() && !self.workspace.library {
                self.workspace
                    .board
                    .offer_preload(gpu.registry.image_format());
            }
        }
        if full {
            self.upload_preloaded();
            self.workspace.board.settle_preload();
        }
    }
    /// Starts video-memory detection once a GPU exists and applies its result.
    fn detect_video_memory(&mut self) {
        if self.video_detection.is_none()
            && let Some(gpu) = &self.shell.gpu
        {
            self.video_detection = Some(moth_platform::gpu::spawn(
                gpu.name.clone(),
                gpu.kind,
                self.shell.wake(),
            ));
        }
        if self.video.is_none()
            && let Some(video) = self
                .video_detection
                .as_ref()
                .and_then(|rx| rx.try_recv().ok())
        {
            self.video = Some(video);
        }
        if self.diagnostics.video != self.video {
            self.diagnostics.video = self.video.clone();
        }
    }
    /// Uploads prepared images for the full-resolution mode, a few milliseconds per
    /// frame so the progress bar stays responsive. Stops the pass, with a warning,
    /// at the first image that would exceed the video memory limit.
    fn upload_preloaded(&mut self) {
        let Some(gpu) = &mut self.shell.gpu else {
            return;
        };
        let board = &mut self.workspace.board;
        let started = std::time::Instant::now();
        let mut uploaded = false;
        while started.elapsed() < std::time::Duration::from_millis(12) {
            let Some(image) = board.next_preloaded() else {
                break;
            };
            if board.intake.preload_overflow {
                continue;
            }
            if !gpu.registry.fits(&image) {
                board.overflow_preload();
                break;
            }
            gpu.registry.upload(&gpu.device, &gpu.queue, &image);
            uploaded = true;
        }
        if uploaded {
            self.redraw();
        }
    }
    fn results(&mut self) {
        self.adapt_gpu_budget();
        let cursor = self.input.cursor;
        let fallback = self
            .shell
            .window
            .as_ref()
            .map(|w| {
                let size = w.inner_size().to_logical::<f32>(w.scale_factor());
                [size.width / 2., size.height / 2.]
            })
            .unwrap_or([0., 0.]);
        let format = self.image_format();
        let events = self.workspace.pump(workspace::Pump {
            format,
            cursor,
            fallback,
            upload: &mut |image| self.shell.upload(image, self.settings.reduced_motion),
            events: Vec::new(),
        });
        self.adapt_gpu_budget();
        for event in events {
            match event {
                workspace::Event::TabChanged => self.reset_board_ui(),
                workspace::Event::Saved => self.remember_appearance(),
                workspace::Event::Opened => {
                    self.editing = None;
                    self.restore_appearance();
                }
                workspace::Event::Language(language) => {
                    self.settings.language = language;
                    moth_ui::set_locale(language);
                }
                workspace::Event::Preferences(values) => {
                    self.window_preferences = values
                        .iter()
                        .filter(|(k, _)| k.starts_with("board_window_"))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    moth_platform::settings::trim_board_windows(&mut self.window_preferences);
                    self.restore_appearance();
                    self.settings.load(&values);
                    self.configure_global_shortcuts();
                    if !self.shortcuts.registry.load(&values) {
                        self.workspace.board.notices.message = moth_ui::Text::InvalidBinding.into();
                    }
                    moth_ui::theme::apply(self.shell.context(), &self.settings.tokens());
                    self.shell.context().set_zoom_factor(self.settings.ui_scale);
                }
            }
        }
        if self.workspace.library
            && self.workspace.refresh_library
            && self
                .library_screen
                .worker
                .requests
                .try_send(moth_platform::library::Request::Refresh(
                    self.workspace.recent_paths().to_vec(),
                ))
                .is_ok()
        {
            self.workspace.refresh_library = false;
        }
        while let Ok(result) = self.library_screen.worker.results.try_recv() {
            match result {
                Ok(entry) if self.workspace.library => {
                    let path = entry.path;
                    let index = self
                        .library_screen
                        .cards
                        .iter()
                        .position(|c| c.path == path);
                    if index.is_none()
                        && self.library_screen.cards.len() >= moth_platform::library::MAX_BOARDS
                    {
                        continue;
                    }
                    let title = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or(moth_ui::text(moth_ui::Text::Untitled))
                        .to_owned();
                    let stack = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_owned();
                    let previews = entry
                        .previews
                        .into_iter()
                        .enumerate()
                        .map(|(i, mip)| {
                            self.shell.context().load_texture(
                                format!("library-{}-{i}", path.display()),
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [mip.width as usize, mip.height as usize],
                                    &mip.bytes,
                                ),
                                egui::TextureOptions::LINEAR,
                            )
                        })
                        .collect();
                    let card = moth_ui::library::Card {
                        path,
                        title,
                        stack,
                        count: entry.count,
                        bytes: entry.bytes,
                        pinned: entry.pinned,
                        tags: entry.tags,
                        error: entry.error,
                        previews,
                    };
                    if let Some(index) = index {
                        self.library_screen.cards[index] = card;
                    } else {
                        self.library_screen.cards.push(card);
                    }
                }
                Err(error) => self.workspace.board.notices.message = error.into(),
                _ => {}
            }
        }
    }

    fn fit(&mut self) {
        let before = self.workspace.board.view.camera;
        self.workspace.board.fit();
        self.animate_camera(before);
    }
    fn animate_camera(&mut self, before: Camera) {
        let target = self.workspace.board.view.camera;
        if before.origin != target.origin || before.zoom != target.zoom {
            self.camera_motion
                .transition(before, target, self.settings.reduced_motion);
            if !self.settings.reduced_motion {
                self.workspace.board.view.camera = before;
            }
            self.redraw();
        }
    }
    /// Applies one user intent as a single side effect. Close-with-save ordering
    /// is expressed by `SaveAndClose` itself, never by the order intents arrive in.
    fn apply(&mut self, intent: Intent, event_loop: &ActiveEventLoop) {
        if self.workspace.board.intake.loading
            && matches!(
                &intent,
                Intent::Save | Intent::SaveAs(_) | Intent::SaveAndClose
            )
        {
            self.workspace.board.notices.message = moth_ui::Text::StorageBusy.into();
            return;
        }
        let before = self.workspace.board.view.camera;
        let animate = match intent {
            Intent::Action(action) => {
                self.run_action(action, event_loop);
                true
            }
            Intent::Session(event) => {
                self.session_event(event);
                true
            }
            intent @ (Intent::View(..)
            | Intent::Fit
            | Intent::Zoom(..)
            | Intent::ZoomTo(..)
            | Intent::Pan(..)
            | Intent::ResetCamera
            | Intent::Focus(..)
            | Intent::FitSelection
            | Intent::FocusItem(..)) => self.apply_camera(intent, event_loop),
            intent @ (Intent::Paste
            | Intent::Undo
            | Intent::Redo
            | Intent::Commands(..)
            | Intent::AddNote(..)
            | Intent::EditNote(..)
            | Intent::CommitNote
            | Intent::NoteFocused
            | Intent::GroupTitle(..)
            | Intent::EnterGroup(..)
            | Intent::SelectItem(..)
            | Intent::MoveLayer(..)
            | Intent::Arrange(..)
            | Intent::OriginalGeometry(..)
            | Intent::Export(..)
            | Intent::DismissArrangePrompt) => self.apply_board(intent, event_loop),
            intent @ (Intent::Library(..)
            | Intent::Home
            | Intent::New
            | Intent::Open
            | Intent::Images
            | Intent::Save
            | Intent::OpenPath(..)
            | Intent::SaveAs(..)
            | Intent::CancelDialog
            | Intent::DialogFocused
            | Intent::SaveAndClose
            | Intent::Discard
            | Intent::KeepOpen
            | Intent::DismissRecovery
            | Intent::RestoreRecovery
            | Intent::Tab(..)
            | Intent::CloseTab(..)) => self.apply_file(intent, event_loop),
            intent @ (Intent::Appearance(..)
            | Intent::Overlay
            | Intent::Pin
            | Intent::PinChoice(..)
            | Intent::CancelWindowMode
            | Intent::Fullscreen
            | Intent::CloseWindow
            | Intent::Drag
            | Intent::WindowBar(..)
            | Intent::DetachTab(..)
            | Intent::MoveTab(..)
            | Intent::GatherTabs
            | Intent::Minimize
            | Intent::Maximize) => self.apply_window(intent, event_loop),
            intent
            @ (Intent::Language(..) | Intent::SettingsRow(..) | Intent::PreferencesDirty) => {
                self.apply_preferences(intent, event_loop)
            }
        };
        if animate {
            self.animate_camera(before);
        }
    }
    /// Fits the camera to the current selection, if any.
    fn fit_selection(&mut self) {
        let before = self.workspace.board.view.camera;
        self.workspace
            .board
            .view
            .fit_selection(&self.workspace.board.scene, &self.workspace.board.selection);
        self.animate_camera(before);
        self.redraw();
    }
    fn native_input(&mut self) {
        let events: Vec<_> = self.input.adapter.native().collect();
        for event in events {
            match event {
                NativeEvent::Pointer(mut p) => {
                    p.position = p
                        .position
                        .map(|value| value / self.shell.context().zoom_factor());
                    if p.kind == moth_platform::input::Kind::Pen && self.settings.swap_pen_buttons {
                        p.button = match p.button {
                            2 => 3,
                            3 => 2,
                            other => other,
                        };
                    }
                    self.input.pen(&p);
                    if p.kind == moth_platform::input::Kind::Touch
                        && self.input.reject_touch(
                            p.id,
                            p.phase == Phase::Down,
                            matches!(p.phase, Phase::Up | Phase::Cancel),
                        )
                    {
                        continue;
                    }
                    let modifiers = self.shell.context().input(|input| input.modifiers);
                    if let Some(state) = &mut self.shell.egui {
                        pen_ui(
                            state.egui_input_mut(),
                            p,
                            modifiers,
                            &mut self.input.native_pressed,
                        );
                    }
                    self.pointer(Input::Pointer(p));
                    self.redraw();
                }
                NativeEvent::Drop(path) => self.import(path),
                NativeEvent::ZipDrop { archive, member } => {
                    let change = self
                        .workspace
                        .import_zip(archive, member, self.image_format());
                    self.tab_change(change);
                }
                NativeEvent::Error(error) => {
                    tracing::warn!(%error, "native input failed");
                    self.workspace.board.notices.message = error.into();
                    self.redraw();
                }
            }
        }
    }
    fn pointer(&mut self, input: Input) {
        if let Input::Pointer(p) = &input
            && p.phase == Phase::Down
            && p.button == 1
            && !self.dragging()
            && let Some(edge) = self.resize_edge(p.position, p.kind)
        {
            self.drag_window(Some(edge));
            return;
        }
        if self.window.moving_window {
            if let Input::Pointer(p) = &input
                && p.phase == Phase::Down
                && p.button == 1
                && !self.shell.context().egui_wants_pointer_input()
            {
                self.window.moving_window = false;
                self.drag_window(None);
            }
            return;
        }
        if self.window.picking_window {
            return;
        }
        self.input.cursor = Some(match &input {
            Input::Zoom { position, .. } | Input::Pan { position, .. } => *position,
            Input::Pointer(p) => p.position,
        });
        if self.workspace.confirm_close
            || self.workspace.board.session.is_some()
            || self.workspace.board.notices.conversion_prompt
            || self.workspace.library
            || self.modal.dialog.is_some()
            || self.workspace.board.notices.arrange_prompt.is_some()
            || self.shortcuts.blocked()
            || self.editing.is_some()
        {
            return;
        }
        match input {
            Input::Pan {
                position,
                delta,
                continuous,
                finished,
            } => {
                if input::canvas_navigation(
                    self.workspace.board.view.viewport,
                    self.tools.blocked() || self.settings_window.open,
                    self.over_panel(position),
                    position,
                ) {
                    self.camera_motion.stop();
                    if continuous {
                        if finished {
                            self.camera_motion
                                .release(Instant::now(), self.settings.reduced_motion);
                        } else {
                            self.camera_motion.pan_sample(delta, Instant::now());
                        }
                    }
                    self.workspace.board.view.camera.origin[0] -=
                        delta[0] / self.workspace.board.view.camera.zoom;
                    self.workspace.board.view.camera.origin[1] -=
                        delta[1] / self.workspace.board.view.camera.zoom;
                    self.rebase_drag();
                }
            }
            Input::Zoom {
                position,
                factor,
                continuous,
            } => {
                if input::canvas_navigation(
                    self.workspace.board.view.viewport,
                    self.tools.blocked() || self.settings_window.open,
                    self.over_panel(position),
                    position,
                ) {
                    if continuous {
                        self.camera_motion.stop();
                        self.workspace.board.view.camera.zoom_at(position, factor);
                    } else {
                        self.camera_motion.zoom(
                            &mut self.workspace.board.view.camera,
                            position,
                            factor,
                            self.settings.reduced_motion || !self.settings.smooth_zoom,
                        );
                    }
                    self.rebase_drag();
                }
            }
            Input::Pointer(mut p) => {
                // Smooth zoom and inertia move the camera between pointer events.
                self.rebase_drag();
                if p.phase == Phase::Down {
                    let action = self.shortcuts.registry.pointer(
                        p.button,
                        self.input.mods(),
                        &self.input.held,
                    );
                    self.input.pointer_action = action;
                    if action.is_none() {
                        return;
                    }
                }
                p.pan = matches!(
                    self.input.pointer_action,
                    Some(
                        bindings::Action::PanDrag
                            | bindings::Action::PanSpaceDrag
                            | bindings::Action::PanAltDrag
                    )
                );
                self.input.hover = p.kind == moth_platform::input::Kind::Mouse
                    || (p.kind == moth_platform::input::Kind::Pen && p.pressure == 0.);
                if p.phase == Phase::Down
                    && self
                        .shell
                        .context()
                        .layer_id_at(egui::pos2(p.position[0], p.position[1]))
                        .is_some_and(|layer| layer.order != egui::Order::Background)
                {
                    return;
                }
                if p.phase == Phase::Down
                    && !self
                        .workspace
                        .board
                        .view
                        .viewport
                        .contains(egui::pos2(p.position[0], p.position[1]))
                {
                    return;
                }
                if self.tool.picks() && p.phase == Phase::Down && self.workspace.board.idle() {
                    self.pick(p.position);
                    return;
                }
                if let gesture::Gesture::Resize { owner, .. } = &self.gesture {
                    if *owner == (p.kind, p.id) {
                        self.resize_pointer(p);
                        return;
                    }
                    self.gesture = gesture::Gesture::Idle;
                }
                if p.phase == Phase::Down
                    && p.button == 1
                    && !p.pan
                    && !self.workspace.board.locked
                    && !self.tools.blocked()
                    && !self.settings_window.open
                    && self.settings.show_outlines
                    && self.workspace.board.idle()
                    && self.input.pointer_action == Some(bindings::Action::MoveDrag)
                    && self.start_resize(p)
                {
                    return;
                }
                if let Some(g) = self.input.gestures.update(p) {
                    self.camera_motion.stop();
                    self.camera_motion.pan_sample(g.pan, Instant::now());
                    self.gesture = gesture::Gesture::Idle;
                    self.workspace.board.view.camera.zoom_at(g.anchor, g.zoom);
                    self.workspace.board.view.camera.origin[0] -=
                        g.pan[0] / self.workspace.board.view.camera.zoom;
                    self.workspace.board.view.camera.origin[1] -=
                        g.pan[1] / self.workspace.board.view.camera.zoom;
                    return;
                }
                if p.phase == Phase::Up && p.kind == moth_platform::input::Kind::Touch {
                    self.camera_motion
                        .release(Instant::now(), self.settings.reduced_motion);
                }
                if self.input.gestures.active() {
                    self.gesture = gesture::Gesture::Idle;
                    return;
                }
                if self
                    .gesture
                    .owner()
                    .is_some_and(|owner| owner != (p.kind, p.id))
                {
                    return;
                }
                if self.input.pointer_action == Some(bindings::Action::ZoomDrag)
                    && (p.phase == Phase::Down
                        || matches!(self.gesture, gesture::Gesture::ZoomDrag { .. }))
                {
                    self.zoom_drag(p);
                    return;
                }
                match p.phase {
                    Phase::Down => self.press(p),
                    Phase::Move => self.drag_to(p),
                    Phase::Up => self.release(p),
                    Phase::Cancel => self.cancel_drag(),
                }
            }
        }
    }
    /// A one-shot picker tool: reports the color or details of the image under
    /// `position`, then falls back to selection.
    fn pick(&mut self, position: [f32; 2]) {
        let world = self.workspace.board.view.camera.world(position);
        let id = self
            .workspace
            .board
            .scene
            .query(Rect {
                x: world[0],
                y: world[1],
                width: 1. / self.workspace.board.view.camera.zoom,
                height: 1. / self.workspace.board.view.camera.zoom,
            })
            .into_iter()
            .rev()
            .find(|item| {
                item.blob().is_some() && item.rect.contains_rotated(item.transform.rotation, world)
            })
            .map(|item| item.id);
        if let Some(id) = id {
            self.workspace
                .persist(moth_platform::boards::Request::Export(
                    if self.tool == gesture::Tool::PickInfo {
                        moth_platform::export::Target::Info { id, point: world }
                    } else {
                        moth_platform::export::Target::Sample {
                            id,
                            point: world,
                            format: match self.tools.pickers.color_format {
                                1 => moth_platform::export::ColorFormat::Rgb,
                                2 => moth_platform::export::ColorFormat::Hsv,
                                _ => moth_platform::export::ColorFormat::Hex,
                            },
                        }
                    },
                ));
        }
        self.tool = gesture::Tool::Select;
    }
    /// A press on a selection handle starts resizing the selection.
    fn start_resize(&mut self, p: moth_platform::input::Pointer) -> bool {
        let items = self
            .workspace
            .board
            .selection
            .iter()
            .filter_map(|id| self.workspace.board.scene.get(*id).cloned())
            .collect();
        let Some(resize) = resize::Resize::start(
            items,
            self.workspace.board.view.camera,
            p.position,
            moth_ui::tokens(self.shell.context()).space_m as f32,
        ) else {
            return false;
        };
        let _ = self.input.gestures.update(p);
        self.camera_motion.stop();
        self.motion
            .start(self.workspace.board.view.camera.world(p.position), 0.);
        // A new resize replaces one still waiting for the journal.
        self.settling.resize = None;
        self.gesture = gesture::Gesture::Resize {
            owner: (p.kind, p.id),
            resize,
        };
        true
    }
    /// Pointer events of the pointer that owns a resize.
    fn resize_pointer(&mut self, p: moth_platform::input::Pointer) {
        let _ = self.input.gestures.update(p);
        match p.phase {
            Phase::Move => {
                let point = self.workspace.board.view.camera.world(p.position);
                if let gesture::Gesture::Resize { resize, .. } = &mut self.gesture {
                    resize.update(point);
                    resize.wrap_note(self.shell.context());
                }
            }
            Phase::Up => {
                let gesture::Gesture::Resize { mut resize, .. } = std::mem::take(&mut self.gesture)
                else {
                    return;
                };
                resize.update(self.workspace.board.view.camera.world(p.position));
                resize.wrap_note(self.shell.context());
                let commands = resize.commands();
                if !commands.is_empty() {
                    let pending = self.workspace.board.intake.in_flight;
                    self.workspace
                        .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                    if self.workspace.board.intake.in_flight > pending {
                        resize.released = true;
                        self.settling.resize = Some(resize);
                    }
                }
            }
            Phase::Cancel => self.gesture = gesture::Gesture::Idle,
            _ => {}
        }
    }
    /// A press on the canvas: grabs the item under it (moving the selection),
    /// starts a selection rectangle on empty canvas, or pans with a pan binding.
    fn press(&mut self, p: moth_platform::input::Pointer) {
        self.camera_motion.stop();
        self.camera_motion.clear_samples();
        let mut grabbed = None;
        if !p.pan && self.workspace.board.idle() {
            let world = self.workspace.board.view.camera.world(p.position);
            let chip = groups::chip_rects(self.shell.context())
                .into_iter()
                .rev()
                .find(|(_, rect)| rect.contains(egui::pos2(p.position[0], p.position[1])))
                .map(|(id, _)| id);
            let board = &mut self.workspace.board;
            let raw = chip.or_else(|| {
                board
                    .scene
                    .query(Rect {
                        x: world[0],
                        y: world[1],
                        width: 1. / board.view.camera.zoom,
                        height: 1. / board.view.camera.zoom,
                    })
                    .into_iter()
                    .rev()
                    // The index only guarantees the rotated envelope, so the
                    // click must still land inside the frame itself.
                    .find(|item| {
                        board.presented(item)
                            && item.rect.contains_rotated(item.transform.rotation, world)
                    })
                    .map(|item| item.id)
            });
            selection::keep_entered(&board.scene, &mut board.entered, raw);
            // The backdrop of the entered group is canvas: it starts a selection
            // rectangle among its members. Its title chip still grabs the whole group.
            if let Some(hit) = raw.filter(|hit| Some(*hit) != board.entered || chip.is_some()) {
                let id = if chip.is_some() {
                    hit
                } else {
                    selection::target(&board.scene, board.entered, hit)
                };
                if !selection::click(&mut board.selection, id, self.input.pointer_action) {
                    selection::with_members(&board.scene, &mut board.selection, Some(id));
                    return;
                }
                selection::with_members(&board.scene, &mut board.selection, None);
                grabbed = board.scene.get(id).map(|item| (item.id, item.rect));
                self.settling.copies =
                    self.input.pointer_action == Some(bindings::Action::DuplicateDrag);
            }
        }
        if grabbed.is_some() {
            let mut bounds = egui::Rect::NOTHING;
            for item in self
                .workspace
                .board
                .selection
                .iter()
                .filter_map(|id| self.workspace.board.scene.get(*id))
            {
                let corners = item.rect.rotated_envelope(item.transform.rotation);
                bounds = bounds.union(egui::Rect::from_min_max(
                    egui::pos2(corners.lower()[0], corners.lower()[1]),
                    egui::pos2(corners.upper()[0], corners.upper()[1]),
                ));
            }
            self.motion.start(
                self.workspace.board.view.camera.world(p.position),
                bounds.size().length() * self.workspace.board.view.camera.zoom,
            );
            self.motion.sample(p.position[0], Instant::now());
        }
        let press = gesture::Press {
            owner: (p.kind, p.id),
            start: p.position,
            last: p.position,
            anchor: self.workspace.board.view.camera.world(p.position),
        };
        self.gesture = match grabbed {
            _ if p.pan => gesture::Gesture::Pan(press),
            Some(grabbed) => gesture::Gesture::Move {
                press,
                grabbed,
                duplicate: self.settling.copies,
            },
            None => gesture::Gesture::Marquee(press),
        };
        self.input.engaged = p.kind == moth_platform::input::Kind::Mouse;
    }
    /// The owning pointer moved during a press.
    fn drag_to(&mut self, p: moth_platform::input::Pointer) {
        let Some((start, _, _)) = self.gesture.drag() else {
            return;
        };
        if !self.input.focus_drag(start, p.position) {
            return;
        }
        if !self.input.engaged {
            if (p.position[0] - start[0]).hypot(p.position[1] - start[1]) < input::TAP_SLOP {
                return;
            }
            self.input.engaged = true;
        }
        if self.gesture.grabbed().is_some() {
            self.motion.sample(p.position[0], Instant::now());
        }
        let pan = matches!(self.gesture, gesture::Gesture::Pan(_));
        let Some(press) = self.gesture.press_mut() else {
            return;
        };
        if pan {
            let last = press.last;
            if p.kind != moth_platform::input::Kind::Mouse {
                self.camera_motion.pan_sample(
                    [p.position[0] - last[0], p.position[1] - last[1]],
                    Instant::now(),
                );
            }
            let camera = &mut self.workspace.board.view.camera;
            camera.origin[0] -= (p.position[0] - last[0]) / camera.zoom;
            camera.origin[1] -= (p.position[1] - last[1]) / camera.zoom;
        }
        press.last = p.position;
    }
    /// The owning pointer was released: a move drops the selection (or copies),
    /// a selection rectangle selects what it covers.
    fn release(&mut self, mut p: moth_platform::input::Pointer) {
        // A pen or finger tap that stayed within the slop is a click.
        if !self.input.engaged
            && let Some((start, _, _)) = self.gesture.drag()
        {
            p.position = start;
        }
        match std::mem::take(&mut self.gesture) {
            gesture::Gesture::Move {
                press, duplicate, ..
            } => self.drop_selection(press.start, p.position, duplicate),
            gesture::Gesture::Marquee(press) => self.select_area(press.start, p.position),
            _ => {}
        }
    }
    /// Commits a move of the selection by the screen distance `start → end`.
    fn drop_selection(&mut self, start: [f32; 2], end: [f32; 2], duplicate: bool) {
        let zoom = self.workspace.board.view.camera.zoom;
        let dx = (end[0] - start[0]) / zoom;
        let dy = (end[1] - start[1]) / zoom;
        if self.workspace.board.locked
            || self.workspace.board.intake.loading
            || dx.abs() + dy.abs() <= 0.5 / zoom
        {
            return;
        }
        let mut items: Vec<_> = self
            .workspace
            .board
            .selection
            .iter()
            .filter_map(|id| self.workspace.board.scene.get(*id))
            .collect();
        items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        let mut commands = arrange::translate(
            &items,
            [dx, dy],
            duplicate,
            self.workspace.board.scene.top(),
        );
        if !duplicate {
            let world = self.workspace.board.view.camera.world(end);
            commands.extend(groups::regroup(
                &self.workspace.board.scene,
                &self.workspace.board.selection,
                world,
            ));
        }
        self.motion.release(
            &commands,
            &self.workspace.board.scene,
            [dx, dy],
            self.settings.reduced_motion,
        );
        self.workspace
            .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
    }
    /// Selects what a selection rectangle from `start` to `end` (screen) covers.
    fn select_area(&mut self, start: [f32; 2], end: [f32; 2]) {
        let zoom = self.workspace.board.view.camera.zoom;
        let a = self.workspace.board.view.camera.world(start);
        let b = self.workspace.board.view.camera.world(end);
        let rect = Rect {
            x: a[0].min(b[0]),
            y: a[1].min(b[1]),
            width: (a[0] - b[0]).abs().max(1. / zoom),
            height: (a[1] - b[1]).abs().max(1. / zoom),
        };
        let images_only =
            self.workspace.board.view.compact && self.workspace.board.appearance.images_only;
        let board = &mut self.workspace.board;
        let entered = board.entered;
        let hits: HashSet<_> = board
            .scene
            .query(rect)
            .iter()
            .filter(|i| !images_only || i.blob().is_some())
            .filter(|i| Some(i.id) != entered)
            .map(|i| selection::target(&board.scene, entered, i.id))
            .collect();
        selection::area(
            &mut board.selection,
            hits.into_iter(),
            self.input.pointer_action,
        );
        selection::with_members(&board.scene, &mut board.selection, None);
    }
    /// Saves boards whose autosave time came and wakes the window when the next
    /// one is due, so an idle app sleeps until then.
    fn schedule_autosave(&mut self) {
        let Some(interval) = self.settings.autosave_interval() else {
            return;
        };
        if let Some(next) = self.workspace.autosave(interval, Instant::now()) {
            self.shell.deadline = Some(self.shell.deadline.map_or(next, |d| d.min(next)));
        }
    }
    /// A panel or popup (Layers, toolbars, menus) lies under `position`, above the
    /// canvas, which egui paints in the background order.
    fn over_panel(&self, position: [f32; 2]) -> bool {
        self.shell
            .context()
            .layer_id_at(egui::pos2(position[0], position[1]))
            .is_some_and(|layer| layer.order != egui::Order::Background)
    }
    /// Vertical drag zooms around the press point: up zooms in, down zooms out.
    /// Reachable with a mouse wheel button or a pen side button plus Ctrl/⌘.
    fn zoom_drag(&mut self, p: moth_platform::input::Pointer) {
        match p.phase {
            Phase::Down
                if self
                    .workspace
                    .board
                    .view
                    .viewport
                    .contains(egui::pos2(p.position[0], p.position[1])) =>
            {
                self.camera_motion.stop();
                self.gesture = gesture::Gesture::ZoomDrag {
                    owner: (p.kind, p.id),
                    anchor: p.position,
                    last: p.position[1],
                };
            }
            Phase::Move => {
                if let gesture::Gesture::ZoomDrag { anchor, last, .. } = &mut self.gesture {
                    let factor = ((*last - p.position[1]) * input::ZOOM_DRAG_RATE).exp();
                    *last = p.position[1];
                    self.workspace.board.view.camera.zoom_at(*anchor, factor);
                }
            }
            Phase::Up | Phase::Cancel => {
                if matches!(self.gesture, gesture::Gesture::ZoomDrag { .. }) {
                    self.gesture = gesture::Gesture::Idle;
                }
            }
            _ => {}
        }
    }
    /// A press, move or resize is under way, or a released resize is settling;
    /// keyboard panning and window-edge resizing wait for it.
    fn dragging(&self) -> bool {
        self.gesture.press().is_some()
            || self.gesture.resize().is_some()
            || self.settling.resize.is_some()
    }
    /// Keeps a board-space drag glued to the board while the camera changes under it.
    /// Item moves, rubber-band selection and cancel all derive their world delta from
    /// `(last - start) / zoom`; re-projecting `start` makes that equal to the real
    /// world distance under the current camera instead of mixing two zoom levels.
    fn rebase_drag(&mut self) {
        let pan = matches!(self.gesture, gesture::Gesture::Pan(_));
        if !pan && let Some(press) = self.gesture.press_mut() {
            press.start = self.workspace.board.view.camera.screen(press.anchor);
        }
    }
    fn cancel_drag(&mut self) {
        if let Some((a, b, false)) = self.gesture.drag()
            && self.gesture.grabbed().is_some()
        {
            let items = self
                .workspace
                .board
                .selection
                .iter()
                .take(301)
                .filter_map(|id| self.workspace.board.scene.get(*id))
                .filter(|item| motion::copyable(item))
                .cloned()
                .collect();
            let zoom = self.workspace.board.view.camera.zoom;
            self.motion.cancel(
                items,
                [(b[0] - a[0]) / zoom, (b[1] - a[1]) / zoom],
                self.settings.reduced_motion,
            );
        }
        self.gesture = gesture::Gesture::Idle;
        self.settling.resize = None;
        self.input.pending_focus = None;
        self.input.held.clear();
        self.input.pan_time = None;
        self.input.pointer_action = None;
        self.camera_motion.stop();
        self.redraw();
    }
    fn frame(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let _span =
            tracing::trace_span!("frame", zoom = self.workspace.board.view.camera.zoom).entered();
        if self.shell.lost() {
            self.recover()?;
        }
        let Some(window) = self.shell.window.clone() else {
            return Ok(());
        };
        let over = self.workspace.board.appearance.over && !self.workspace.library;
        if over != self.window.overlay.is_some() {
            if self.workspace.library {
                let appearance = self.workspace.board.appearance;
                self.toggle_overlay(event_loop);
                self.workspace.board.appearance = appearance;
                self.remember_appearance();
            } else {
                self.toggle_overlay(event_loop);
                // Unsupported windows must not retry creating an overlay on every frame.
                self.workspace.board.appearance.over = self.window.overlay.is_some();
            }
        }
        if let Some(overlay) = &mut self.window.overlay
            && overlay.appearance != self.workspace.board.appearance
        {
            overlay.appearance = self.workspace.board.appearance;
            overlay.window.request_redraw();
        }
        self.workspace.board.view.compact = self.compact();
        if self.workspace.board.view.compact && self.workspace.board.appearance.images_only {
            let board = &mut self.workspace.board;
            board.selection.retain(|id| {
                board
                    .scene
                    .get(*id)
                    .is_some_and(|item| item.blob().is_some())
            });
        }
        let appearance = self.workspace.board.appearance;
        let background = (
            window.id(),
            !self.workspace.library && (appearance.background != 0 || appearance.window < 1.),
            !self.workspace.library && appearance.background == 2,
        );
        if self.window.applied_background != Some(background) {
            moth_platform::Desktop.window_background(&window, background.1, background.2);
            self.window.applied_background = Some(background);
        }
        if self.window.picking_window {
            match moth_platform::Desktop.pick_pin_target(&mut self.window.pin, &window) {
                Some(true) => {
                    self.window.picking_window = false;
                    self.window.target_pin = true;
                    self.apply_pin(true);
                }
                Some(false) => self.cancel_window_mode(),
                None => self
                    .shell
                    .context()
                    .request_repaint_after(std::time::Duration::from_millis(30)),
            }
        }
        if self.window.pinned
            && self.window.target_pin
            && !moth_platform::Desktop.pin_target_available(&self.window.pin)
        {
            self.apply_pin(false);
        }
        if let Some(error) = moth_platform::Desktop.take_window_error() {
            self.workspace.board.notices.message = error.into();
        }
        self.results();
        if let Some(gpu) = &mut self.shell.gpu {
            let active = self.shell.refinements.step(
                Instant::now(),
                self.settings.reduced_motion,
                |hash, level| gpu.registry.set_detail_lod(*hash, level),
            );
            if active {
                self.shell.context().request_repaint();
            }
        }
        self.motion.sync(
            &self.workspace.board.scene,
            &mut self.workspace.board.motion,
            self.workspace.board.intake.in_flight == 0,
        );
        if self.workspace.board.idle()
            && let Some(resize) = self.settling.resize.take()
        {
            for item in resize.previews() {
                self.workspace.board.motion.remove(item.id);
            }
        }
        self.items_active = self
            .workspace
            .board
            .motion
            .step(Instant::now(), self.settings.reduced_motion);
        self.camera_active = self.camera_motion.step(
            &mut self.workspace.board.view.camera,
            Instant::now(),
            self.settings.reduced_motion,
        );
        if self.settings.keyboard_pan_mode == 1 {
            let blocked = self.canvas_keys_blocked()
                || self.workspace.library
                || self.tools.blocked()
                || self.settings_window.open
                || self.dragging()
                || self.window.moving_window
                || self.window.picking_window;
            if let Some(delta) = self.input.flying_pan(
                &self.shortcuts.registry,
                Instant::now(),
                self.settings.keyboard_pan_speed,
                blocked,
            ) {
                self.camera_motion.stop();
                let camera = &mut self.workspace.board.view.camera;
                camera.origin[0] += delta[0] / camera.zoom;
                camera.origin[1] += delta[1] / camera.zoom;
                self.camera_active = true;
            }
        } else {
            self.input.pan_time = None;
        }
        self.rebase_drag();
        self.motion
            .configure(self.settings.drag_animation, self.settings.drag_opacity);
        self.motion.step(
            Instant::now(),
            self.settings.reduced_motion,
            self.gesture.grabbed().is_some(),
        );
        if self.workspace.board.file.discarded {
            self.finish_close(event_loop)?;
            if self.workspace.closing_window
                && self.workspace.tabs.is_empty()
                && self.workspace.board.file.discarded
            {
                return Ok(());
            }
        }
        let Some((output, intents)) = self.run_ui(&window) else {
            return Ok(());
        };
        for intent in intents {
            self.apply(intent, event_loop);
        }
        self.settle_session_window();
        self.rebase_drag();
        let result = self.render(output);
        self.schedule_autosave();
        result
    }
    fn run_ui(&mut self, window: &Window) -> Option<(egui::FullOutput, Vec<Intent>)> {
        let compact_mode = self.compact();
        let mut intents: Vec<Intent> = Vec::new();
        let ctx = self.shell.context().clone();
        ctx.all_styles_mut(|style| {
            style.animation_time = if self.settings.reduced_motion {
                0.
            } else {
                0.1
            }
        });
        self.shortcuts.registry.publish(&ctx);
        let enabled: Vec<_> = bindings::ACTIONS
            .iter()
            .map(|a| self.action_enabled(*a))
            .collect();
        // Read before the UI closure borrows parts of `self` mutably.
        let cover = self.covered_by_preload().then(|| {
            (
                self.workspace.board.view.viewport,
                self.workspace.board.preload_progress(),
            )
        });
        let window_titles = self.window_titles();
        let plans = self.session_plans();
        let dragging = self.dragging();
        let mut output = {
            let state = self.shell.egui.as_mut();
            let gpu = self.shell.gpu.as_ref();
            let (Some(state), Some(gpu)) = (state, gpu) else {
                return None;
            };
            ctx.run_ui(state.take_egui_input(window), |ui| {
                if self.motion.frames.active() || self.camera_active || self.items_active {
                    ui.ctx().request_repaint();
                }
                if self.window.picking_window || self.window.moving_window {
                    egui::Panel::top("pick_window").show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(moth_ui::text(if self.window.moving_window {
                                moth_ui::Text::MoveWindowHint
                            } else {
                                moth_ui::Text::PickWindow
                            }));
                            if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                            {
                                intents.push(Intent::CancelWindowMode);
                            }
                        });
                    });
                }
                let covered = self
                    .workspace
                    .board
                    .session
                    .as_ref()
                    .is_some_and(session_ui::State::covers_board);
                if covered {
                    let board = &mut self.workspace.board;
                    let group =
                        selection::single_group(&board.scene, &board.selection).or(board.entered);
                    let events = session_ui::show(
                        ui,
                        &mut board.session,
                        &session_ui::Board {
                            scene: &board.scene,
                            selection: &board.selection,
                            group,
                            plans: &plans,
                        },
                    );
                    intents.extend(events.into_iter().map(Intent::Session));
                    return;
                }
                let chrome = chrome::chrome(
                    ui,
                    chrome::Chrome {
                        compact: compact_mode,
                        controls_right: self.settings.view_controls_right,
                        zoom_increment: self.settings.zoom_increment,
                        board: &mut self.workspace.board,
                        tabs: &self.workspace.tabs,
                        gpu,
                        library: self.workspace.library,
                        pinned: self.window.pinned,
                        target_pin: self.window.target_pin,
                        catalog: &mut self.library_screen.catalog,
                        cards: &self.library_screen.cards,
                        window,
                        window_bar: &mut self.window.window_bar,
                        windows: &window_titles,
                        enabled: &enabled,
                        registry: &self.shortcuts.registry,
                    },
                );
                if !self.workspace.library {
                    intents.extend(canvas::canvas(
                        ui,
                        canvas::Canvas {
                            board: &self.workspace.board,
                            tools: &mut self.tools,
                            tool: &mut self.tool,
                            settings: &mut self.settings,
                            enabled: &enabled,
                            registry: &self.shortcuts.registry,
                            editing: &mut self.editing,
                            drag: self.gesture.drag(),
                            moving: self.gesture.grabbed(),
                            resizing: self.gesture.resize().or(self.settling.resize.as_ref()),
                            pointer: self.input.cursor.filter(|_| self.input.hover),
                            motion: &self.motion,
                            duplicate: self.settling.copies,
                            gpu,
                            compact: compact_mode,
                            overlay: self.window.overlay.is_some(),
                            activate: !dragging,
                            secondary_gesture: self
                                .input
                                .pointer_action
                                .and_then(|a| self.shortcuts.registry.get(a))
                                .is_some_and(|c| {
                                    matches!(c.trigger, bindings::Trigger::Drag(2, _))
                                }),
                        },
                    ));
                }
                if let Some((viewport, progress)) = cover {
                    canvas::preload_cover(ui, viewport, progress);
                }
                if self.workspace.board.session.is_some() {
                    let board = &mut self.workspace.board;
                    let group =
                        selection::single_group(&board.scene, &board.selection).or(board.entered);
                    let events = session_ui::show(
                        ui,
                        &mut board.session,
                        &session_ui::Board {
                            scene: &board.scene,
                            selection: &board.selection,
                            group,
                            plans: &plans,
                        },
                    );
                    intents.extend(events.into_iter().map(Intent::Session));
                }
                intents.extend(chrome);
                if self.modal.dialog.is_none()
                    && !self.workspace.board.notices.conversion_prompt
                    && !self.workspace.confirm_close
                    && self.workspace.board.notices.arrange_prompt.is_none()
                {
                    let (action, changed, setting) =
                        self.shortcuts.show(ui, &|a| enabled[a as usize]);
                    if let Some(action) = action {
                        intents.push(Intent::Action(action));
                    }
                    if let Some(index) = setting {
                        intents.push(Intent::SettingsRow(index));
                    }
                    if changed {
                        intents.push(Intent::PreferencesDirty);
                    }
                }
                intents.extend(self.settings_window.show(
                    ui,
                    &mut self.settings,
                    &mut self.diagnostics,
                    &self.workspace.board,
                    &format!(
                        "{} · {:.1}/{:.1} MiB",
                        gpu.adapter,
                        gpu.registry.allocated_bytes() as f64 / 1048576.,
                        gpu.registry.budget_bytes() as f64 / 1048576.
                    ),
                    &enabled,
                ));
                self.shortcuts.toast(
                    ui.ctx(),
                    self.settings.shortcut_feedback,
                    self.settings.reduced_motion,
                );
                intents.extend(self.modal.show(
                    ui,
                    &self.workspace,
                    &self.settings,
                    &enabled,
                    &self.shortcuts.registry,
                ));
            })
        };
        if !self.dragging()
            && let Some(position) = ctx.pointer_hover_pos()
            && let Some(edge) =
                self.resize_edge([position.x, position.y], moth_platform::input::Kind::Mouse)
        {
            use winit::window::ResizeDirection::*;
            output.platform_output.cursor_icon = match edge {
                East | West => egui::CursorIcon::ResizeHorizontal,
                North | South => egui::CursorIcon::ResizeVertical,
                NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
                NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
            };
        }
        Some((output, intents))
    }
    fn render(&mut self, output: egui::FullOutput) -> anyhow::Result<()> {
        let covered = self.covered_by_preload();
        let canvas_color = moth_ui::tokens(self.shell.context()).canvas;
        let stage = self.session_stage();
        let Some(gpu) = self.shell.gpu.as_mut() else {
            return Ok(());
        };
        let frame = shell::Frame {
            grid: false,
            ui_opacity: if self.workspace.library {
                1.
            } else {
                self.workspace.board.appearance.window
            },
            opacity: self.workspace.board.appearance.images
                * self.workspace.board.appearance.window,
            clear: {
                let [r, g, b, _] = canvas_color.to_array();
                let appearance = if self.workspace.library {
                    window_mode::Appearance::default()
                } else {
                    self.workspace.board.appearance
                };
                let alpha = if appearance.background == 0 {
                    1.
                } else {
                    appearance.canvas as f64
                } * appearance.window as f64;
                wgpu::Color {
                    r: r as f64 / 255. * alpha,
                    g: g as f64 / 255. * alpha,
                    b: b as f64 / 255. * alpha,
                    a: alpha,
                }
            },
            viewport: self.workspace.board.view.viewport,
            grayscale: self.workspace.board.grayscale,
            tint: self
                .settings
                .grayscale_tint
                .map(|rgb| rgb.map(|c| f32::from(c) / 255.)),
            sampling: moth_scene::Sampling::from_code(self.settings.sampling + 1)
                .unwrap_or_default(),
        };
        let (frame, camera) = match &stage {
            // A session draws its slides in screen points on its own dark stage.
            Some((_, clip, color)) => (
                shell::Frame {
                    grid: false,
                    ui_opacity: 1.,
                    opacity: 1.,
                    clear: *color,
                    viewport: *clip,
                    ..frame
                },
                moth_scene::Camera::default(),
            ),
            None => (frame, self.workspace.board.view.camera),
        };
        let mut previews: Vec<_> = if self.gesture.grabbed().is_some()
            && !self.workspace.board.locked
            && !self.workspace.board.intake.loading
            && !self.workspace.library
        {
            self.workspace
                .board
                .selection
                .iter()
                .filter_map(|id| self.workspace.board.scene.get(*id))
                .map(|item| {
                    self.motion.preview(
                        item,
                        self.gesture.drag(),
                        camera.zoom,
                        self.settings.reduced_motion,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        previews.extend(
            self.motion
                .returning(self.motion.now(), self.settings.reduced_motion),
        );
        if let Some(resizing) = self.gesture.resize().or(self.settling.resize.as_ref()) {
            previews.extend(resizing.previews());
        }
        let drag_ids: HashSet<_> = previews.iter().map(|item| item.id).collect();
        previews.extend(
            self.workspace
                .board
                .motion
                .copies(self.motion.now())
                .into_iter()
                .filter(|item| !drag_ids.contains(&item.id)),
        );
        if self.workspace.library {
            previews.clear();
        }
        let preview_ids: HashSet<_> = previews.iter().map(|item| item.id).collect();
        let mut visible = self.workspace.board.visible(self.workspace.library);
        if self.workspace.board.view.compact && self.workspace.board.appearance.images_only {
            visible.retain(|item| item.blob().is_some());
        }
        if !previews.is_empty() {
            visible.retain(|item| {
                !preview_ids.contains(&item.id)
                    || (self.gesture.duplicate() && drag_ids.contains(&item.id))
            });
            visible.extend(previews.iter());
            visible.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        }
        // While the full-resolution preload covers the board, viewport requests would
        // only duplicate its work.
        if !covered {
            self.workspace.board.request_images(
                &mut gpu.registry,
                &visible,
                self.workspace.library,
                output.pixels_per_point,
                self.camera_motion.target(camera),
                &self
                    .window
                    .floating
                    .iter()
                    .filter_map(|image| {
                        let item = self.workspace.board.scene.get(image.item)?;
                        let size = image.window.inner_size();
                        let screen = [size.width as f32, size.height as f32];
                        if screen[0] == 0. || screen[1] == 0. {
                            return None;
                        }
                        let e = item.rect.rotated_envelope(item.transform.rotation);
                        let bounds = Rect {
                            x: e.lower()[0],
                            y: e.lower()[1],
                            width: e.upper()[0] - e.lower()[0],
                            height: e.upper()[1] - e.lower()[1],
                        };
                        Some((item, floating::fit_image(bounds, screen), screen))
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if let Some((items, _, _)) = &stage {
            visible = items.iter().collect();
            if !covered {
                self.workspace.board.request_images(
                    &mut gpu.registry,
                    &visible,
                    false,
                    output.pixels_per_point,
                    camera,
                    &[],
                );
            }
        }
        groups::backdrops_first(&self.workspace.board.scene, &mut visible);
        let overlay = if stage.is_some() || self.workspace.library {
            Vec::new()
        } else {
            minimap_items(
                &self.workspace.board.scene,
                moth_ui::canvas::minimap_layout(self.shell.context()),
                camera,
            )
        };
        let overlay: Vec<_> = overlay.iter().collect();
        match self
            .shell
            .render(output, &visible, &overlay, camera, frame)?
        {
            shell::Present::Presented => {}
            shell::Present::Recovered => {
                self.reset_renderer_ui();
                self.redraw();
                return Ok(());
            }
            shell::Present::Skipped => return Ok(()),
        }
        Ok(())
    }
}
impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shell.window.is_none() {
            match event_loop.create_window(panes::attributes()) {
                Ok(w) => self.shell.window = Some(Arc::new(w)),
                Err(e) => {
                    tracing::error!(%e,"window creation failed");
                    self.startup_error = Some(e.into());
                    event_loop.exit();
                    return;
                }
            }
        }
        if let Some(window) = &self.shell.window
            && let Err(error) = self.input.adapter.attach(
                window.clone(),
                event_loop.owned_display_handle(),
                self.shell.wake(),
            )
        {
            tracing::warn!(%error, "native input startup failed");
            self.workspace.board.notices.message = error.to_string().into();
        }
        if let Err(e) = self.recover() {
            tracing::error!(%e,"renderer creation failed");
            self.workspace.board.notices.message = e.to_string().into();
            self.startup_error = Some(e);
            event_loop.exit();
        }
        let startup = std::mem::take(&mut self.startup);
        if !startup.is_empty() {
            self.workspace.library = false;
        }
        for path in startup {
            self.import(path);
        }
        self.redraw();
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.input.adapter.event(&WindowEvent::Focused(false), 1.);
        self.input.native_pressed = None;
        self.gesture = gesture::Gesture::Idle;
        self.settling.resize = None;
        self.input.gestures.cancel();
        self.input.modifiers = egui::Modifiers::NONE;
        self.input.super_key = false;
        self.input.held.clear();
        self.input.pending_focus = None;
        for image in &mut self.window.floating {
            image.surface = None;
        }
        self.shell.gpu = None;
        self.shell.egui = None;
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wake) {
        tracing::trace!(?event, "wake");
        match event {
            Wake::Shutdown => event_loop.exit(),
            Wake::Global(shortcut) => {
                if self.settings.global_shortcuts {
                    use bindings::Action;
                    use moth_platform::shortcuts::Shortcut;
                    self.run_action(
                        match shortcut {
                            Shortcut::Overlay => Action::Overlay,
                            Shortcut::ClickThrough => Action::ClickThrough,
                            Shortcut::OpacityUp => Action::OpacityUp,
                            Shortcut::OpacityDown => Action::OpacityDown,
                        },
                        event_loop,
                    );
                    self.redraw();
                }
            }
            Wake::Ready => self.each_pane(|app| app.ready(event_loop)),
            Wake::Repaint(delay) => self.each_pane(|app| {
                if delay.is_zero() {
                    app.redraw();
                } else if let Some(deadline) = Instant::now().checked_add(delay) {
                    app.shell.deadline =
                        Some(app.shell.deadline.map_or(deadline, |d| d.min(deadline)));
                }
            }),
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !self.enter(id) {
            return;
        }
        self.current_window_event(event_loop, id, event);
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.run_window_ops(event_loop);
        if !self.reap_windows() {
            event_loop.exit();
            return;
        }
        let mut next: Option<Instant> = None;
        self.each_pane(|app| {
            app.flush_appearance();
            if let Some(deadline) = app.shell.deadline {
                if deadline <= Instant::now() {
                    app.shell.deadline = None;
                    app.redraw();
                } else {
                    next = Some(next.map_or(deadline, |n| n.min(deadline)));
                }
            }
        });
        match next {
            Some(deadline)
                if deadline.duration_since(Instant::now()) < Duration::from_secs(3600) =>
            {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
            _ => event_loop.set_control_flow(ControlFlow::Wait),
        }
    }
}

impl App {
    /// Worker wake-up for the current window: native input and board results.
    fn ready(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(error) = self
            .global_shortcuts
            .as_ref()
            .and_then(|keys| keys.errors.try_recv().ok())
        {
            self.workspace.board.notices.message = error.into();
            self.settings.global_shortcuts = false;
            self.global_shortcuts = None;
        }
        self.native_input();
        self.results();
        if self.workspace.board.file.discarded
            && let Err(error) = self.finish_close(event_loop)
        {
            self.workspace.board.notices.message = error.to_string().into();
        }
        self.redraw();
    }
    fn current_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        if self.floating_event(id, &event) {
            return;
        }
        if self
            .window
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.window.id() == id)
        {
            if let Some(overlay) = &mut self.window.overlay {
                if matches!(event, WindowEvent::RedrawRequested) {
                    if let Err(error) = overlay.frame() {
                        self.workspace.board.notices.message = error.to_string().into();
                        overlay.exit = true;
                    }
                    if std::mem::take(&mut overlay.hint_dismissed) {
                        self.settings.over_hint = false;
                        self.workspace
                            .persist(moth_platform::boards::Request::Preferences(
                                self.settings
                                    .preferences()
                                    .into_iter()
                                    .map(|(k, v)| (k.to_owned(), v))
                                    .chain(self.shortcuts.registry.preferences())
                                    .collect(),
                            ));
                    }
                } else {
                    overlay.event(&event);
                }
                if overlay.applied != (overlay.on_top, overlay.click_through) {
                    self.window.applied_background = None;
                    if let Some(window) = &self.shell.window
                        && let Err(error) = moth_platform::Desktop.window_effects(
                            window,
                            true,
                            overlay.on_top,
                            overlay.click_through,
                        )
                    {
                        self.workspace.board.notices.message = error.into();
                        overlay.click_through = false;
                    } else {
                        overlay.applied = (overlay.on_top, overlay.click_through);
                        if let Err(error) = overlay.restack() {
                            self.workspace.board.notices.message = error.into();
                        }
                    }
                }
            }
            if let Some(overlay) = &self.window.overlay {
                let appearance = overlay.appearance;
                if appearance != self.workspace.board.appearance {
                    self.workspace.board.appearance = appearance;
                    self.remember_appearance();
                }
            }
            if self
                .window
                .overlay
                .as_ref()
                .is_some_and(|overlay| overlay.exit)
            {
                self.toggle_overlay(event_loop);
            }
            self.redraw();
            return;
        }
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        if window.id() != id {
            return;
        }
        self.input.modifiers_event(&event);
        if let WindowEvent::Touch(touch) = &event
            && self.input.reject_touch(
                touch.id,
                touch.phase == winit::event::TouchPhase::Started,
                matches!(
                    touch.phase,
                    winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled
                ),
            )
        {
            return;
        }
        let egui_events_before = self
            .shell
            .egui
            .as_ref()
            .map_or(0, |state| state.egui_input().events.len());
        if let Some(state) = &mut self.shell.egui {
            let response = state.on_window_event(&window, &event);
            if matches!(event, WindowEvent::ModifiersChanged(_)) {
                self.input.modifiers = state
                    .egui_input()
                    .events
                    .iter()
                    .rev()
                    .find_map(|event| match event {
                        egui::Event::ModifiersChanged(m) => Some(*m),
                        _ => None,
                    })
                    .unwrap_or_default();
            }
            if response.repaint && !matches!(event, WindowEvent::RedrawRequested) {
                self.redraw();
            }
        }
        self.input.adapter.wheel = self.settings.wheel;
        self.input.adapter.zoom_increment = self.settings.zoom_increment;
        if let Some(input) = self
            .input
            .event(&event, window.scale_factor(), self.shell.context())
        {
            self.pointer(input);
            self.redraw();
        }
        match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.frame(event_loop) {
                    tracing::error!(%e,"frame failed");
                    self.workspace.board.notices.message = e.to_string().into();
                    if self.window.overlay.is_some() {
                        self.toggle_overlay(event_loop);
                    }
                }
            }
            WindowEvent::Moved(position) => {
                if self.window_position_locked()
                    && !window.is_maximized()
                    && let Some(locked) = self.window.locked_position
                    && position != locked
                {
                    window.set_outer_position(locked);
                }
            }
            WindowEvent::Resized(size) => {
                if window.is_maximized() && !self.settings.lock_maximized {
                    self.window.locked_position = None;
                }
                if let Some(gpu) = &mut self.shell.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.keep_restored(size);
                self.redraw();
            }
            WindowEvent::Occluded(false) | WindowEvent::ScaleFactorChanged { .. } => {
                let size = window.inner_size();
                if let Some(gpu) = &mut self.shell.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::CloseRequested => self.request_close(true),
            WindowEvent::Focused(true) => {
                if self.settings.disable_click_through_on_activate
                    && let Some(overlay) = &mut self.window.overlay
                {
                    overlay.click_through = false;
                    overlay.window.request_redraw();
                }
            }
            WindowEvent::Focused(false) => {
                if let (Some((kind, id)), Some(state)) =
                    (self.input.native_pressed, &mut self.shell.egui)
                {
                    pen_ui(
                        state.egui_input_mut(),
                        moth_platform::input::Pointer {
                            kind,
                            id,
                            position: [0.; 2],
                            pressure: 0.,
                            phase: Phase::Cancel,
                            pan: false,
                            button: 1,
                        },
                        egui::Modifiers::default(),
                        &mut self.input.native_pressed,
                    );
                }
                self.input.native_pressed = None;
                self.cancel_drag();
                self.input.gestures.cancel();
                self.input.modifiers = egui::Modifiers::NONE;
                self.input.super_key = false;
                self.input.held.clear();
                self.input.pan_time = None;
                self.input.pending_focus = None;
            }
            WindowEvent::DroppedFile(path) => self.import(path),
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(key) = input::key(&event) else {
                    return;
                };
                // A drawing session reads its own keys (Space, arrows, Esc) from egui.
                if self.workspace.board.session.is_some() {
                    self.redraw();
                    return;
                }
                let pressed = event.state == ElementState::Pressed;
                let blocked = self.canvas_keys_blocked();
                let action = self.input.key_action(
                    &self.shortcuts.registry,
                    key,
                    pressed,
                    event.repeat,
                    blocked,
                    self.dragging(),
                );
                if pressed && key == egui::Key::Escape && self.dragging() {
                    self.cancel_drag();
                    return;
                }
                if pressed && key == egui::Key::Escape {
                    let closing_surface = self.tools.blocked()
                        || self.tool.picks()
                        || self.tools.pickers.open
                        || self.settings_window.open
                        || self.window.moving_window
                        || self.window.picking_window;
                    self.cancel_window_mode();
                    if !blocked
                        && !event.repeat
                        && !closing_surface
                        && self.action_enabled(bindings::Action::Deselect)
                    {
                        self.run_action(bindings::Action::Deselect, event_loop);
                        self.redraw();
                        return;
                    }
                    if !blocked {
                        self.tool = gesture::Tool::Select;
                        self.tools.pickers.open = false;
                        self.tools.edit.open = false;
                        self.tools.export.open = false;
                        self.tools.arrange.open = false;
                        self.settings_window.open = false;
                        self.redraw();
                    }
                }
                if pressed && !blocked && key == egui::Key::Escape && self.window.overlay.is_some()
                {
                    self.toggle_overlay(event_loop);
                    return;
                }
                if let Some(action) = action {
                    if self.settings.keyboard_pan_mode == 1 && action.pan_direction().is_some() {
                        self.redraw();
                        return;
                    }
                    if let Some(state) = &mut self.shell.egui {
                        state.egui_input_mut().events.truncate(egui_events_before);
                    }
                    if !(pressed && action == bindings::Action::Focus) {
                        if self.action_enabled(action) && self.settings.shortcut_feedback {
                            self.shortcuts.feedback(action, Instant::now());
                        }
                        self.run_action(action, event_loop);
                    }
                    self.redraw();
                }
            }
            _ => {}
        }
    }
}

/// The minimap as renderer items in board coordinates: an opaque panel, then a small
/// copy of every image (at most a few thousand, which covers the board's shape), placed
/// where the minimap shows it on screen.
fn minimap_items(
    scene: &Scene,
    layout: Option<(egui::Rect, egui::Rect, f32)>,
    camera: moth_scene::Camera,
) -> Vec<moth_scene::Item> {
    const MAX_THUMBNAILS: usize = 4000;
    let (Some((panel, map, opacity)), Some(bounds)) = (layout, scene.bounds()) else {
        return Vec::new();
    };
    let world = |p: egui::Pos2| camera.world([p.x, p.y]);
    let to_world = |rect: egui::Rect| {
        let (a, b) = (world(rect.min), world(rect.max));
        Rect {
            x: a[0],
            y: a[1],
            width: b[0] - a[0],
            height: b[1] - a[1],
        }
    };
    if opacity <= 0. {
        return Vec::new();
    }
    // The navigator's opacity preference reaches the renderer through the copies.
    let mut backdrop = moth_scene::Item::group(
        moth_scene::Group {
            title: String::new(),
            color: moth_scene::GROUP_COLORS,
        },
        to_world(panel),
        Default::default(),
    );
    backdrop.transform.opacity = opacity;
    let mut items = vec![backdrop];
    let scale = egui::vec2(map.width() / bounds.width, map.height() / bounds.height);
    let mut sorted: Vec<_> = scene
        .items()
        .filter(|item| item.blob().is_some() || item.is_group())
        .take(MAX_THUMBNAILS)
        .collect();
    sorted.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
    groups::backdrops_first(scene, &mut sorted);
    for item in sorted {
        let screen = egui::Rect::from_min_size(
            map.min
                + egui::vec2(
                    (item.rect.x - bounds.x) * scale.x,
                    (item.rect.y - bounds.y) * scale.y,
                ),
            egui::vec2(item.rect.width * scale.x, item.rect.height * scale.y),
        );
        let rect = to_world(screen);
        if rect.valid() {
            let mut copy = item.clone();
            copy.rect = rect;
            copy.transform.opacity *= opacity;
            items.push(copy);
        }
    }
    items
}
/// Selection transforms share one definition so the menu and the keys cannot drift apart.
fn rotated(transform: Transform, quarters: f32) -> Transform {
    Transform {
        rotation: transform.rotation + quarters * std::f32::consts::FRAC_PI_2,
        ..transform
    }
}
/// Mirroring is a flag, not a sign, so repeated flips stay reversible.
fn mirrored(transform: Transform, axis: usize) -> Transform {
    let mut flip = transform.flip;
    flip[axis] = !flip[axis];
    Transform { flip, ..transform }
}
/// Double-clicking within a group focuses its combined bounds.
fn focus_selection(selection: &mut HashSet<Uuid>, id: Uuid) {
    if !selection.contains(&id) {
        *selection = HashSet::from([id]);
    }
}

/// One transform change per selected item, grouped into one undo step by the caller.
fn transform_selection(
    scene: &Scene,
    selection: &HashSet<Uuid>,
    change: impl Fn(Transform) -> Transform,
) -> Vec<Command> {
    let items: Vec<_> = selection
        .iter()
        .filter_map(|id| scene.get(*id))
        .filter(|item| !item.is_group())
        .collect();
    let mut bounds = egui::Rect::NOTHING;
    for item in &items {
        let envelope = item.rect.rotated_envelope(item.transform.rotation);
        bounds = bounds.union(egui::Rect::from_min_max(
            egui::pos2(envelope.lower()[0], envelope.lower()[1]),
            egui::pos2(envelope.upper()[0], envelope.upper()[1]),
        ));
    }
    let center = bounds.center();
    let mut commands = Vec::new();
    for item in items {
        let mut transform = change(item.transform);
        let delta = transform.rotation - item.transform.rotation;
        let mut offset = egui::vec2(
            item.rect.x + item.rect.width / 2. - center.x,
            item.rect.y + item.rect.height / 2. - center.y,
        );
        let (sin, cos) = delta.sin_cos();
        offset = egui::vec2(
            cos * offset.x - sin * offset.y,
            sin * offset.x + cos * offset.y,
        );
        for axis in 0..2 {
            if transform.flip[axis] != item.transform.flip[axis] {
                offset[axis] = -offset[axis];
                transform.rotation = -transform.rotation;
            }
        }
        let rect = Rect {
            x: center.x + offset.x - item.rect.width / 2.,
            y: center.y + offset.y - item.rect.height / 2.,
            ..item.rect
        };
        if !rect.valid() || !transform.valid() {
            return Vec::new();
        }
        commands.push(Command::SetRect { id: item.id, rect });
        commands.push(Command::SetTransform {
            id: item.id,
            transform,
        });
    }
    commands
}

fn pen_ui(
    input: &mut egui::RawInput,
    p: moth_platform::input::Pointer,
    modifiers: egui::Modifiers,
    pressed: &mut Option<(moth_platform::input::Kind, u64)>,
) {
    let owner = Some((p.kind, p.id));
    match p.phase {
        Phase::Down => *pressed = owner,
        Phase::Up | Phase::Cancel if *pressed != owner => {
            if p.phase == Phase::Cancel && pressed.is_none() {
                input.events.push(egui::Event::PointerGone);
            }
            return;
        }
        Phase::Up | Phase::Cancel => *pressed = None,
        Phase::Move if pressed.is_some() && *pressed != owner => return,
        _ => (),
    }
    // Release outside the window so cancellation cannot click a toolbar action.
    let pos = if p.phase == Phase::Cancel {
        egui::pos2(-1., -1.)
    } else {
        egui::pos2(p.position[0], p.position[1])
    };
    input.events.push(egui::Event::PointerMoved(pos));
    if matches!(p.phase, Phase::Down | Phase::Up | Phase::Cancel) {
        input.events.push(egui::Event::PointerButton {
            pos,
            // Pen side buttons arrive as secondary (context menu) and middle.
            button: match p.button {
                2 => egui::PointerButton::Secondary,
                3 => egui::PointerButton::Middle,
                _ => egui::PointerButton::Primary,
            },
            pressed: p.phase == Phase::Down,
            modifiers,
        });
    }
    if p.phase == Phase::Cancel {
        input.events.push(egui::Event::PointerGone);
    }
}

#[cfg(test)]
mod tests {
    proptest::proptest! {
        #[test]
        fn group_transforms_roundtrip_and_undo(x in -1000f32..1000., y in -1000f32..1000.) {
            let mut scene = Scene::default();
            let a = Item::text("a", Rect { x, y, width: 20., height: 10. }, None);
            let b = Item::text("b", Rect { x: x + 100., y, width: 20., height: 10. }, Some(&a.z));
            for item in [&a, &b] { scene.apply(Command::Insert(item.clone())).unwrap(); }
            let selected = HashSet::from([a.id, b.id]);
            for change in [
                (|t| rotated(t, 1.)) as fn(Transform) -> Transform,
                |t| mirrored(t, 0),
                |t| mirrored(t, 1),
            ] {
                let commands = transform_selection(&scene, &selected, change);
                let mut inverse = Vec::new();
                for command in commands { inverse.push(scene.apply(command).unwrap()); }
                let payload = moth_format::board::snapshot(&scene).unwrap();
                let mut bytes = Vec::new();
                moth_format::board::initialize(&mut bytes).unwrap();
                let offset = bytes.len() as u64;
                moth_format::board::append(&mut bytes, offset, 3, &payload).unwrap();
                let loaded = moth_format::board::read(&mut std::io::Cursor::new(bytes)).unwrap();
                for item in scene.items() { proptest::prop_assert_eq!(loaded.scene.get(item.id), Some(item)); }
                for command in inverse.into_iter().rev() { scene.apply(command).unwrap(); }
                proptest::prop_assert_eq!(scene.get(a.id), Some(&a));
                proptest::prop_assert_eq!(scene.get(b.id), Some(&b));
            }
        }
    }
    #[test]
    fn rotation_and_flip_move_the_group_around_its_center() {
        let mut scene = Scene::default();
        let a = Item::text(
            "a",
            Rect {
                x: 0.,
                y: 0.,
                width: 20.,
                height: 10.,
            },
            None,
        );
        let b = Item::text("b", Rect { x: 100., ..a.rect }, Some(&a.z));
        for item in [&a, &b] {
            scene.apply(Command::Insert(item.clone())).unwrap();
        }
        let selected = HashSet::from([a.id, b.id]);
        for command in transform_selection(&scene, &selected, |t| rotated(t, 1.)) {
            scene.apply(command).unwrap();
        }
        let first = scene.get(a.id).unwrap();
        assert!((first.rect.x - 50.).abs() < 0.001);
        assert!((first.rect.y + 50.).abs() < 0.001);
        for command in transform_selection(&scene, &selected, |t| mirrored(t, 1)) {
            scene.apply(command).unwrap();
        }
        assert!((scene.get(a.id).unwrap().rect.y - 50.).abs() < 0.001);
    }
    #[test]
    fn double_click_preserves_selected_group_but_focuses_an_unselected_image() {
        let ids = [
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        ];
        let mut selection = HashSet::from(ids);
        focus_selection(&mut selection, ids[2]);
        assert_eq!(selection, HashSet::from(ids));
        let outside = Uuid::now_v7();
        focus_selection(&mut selection, outside);
        assert_eq!(selection, HashSet::from([outside]));
    }
    #[test]
    fn resetting_geometry_keeps_image_appearance() {
        let transform = Transform {
            rotation: 1.,
            content_scale: 2.,
            opacity: 0.3,
            grayscale: true,
            ..Transform::default()
        };
        assert_eq!(
            transform.reset_geometry(),
            Transform {
                opacity: 0.3,
                grayscale: true,
                ..Transform::default()
            }
        );
    }
    use super::*;
    #[test]
    fn pen_side_buttons_reach_egui_as_secondary_and_middle() {
        for (button, expected) in [
            (1, egui::PointerButton::Primary),
            (2, egui::PointerButton::Secondary),
            (3, egui::PointerButton::Middle),
        ] {
            let mut raw = egui::RawInput::default();
            let p = moth_platform::input::Pointer {
                id: 5,
                kind: moth_platform::input::Kind::Pen,
                position: [10., 10.],
                pressure: 0.,
                phase: Phase::Down,
                pan: false,
                button,
            };
            pen_ui(&mut raw, p, egui::Modifiers::default(), &mut None);
            assert!(raw.events.iter().any(|e| matches!(
                e,
                egui::Event::PointerButton { button, pressed: true, .. } if *button == expected
            )));
        }
    }
    #[test]
    fn pen_hover_cancellation_does_not_release_another_pointer() {
        let p = moth_platform::input::Pointer {
            id: 42,
            kind: moth_platform::input::Kind::Pen,
            position: [10., 10.],
            pressure: 0.,
            phase: Phase::Cancel,
            pan: false,
            button: 1,
        };
        let mut raw = egui::RawInput::default();
        pen_ui(&mut raw, p, egui::Modifiers::default(), &mut None);
        assert!(
            !raw.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerButton { .. }))
        );
    }
    #[test]
    fn pen_release_clicks_but_cancellation_cannot_activate_a_button() {
        for (phases, expected) in [
            (vec![Phase::Up], true),
            (vec![Phase::Cancel], false),
            (vec![Phase::Up, Phase::Cancel], true),
        ] {
            let ctx = egui::Context::default();
            moth_ui::theme(&ctx, &moth_ui::Tokens::DARK);
            let raw = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300., 200.),
                )),
                ..Default::default()
            };
            let mut rect = egui::Rect::NOTHING;
            ctx.run_ui(raw(), |ui| {
                rect = ui.button("Test").rect;
            })
            .textures_delta
            .clear();
            let p = moth_platform::input::Pointer {
                id: 42,
                kind: moth_platform::input::Kind::Pen,
                position: [rect.center().x, rect.center().y],
                pressure: 0.5,
                phase: Phase::Down,
                pan: false,
                button: 1,
            };
            let mut pressed = None;
            let mut down = raw();
            pen_ui(&mut down, p, egui::Modifiers::default(), &mut pressed);
            ctx.run_ui(down, |ui| {
                let _ = ui.button("Test");
            })
            .textures_delta
            .clear();
            let mut end = raw();
            for phase in &phases {
                pen_ui(
                    &mut end,
                    moth_platform::input::Pointer { phase: *phase, ..p },
                    egui::Modifiers::default(),
                    &mut pressed,
                );
            }
            let mut clicked = false;
            ctx.run_ui(end, |ui| {
                clicked = ui.button("Test").clicked();
            })
            .textures_delta
            .clear();
            assert_eq!(clicked, expected, "{phases:?}");
            assert!(!ctx.input(|input| input.pointer.primary_down()));
        }
    }
}
