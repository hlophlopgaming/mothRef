// SPDX-License-Identifier: GPL-3.0-or-later
use crate::gpu::Gpu;
use moth_format::Handle;
use moth_platform::Platform;
use moth_platform::input::{Adapter, Gestures, Input, NativeEvent, Phase};
use moth_scene::{BlobId, Camera, Command, Item, Rect, Scene, Transform, Uuid};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
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
    Preview(BlobId),
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
mod arrange;
mod canvas;
mod chrome;
mod intent;
mod modal;
mod overlay;
mod session;
mod shell;
mod tools;
use intent::Intent;
use session::Session;
mod workspace;

pub struct App {
    workspace: workspace::Workspace,
    tools: tools::Tools,
    overlay: Option<overlay::Overlay>,
    pinned: bool,
    library_worker: moth_platform::library::Worker,
    cards: Vec<moth_ui::library::Card>,
    catalog: moth_ui::library::Library,
    modal: modal::Modal,
    moving: Option<(Uuid, Rect)>,
    drag_duplicate: bool,
    editing: Option<Editing>,
    pub startup_error: Option<anyhow::Error>,
    shell: shell::Shell,
    input: Adapter,
    gestures: Gestures,
    cursor: Option<[f32; 2]>,
    command: bool,
    drag: Option<([f32; 2], [f32; 2], bool)>,
    drag_pointer: Option<(moth_platform::input::Kind, u64)>,
    native_pressed: Option<(moth_platform::input::Kind, u64)>,
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
        let board = Session::new(wake.clone(), true)?;
        let library_worker = moth_platform::library::Worker::new(
            Arc::new(moth_platform::DesktopStorage::default()),
            wake.clone(),
        )?;
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx);
        let repaint = proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            let _ = repaint.send_event(Wake::Repaint(info.delay));
        });
        let shell = shell::Shell {
            window: None,
            gpu: None,
            ctx,
            egui: None,
            software: std::env::args().any(|a| a == "--software"),
            wake,
            proxy,
            deadline: None,
        };
        Ok(Self {
            workspace: workspace::Workspace::new(board, shell.wake.clone()),
            tools: tools::Tools::default(),
            overlay: None,
            pinned: false,
            library_worker,
            cards: Vec::new(),
            catalog: Default::default(),
            modal: modal::Modal::default(),
            moving: None,
            drag_duplicate: false,
            editing: None,
            startup_error: None,
            shell,
            input: Adapter::default(),
            gestures: Gestures::default(),
            cursor: None,
            command: false,
            drag: None,
            drag_pointer: None,
            native_pressed: None,
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
                self.shell.wake.clone(),
                self.shell.software,
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
                    self.cards.clear();
                    self.catalog.dismiss_editor();
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
        self.cards.clear();
        self.catalog.dismiss_editor();
        self.editing = None;
        self.drag = None;
        self.moving = None;
        self.drag_pointer = None;
        self.gestures.cancel();
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
    fn recover(&mut self) -> anyhow::Result<()> {
        self.native_pressed = None;
        self.cards.clear();
        self.catalog.dismiss_editor();
        self.workspace.refresh_library = self.workspace.library;
        self.shell.recover()
    }
    fn paste(&mut self) {
        if self.tools.locked {
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
            .clipboard
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
        if self.tools.locked {
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
        if self.tools.locked {
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
            let [width, height] = canvas::measure_note(&self.shell.ctx, &editing.draft);
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

    fn results(&mut self) {
        let cursor = self.cursor;
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
            upload: &mut |image| self.shell.upload(image),
            events: Vec::new(),
        });
        for event in events {
            match event {
                workspace::Event::TabChanged => self.reset_board_ui(),
                workspace::Event::Opened => self.editing = None,
                workspace::Event::Preferences(values) => {
                    self.tools.load(&values);
                    self.shell.ctx.set_theme(if self.tools.dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    self.shell.ctx.set_zoom_factor(self.tools.ui_scale);
                }
            }
        }
        if self.workspace.library
            && self.workspace.refresh_library
            && self
                .library_worker
                .requests
                .try_send(moth_platform::library::Request::Refresh(
                    self.workspace.recent_paths.clone(),
                ))
                .is_ok()
        {
            self.workspace.refresh_library = false;
        }
        while let Ok(result) = self.library_worker.results.try_recv() {
            match result {
                Ok(entry) if self.workspace.library => {
                    let path = entry.path;
                    let index = self.cards.iter().position(|c| c.path == path);
                    if index.is_none() && self.cards.len() >= moth_platform::library::MAX_BOARDS {
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
                            self.shell.ctx.load_texture(
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
                        self.cards[index] = card;
                    } else {
                        self.cards.push(card);
                    }
                }
                Err(error) => self.workspace.board.message = error,
                _ => {}
            }
        }
    }

    fn fit(&mut self) {
        self.workspace.board.fit();
    }
    /// Applies one user intent as a single side effect. Close-with-save ordering
    /// is expressed by `SaveAndClose` itself, never by the order intents arrive in.
    fn apply(&mut self, intent: Intent, event_loop: &ActiveEventLoop) {
        match intent {
            Intent::Paste => {
                if !self.workspace.board.discarding {
                    self.paste();
                }
            }
            Intent::Fit => {
                self.fit();
                self.redraw();
            }
            Intent::Zoom(factor) => {
                self.workspace.board.camera.zoom_at(
                    [
                        self.workspace.board.viewport.center().x,
                        self.workspace.board.viewport.center().y,
                    ],
                    factor,
                );
                self.redraw();
            }
            Intent::Home => {
                self.workspace.library = true;
                self.workspace.refresh_library = true;
                self.workspace.board.viewport = egui::Rect::NOTHING;
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
                let target_width =
                    self.workspace.board.viewport.width() / self.workspace.board.camera.zoom;
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
            Intent::NeedsFit => self.workspace.board.needs_fit = true,
            Intent::ResetCamera => self.workspace.board.camera = Camera::default(),
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
                self.shell.ctx.set_theme(if self.tools.dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                });
                self.shell.ctx.set_zoom_factor(self.tools.ui_scale);
                for tab in &mut self.workspace.tabs {
                    let values = std::collections::BTreeMap::from([(
                        "undo_limit".to_owned(),
                        self.tools.undo_limit.to_string(),
                    )]);
                    if tab
                        .boards
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
                    .focus_return
                    .as_ref()
                    .is_some_and(|(rid, _)| *rid == id)
                {
                    if let Some((_, camera)) = self.workspace.board.focus_return.take() {
                        self.workspace.board.camera = camera;
                    }
                } else {
                    self.workspace.board.focus_return = Some((id, self.workspace.board.camera));
                    self.workspace.board.selection = HashSet::from([id]);
                    self.fit_selection();
                }
            }
            Intent::SetViewFlags { locked, grayscale } => {
                self.workspace.board.locked = locked;
                self.workspace.board.grayscale = grayscale;
            }
            Intent::DismissRecovery => self.workspace.recovery_paths.clear(),
        }
    }
    /// Fits the camera to the current selection, if any.
    fn fit_selection(&mut self) {
        let selected: Vec<_> = self
            .workspace
            .board
            .selection
            .iter()
            .filter_map(|id| self.workspace.board.scene.get(*id))
            .collect();
        if selected.is_empty() {
            return;
        }
        let min = selected.iter().fold([f32::INFINITY; 2], |a, i| {
            let e = i.rect.rotated_envelope(i.transform.rotation);
            [a[0].min(e.lower()[0]), a[1].min(e.lower()[1])]
        });
        let max = selected.iter().fold([f32::NEG_INFINITY; 2], |a, i| {
            [
                a[0].max(i.rect.rotated_envelope(i.transform.rotation).upper()[0]),
                a[1].max(i.rect.rotated_envelope(i.transform.rotation).upper()[1]),
            ]
        });
        self.workspace.board.camera.zoom = ((self.workspace.board.viewport.width() - 48.)
            / (max[0] - min[0]))
            .min((self.workspace.board.viewport.height() - 48.) / (max[1] - min[1]))
            .clamp(0.01, 64.);
        self.workspace.board.camera.origin = [
            (min[0] + max[0]) / 2.
                - self.workspace.board.viewport.center().x / self.workspace.board.camera.zoom,
            (min[1] + max[1]) / 2.
                - self.workspace.board.viewport.center().y / self.workspace.board.camera.zoom,
        ];
        self.redraw();
    }
    /// Rotation, mirroring and reset keys; they act on the selection and need an idle board.
    fn transform_keys(&self, event: &KeyEvent) -> Option<Vec<Command>> {
        if self.command
            || self.shell.ctx.egui_wants_keyboard_input()
            || self.workspace.board.selection.is_empty()
            || !self.workspace.board.idle()
        {
            return None;
        }
        let shift = self.shell.ctx.input(|input| input.modifiers.shift);
        let transform_selection = |change: fn(Transform) -> Transform| {
            transform_selection(
                &self.workspace.board.scene,
                &self.workspace.board.selection,
                change,
            )
        };
        Some(match (event.physical_key, shift) {
            (PhysicalKey::Code(KeyCode::KeyR), false) => {
                transform_selection(|transform| rotated(transform, 1.))
            }
            (PhysicalKey::Code(KeyCode::KeyR), true) => {
                transform_selection(|transform| rotated(transform, -1.))
            }
            (PhysicalKey::Code(KeyCode::KeyH), true) => {
                transform_selection(|transform| mirrored(transform, 0))
            }
            (PhysicalKey::Code(KeyCode::KeyV), true) => {
                transform_selection(|transform| mirrored(transform, 1))
            }
            (PhysicalKey::Code(KeyCode::Digit0), true) => Vec::new(),
            _ => return None,
        })
    }
    fn native_input(&mut self) {
        let events: Vec<_> = self.input.native().collect();
        for event in events {
            match event {
                NativeEvent::Pointer(p) => {
                    let modifiers = self.shell.ctx.input(|input| input.modifiers);
                    if let Some(state) = &mut self.shell.egui {
                        pen_ui(
                            state.egui_input_mut(),
                            p,
                            modifiers,
                            &mut self.native_pressed,
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
        self.cursor = Some(match &input {
            Input::Zoom { position, .. } | Input::Pan { position, .. } => *position,
            Input::Pointer(p) => p.position,
        });
        if self.workspace.confirm_close || self.workspace.library || self.modal.dialog.is_some() {
            return;
        }
        match input {
            Input::Pan { position, delta } => {
                if self
                    .workspace
                    .board
                    .viewport
                    .contains(egui::pos2(position[0], position[1]))
                {
                    self.workspace.board.camera.origin[0] -=
                        delta[0] / self.workspace.board.camera.zoom;
                    self.workspace.board.camera.origin[1] -=
                        delta[1] / self.workspace.board.camera.zoom;
                }
            }
            Input::Zoom { position, factor } => {
                if self
                    .workspace
                    .board
                    .viewport
                    .contains(egui::pos2(position[0], position[1]))
                {
                    self.workspace.board.camera.zoom_at(position, factor);
                }
            }
            Input::Pointer(p) => {
                if p.phase == Phase::Down
                    && self
                        .shell
                        .ctx
                        .layer_id_at(egui::pos2(p.position[0], p.position[1]))
                        .is_some_and(|layer| layer.order != egui::Order::Background)
                {
                    return;
                }
                if p.phase == Phase::Down
                    && !self
                        .workspace
                        .board
                        .viewport
                        .contains(egui::pos2(p.position[0], p.position[1]))
                {
                    return;
                }
                if (self.tools.picking || self.tools.coordinates)
                    && p.phase == Phase::Down
                    && self.workspace.board.idle()
                {
                    let world = self.workspace.board.camera.world(p.position);
                    let id = self
                        .workspace
                        .board
                        .scene
                        .query(Rect {
                            x: world[0],
                            y: world[1],
                            width: 1. / self.workspace.board.camera.zoom,
                            height: 1. / self.workspace.board.camera.zoom,
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
                if let Some(g) = self.gestures.update(p) {
                    self.drag = None;
                    self.moving = None;
                    self.drag_pointer = None;
                    self.workspace.board.camera.zoom_at(g.anchor, g.zoom);
                    self.workspace.board.camera.origin[0] -=
                        g.pan[0] / self.workspace.board.camera.zoom;
                    self.workspace.board.camera.origin[1] -=
                        g.pan[1] / self.workspace.board.camera.zoom;
                    return;
                }
                if self.gestures.active() {
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
                            .viewport
                            .contains(egui::pos2(p.position[0], p.position[1])) =>
                    {
                        if !p.pan && !self.workspace.board.pan && self.workspace.board.idle() {
                            let world = self.workspace.board.camera.world(p.position);
                            if let Some(item) = self
                                .workspace
                                .board
                                .scene
                                .query(Rect {
                                    x: world[0],
                                    y: world[1],
                                    width: 1. / self.workspace.board.camera.zoom,
                                    height: 1. / self.workspace.board.camera.zoom,
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
                                self.drag_duplicate = self.shell.ctx.input(|i| i.modifiers.alt);
                                if !self.workspace.board.selection.contains(&item.id) {
                                    if !self.shell.ctx.input(|i| i.modifiers.shift) {
                                        self.workspace.board.selection.clear();
                                    }
                                    self.workspace.board.selection.insert(item.id);
                                }
                            }
                        }
                        self.drag =
                            Some((p.position, p.position, p.pan || self.workspace.board.pan));
                        self.drag_pointer = Some((p.kind, p.id));
                    }
                    Phase::Move => {
                        if let Some((_, last, pan)) = self.drag.as_mut() {
                            if *pan {
                                self.workspace.board.camera.origin[0] -=
                                    (p.position[0] - last[0]) / self.workspace.board.camera.zoom;
                                self.workspace.board.camera.origin[1] -=
                                    (p.position[1] - last[1]) / self.workspace.board.camera.zoom;
                            }
                            *last = p.position;
                        }
                    }
                    Phase::Up => {
                        self.drag_pointer = None;
                        if let Some((start, _, false)) = self.drag.take() {
                            if let Some((_id, _rect)) = self.moving.take() {
                                let dx =
                                    (p.position[0] - start[0]) / self.workspace.board.camera.zoom;
                                let dy =
                                    (p.position[1] - start[1]) / self.workspace.board.camera.zoom;
                                if !self.tools.locked
                                    && dx.abs() + dy.abs() > 0.5 / self.workspace.board.camera.zoom
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
                                    self.workspace.persist_batch(
                                        commands.into_iter().map(|c| (c, None)).collect(),
                                    );
                                }
                                return;
                            }
                            let a = self.workspace.board.camera.world(start);
                            let b = self.workspace.board.camera.world(p.position);
                            let rect = Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0])
                                    .abs()
                                    .max(1. / self.workspace.board.camera.zoom),
                                height: (a[1] - b[1])
                                    .abs()
                                    .max(1. / self.workspace.board.camera.zoom),
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
                    Phase::Cancel => {
                        self.drag = None;
                        self.moving = None;
                        self.drag_pointer = None;
                    }
                    _ => {}
                }
            }
        }
    }
    fn frame(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let _span =
            tracing::trace_span!("frame", zoom = self.workspace.board.camera.zoom).entered();
        if self.shell.lost() {
            self.recover()?;
        }
        let Some(window) = self.shell.window.clone() else {
            return Ok(());
        };
        self.results();
        if self.workspace.board.discarded {
            self.finish_close(event_loop)?;
            if self.workspace.closing_window
                && self.workspace.tabs.is_empty()
                && self.workspace.board.discarded
            {
                return Ok(());
            }
        }
        let mut actions = moth_ui::Actions::default();
        let mut intents: Vec<Intent> = Vec::new();
        let output = {
            let state = self.shell.egui.as_mut();
            let gpu = self.shell.gpu.as_ref();
            let (Some(state), Some(gpu)) = (state, gpu) else {
                return Ok(());
            };
            self.shell.ctx.run_ui(state.take_egui_input(&window), |ui| {
                if self.overlay.is_none() {
                    chrome::chrome(
                        ui,
                        chrome::Chrome {
                            board: &mut self.workspace.board,
                            tabs: &self.workspace.tabs,
                            gpu,
                            library: self.workspace.library,
                            pinned: self.pinned,
                            catalog: &mut self.catalog,
                            cards: &self.cards,
                            library_worker: &self.library_worker,
                            window: window.as_ref(),
                        },
                        &mut actions,
                        &mut intents,
                    );
                }
                if !self.workspace.library {
                    canvas::canvas(
                        ui,
                        canvas::Canvas {
                            board: &mut self.workspace.board,
                            tools: &mut self.tools,
                            editing: &mut self.editing,
                            drag: self.drag,
                            moving: self.moving,
                            gpu,
                            overlay: self.overlay.is_some(),
                        },
                        &mut actions,
                        &mut intents,
                    );
                }
                self.modal.show(
                    ui,
                    &self.workspace,
                    self.tools.auto_arrange,
                    self.tools.default_arrange,
                    &mut intents,
                );
            })
        };
        intents.extend(intent::intents_from_actions(actions));
        for intent in intents {
            self.apply(intent, event_loop);
        }
        // Visible items and decode requests are document work, not rendering.
        let visible = {
            let gpu = self.shell.gpu.as_ref();
            let Some(gpu) = gpu else {
                return Ok(());
            };
            let p = self.workspace.board.camera.world([
                self.workspace.board.viewport.min.x,
                self.workspace.board.viewport.min.y,
            ]);
            let visible = self.workspace.board.scene.query(Rect {
                x: p[0],
                y: p[1],
                width: if self.workspace.library {
                    0.
                } else {
                    self.workspace.board.viewport.width() / self.workspace.board.camera.zoom
                },
                height: self.workspace.board.viewport.height() / self.workspace.board.camera.zoom,
            });
            for item in &visible {
                // Notes have no source bytes; only images request decode previews.
                let Some(blob) = item.blob() else {
                    continue;
                };
                let side = moth_render::preview_side(
                    item.rect.width.max(item.rect.height)
                        * self.workspace.board.camera.zoom
                        * output.pixels_per_point,
                );
                if !gpu.registry.resident(blob, side)
                    && let Some(&handle) = self.workspace.board.sources.get(&blob)
                    && !self.workspace.board.pending.contains_key(&handle)
                    && !self.workspace.board.failed.contains(&handle)
                    && self
                        .workspace
                        .board
                        .decode
                        .requests
                        .try_send(moth_io::Request {
                            handle,
                            side,
                            format: gpu.registry.image_format(),
                        })
                        .is_ok()
                {
                    self.workspace
                        .board
                        .pending
                        .insert(handle, Pending::Preview(blob));
                }
            }
            visible
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
            viewport: self.workspace.board.viewport,
            grayscale: self.tools.grayscale,
        };
        match self
            .shell
            .render(output, &visible, self.workspace.board.camera, frame)?
        {
            shell::Present::Presented => {}
            shell::Present::Recovered => {
                self.cards.clear();
                self.catalog.dismiss_editor();
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
            && let Err(error) = self.input.attach(
                window.clone(),
                event_loop.owned_display_handle(),
                self.shell.wake.clone(),
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
        self.input.event(&WindowEvent::Focused(false), 1.);
        self.native_pressed = None;
        self.drag = None;
        self.moving = None;
        self.drag_pointer = None;
        self.gestures.cancel();
        self.command = false;
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
        if let Some(state) = &mut self.shell.egui {
            let response = state.on_window_event(&window, &event);
            if matches!(event, WindowEvent::ModifiersChanged(_)) {
                self.command = state
                    .egui_input()
                    .events
                    .iter()
                    .rev()
                    .find_map(|event| match event {
                        egui::Event::ModifiersChanged(m) => Some(m.command),
                        _ => None,
                    })
                    .unwrap_or(false);
            }
            if response.repaint && !matches!(event, WindowEvent::RedrawRequested) {
                self.redraw();
            }
        }
        self.input.wheel = self.tools.wheel;
        if let Some(input) = self.input.event(&event, window.scale_factor()) {
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
                if let (Some((kind, id)), Some(state)) = (self.native_pressed, &mut self.shell.egui)
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
                        },
                        egui::Modifiers::default(),
                        &mut self.native_pressed,
                    );
                }
                self.native_pressed = None;
                self.drag = None;
                self.moving = None;
                self.drag_pointer = None;
                self.gestures.cancel();
                self.command = false;
            }
            WindowEvent::DroppedFile(path) => self.import(path),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.modal.dialog.is_some()
                    || self.workspace.confirm_close
                    || self.workspace.board.arrange_prompt.is_some()
                {
                    return;
                }
                if self.command
                    && !self.shell.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyN)
                {
                    self.new_board();
                } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyO) {
                    self.open_files();
                } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyL) {
                    self.workspace.library = true;
                    self.workspace.refresh_library = true;
                    self.redraw();
                } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyW) {
                    self.request_close(false);
                } else if self.command && event.logical_key == Key::Named(NamedKey::Tab) {
                    if !self.workspace.tabs.is_empty() {
                        self.activate(1);
                    }
                } else if (self.overlay.is_some()
                    && event.logical_key == Key::Named(NamedKey::Escape))
                    || (self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyT))
                {
                    self.toggle_overlay(event_loop);
                } else if self.command
                    && !self.shell.ctx.egui_wants_keyboard_input()
                    && (event.physical_key == PhysicalKey::Code(KeyCode::KeyV)
                        || matches!(event.logical_key,Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
                {
                    if !self.workspace.board.discarding {
                        self.paste();
                    }
                } else if self.command
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyS)
                    && self.workspace.board.idle()
                {
                    if self.workspace.board.path.is_empty() {
                        self.modal.dialog = Some(());
                        self.modal.dialog_focus = true;
                        self.modal.dialog_path.clear();
                        self.redraw();
                    } else {
                        self.workspace.persist(moth_platform::boards::Request::Save(
                            std::path::PathBuf::from(&self.workspace.board.path),
                        ));
                    }
                } else if self.command
                    && !self.shell.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyZ)
                    && self.workspace.board.in_flight == 0
                {
                    self.workspace.persist(moth_platform::boards::Request::Undo);
                } else if self.command
                    && !self.shell.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyY)
                    && self.workspace.board.in_flight == 0
                {
                    self.workspace.persist(moth_platform::boards::Request::Redo);
                } else if !self.shell.ctx.egui_wants_keyboard_input()
                    && !self.command
                    && self.workspace.board.idle()
                    && matches!(
                        event.logical_key,
                        Key::Named(
                            NamedKey::ArrowLeft
                                | NamedKey::ArrowRight
                                | NamedKey::ArrowUp
                                | NamedKey::ArrowDown
                        )
                    )
                {
                    let distance = if self.shell.ctx.input(|i| i.modifiers.shift) {
                        10.
                    } else {
                        1.
                    };
                    let delta = match event.logical_key {
                        Key::Named(NamedKey::ArrowLeft) => [-distance, 0.],
                        Key::Named(NamedKey::ArrowRight) => [distance, 0.],
                        Key::Named(NamedKey::ArrowUp) => [0., -distance],
                        _ => [0., distance],
                    };
                    let items: Vec<_> = self
                        .workspace
                        .board
                        .selection
                        .iter()
                        .filter_map(|id| self.workspace.board.scene.get(*id))
                        .collect();
                    let commands =
                        arrange::translate(&items, delta, false, self.workspace.board.scene.top());
                    self.workspace
                        .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                } else if let Some(commands) = self.transform_keys(&event) {
                    if event.physical_key == PhysicalKey::Code(KeyCode::Digit0)
                        && self.shell.ctx.input(|i| i.modifiers.shift)
                    {
                        self.workspace
                            .persist(moth_platform::boards::Request::OriginalGeometry {
                                ids: self.workspace.board.selection.iter().copied().collect(),
                                mode: moth_platform::export::Geometry::All,
                            });
                    } else {
                        self.workspace
                            .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                    }
                } else if !self.shell.ctx.egui_wants_keyboard_input()
                    && event.logical_key == Key::Character("v".into())
                {
                    self.workspace.board.pan = false;
                    self.redraw();
                } else if !self.shell.ctx.egui_wants_keyboard_input()
                    && event.logical_key == Key::Character("h".into())
                {
                    self.workspace.board.pan = true;
                    self.redraw();
                } else if !self.shell.ctx.egui_wants_keyboard_input()
                    && event.logical_key == Key::Named(NamedKey::Home)
                {
                    self.fit();
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
/// One transform change per selected item, grouped into one undo step by the caller.
fn transform_selection(
    scene: &Scene,
    selection: &HashSet<Uuid>,
    change: impl Fn(Transform) -> Transform,
) -> Vec<Command> {
    selection
        .iter()
        .filter_map(|id| scene.get(*id))
        .filter_map(|item| {
            let transform = change(item.transform);
            transform.valid().then_some(Command::SetTransform {
                id: item.id,
                transform,
            })
        })
        .collect()
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
