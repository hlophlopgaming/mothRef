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
    focus: bool,
}
mod actions;
mod arrange;
mod bindings;
mod camera_motion;
mod canvas;
mod chrome;
mod document;
mod input;
mod intent;
mod item_motion;
mod library;
mod modal;
mod motion;
mod overlay;
mod shell;
mod shortcuts;
mod tools;
mod workspace;
use document::Document;
use intent::Intent;

pub struct App {
    memory: moth_platform::memory::Monitor,
    cache_maintenance: moth_platform::lod_store::Maintenance,
    workspace: workspace::Workspace,
    tools: tools::Tools,
    shortcuts: shortcuts::Shortcuts,
    overlay: Option<overlay::Overlay>,
    pinned: bool,
    library_screen: library::LibraryScreen,
    modal: modal::Modal,
    camera_motion: camera_motion::CameraMotion,
    camera_active: bool,
    items_active: bool,
    motion: motion::DragMotion,
    moving: Option<(Uuid, Rect)>,
    drag_duplicate: bool,
    editing: Option<Editing>,
    pub startup_error: Option<anyhow::Error>,
    shell: shell::Shell,
    drag: Option<([f32; 2], [f32; 2], bool)>,
    drag_pointer: Option<(moth_platform::input::Kind, u64)>,
    input: InputState,
    startup: Vec<std::path::PathBuf>,
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
            cache_maintenance,
            workspace: workspace::Workspace::new(board, shell.wake()),
            tools: tools::Tools::default(),
            shortcuts: shortcuts::Shortcuts::default(),
            overlay: None,
            pinned: false,
            library_screen: library::LibraryScreen::new(library_worker),
            modal: modal::Modal::default(),
            camera_motion: camera_motion::CameraMotion::default(),
            camera_active: false,
            items_active: false,
            motion: motion::DragMotion::default(),
            moving: None,
            drag_duplicate: false,
            editing: None,
            startup_error: None,
            shell,
            drag: None,
            drag_pointer: None,
            input: InputState::default(),
            startup: std::env::args_os()
                .skip(1)
                .filter(|a| a != "--software")
                .map(std::path::PathBuf::from)
                .collect(),
        })
    }
    fn toggle_overlay(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        if self.overlay.is_some() {
            if let Err(error) =
                moth_platform::Desktop.window_effects(&window, false, self.pinned, false)
            {
                self.workspace.board.message = error;
            } else {
                self.overlay = None;
            }
        } else if self.shell.gpu.as_ref().is_some_and(|gpu| gpu.transparent) {
            match overlay::Overlay::new(
                event_loop,
                self.shell.wake(),
                self.shell.software(),
                moth_platform::Desktop.window_capabilities(&window).on_top,
            ) {
                Ok(overlay) => {
                    if let Err(error) =
                        moth_platform::Desktop.window_effects(&window, true, overlay.on_top, false)
                    {
                        self.workspace.board.message = error;
                    }
                    self.overlay = Some(overlay);
                    self.workspace.library = false;
                    self.library_screen.cards.clear();
                    self.library_screen.catalog.dismiss_editor();
                }
                Err(error) => self.workspace.board.message = error.to_string(),
            }
        } else {
            self.workspace.board.message = moth_ui::text(moth_ui::Text::OverlayUnsupported).into();
        }
        self.redraw();
    }

    fn reset_board_ui(&mut self) {
        let preferences = self
            .tools
            .preferences()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        self.tools = tools::Tools::default();
        self.tools.load(&preferences);
        self.tools.locked = self.workspace.board.locked;
        self.tools.grayscale = self.workspace.board.grayscale;
        self.library_screen.cards.clear();
        self.library_screen.catalog.dismiss_editor();
        self.editing = None;
        self.motion = motion::DragMotion::default();
        self.shell.refinements.clear();
        self.camera_motion.stop();
        self.drag = None;
        self.moving = None;
        self.drag_pointer = None;
        self.input.pending_focus = None;
        self.input.pointer_action = None;
        self.input.gestures.cancel();
        if let Some(gpu) = &mut self.shell.gpu {
            gpu.registry.clear();
        }
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
            event_loop.exit();
        }
        self.tab_change(change);
        Ok(())
    }
    fn redraw(&self) {
        if let Some(w) = &self.shell.window {
            w.request_redraw();
        }
    }
    fn reset_renderer_ui(&mut self) {
        self.input.native_pressed = None;
        self.library_screen.cards.clear();
        self.library_screen.catalog.dismiss_editor();
        self.workspace.refresh_library = self.workspace.library;
    }
    fn recover(&mut self) -> anyhow::Result<()> {
        self.reset_renderer_ui();
        self.shell.recover()
    }
    fn paste(&mut self) {
        if self.tools.locked || self.workspace.board.loading {
            return;
        }
        if self.workspace.board.discarding {
            return;
        }
        // A new paste supersedes a pending arrangement prompt.
        self.workspace.board.arrange_prompt = None;
        match self
            .workspace
            .board
            .clipboard()
            .requests
            .try_send(moth_platform::ClipboardRequest::Paste)
        {
            Ok(()) => self.workspace.board.pasting += 1,
            Err(_) => {
                self.workspace.board.message = moth_ui::text(moth_ui::Text::ClipboardBusy).into()
            }
        }
    }

    fn open_files(&mut self) {
        if self.workspace.confirm_close {
            return;
        }
        let files = rfd::FileDialog::new()
            .set_title(moth_ui::text(moth_ui::Text::Open))
            .pick_files()
            .unwrap_or_default();
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
        if self.tools.locked || self.workspace.board.loading {
            return;
        }
        if self.workspace.board.discarding || !self.workspace.board.idle() {
            return;
        }
        let [width, height] = moth_scene::note_size("");
        let item = Item::text(
            "",
            Rect {
                x: world[0],
                y: world[1],
                width,
                height,
            },
            self.workspace.board.scene.top(),
        );
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
            focus: true,
        });
    }
    /// Opens the inline editor over an existing note.
    fn edit_note(&mut self, id: Uuid) {
        if self.tools.locked || self.workspace.board.loading {
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
            let [width, height] = canvas::measure_note(self.shell.context(), &editing.draft);
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
            .update(self.tools.open, self.tools.clear_disk_cache)
        {
            self.tools.clear_disk_cache = false;
        }
        self.tools.disk_cache_usage = self.cache_maintenance.usage.clone();
        self.tools.disk_cache_busy = self.cache_maintenance.busy;
        let available = self.memory.available();
        let cache = moth_io::LodCache::shared();
        let mode = self.tools.performance_mode;
        if cache.mode() != mode {
            cache.set_mode(mode);
        }
        cache.set_memory_budget(moth_platform::memory::lod_cache_budget(
            available,
            cache.memory_bytes(),
            mode.uses_memory(),
        ));
        if let Some(gpu) = &mut self.shell.gpu {
            // Leave room for decoded imports awaiting their journal acknowledgement.
            let active = self.workspace.board.image_count > 0 || !self.workspace.board.idle();
            let ceiling = moth_platform::memory::texture_ceiling(
                available,
                gpu.registry.image_bytes(),
                u64::from(self.tools.cache_limit_mib) * 1024 * 1024,
                12.5,
            );
            gpu.registry.adapt_budget(active, ceiling);
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
            auto_arrange: self.tools.auto_arrange,
            threshold: self.tools.threshold,
            upload: &mut |image| self.shell.upload(image, self.tools.reduced_motion),
            events: Vec::new(),
        });
        self.adapt_gpu_budget();
        for event in events {
            match event {
                workspace::Event::TabChanged => self.reset_board_ui(),
                workspace::Event::Opened => self.editing = None,
                workspace::Event::Preferences(values) => {
                    self.tools.load(&values);
                    if !self.shortcuts.registry.load(&values) {
                        self.workspace.board.message =
                            moth_ui::text(moth_ui::Text::InvalidBinding).into();
                    }
                    self.shell.context().set_theme(if self.tools.dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    self.shell.context().set_zoom_factor(self.tools.ui_scale);
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
                Err(error) => self.workspace.board.message = error,
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
                .transition(before, target, self.tools.reduced_motion);
            if !self.tools.reduced_motion {
                self.workspace.board.view.camera = before;
            }
            self.redraw();
        }
    }
    /// Applies one user intent as a single side effect. Close-with-save ordering
    /// is expressed by `SaveAndClose` itself, never by the order intents arrive in.
    fn apply(&mut self, intent: Intent, event_loop: &ActiveEventLoop) {
        if self.workspace.board.loading
            && matches!(
                &intent,
                Intent::Save | Intent::SaveAs(_) | Intent::SaveAndClose
            )
        {
            self.workspace.board.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
            return;
        }
        let before = self.workspace.board.view.camera;
        match intent {
            Intent::Action(action) => self.run_action(action, event_loop),
            Intent::View(view) => self.workspace.board.view = view,
            Intent::ToggleTools => self.tools.open = !self.tools.open,
            Intent::NoteFocused => {
                if let Some(editing) = &mut self.editing {
                    editing.focus = false;
                }
            }
            Intent::DialogFocused => self.modal.dialog_focus = false,
            Intent::Library(request) => {
                if self
                    .library_screen
                    .worker
                    .requests
                    .try_send(request)
                    .is_err()
                {
                    self.workspace.board.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
                }
            }

            Intent::Paste => {
                if !self.workspace.board.discarding {
                    self.paste();
                }
            }
            Intent::Fit => {
                self.fit();
                self.redraw();
            }
            Intent::Zoom(mut factor) | Intent::ZoomTo(mut factor) => {
                if matches!(intent, Intent::ZoomTo(_)) {
                    self.camera_motion.stop();
                    factor /= self.workspace.board.view.camera.zoom;
                }
                self.camera_motion.zoom(
                    &mut self.workspace.board.view.camera,
                    [
                        self.workspace.board.view.viewport.center().x,
                        self.workspace.board.view.viewport.center().y,
                    ],
                    factor,
                    self.tools.reduced_motion || !self.tools.smooth_zoom,
                );
                self.redraw();
            }
            Intent::Home => {
                self.workspace.library = true;
                self.workspace.refresh_library = true;
                self.workspace.board.view.viewport = egui::Rect::NOTHING;
            }
            Intent::New => self.new_board(),
            Intent::Open => self.open_files(),
            Intent::Save => {
                if self.workspace.board.path.is_empty() {
                    self.modal.dialog = Some(());
                    self.modal.dialog_focus = true;
                    self.modal.dialog_path.clear();
                } else {
                    self.workspace.board.save();
                }
            }
            Intent::Undo => self.workspace.persist(moth_platform::boards::Request::Undo),
            Intent::Redo => self.workspace.persist(moth_platform::boards::Request::Redo),
            Intent::Overlay => self.toggle_overlay(event_loop),
            Intent::Pin => {
                self.pinned = !self.pinned;
                if let Some(window) = &self.shell.window
                    && let Err(error) = moth_platform::Desktop.window_effects(
                        window,
                        self.overlay.is_some(),
                        self.pinned,
                        false,
                    )
                {
                    self.workspace.board.message = error;
                }
            }
            Intent::Tab(index) => self.activate(index),
            Intent::CloseTab(index) => {
                self.activate(index);
                self.request_close(false);
            }
            Intent::CloseWindow => self.request_close(true),
            Intent::Drag => {
                if let Some(window) = &self.shell.window {
                    let _ = window.drag_window();
                }
            }
            Intent::Minimize => {
                if let Some(window) = &self.shell.window {
                    window.set_minimized(true);
                }
            }
            Intent::Maximize => {
                if let Some(window) = &self.shell.window {
                    window.set_maximized(!window.is_maximized());
                }
            }
            Intent::Language(language) => {
                moth_ui::set_locale(language);
                self.workspace
                    .persist(moth_platform::boards::Request::SaveSettings(
                        language.code().to_owned(),
                    ));
            }
            Intent::OpenPath(path) => {
                if path
                    .extension()
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
            Intent::SaveAs(path) => {
                self.modal.dialog = None;
                self.workspace.board.path = path.to_string_lossy().into_owned();
                self.workspace.board.save();
            }
            Intent::CancelDialog => self.modal.dialog = None,
            Intent::SaveAndClose => {
                self.workspace.board.close_after_save = true;
                if self.workspace.board.path.is_empty() {
                    self.modal.dialog_path.clear();
                    self.modal.dialog = Some(());
                    self.modal.dialog_focus = true;
                } else {
                    self.workspace.board.save();
                }
            }
            Intent::Discard => self.workspace.discard(),
            Intent::KeepOpen => {
                self.workspace.confirm_close = false;
                self.workspace.closing_window = false;
                self.workspace.board.close_after_save = false;
            }
            Intent::AddNote(world) => self.add_note(world),
            Intent::EditNote(id) => self.edit_note(id),
            Intent::CommitNote => self.finish_editing(),
            Intent::Commands(commands) => {
                self.workspace
                    .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
            }
            Intent::Arrange(mode, ids) => {
                let mut items: Vec<_> = ids
                    .iter()
                    .filter_map(|id| self.workspace.board.scene.get(*id))
                    .collect();
                items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                let target_width = self.workspace.board.view.viewport.width()
                    / self.workspace.board.view.camera.zoom;
                let commands = arrange::arrange(&items, mode, target_width, self.tools.gap);
                if !commands.is_empty() {
                    self.workspace
                        .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                }
            }
            Intent::FitSelection => self.fit_selection(),
            Intent::Export(target) => self
                .workspace
                .persist(moth_platform::boards::Request::Export(target)),
            Intent::NeedsFit => self.workspace.board.view.needs_fit = true,
            Intent::ResetCamera => self.workspace.board.view.camera = Camera::default(),
            Intent::Focus(step) => {
                let mut items: Vec<_> = self
                    .workspace
                    .board
                    .scene
                    .items()
                    .filter(|i| i.blob().is_some())
                    .collect();
                items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                if !items.is_empty() {
                    let current = items
                        .iter()
                        .position(|i| self.workspace.board.selection.contains(&i.id));
                    let next = current.map_or(if step > 0 { 0 } else { items.len() - 1 }, |i| {
                        (i as i32 + step).rem_euclid(items.len() as i32) as usize
                    });
                    self.workspace.board.selection = HashSet::from([items[next].id]);
                    self.fit_selection();
                }
            }
            Intent::PreferencesDirty => {
                self.adapt_gpu_budget();
                self.shell.context().set_theme(if self.tools.dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                });
                self.shell.context().set_zoom_factor(self.tools.ui_scale);
                for tab in &mut self.workspace.tabs {
                    let values = std::collections::BTreeMap::from([(
                        "undo_limit".to_owned(),
                        self.tools.undo_limit.to_string(),
                    )]);
                    if tab
                        .boards()
                        .requests
                        .try_send(moth_platform::boards::Request::Preferences(values))
                        .is_ok()
                    {
                        tab.in_flight += 1;
                    } else {
                        tab.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
                    }
                }
                self.workspace
                    .persist(moth_platform::boards::Request::Preferences(
                        self.tools
                            .preferences()
                            .into_iter()
                            .map(|(k, v)| (k.to_owned(), v))
                            .chain(self.shortcuts.registry.preferences())
                            .collect(),
                    ));
            }
            Intent::OriginalGeometry(ids, mode) => {
                self.workspace
                    .persist(moth_platform::boards::Request::OriginalGeometry { ids, mode });
            }
            Intent::DismissArrangePrompt => self.workspace.board.arrange_prompt = None,
            Intent::FocusItem(id) => {
                if self
                    .workspace
                    .board
                    .view
                    .focus_return
                    .as_ref()
                    .is_some_and(|(rid, _)| *rid == id)
                {
                    if let Some((_, camera)) = self.workspace.board.view.focus_return.take() {
                        self.workspace.board.view.camera = camera;
                    }
                } else {
                    self.workspace.board.view.focus_return =
                        Some((id, self.workspace.board.view.camera));
                    focus_selection(&mut self.workspace.board.selection, id);
                    self.fit_selection();
                }
            }
            Intent::SetViewFlags { locked, grayscale } => {
                self.workspace.board.locked = locked;
                self.workspace.board.grayscale = grayscale;
            }
            Intent::DismissRecovery => self.workspace.dismiss_recovery(),
        }
        self.animate_camera(before);
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
                NativeEvent::Pointer(p) => {
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
                NativeEvent::Error(error) => {
                    tracing::warn!(%error, "native input failed");
                    self.workspace.board.message = error;
                    self.redraw();
                }
            }
        }
    }
    fn pointer(&mut self, input: Input) {
        self.input.cursor = Some(match &input {
            Input::Zoom { position, .. } | Input::Pan { position, .. } => *position,
            Input::Pointer(p) => p.position,
        });
        if self.workspace.confirm_close
            || self.workspace.library
            || self.modal.dialog.is_some()
            || self.workspace.board.arrange_prompt.is_some()
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
                    self.tools.open,
                    position,
                ) {
                    self.camera_motion.stop();
                    if continuous {
                        if finished {
                            self.camera_motion
                                .release(Instant::now(), self.tools.reduced_motion);
                        } else {
                            self.camera_motion.pan_sample(delta, Instant::now());
                        }
                    }
                    self.workspace.board.view.camera.origin[0] -=
                        delta[0] / self.workspace.board.view.camera.zoom;
                    self.workspace.board.view.camera.origin[1] -=
                        delta[1] / self.workspace.board.view.camera.zoom;
                }
            }
            Input::Zoom {
                position,
                factor,
                continuous,
            } => {
                if input::canvas_navigation(
                    self.workspace.board.view.viewport,
                    self.tools.open,
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
                            self.tools.reduced_motion || !self.tools.smooth_zoom,
                        );
                    }
                }
            }
            Input::Pointer(mut p) => {
                if p.phase == Phase::Down {
                    let action = self.shortcuts.registry.pointer(
                        p.button,
                        self.input.mods(),
                        &self.input.held,
                    );
                    self.input.pointer_action = action;
                    if action.is_none() && !self.workspace.board.view.pan {
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
                if (self.tools.picking || self.tools.coordinates)
                    && p.phase == Phase::Down
                    && self.workspace.board.idle()
                {
                    let world = self.workspace.board.view.camera.world(p.position);
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
                            item.blob().is_some()
                                && item.rect.contains_rotated(item.transform.rotation, world)
                        })
                        .map(|item| item.id);
                    if let Some(id) = id {
                        self.workspace
                            .persist(moth_platform::boards::Request::Export(
                                if self.tools.coordinates {
                                    moth_platform::export::Target::Info { id, point: world }
                                } else {
                                    moth_platform::export::Target::Sample {
                                        id,
                                        point: world,
                                        format: match self.tools.color_format {
                                            1 => moth_platform::export::ColorFormat::Rgb,
                                            2 => moth_platform::export::ColorFormat::Hsv,
                                            _ => moth_platform::export::ColorFormat::Hex,
                                        },
                                    }
                                },
                            ));
                    }
                    self.tools.picking = false;
                    self.tools.coordinates = false;
                    return;
                }
                if let Some(g) = self.input.gestures.update(p) {
                    self.camera_motion.stop();
                    self.camera_motion.pan_sample(g.pan, Instant::now());
                    self.drag = None;
                    self.moving = None;
                    self.drag_pointer = None;
                    self.workspace.board.view.camera.zoom_at(g.anchor, g.zoom);
                    self.workspace.board.view.camera.origin[0] -=
                        g.pan[0] / self.workspace.board.view.camera.zoom;
                    self.workspace.board.view.camera.origin[1] -=
                        g.pan[1] / self.workspace.board.view.camera.zoom;
                    return;
                }
                if p.phase == Phase::Up && p.kind == moth_platform::input::Kind::Touch {
                    self.camera_motion
                        .release(Instant::now(), self.tools.reduced_motion);
                }
                if self.input.gestures.active() {
                    self.drag = None;
                    self.moving = None;
                    self.drag_pointer = None;
                    return;
                }
                if self
                    .drag_pointer
                    .is_some_and(|owner| owner != (p.kind, p.id))
                {
                    return;
                }
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
                        self.camera_motion.clear_samples();
                        if !p.pan && !self.workspace.board.view.pan && self.workspace.board.idle() {
                            let world = self.workspace.board.view.camera.world(p.position);
                            if let Some(item) = self
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
                                // The index only guarantees the rotated envelope, so the
                                // click must still land inside the frame itself.
                                .find(|item| {
                                    item.rect.contains_rotated(item.transform.rotation, world)
                                })
                            {
                                self.moving = Some((item.id, item.rect));
                                self.drag_duplicate = self.input.pointer_action
                                    == Some(bindings::Action::DuplicateDrag);
                                if !self.workspace.board.selection.contains(&item.id) {
                                    if self.input.pointer_action
                                        != Some(bindings::Action::ExtendDrag)
                                    {
                                        self.workspace.board.selection.clear();
                                    }
                                    self.workspace.board.selection.insert(item.id);
                                }
                            }
                        }
                        if self.moving.is_some() {
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
                        self.drag = Some((
                            p.position,
                            p.position,
                            p.pan || self.workspace.board.view.pan,
                        ));
                        self.drag_pointer = Some((p.kind, p.id));
                    }
                    Phase::Move => {
                        if let Some((start, _, _)) = self.drag
                            && !self.input.focus_drag(start, p.position)
                        {
                            return;
                        }
                        if self.moving.is_some() {
                            self.motion.sample(p.position[0], Instant::now());
                        }
                        if let Some((_, last, pan)) = self.drag.as_mut() {
                            if *pan {
                                if p.kind != moth_platform::input::Kind::Mouse {
                                    self.camera_motion.pan_sample(
                                        [p.position[0] - last[0], p.position[1] - last[1]],
                                        Instant::now(),
                                    );
                                }
                                self.workspace.board.view.camera.origin[0] -= (p.position[0]
                                    - last[0])
                                    / self.workspace.board.view.camera.zoom;
                                self.workspace.board.view.camera.origin[1] -= (p.position[1]
                                    - last[1])
                                    / self.workspace.board.view.camera.zoom;
                            }
                            *last = p.position;
                        }
                    }
                    Phase::Up => {
                        self.drag_pointer = None;
                        if let Some((start, _, false)) = self.drag.take() {
                            if let Some((_id, _rect)) = self.moving.take() {
                                let dx = (p.position[0] - start[0])
                                    / self.workspace.board.view.camera.zoom;
                                let dy = (p.position[1] - start[1])
                                    / self.workspace.board.view.camera.zoom;
                                if !self.tools.locked
                                    && !self.workspace.board.loading
                                    && dx.abs() + dy.abs()
                                        > 0.5 / self.workspace.board.view.camera.zoom
                                {
                                    let mut items: Vec<_> = self
                                        .workspace
                                        .board
                                        .selection
                                        .iter()
                                        .filter_map(|id| self.workspace.board.scene.get(*id))
                                        .collect();
                                    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                                    let commands = arrange::translate(
                                        &items,
                                        [dx, dy],
                                        self.drag_duplicate,
                                        self.workspace.board.scene.top(),
                                    );
                                    self.motion.release(
                                        &commands,
                                        &self.workspace.board.scene,
                                        [dx, dy],
                                        self.tools.reduced_motion,
                                    );
                                    self.workspace.persist_batch(
                                        commands.into_iter().map(|c| (c, None)).collect(),
                                    );
                                }
                                return;
                            }
                            let a = self.workspace.board.view.camera.world(start);
                            let b = self.workspace.board.view.camera.world(p.position);
                            let rect = Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0])
                                    .abs()
                                    .max(1. / self.workspace.board.view.camera.zoom),
                                height: (a[1] - b[1])
                                    .abs()
                                    .max(1. / self.workspace.board.view.camera.zoom),
                            };
                            self.workspace.board.selection = self
                                .workspace
                                .board
                                .scene
                                .query(rect)
                                .iter()
                                .map(|i| i.id)
                                .collect();
                        }
                    }
                    Phase::Cancel => self.cancel_drag(),
                    _ => {}
                }
            }
        }
    }
    fn cancel_drag(&mut self) {
        if self.moving.is_some()
            && let Some((a, b, false)) = self.drag
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
                self.tools.reduced_motion,
            );
        }
        self.drag = None;
        self.moving = None;
        self.drag_pointer = None;
        self.input.pending_focus = None;
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
        self.results();
        if let Some(gpu) = &mut self.shell.gpu {
            let active = self.shell.refinements.step(
                Instant::now(),
                self.tools.reduced_motion,
                |hash, level| gpu.registry.set_detail_lod(*hash, level),
            );
            if active {
                self.shell.context().request_repaint();
            }
        }
        self.motion.sync(
            &self.workspace.board.scene,
            &mut self.workspace.board.motion,
            self.workspace.board.in_flight == 0,
        );
        self.items_active = self
            .workspace
            .board
            .motion
            .step(Instant::now(), self.tools.reduced_motion);
        self.camera_active = self.camera_motion.step(
            &mut self.workspace.board.view.camera,
            Instant::now(),
            self.tools.reduced_motion,
        );
        self.motion
            .configure(self.tools.drag_animation, self.tools.drag_opacity);
        self.motion.step(
            Instant::now(),
            self.tools.reduced_motion,
            self.moving.is_some(),
        );
        if self.workspace.board.discarded {
            self.finish_close(event_loop)?;
            if self.workspace.closing_window
                && self.workspace.tabs.is_empty()
                && self.workspace.board.discarded
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
        self.render(output)
    }
    fn run_ui(&mut self, window: &Window) -> Option<(egui::FullOutput, Vec<Intent>)> {
        let mut intents: Vec<Intent> = Vec::new();
        let ctx = self.shell.context().clone();
        ctx.all_styles_mut(|style| {
            style.animation_time = if self.tools.reduced_motion { 0. } else { 0.1 }
        });
        self.shortcuts.registry.publish(&ctx);
        let enabled: Vec<_> = bindings::ACTIONS
            .iter()
            .map(|a| self.action_enabled(*a))
            .collect();
        let output = {
            let state = self.shell.egui.as_mut();
            let gpu = self.shell.gpu.as_ref();
            let (Some(state), Some(gpu)) = (state, gpu) else {
                return None;
            };
            ctx.run_ui(state.take_egui_input(window), |ui| {
                if self.motion.frames.active() || self.camera_active || self.items_active {
                    ui.ctx().request_repaint();
                }
                if self.overlay.is_none() {
                    egui::Panel::top("commands_menu").show(ui, |ui| {
                        ui.menu_button(moth_ui::text(moth_ui::Text::Commands), |ui| {
                            for action in [bindings::Action::Palette, bindings::Action::Shortcuts] {
                                if ui
                                    .add_enabled(
                                        enabled[action as usize],
                                        egui::Button::new(moth_ui::tooltip(
                                            ui.ctx(),
                                            action.label(),
                                        )),
                                    )
                                    .clicked()
                                {
                                    intents.push(Intent::Action(action));
                                    ui.close();
                                }
                            }
                        });
                    });
                }
                let chrome = if self.overlay.is_none() {
                    chrome::chrome(
                        ui,
                        chrome::Chrome {
                            zoom_increment: self.tools.zoom_increment,
                            board: &mut self.workspace.board,
                            tabs: &self.workspace.tabs,
                            gpu,
                            library: self.workspace.library,
                            pinned: self.pinned,
                            catalog: &mut self.library_screen.catalog,
                            cards: &self.library_screen.cards,
                            window,
                        },
                    )
                } else {
                    Vec::new()
                };
                if !self.workspace.library {
                    intents.extend(canvas::canvas(
                        ui,
                        canvas::Canvas {
                            board: &self.workspace.board,
                            tools: &mut self.tools,
                            editing: &mut self.editing,
                            drag: self.drag,
                            moving: self.moving,
                            pointer: self.input.cursor.filter(|_| self.input.hover),
                            motion: &self.motion,
                            duplicate: self.drag_duplicate,
                            gpu,
                            overlay: self.overlay.is_some(),
                            activate: self.input.pointer_action == Some(bindings::Action::MoveDrag)
                                && !self.workspace.board.view.pan,
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
                intents.extend(chrome);
                if self.modal.dialog.is_none()
                    && !self.workspace.confirm_close
                    && self.workspace.board.arrange_prompt.is_none()
                {
                    let (action, changed) = self.shortcuts.show(ui, &|a| enabled[a as usize]);
                    if let Some(action) = action {
                        intents.push(Intent::Action(action));
                    }
                    if changed {
                        intents.push(Intent::PreferencesDirty);
                    }
                }
                intents.extend(self.modal.show(
                    ui,
                    &self.workspace,
                    self.tools.auto_arrange,
                    self.tools.default_arrange,
                ));
            })
        };
        Some((output, intents))
    }
    fn render(&mut self, output: egui::FullOutput) -> anyhow::Result<()> {
        let Some(gpu) = self.shell.gpu.as_mut() else {
            return Ok(());
        };
        let frame = shell::Frame {
            grid: !self.workspace.library
                && !self.workspace.board.scene.is_empty()
                && self.overlay.is_none(),
            opacity: self.overlay.as_ref().map_or(1., |overlay| overlay.opacity),
            clear: if self.overlay.is_some() {
                wgpu::Color::TRANSPARENT
            } else if !self.tools.dark {
                wgpu::Color {
                    r: 0.9,
                    g: 0.91,
                    b: 0.93,
                    a: 1.,
                }
            } else {
                wgpu::Color {
                    r: 17. / 255.,
                    g: 19. / 255.,
                    b: 23. / 255.,
                    a: 1.,
                }
            },
            viewport: self.workspace.board.view.viewport,
            grayscale: self.tools.grayscale,
        };
        let camera = self.workspace.board.view.camera;
        let mut previews: Vec<_> = if self.moving.is_some()
            && !self.tools.locked
            && !self.workspace.board.loading
            && !self.workspace.library
        {
            self.workspace
                .board
                .selection
                .iter()
                .filter_map(|id| self.workspace.board.scene.get(*id))
                .map(|item| {
                    self.motion
                        .preview(item, self.drag, camera.zoom, self.tools.reduced_motion)
                })
                .collect()
        } else {
            Vec::new()
        };
        previews.extend(
            self.motion
                .returning(self.motion.now(), self.tools.reduced_motion),
        );
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
        if !previews.is_empty() {
            visible.retain(|item| {
                !preview_ids.contains(&item.id)
                    || (self.drag_duplicate && self.moving.is_some() && drag_ids.contains(&item.id))
            });
            visible.extend(previews.iter());
            visible.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        }
        self.workspace.board.request_images(
            &mut gpu.registry,
            &visible,
            self.workspace.library,
            output.pixels_per_point,
            self.camera_motion.target(camera),
        );
        match self.shell.render(output, &visible, camera, frame)? {
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
            match event_loop.create_window(
                Window::default_attributes()
                    .with_title(moth_ui::text(moth_ui::Text::Title))
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_inner_size(winit::dpi::LogicalSize::new(1440., 900.)),
            ) {
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
            self.workspace.board.message = error.to_string();
        }
        if let Err(e) = self.recover() {
            tracing::error!(%e,"renderer creation failed");
            self.workspace.board.message = e.to_string();
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
        self.drag = None;
        self.moving = None;
        self.drag_pointer = None;
        self.input.gestures.cancel();
        self.input.modifiers = egui::Modifiers::NONE;
        self.input.super_key = false;
        self.input.held.clear();
        self.input.pending_focus = None;
        self.shell.gpu = None;
        self.shell.egui = None;
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wake) {
        tracing::trace!(?event, "wake");
        match event {
            Wake::Shutdown => event_loop.exit(),
            Wake::Ready => {
                self.native_input();
                self.results();
                if self.workspace.board.discarded
                    && let Err(error) = self.finish_close(event_loop)
                {
                    self.workspace.board.message = error.to_string();
                }
                self.redraw();
            }
            Wake::Repaint(delay) => {
                if delay.is_zero() {
                    self.redraw();
                } else if let Some(deadline) = Instant::now().checked_add(delay) {
                    self.shell.deadline =
                        Some(self.shell.deadline.map_or(deadline, |d| d.min(deadline)));
                }
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self
            .overlay
            .as_ref()
            .is_some_and(|overlay| overlay.window.id() == id)
        {
            if let Some(overlay) = &mut self.overlay {
                if matches!(event, WindowEvent::RedrawRequested) {
                    if let Err(error) = overlay.frame() {
                        self.workspace.board.message = error.to_string();
                        overlay.exit = true;
                    }
                } else {
                    overlay.event(&event);
                }
                if overlay.applied != (overlay.on_top, overlay.click_through) {
                    if let Some(window) = &self.shell.window
                        && let Err(error) = moth_platform::Desktop.window_effects(
                            window,
                            true,
                            overlay.on_top,
                            overlay.click_through,
                        )
                    {
                        self.workspace.board.message = error;
                        overlay.click_through = false;
                    } else {
                        overlay.applied = (overlay.on_top, overlay.click_through);
                    }
                }
            }
            if self.overlay.as_ref().is_some_and(|overlay| overlay.exit) {
                self.toggle_overlay(event_loop);
            }
            self.redraw();
            return;
        }
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        self.input.modifiers_event(&event);
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
        self.input.adapter.wheel = self.tools.wheel;
        self.input.adapter.zoom_increment = self.tools.zoom_increment;
        if let Some(input) = self.input.adapter.event(&event, window.scale_factor()) {
            self.pointer(input);
            self.redraw();
        }
        match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.frame(event_loop) {
                    tracing::error!(%e,"frame failed");
                    self.workspace.board.message = e.to_string();
                    if self.overlay.is_some() {
                        self.toggle_overlay(event_loop);
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.shell.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::CloseRequested => self.request_close(true),
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
                self.input.pending_focus = None;
            }
            WindowEvent::DroppedFile(path) => self.import(path),
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(key) = input::key(&event) else {
                    return;
                };
                let pressed = event.state == ElementState::Pressed;
                let blocked = self.modal.dialog.is_some()
                    || self.workspace.confirm_close
                    || self.workspace.board.arrange_prompt.is_some()
                    || self.shortcuts.blocked()
                    || self.editing.is_some()
                    || self.shell.context().text_edit_focused()
                    || egui::Popup::is_any_open(self.shell.context());
                let action = self.input.key_action(
                    &self.shortcuts.registry,
                    key,
                    pressed,
                    event.repeat,
                    blocked,
                    self.drag.is_some(),
                );
                if pressed && key == egui::Key::Escape && self.drag.is_some() {
                    self.cancel_drag();
                    return;
                }
                if pressed && !blocked && key == egui::Key::Escape && self.overlay.is_some() {
                    self.toggle_overlay(event_loop);
                    return;
                }
                if let Some(action) = action {
                    if let Some(state) = &mut self.shell.egui {
                        state.egui_input_mut().events.truncate(egui_events_before);
                    }
                    if !(pressed && action == bindings::Action::Focus) {
                        self.run_action(action, event_loop);
                    }
                    self.redraw();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.shell.deadline {
            if deadline <= Instant::now() {
                self.shell.deadline = None;
                self.redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else if deadline.duration_since(Instant::now()) < Duration::from_secs(3600) {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
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
    let items: Vec<_> = selection.iter().filter_map(|id| scene.get(*id)).collect();
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
            button: egui::PointerButton::Primary,
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
            moth_ui::theme(&ctx);
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
