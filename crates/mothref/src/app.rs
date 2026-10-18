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
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
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
mod arrange;
mod document;
mod gesture;
mod intent;
mod library;
mod modal;
mod overlay;
mod session;
mod shell;
mod tools;
mod view;
mod workspace;
use gesture::Gesture;
use intent::Intent;
use library::LibraryScreen;
use modal::{Editing, Modal, Screen};
use session::{MAX_TABS, Session};
use shell::Shell;
use workspace::Workspace;

pub struct App {
    shell: Shell,
    workspace: Workspace,
    library: LibraryScreen,
    tools: tools::Tools,
    overlay: Option<overlay::Overlay>,
    pinned: bool,
    modal: Modal,
    gesture: Gesture,
    pub startup_error: Option<anyhow::Error>,
    input: Adapter,
    gestures: Gestures,
    cursor: Option<[f32; 2]>,
    command: bool,
    native_pressed: Option<(moth_platform::input::Kind, u64)>,
}
impl App {
    pub fn new(proxy: EventLoopProxy<Wake>) -> anyhow::Result<Self> {
        let ready = proxy.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = ready.send_event(Wake::Ready);
        });
        let board = Session::new(wake.clone(), true)?;
        let library = LibraryScreen {
            worker: moth_platform::library::Worker::new(
                Arc::new(moth_platform::DesktopStorage::default()),
                wake.clone(),
            )?,
            cards: Vec::new(),
            catalog: Default::default(),
            refresh: false,
        };
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx);
        let repaint = proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            let _ = repaint.send_event(Wake::Repaint(info.delay));
        });
        let shell = Shell {
            window: None,
            gpu: None,
            ctx,
            egui: None,
            proxy: proxy.clone(),
            wake: wake.clone(),
            deadline: None,
            software: std::env::args().any(|a| a == "--software"),
        };
        let workspace = Workspace {
            board,
            tabs: Vec::new(),
            screen: Screen::Library,
            closing_window: false,
            recovery_paths: Vec::new(),
            recent_paths: Vec::new(),
            startup: std::env::args_os()
                .skip(1)
                .filter(|a| a != "--software")
                .map(std::path::PathBuf::from)
                .collect(),
        };
        Ok(Self {
            shell,
            workspace,
            library,
            tools: tools::Tools::default(),
            overlay: None,
            pinned: false,
            modal: Modal::None,
            gesture: Gesture::Idle,
            startup_error: None,
            input: Adapter::default(),
            gestures: Gestures::default(),
            cursor: None,
            command: false,
            native_pressed: None,
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
                self.workspace.board.view.message = error;
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
                        self.workspace.board.view.message = error;
                    }
                    self.overlay = Some(overlay);
                    self.workspace.screen = Screen::Board;
                    self.library.cards.clear();
                    self.library.catalog.dismiss_editor();
                }
                Err(error) => self.workspace.board.view.message = error.to_string(),
            }
        } else {
            self.workspace.board.view.message =
                moth_ui::text(moth_ui::Text::OverlayUnsupported).into();
        }
        self.shell.redraw();
    }
    fn activate(&mut self, index: usize) {
        if self.modal.confirming() || index > self.workspace.tabs.len() {
            return;
        }
        if index > 0 {
            std::mem::swap(
                &mut self.workspace.board,
                &mut self.workspace.tabs[index - 1],
            );
        }
        self.workspace.screen = Screen::Board;
        let preferences = self
            .tools
            .preferences()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        self.tools = tools::Tools::default();
        self.tools.load(&preferences);
        self.tools.locked = self.workspace.board.view.locked;
        self.tools.grayscale = self.workspace.board.view.grayscale;
        self.library.cards.clear();
        self.library.catalog.dismiss_editor();
        self.modal.take_editor();
        self.gesture.cancel();
        self.gestures.cancel();
        if let Some(gpu) = &mut self.shell.gpu {
            gpu.registry.clear();
        }
        self.shell.redraw();
    }
    fn new_board(&mut self) {
        if self.modal.confirming() {
            return;
        }
        if self.workspace.board.document.scene.is_empty()
            && self.workspace.board.document.path.is_empty()
            && self.workspace.board.document.idle()
        {
            self.activate(0);
            return;
        }
        if self.workspace.tabs.len() + 1 >= MAX_TABS {
            self.workspace.board.view.message = moth_ui::text(moth_ui::Text::TabLimit).into();
            return;
        }
        match Session::new(self.shell.wake.clone(), false) {
            Ok(board) => {
                let previous = std::mem::replace(&mut self.workspace.board, board);
                self.workspace.tabs.push(previous);
                self.activate(0);
            }
            Err(error) => self.workspace.board.view.message = error.to_string(),
        }
    }
    fn open_in_tab(&mut self, path: std::path::PathBuf) {
        if self.modal.confirming() {
            return;
        }
        if std::path::Path::new(&self.workspace.board.document.path) == path {
            self.activate(0);
            return;
        }
        if let Some(index) = self
            .workspace
            .tabs
            .iter()
            .position(|b| std::path::Path::new(&b.document.path) == path)
        {
            self.activate(index + 1);
            return;
        }
        if !self.workspace.board.document.scene.is_empty()
            || !self.workspace.board.document.path.is_empty()
            || !self.workspace.board.document.idle()
        {
            let count = self.workspace.tabs.len();
            self.new_board();
            if self.workspace.tabs.len() == count {
                return;
            }
        }
        self.workspace.screen = Screen::Board;
        self.library.cards.clear();
        self.library.catalog.dismiss_editor();
        self.open_board(path);
    }
    fn request_close(&mut self, window: bool) {
        self.workspace.closing_window = window;
        self.workspace.screen = Screen::Board;
        if self.workspace.board.document.dirty || !self.workspace.board.document.idle() {
            self.modal.confirm_close();
        } else {
            self.workspace.board.discard();
        }
        self.shell.redraw();
    }
    fn finish_close(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        self.modal.keep_open();
        if let Some(board) = self.workspace.tabs.pop() {
            self.workspace.board = board;
            self.activate(0);
            if self.workspace.closing_window {
                self.request_close(true);
            }
        } else if self.workspace.closing_window {
            event_loop.exit();
        } else {
            self.workspace.board = Session::new(self.shell.wake.clone(), false)?;
            self.workspace.screen = Screen::Library;
            self.library.refresh = true;
            if let Some(gpu) = &mut self.shell.gpu {
                gpu.registry.clear();
            }
        }
        Ok(())
    }
    fn recover(&mut self) -> anyhow::Result<()> {
        self.native_pressed = None;
        self.library.cards.clear();
        self.library.catalog.dismiss_editor();
        self.library.refresh = self.workspace.screen == Screen::Library;
        self.shell.recreate()
    }
    fn open_board(&mut self, path: std::path::PathBuf) {
        if self.workspace.board.document.scene.is_empty()
            && self.workspace.board.document.in_flight == 0
            && self.workspace.board.document.pasting == 0
            && self.workspace.board.document.pending.is_empty()
            && self.workspace.board.document.waiting.is_empty()
            && !self.workspace.board.document.discarding
        {
            self.workspace.board.document.path = path.to_string_lossy().into_owned();
            self.workspace
                .board
                .persist(moth_platform::boards::Request::Open(path));
        } else {
            self.workspace.board.view.message = moth_ui::text(moth_ui::Text::OpenEmpty).into();
        }
    }
    fn import_foreign(&mut self, path: std::path::PathBuf) {
        if self.modal.confirming() {
            return;
        }
        if !self.workspace.board.document.scene.is_empty()
            || !self.workspace.board.document.path.is_empty()
            || !self.workspace.board.document.idle()
        {
            let count = self.workspace.tabs.len();
            self.new_board();
            if self.workspace.tabs.len() == count {
                return;
            }
        }
        self.workspace.screen = Screen::Board;
        self.library.cards.clear();
        self.library.catalog.dismiss_editor();
        self.workspace.board.document.path = String::new();
        if self.workspace.board.document.idle() {
            let request = if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pur"))
            {
                moth_platform::boards::Request::ImportPur(path)
            } else {
                moth_platform::boards::Request::ImportBee(path)
            };
            self.workspace.board.persist(request);
        }
    }
    fn import(&mut self, path: std::path::PathBuf) {
        if self.tools.locked {
            return;
        }
        if self.workspace.board.document.discarding {
            return;
        }
        // A new drop supersedes a pending arrangement prompt.
        self.workspace.board.document.arrange_prompt = None;
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
        {
            self.open_board(path);
            return;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("bee") || e.eq_ignore_ascii_case("pur"))
        {
            self.import_foreign(path);
            return;
        }
        if self.workspace.board.document.scene.len()
            + self.workspace.board.document.waiting.len()
            + self.workspace.board.document.pending.len()
            >= 1_000_000
        {
            self.workspace.board.view.message = moth_ui::text(moth_ui::Text::BoardFull).into();
            return;
        }
        if self.workspace.board.document.waiting.len() >= 1024 {
            self.workspace.board.view.message = moth_ui::text(moth_ui::Text::ImportBusy).into();
            return;
        }
        match self.workspace.board.document.storage.register(path) {
            Ok(h) => self.workspace.board.document.waiting.push(h),
            Err(e) => self.workspace.board.view.message = e.to_string(),
        }
        self.workspace
            .board
            .document
            .pump(self.shell.image_format());
    }
    fn open_files(&mut self) {
        if self.modal.confirming() {
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
                if self.workspace.screen == Screen::Library {
                    self.new_board();
                }
                self.import(path);
            }
        }
    }
    fn insertion_point(&self) -> [f32; 2] {
        if self.workspace.board.view.viewport.is_finite() {
            let p = self.workspace.board.view.viewport.center();
            return self.workspace.board.view.camera.world([p.x, p.y]);
        }
        let point = self
            .shell
            .window
            .as_ref()
            .map(|w| {
                let size = w.inner_size().to_logical::<f32>(w.scale_factor());
                [size.width / 2., size.height / 2.]
            })
            .unwrap_or([0., 0.]);
        self.workspace.board.view.camera.world(point)
    }
    /// The cursor in world space, falling back to the viewport centre when unknown or off-canvas.
    fn cursor_point(&self) -> [f32; 2] {
        if let Some(cursor) = self.cursor
            && self.workspace.board.view.viewport.is_finite()
            && self
                .workspace
                .board
                .view
                .viewport
                .contains(egui::pos2(cursor[0], cursor[1]))
        {
            return self.workspace.board.view.camera.world(cursor);
        }
        self.insertion_point()
    }
    /// Creates an empty note at `world` and opens the inline editor for it.
    fn add_note(&mut self, world: [f32; 2]) {
        if self.tools.locked {
            return;
        }
        if self.workspace.board.document.discarding || !self.workspace.board.document.idle() {
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
            self.workspace.board.document.scene.top(),
        );
        let id = item.id;
        self.workspace
            .board
            .persist(moth_platform::boards::Request::Apply(
                Command::Insert(item),
                None,
            ));
        self.modal.edit(Editing {
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
        let Some(item) = self.workspace.board.document.scene.get(id) else {
            return;
        };
        let Some(text) = item.note() else {
            return;
        };
        self.modal.edit(Editing {
            id,
            original: text.to_owned(),
            draft: text.to_owned(),
            anchor: [item.rect.x, item.rect.y],
            focus: true,
        });
    }
    /// Commits or discards the inline editor's draft once it loses focus.
    fn finish_editing(&mut self) {
        let Some(editing) = self.modal.take_editor() else {
            return;
        };
        if editing.draft == editing.original {
            return;
        }
        let mut commands = Vec::new();
        if editing.draft.trim().is_empty() {
            commands.push(Command::Remove(editing.id));
        } else {
            let [width, height] = measure_note(&self.shell.ctx, &editing.draft);
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
                .document
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
            .board
            .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
    }
    fn complete(&mut self, done: moth_io::Completion) {
        let Some(pending) = self
            .workspace
            .board
            .document
            .pending
            .remove(&done.request.handle)
        else {
            return;
        };
        let paste = matches!(pending, Pending::Paste);
        let insert = paste || matches!(pending, Pending::Import);
        match done.result {
            Ok(image) => {
                if matches!(pending,Pending::Preview(hash) if hash!=image.hash) {
                    self.workspace
                        .board
                        .document
                        .failed
                        .insert(done.request.handle);
                    self.workspace.board.view.message =
                        moth_ui::text(moth_ui::Text::SourceChanged).into();
                    return;
                }
                if insert {
                    self.workspace.board.document.begin_batch();
                    // Both pastes and drops land under the cursor; drops cascade to stay visible.
                    let (p, offset) = if paste {
                        (self.cursor_point(), 0.)
                    } else {
                        (
                            self.cursor_point(),
                            (self.workspace.board.document.scene.len() % 10) as f32 * 24.,
                        )
                    };
                    let width = (image.original[0] as f32).min(600.);
                    let height = width * image.original[1] as f32 / image.original[0] as f32;
                    let mut item = Item::image(
                        image.hash,
                        Rect {
                            x: p[0] - width / 2. + offset,
                            y: p[1] - height / 2. + offset,
                            width,
                            height,
                        },
                        self.workspace.board.document.scene.top(),
                    );
                    if !paste {
                        item.name = self
                            .workspace
                            .board
                            .document
                            .storage
                            .source_name(done.request.handle);
                    }
                    self.workspace.board.document.burst.push(item.id);
                    self.workspace
                        .board
                        .persist(moth_platform::boards::Request::Apply(
                            Command::Insert(item),
                            Some(done.request.handle),
                        ));
                }
                if let Some(gpu) = &mut self.shell.gpu {
                    gpu.registry.upload(&gpu.device, &gpu.queue, &image);
                }
            }
            Err(e) => {
                if insert && let Some(hash) = done.failure_hash {
                    self.workspace.board.document.begin_batch();
                    let p = self.cursor_point();
                    let mut item = Item::image(
                        hash,
                        Rect {
                            x: p[0] - 120.,
                            y: p[1] - 90.,
                            width: 240.,
                            height: 180.,
                        },
                        self.workspace.board.document.scene.top(),
                    );
                    if !paste {
                        item.name = self
                            .workspace
                            .board
                            .document
                            .storage
                            .source_name(done.request.handle);
                    }
                    self.workspace.board.document.burst.push(item.id);
                    self.workspace
                        .board
                        .persist(moth_platform::boards::Request::Apply(
                            Command::Insert(item),
                            Some(done.request.handle),
                        ));
                }
                self.workspace
                    .board
                    .document
                    .failed
                    .insert(done.request.handle);
                self.workspace.board.view.message = e.to_string();
                tracing::warn!(error=%e,"image load failed");
            }
        }
    }
    fn results(&mut self) {
        // Inactive tabs finish journal work, but share one GPU cache with the active tab.
        let gpu = self.shell.gpu.take();
        for index in 0..self.workspace.tabs.len() {
            std::mem::swap(&mut self.workspace.board, &mut self.workspace.tabs[index]);
            self.drain_results();
            std::mem::swap(&mut self.workspace.board, &mut self.workspace.tabs[index]);
        }
        self.shell.gpu = gpu;
        self.drain_results();
        self.library.pump(
            &self.shell.ctx,
            &mut self.workspace.board.view.message,
            &self.workspace.recent_paths,
            self.workspace.screen == Screen::Library,
        );
    }
    fn drain_results(&mut self) {
        while let Ok(done) = self.workspace.board.document.boards.results.try_recv() {
            use moth_platform::boards::Completion;
            match done {
                Completion::Recovery(paths) => self.workspace.recovery_paths = paths,
                Completion::Recent(paths) => {
                    if self.workspace.recent_paths.is_empty() {
                        self.workspace.recent_paths = paths;
                        self.library.refresh = true;
                    }
                }
                Completion::Preferences(values) => {
                    self.tools.load(&values);
                    self.shell.ctx.set_theme(if self.tools.dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    self.shell.ctx.set_zoom_factor(self.tools.ui_scale);
                }
                Completion::Settings(code) => {
                    if let Some(language) = moth_ui::Locale::from_code(&code) {
                        moth_ui::set_locale(language);
                    }
                }
                done => {
                    self.workspace.board.document.in_flight =
                        self.workspace.board.document.in_flight.saturating_sub(1);
                    match done {
                        Completion::Applied(command, source) => {
                            match self.workspace.board.document.scene.apply(command) {
                                Ok(_) => {
                                    if let Some((hash, handle)) = source {
                                        self.workspace.board.document.sources.insert(hash, handle);
                                    }
                                    self.workspace.board.document.dirty = true;
                                }
                                Err(e) => self.workspace.board.view.message = e.to_string(),
                            }
                            self.workspace.board.view.selection.retain(|id| {
                                self.workspace.board.document.scene.get(*id).is_some()
                            });
                        }
                        Completion::AppliedBatch(commands, sources) => {
                            let inserted: HashSet<_> = commands
                                .iter()
                                .filter_map(|c| match c {
                                    Command::Insert(item)
                                        if self
                                            .workspace
                                            .board
                                            .document
                                            .scene
                                            .get(item.id)
                                            .is_none() =>
                                    {
                                        Some(item.id)
                                    }
                                    _ => None,
                                })
                                .collect();
                            for command in commands {
                                if let Err(e) = self.workspace.board.document.scene.apply(command) {
                                    self.workspace.board.view.message = e.to_string();
                                }
                            }
                            for (hash, handle) in sources {
                                self.workspace.board.document.sources.insert(hash, handle);
                            }
                            if !inserted.is_empty() {
                                self.workspace.board.view.selection = inserted;
                            }
                            self.workspace.board.document.dirty = true;
                            self.workspace.board.view.selection.retain(|id| {
                                self.workspace.board.document.scene.get(*id).is_some()
                            });
                        }
                        Completion::Opened(scene, sources, recovered) => {
                            self.workspace.board.document.scene = scene;
                            self.workspace.board.document.sources = sources;
                            self.modal.take_editor();
                            self.workspace.recovery_paths.clear();
                            self.workspace.board.document.dirty = recovered;
                            self.workspace.board.view.message = moth_ui::text(if recovered {
                                moth_ui::Text::Recovered
                            } else {
                                moth_ui::Text::Opened
                            })
                            .into();
                            self.fit();
                        }
                        Completion::Imported(commands, sources, skipped) => {
                            self.workspace.board.document.scene = Scene::default();
                            self.modal.take_editor();
                            for command in commands {
                                if let Err(e) = self.workspace.board.document.scene.apply(command) {
                                    self.workspace.board.view.message = e.to_string();
                                }
                            }
                            self.workspace.board.document.sources = sources;
                            self.workspace.board.document.path = String::new();
                            self.workspace.board.document.dirty = true;
                            self.workspace.board.view.message = moth_ui::text(if skipped == 0 {
                                moth_ui::Text::Imported
                            } else {
                                moth_ui::Text::ImportSkipped
                            })
                            .into();
                            self.fit();
                        }
                        Completion::Exported(value) => {
                            self.workspace.board.view.message = if value.is_empty() {
                                moth_ui::text(moth_ui::Text::Exported).into()
                            } else {
                                value
                            };
                        }
                        Completion::Saved => {
                            self.workspace.board.document.dirty = false;
                            let path =
                                std::path::PathBuf::from(&self.workspace.board.document.path);
                            self.workspace.recent_paths.retain(|p| p != &path);
                            self.workspace.recent_paths.insert(0, path);
                            self.workspace
                                .recent_paths
                                .truncate(moth_platform::library::MAX_BOARDS);
                            self.library.refresh = true;
                            self.workspace.board.view.message =
                                moth_ui::text(moth_ui::Text::Saved).into();
                            if self.workspace.board.document.close_after_save {
                                self.workspace.board.discard();
                            }
                        }
                        Completion::Failed(e) => {
                            self.workspace.board.view.message = e;
                            self.workspace.board.document.close_after_save = false;
                        }
                        _ => {}
                    }
                }
            }
        }
        while let Ok(done) = self.workspace.board.document.decode.results.try_recv() {
            self.complete(done);
        }
        while let Ok(result) = self.workspace.board.document.clipboard.results.try_recv() {
            match result {
                moth_platform::ClipboardCompletion::Discarded(result) => match result {
                    Ok(()) => self.workspace.board.document.discarded = true,
                    Err(error) => {
                        self.workspace.board.document.discarding = false;
                        self.workspace.board.view.message = error.to_string();
                    }
                },
                moth_platform::ClipboardCompletion::Pasted(Ok((handle, image))) => {
                    self.workspace.board.document.pasting =
                        self.workspace.board.document.pasting.saturating_sub(1);
                    if self.workspace.board.document.scene.len()
                        + self.workspace.board.document.waiting.len()
                        + self.workspace.board.document.pending.len()
                        < 1_000_000
                    {
                        self.workspace
                            .board
                            .document
                            .pending
                            .insert(handle, Pending::Paste);
                        self.complete(moth_io::Completion {
                            request: moth_io::Request {
                                handle,
                                side: 256,
                                format: self.shell.image_format(),
                            },
                            result: Ok(image),
                            failure_hash: None,
                        });
                    } else {
                        self.workspace.board.view.message =
                            moth_ui::text(moth_ui::Text::BoardFull).into();
                    }
                }
                moth_platform::ClipboardCompletion::Files(paths) => {
                    self.workspace.board.document.pasting =
                        self.workspace.board.document.pasting.saturating_sub(1);
                    for path in paths {
                        self.import(path);
                    }
                }
                moth_platform::ClipboardCompletion::Pasted(Err(
                    moth_platform::PlatformError::EmptyClipboard,
                )) => {
                    self.workspace.board.document.pasting =
                        self.workspace.board.document.pasting.saturating_sub(1);
                    self.workspace.board.view.message =
                        moth_ui::text(moth_ui::Text::ClipboardEmpty).into();
                }
                moth_platform::ClipboardCompletion::Pasted(Err(e)) => {
                    self.workspace.board.document.pasting =
                        self.workspace.board.document.pasting.saturating_sub(1);
                    self.workspace.board.view.message = e.to_string();
                }
            }
        }
        self.workspace
            .board
            .document
            .pump(self.shell.image_format());
        if self.workspace.board.document.batching
            && self.workspace.board.document.idle()
            && self.workspace.board.document.arrange_prompt.is_none()
        {
            if !self.workspace.board.document.burst.is_empty()
                && (self.tools.auto_arrange
                    || self.workspace.board.document.burst.len() > self.tools.threshold)
            {
                self.workspace.board.document.arrange_prompt =
                    Some(std::mem::take(&mut self.workspace.board.document.burst));
            } else {
                self.workspace.board.document.burst.clear();
                self.workspace.board.document.end_batch();
            }
        }
    }
    fn fit(&mut self) {
        self.workspace.board.fit();
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
                    self.shell.redraw();
                }
                NativeEvent::Drop(path) => self.import(path),
                NativeEvent::Error(error) => {
                    tracing::warn!(%error, "native input failed");
                    self.workspace.board.view.message = error;
                    self.shell.redraw();
                }
            }
        }
    }
    fn frame(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let _span =
            tracing::trace_span!("frame", zoom = self.workspace.board.view.camera.zoom).entered();
        if self
            .shell
            .gpu
            .as_ref()
            .is_some_and(|g| g.lost.load(Ordering::Acquire))
        {
            self.recover()?;
        }
        let Some(window) = self.shell.window.clone() else {
            return Ok(());
        };
        self.results();
        if self.workspace.board.document.discarded {
            self.finish_close(event_loop)?;
            if self.workspace.closing_window
                && self.workspace.tabs.is_empty()
                && self.workspace.board.document.discarded
            {
                return Ok(());
            }
        }
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let Some(gpu) = &mut self.shell.gpu else {
            return Ok(());
        };
        let Some(state) = &mut self.shell.egui else {
            return Ok(());
        };
        let texture = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.recover()?;
                self.shell.redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            _ => return Err(anyhow::anyhow!("Surface validation failed")),
        };
        let mut actions = moth_ui::Actions::default();
        let mut close = false;
        let mut save = false;
        let mut open = None;
        let mut undo = false;
        let mut redo = false;
        let mut commands = Vec::new();
        let mut arranged: Option<(arrange::Arrange, Vec<Uuid>)> = None;
        let mut fit_selection = false;
        let mut commit_note = false;
        let mut add_note_at: Option<[f32; 2]> = None;
        let mut edit_note_id: Option<Uuid> = None;
        let mut export = None;
        let mut output = self.shell.ctx.run_ui(state.take_egui_input(&window), |ui| {
            if self.overlay.is_none() {
                let active_title = self.workspace.board.document.title().to_owned();
                let mut ordered = vec![(
                    self.workspace.board.ui_id,
                    moth_ui::Tab {
                        title: &active_title,
                        dirty: self.workspace.board.document.dirty,
                        active: true,
                        slot: 0,
                    },
                )];
                ordered.extend(self.workspace.tabs.iter().enumerate().map(|(i, tab)| {
                    (
                        tab.ui_id,
                        moth_ui::Tab {
                            title: tab.document.title(),
                            dirty: tab.document.dirty,
                            active: false,
                            slot: i + 1,
                        },
                    )
                }));
                ordered.sort_unstable_by_key(|(id, _)| *id);
                let tab_labels: Vec<_> = ordered.into_iter().map(|(_, tab)| tab).collect();
                actions = moth_ui::toolbar(
                    ui,
                    moth_ui::Status {
                        path: &self.workspace.board.document.path,
                        adapter: &gpu.adapter,
                        count: self.workspace.board.document.scene.len(),
                        selected: self.workspace.board.view.selection.len(),
                        zoom: self.workspace.board.view.camera.zoom,
                        texture_bytes: gpu.registry.allocated_bytes(),
                        message: &self.workspace.board.view.message,
                        idle: self.workspace.board.document.idle(),
                        dirty: self.workspace.board.document.dirty,
                        library: (self.workspace.screen == Screen::Library),
                        pinned: self.pinned,
                        can_pin: moth_platform::Desktop.window_capabilities(&window).on_top,
                    },
                    &mut self.workspace.board.view.pan,
                    &tab_labels,
                );
                if self.workspace.screen == Screen::Library {
                    for intent in self
                        .library
                        .show(ui, &mut self.workspace.board.view.message)
                    {
                        match intent {
                            Intent::NewBoard => actions.new = true,
                            Intent::OpenDialog => actions.open = true,
                            Intent::Open(path) => open = Some(path),
                            _ => {}
                        }
                    }
                }
            }
            if self.workspace.screen != Screen::Library && self.overlay.is_none() {
                egui::Panel::bottom("board_tools_button").show(ui, |ui| {
                    if ui.button(moth_ui::text(moth_ui::Text::Tools)).clicked() {
                        self.tools.open = !self.tools.open;
                    }
                });
                let (edits, target, note) = self.tools.show(
                    ui,
                    &mut self.workspace.board,
                    self.modal.editing().is_some(),
                );
                commands.extend(edits);
                export = target;
                if note {
                    add_note_at = Some(self.workspace.board.view.camera.world([
                        self.workspace.board.view.viewport.center().x,
                        self.workspace.board.view.viewport.center().y,
                    ]));
                }
            }
            if !self.workspace.recovery_paths.is_empty()
                && self.workspace.board.document.scene.is_empty()
            {
                egui::Window::new(moth_ui::text(moth_ui::Text::Recovery)).show(ui.ctx(), |ui| {
                    for path in &self.workspace.recovery_paths {
                        if ui
                            .add_enabled(
                                self.workspace.board.document.in_flight == 0,
                                egui::Button::new(path.display().to_string()),
                            )
                            .clicked()
                        {
                            open = Some(path.clone());
                        }
                    }
                    if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                        self.workspace.recovery_paths.clear();
                    }
                });
            }
            if self.workspace.screen != Screen::Library {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        let previous = self.workspace.board.view.viewport;
                        self.workspace.board.view.viewport = ui.max_rect();
                        if self.tools.scale_with_window
                            && previous.is_positive()
                            && previous.size() != self.workspace.board.view.viewport.size()
                        {
                            let anchor = self
                                .workspace
                                .board
                                .view
                                .camera
                                .world([previous.center().x, previous.center().y]);
                            self.workspace.board.view.camera.zoom =
                                (self.workspace.board.view.camera.zoom
                                    * (self.workspace.board.view.viewport.width()
                                        / previous.width())
                                    .min(
                                        self.workspace.board.view.viewport.height()
                                            / previous.height(),
                                    ))
                                .clamp(0.01, 64.);
                            self.workspace.board.view.camera.origin = [
                                anchor[0]
                                    - self.workspace.board.view.viewport.center().x
                                        / self.workspace.board.view.camera.zoom,
                                anchor[1]
                                    - self.workspace.board.view.viewport.center().y
                                        / self.workspace.board.view.camera.zoom,
                            ];
                        }
                        if self.workspace.board.view.needs_fit
                            && self.workspace.board.document.in_flight == 0
                        {
                            self.workspace.board.fit();
                        }
                        if self.workspace.board.document.scene.is_empty() {
                            let empty = moth_ui::empty_board(ui);
                            actions.paste |= empty.paste;
                            actions.open |= empty.open;
                        }
                        let origin = self.workspace.board.view.camera.world([
                            self.workspace.board.view.viewport.min.x,
                            self.workspace.board.view.viewport.min.y,
                        ]);
                        for item in self.workspace.board.document.scene.query(Rect {
                            x: origin[0],
                            y: origin[1],
                            width: self.workspace.board.view.viewport.width()
                                / self.workspace.board.view.camera.zoom,
                            height: self.workspace.board.view.viewport.height()
                                / self.workspace.board.view.camera.zoom,
                        }) {
                            match item.blob() {
                                // Non-resident images show a placeholder until decoded.
                                Some(blob) if !gpu.registry.resident(blob, 1) => {
                                    let p = self
                                        .workspace
                                        .board
                                        .view
                                        .camera
                                        .screen([item.rect.x, item.rect.y]);
                                    let rect = egui::Rect::from_min_size(
                                        egui::pos2(p[0], p[1]),
                                        egui::vec2(
                                            item.rect.width * self.workspace.board.view.camera.zoom,
                                            item.rect.height
                                                * self.workspace.board.view.camera.zoom,
                                        ),
                                    );
                                    ui.painter().rect_filled(rect, 2., moth_ui::BACKGROUND);
                                    ui.painter().rect_stroke(
                                        rect,
                                        2.,
                                        egui::Stroke::new(1., moth_ui::ACCENT),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                // Notes are drawn by the UI layer in screen space.
                                None => {
                                    if self.modal.editing().is_none_or(|e| e.id != item.id) {
                                        draw_note(ui, &self.workspace.board.view.camera, item);
                                    }
                                }
                                Some(_) => {}
                            }
                        }
                        for &id in &self.workspace.board.view.selection {
                            if self.overlay.is_some() {
                                break;
                            }
                            if let Some(item) = self.workspace.board.document.scene.get(id) {
                                let corners = screen_corners(
                                    &self.workspace.board.view.camera,
                                    item.rect,
                                    item.transform.rotation,
                                );
                                moth_ui::canvas::selection(ui, corners);
                            }
                        }
                        if self.overlay.is_none() {
                            let marquee = self.gesture.marquee().map(|(a, b)| {
                                let a = self.workspace.board.view.camera.world(a);
                                let b = self.workspace.board.view.camera.world(b);
                                Rect {
                                    x: a[0].min(b[0]),
                                    y: a[1].min(b[1]),
                                    width: (a[0] - b[0]).abs(),
                                    height: (a[1] - b[1]).abs(),
                                }
                            });
                            let mut bounds = egui::Rect::NOTHING;
                            let mut count = 0;
                            for item in self
                                .workspace
                                .board
                                .view
                                .selection
                                .iter()
                                .filter_map(|id| self.workspace.board.document.scene.get(*id))
                            {
                                let envelope = item.rect.rotated_envelope(item.transform.rotation);
                                let lo = envelope.lower();
                                let hi = envelope.upper();
                                bounds = bounds.union(egui::Rect::from_min_max(
                                    egui::pos2(lo[0], lo[1]),
                                    egui::pos2(hi[0], hi[1]),
                                ));
                                count += 1;
                            }
                            if let Some(rect) = marquee {
                                count = self.workspace.board.document.scene.query(rect).len();
                                bounds = egui::Rect::from_min_size(
                                    egui::pos2(rect.x, rect.y),
                                    egui::vec2(rect.width, rect.height),
                                );
                            }
                            if bounds.is_finite() {
                                moth_ui::canvas::selection_badge(
                                    ui.ctx(),
                                    self.workspace.board.view.viewport,
                                    count,
                                    bounds.size(),
                                );
                            }
                        }
                        if self.overlay.is_none() {
                            let response = ui.interact(
                                self.workspace.board.view.viewport,
                                egui::Id::new("canvas_context"),
                                egui::Sense::click(),
                            );
                            response.context_menu(|ui| {
                                if ui
                                    .add_enabled(
                                        self.workspace.board.document.idle()
                                            && self.modal.editing().is_none(),
                                        egui::Button::new(moth_ui::text(moth_ui::Text::AddNote)),
                                    )
                                    .clicked()
                                {
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        let world =
                                            self.workspace.board.view.camera.world([pos.x, pos.y]);
                                        add_note_at = Some(world);
                                    }
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(moth_ui::text(moth_ui::Text::Fit)).clicked() {
                                    actions.fit = true;
                                    ui.close();
                                }
                                ui.add_enabled_ui(self.workspace.board.document.idle(), |ui| {
                                    if ui.button(moth_ui::text(moth_ui::Text::Undo)).clicked() {
                                        undo = true;
                                        ui.close();
                                    }
                                    if ui.button(moth_ui::text(moth_ui::Text::Redo)).clicked() {
                                        redo = true;
                                        ui.close();
                                    }
                                    ui.separator();
                                    let enabled = !self.workspace.board.document.scene.is_empty();
                                    ui.add_enabled_ui(enabled, |ui| {
                                        let mut items: Vec<_> = self
                                            .workspace
                                            .board
                                            .document
                                            .scene
                                            .items()
                                            .filter(|i| {
                                                self.workspace.board.view.selection.is_empty()
                                                    || self
                                                        .workspace
                                                        .board
                                                        .view
                                                        .selection
                                                        .contains(&i.id)
                                            })
                                            .collect();
                                        items.sort_unstable_by(|a, b| {
                                            a.z.cmp(&b.z).then(a.id.cmp(&b.id))
                                        });
                                        if ui
                                            .button(moth_ui::text(moth_ui::Text::FitSelection))
                                            .clicked()
                                        {
                                            fit_selection = true;
                                            ui.close();
                                        }
                                        let target_width =
                                            self.workspace.board.view.viewport.width()
                                                / self.workspace.board.view.camera.zoom;
                                        ui.menu_button(
                                            moth_ui::text(moth_ui::Text::Arrange),
                                            |ui| {
                                                for (label, mode) in [
                                                    (
                                                        moth_ui::Text::Horizontal,
                                                        arrange::Arrange::Horizontal,
                                                    ),
                                                    (
                                                        moth_ui::Text::Vertical,
                                                        arrange::Arrange::Vertical,
                                                    ),
                                                    (moth_ui::Text::Grid, arrange::Arrange::Grid),
                                                    (
                                                        moth_ui::Text::Optimal,
                                                        arrange::Arrange::Optimal,
                                                    ),
                                                ] {
                                                    if ui.button(moth_ui::text(label)).clicked() {
                                                        commands.extend(self.tools.arrange(
                                                            &mut items,
                                                            mode,
                                                            target_width,
                                                        ));
                                                        ui.close();
                                                    }
                                                }
                                            },
                                        );
                                        ui.menu_button(
                                            moth_ui::text(moth_ui::Text::Normalize),
                                            |ui| {
                                                for (label, mode) in [
                                                    (
                                                        moth_ui::Text::NormalizeWidth,
                                                        arrange::Normalize::Width,
                                                    ),
                                                    (
                                                        moth_ui::Text::NormalizeHeight,
                                                        arrange::Normalize::Height,
                                                    ),
                                                    (
                                                        moth_ui::Text::NormalizeSize,
                                                        arrange::Normalize::Size,
                                                    ),
                                                    (
                                                        moth_ui::Text::NormalizeArea,
                                                        arrange::Normalize::Area,
                                                    ),
                                                ] {
                                                    if ui.button(moth_ui::text(label)).clicked() {
                                                        commands.extend(arrange::normalize(
                                                            &items, mode,
                                                        ));
                                                        ui.close();
                                                    }
                                                }
                                            },
                                        );
                                        for (label, quarters) in [
                                            (moth_ui::Text::RotateCw, 1.),
                                            (moth_ui::Text::RotateCcw, -1.),
                                        ] {
                                            if ui.button(moth_ui::text(label)).clicked() {
                                                commands.extend(transform_selection(
                                                    &self.workspace.board.document.scene,
                                                    &self.workspace.board.view.selection,
                                                    |transform| rotated(transform, quarters),
                                                ));
                                                ui.close();
                                            }
                                        }
                                        for (label, axis) in [
                                            (moth_ui::Text::FlipHorizontal, 0),
                                            (moth_ui::Text::FlipVertical, 1),
                                        ] {
                                            if ui.button(moth_ui::text(label)).clicked() {
                                                commands.extend(transform_selection(
                                                    &self.workspace.board.document.scene,
                                                    &self.workspace.board.view.selection,
                                                    |transform| mirrored(transform, axis),
                                                ));
                                                ui.close();
                                            }
                                        }
                                        if ui
                                            .button(moth_ui::text(moth_ui::Text::ResetTransform))
                                            .clicked()
                                        {
                                            self.tools.image_scale = Some((
                                                items.iter().map(|i| i.id).collect(),
                                                moth_platform::export::Geometry::All,
                                            ));
                                            ui.close();
                                        }
                                        if ui.button(moth_ui::text(moth_ui::Text::Delete)).clicked()
                                        {
                                            for item in &items {
                                                commands.push(Command::Remove(item.id));
                                            }
                                            ui.close();
                                        }
                                    });
                                });
                            });
                            // Double-click edits a note in place, or creates one on empty canvas.
                            if response.double_clicked()
                                && self.workspace.board.document.idle()
                                && let Some(pos) = response.interact_pointer_pos()
                            {
                                let world = self.workspace.board.view.camera.world([pos.x, pos.y]);
                                let hit = self
                                    .workspace
                                    .board
                                    .document
                                    .scene
                                    .query(Rect {
                                        x: world[0],
                                        y: world[1],
                                        width: 1. / self.workspace.board.view.camera.zoom,
                                        height: 1. / self.workspace.board.view.camera.zoom,
                                    })
                                    .into_iter()
                                    .rev()
                                    .find(|i| i.rect.contains_rotated(i.transform.rotation, world));
                                match hit {
                                    Some(item) if item.note().is_some() => {
                                        edit_note_id = Some(item.id)
                                    }
                                    Some(item) => {
                                        if self
                                            .workspace
                                            .board
                                            .view
                                            .focus_return
                                            .as_ref()
                                            .is_some_and(|(id, _)| *id == item.id)
                                        {
                                            if let Some((_, camera)) =
                                                self.workspace.board.view.focus_return.take()
                                            {
                                                self.workspace.board.view.camera = camera;
                                            }
                                        } else {
                                            self.workspace.board.view.focus_return =
                                                Some((item.id, self.workspace.board.view.camera));
                                            self.workspace.board.view.selection =
                                                HashSet::from([item.id]);
                                            fit_selection = true;
                                        }
                                    }
                                    None => add_note_at = Some(world),
                                }
                            }
                            // The inline note editor floats above the canvas.
                            if let Some(editing) = self.modal.editing_mut() {
                                let anchor =
                                    self.workspace.board.view.camera.screen(editing.anchor);
                                let zoom = self.workspace.board.view.camera.zoom;
                                egui::Area::new(egui::Id::new("note_editor"))
                                    .fixed_pos(egui::pos2(anchor[0], anchor[1]))
                                    .show(ui.ctx(), |ui| {
                                        let response =
                                            egui::TextEdit::multiline(&mut editing.draft)
                                                .desired_width((200. * zoom).max(120.))
                                                .desired_rows(3)
                                                .show(ui);
                                        if editing.focus {
                                            response.response.request_focus();
                                            editing.focus = false;
                                        }
                                        if response.response.lost_focus() {
                                            commit_note = true;
                                        }
                                    });
                            }
                        }
                        if !self.workspace.board.document.scene.is_empty() && self.overlay.is_none()
                        {
                            let tool = moth_ui::canvas::tools(
                                ui.ctx(),
                                self.workspace.board.view.viewport,
                                &mut self.workspace.board.view.pan,
                            );
                            actions.paste |= tool.paste;
                            actions.fit |= tool.fit;
                            actions.open |= tool.open;
                            actions.overlay |= tool.overlay;
                            if let Some(bounds) = self.workspace.board.document.scene.bounds() {
                                let normalize = |x: f32, y: f32| {
                                    egui::pos2(
                                        (x - bounds.x) / bounds.width,
                                        (y - bounds.y) / bounds.height,
                                    )
                                };
                                // The minimap samples at most 128 items; the indexed bounds still include every item.
                                let samples: Vec<_> = self
                                    .workspace
                                    .board
                                    .document
                                    .scene
                                    .items()
                                    .take(128)
                                    .map(|item| {
                                        let envelope =
                                            item.rect.rotated_envelope(item.transform.rotation);
                                        let lower = envelope.lower();
                                        let upper = envelope.upper();
                                        egui::Rect::from_min_max(
                                            normalize(lower[0], lower[1]),
                                            normalize(upper[0], upper[1]),
                                        )
                                    })
                                    .collect();
                                let world_min = self.workspace.board.view.camera.world([
                                    self.workspace.board.view.viewport.left(),
                                    self.workspace.board.view.viewport.top(),
                                ]);
                                let world_max = self.workspace.board.view.camera.world([
                                    self.workspace.board.view.viewport.right(),
                                    self.workspace.board.view.viewport.bottom(),
                                ]);
                                if let Some(p) = moth_ui::canvas::minimap(
                                    ui.ctx(),
                                    self.workspace.board.view.viewport,
                                    &samples,
                                    egui::Rect::from_min_max(
                                        normalize(world_min[0], world_min[1]),
                                        normalize(world_max[0], world_max[1]),
                                    ),
                                ) {
                                    self.workspace.board.view.camera.origin = [
                                        bounds.x + p.x * bounds.width
                                            - self.workspace.board.view.viewport.center().x
                                                / self.workspace.board.view.camera.zoom,
                                        bounds.y + p.y * bounds.height
                                            - self.workspace.board.view.viewport.center().y
                                                / self.workspace.board.view.camera.zoom,
                                    ];
                                }
                            }
                        }
                        if let Gesture::Moving {
                            start: a,
                            now: b,
                            origin,
                            preview,
                            ..
                        } = &self.gesture
                            && let Some((_, rect)) = origin.iter().find(|(id, _)| id == preview)
                        {
                            let moved = Rect {
                                x: rect.x + (b[0] - a[0]) / self.workspace.board.view.camera.zoom,
                                y: rect.y + (b[1] - a[1]) / self.workspace.board.view.camera.zoom,
                                ..*rect
                            };
                            let rotation = self
                                .workspace
                                .board
                                .document
                                .scene
                                .get(*preview)
                                .map_or(0., |item| item.transform.rotation);
                            let corners =
                                screen_corners(&self.workspace.board.view.camera, moved, rotation);
                            moth_ui::canvas::selection(ui, corners);
                        }
                        if let Some((a, b)) = self.gesture.marquee() {
                            ui.painter().rect_stroke(
                                egui::Rect::from_two_pos(
                                    egui::pos2(a[0], a[1]),
                                    egui::pos2(b[0], b[1]),
                                ),
                                0.,
                                egui::Stroke::new(1., moth_ui::ACCENT),
                                egui::StrokeKind::Inside,
                            );
                        }
                    });
            }
            if self.modal.path().is_some() {
                egui::Modal::new(egui::Id::new("board_path")).show(ui.ctx(), |ui| {
                    ui.set_width(ui.ctx().content_rect().width().min(460.) - 40.);
                    ui.heading(moth_ui::text(moth_ui::Text::PathDialog));
                    ui.label(moth_ui::text(moth_ui::Text::Path));
                    let Some((path, focus)) = self.modal.path_editor() else {
                        return;
                    };
                    let input = ui.add(
                        egui::TextEdit::singleline(path)
                            .char_limit(4096)
                            .desired_width(f32::INFINITY),
                    );
                    if *focus {
                        input.request_focus();
                        *focus = false;
                    }
                    let submit =
                        input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let empty = path.trim().is_empty();
                    let path_text = path.clone();
                    ui.horizontal(|ui| {
                        if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                            self.modal.dismiss_path();
                        }
                        if ui
                            .add_enabled(
                                !empty,
                                egui::Button::new(moth_ui::text(moth_ui::Text::Save)),
                            )
                            .clicked()
                            || submit
                        {
                            let mut path = std::path::PathBuf::from(&path_text);
                            if path.extension().is_none() {
                                path.set_extension("moth");
                            }
                            self.workspace.board.document.path =
                                path.to_string_lossy().into_owned();
                            save = true;
                            self.modal.dismiss_path();
                        }
                    });
                });
            }
            if self.modal.confirming() && self.modal.path().is_none() {
                egui::Modal::new(egui::Id::new("confirm_close")).show(ui.ctx(), |ui| {
                    ui.set_max_width(380.);
                    ui.heading(moth_ui::text(moth_ui::Text::SaveQuestion));
                    ui.add_space(8.);
                    ui.label(moth_ui::text(moth_ui::Text::CloseWarning));
                    ui.add_space(12.);
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(moth_ui::text(moth_ui::Text::Close))
                                        .color(egui::Color32::from_rgb(209, 133, 123)),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            close = true;
                        }
                        if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                            self.modal.keep_open();
                            self.workspace.closing_window = false;
                            self.workspace.board.document.close_after_save = false;
                        }
                        if ui
                            .add_enabled(
                                self.workspace.board.document.in_flight == 0
                                    && self.workspace.board.document.pasting == 0
                                    && self.workspace.board.document.pending.is_empty()
                                    && self.workspace.board.document.waiting.is_empty(),
                                egui::Button::new(
                                    egui::RichText::new(moth_ui::text(moth_ui::Text::Save))
                                        .color(moth_ui::BACKGROUND),
                                )
                                .fill(moth_ui::ACCENT),
                            )
                            .clicked()
                        {
                            if self.workspace.board.document.path.is_empty() {
                                self.modal.open_path();
                            } else {
                                save = true;
                            }
                            self.workspace.board.document.close_after_save = true;
                        }
                    });
                });
            }
            if let Some(ids) = self.workspace.board.document.arrange_prompt.clone() {
                if self.tools.auto_arrange {
                    arranged = Some((tools::layout(self.tools.default_arrange).1, ids.clone()));
                    self.workspace.board.document.arrange_prompt = None;
                } else {
                    egui::Modal::new(egui::Id::new("arrange_prompt")).show(ui.ctx(), |ui| {
                        ui.set_max_width(380.);
                        ui.heading(moth_ui::text(moth_ui::Text::ArrangePrompt));
                        ui.add_space(12.);
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            for (label, mode) in [
                                (moth_ui::Text::Horizontal, arrange::Arrange::Horizontal),
                                (moth_ui::Text::Vertical, arrange::Arrange::Vertical),
                                (moth_ui::Text::Grid, arrange::Arrange::Grid),
                                (moth_ui::Text::Optimal, arrange::Arrange::Optimal),
                            ] {
                                if ui.button(moth_ui::text(label)).clicked() {
                                    arranged = Some((mode, ids.clone()));
                                    self.workspace.board.document.arrange_prompt = None;
                                }
                            }
                            if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                                self.workspace.board.document.arrange_prompt = None;
                            }
                        });
                    });
                }
            }
        });
        state.handle_platform_output(&window, output.platform_output);
        if std::mem::take(&mut self.tools.reset_camera) {
            self.workspace.board.view.camera = Camera::default();
        }
        if self.tools.focus != 0 {
            let step = std::mem::take(&mut self.tools.focus);
            let mut items: Vec<_> = self
                .workspace
                .board
                .document
                .scene
                .items()
                .filter(|i| i.blob().is_some())
                .collect();
            items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
            if !items.is_empty() {
                let current = items
                    .iter()
                    .position(|i| self.workspace.board.view.selection.contains(&i.id));
                let next = current.map_or(if step > 0 { 0 } else { items.len() - 1 }, |i| {
                    (i as i32 + step).rem_euclid(items.len() as i32) as usize
                });
                self.workspace.board.view.selection = HashSet::from([items[next].id]);
                fit_selection = true;
            }
        }
        gpu.registry.grayscale = self.tools.grayscale;
        let p = self.workspace.board.view.camera.world([
            self.workspace.board.view.viewport.min.x,
            self.workspace.board.view.viewport.min.y,
        ]);
        let visible = self.workspace.board.document.scene.query(Rect {
            x: p[0],
            y: p[1],
            width: if self.workspace.screen == Screen::Library {
                0.
            } else {
                self.workspace.board.view.viewport.width() / self.workspace.board.view.camera.zoom
            },
            height: self.workspace.board.view.viewport.height()
                / self.workspace.board.view.camera.zoom,
        });
        for item in &visible {
            // Notes have no source bytes; only images request decode previews.
            let Some(blob) = item.blob() else {
                continue;
            };
            let side = moth_render::preview_side(
                item.rect.width.max(item.rect.height)
                    * self.workspace.board.view.camera.zoom
                    * output.pixels_per_point,
            );
            if !gpu.registry.resident(blob, side)
                && let Some(&handle) = self.workspace.board.document.sources.get(&blob)
                && !self.workspace.board.document.pending.contains_key(&handle)
                && !self.workspace.board.document.failed.contains(&handle)
                && self
                    .workspace
                    .board
                    .document
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
                    .document
                    .pending
                    .insert(handle, Pending::Preview(blob));
            }
        }
        let jobs = self
            .shell
            .ctx
            .tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: output.pixels_per_point,
        };
        for (id, delta) in &output.textures_delta.set {
            for part in delta {
                gpu.registry
                    .ui
                    .update_texture(&gpu.device, &gpu.queue, *id, part);
            }
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let buffers =
            gpu.registry
                .ui
                .update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
        let view = texture.texture.create_view(&Default::default());
        let scale = output.pixels_per_point;
        let draw_result = gpu.registry.draw(
            &gpu.device,
            &gpu.queue,
            moth_render::Target {
                grid: self.workspace.screen != Screen::Library
                    && !self.workspace.board.document.scene.is_empty()
                    && self.overlay.is_none(),
                opacity: self.overlay.as_ref().map_or(1., |overlay| overlay.opacity),
                view: &view,
                screen: [size.width as f32 / scale, size.height as f32 / scale],
                clip: Some(canvas_clip(
                    self.workspace.board.view.viewport,
                    scale,
                    [size.width, size.height],
                )),
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
            },
            &visible,
            self.workspace.board.view.camera,
        );
        if let Err(error) = draw_result {
            for id in &output.textures_delta.free {
                gpu.registry.ui.free_texture(id);
            }
            output.textures_delta.clear();
            return Err(error.into());
        }
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            gpu.registry
                .ui
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        gpu.queue
            .submit(buffers.into_iter().chain([encoder.finish()]));
        window.pre_present_notify();
        gpu.queue.present(texture);
        for id in &output.textures_delta.free {
            gpu.registry.ui.free_texture(id);
        }
        output.textures_delta.clear();
        self.shell.deadline = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|o| Instant::now().checked_add(o.repaint_delay));
        if std::mem::take(&mut self.tools.preference_dirty) {
            self.apply(Intent::Preferences, event_loop);
        }
        if std::mem::take(&mut self.tools.fit) {
            self.apply(Intent::ScheduleFit, event_loop);
        }
        if fit_selection {
            self.apply(Intent::FitSelection, event_loop);
        }
        if !commands.is_empty() {
            self.apply(Intent::Commands(commands), event_loop);
        }
        if let Some((mode, ids)) = arranged {
            self.apply(Intent::Arrange(mode, ids), event_loop);
        }
        if let Some((ids, mode)) = self.tools.image_scale.take() {
            self.apply(Intent::OriginalGeometry { ids, mode }, event_loop);
        }
        if let Some(target) = export {
            self.apply(Intent::Export(target), event_loop);
        }
        if actions.paste && !self.workspace.board.document.discarding {
            self.apply(Intent::Paste, event_loop);
        }
        if actions.fit {
            self.apply(Intent::FitAll, event_loop);
            self.shell.redraw();
        }
        if actions.zoom > 0. {
            self.apply(Intent::Zoom(actions.zoom), event_loop);
        }
        if actions.overlay {
            self.apply(Intent::ToggleOverlay, event_loop);
        }
        if actions.pin {
            self.apply(Intent::TogglePin, event_loop);
        }
        if actions.home {
            self.apply(Intent::Library, event_loop);
            self.workspace.board.view.viewport = egui::Rect::NOTHING;
        }
        if actions.new {
            self.apply(Intent::NewBoard, event_loop);
        }
        if let Some(index) = actions.tab {
            self.apply(Intent::ActivateTab(index), event_loop);
        }
        if let Some(index) = actions.close_tab {
            self.apply(Intent::CloseTab(index), event_loop);
        }
        if actions.close {
            self.apply(Intent::Close { window: true }, event_loop);
        }
        if actions.minimize {
            self.apply(Intent::Minimize, event_loop);
        }
        if actions.maximize {
            self.apply(Intent::ToggleMaximize, event_loop);
        }
        if actions.drag {
            self.apply(Intent::DragWindow, event_loop);
        }
        if let Some(language) = actions.language {
            self.apply(Intent::Language(language), event_loop);
        }
        if actions.open {
            self.apply(Intent::OpenDialog, event_loop);
        }
        if actions.save {
            if self.workspace.board.document.path.is_empty() {
                self.apply(Intent::SaveDialog, event_loop);
            } else {
                save = true;
            }
        }
        if let Some(path) = open {
            self.apply(Intent::Open(path), event_loop);
        }
        undo |= actions.undo;
        redo |= actions.redo;
        if save {
            self.apply(Intent::Save, event_loop);
        }
        if undo {
            self.apply(Intent::Undo, event_loop);
        }
        if redo {
            self.apply(Intent::Redo, event_loop);
        }
        if close {
            self.apply(Intent::Discard, event_loop);
        }
        if let Some(world) = add_note_at {
            self.apply(Intent::AddNote(world), event_loop);
        }
        if let Some(id) = edit_note_id {
            self.apply(Intent::EditNote(id), event_loop);
        }
        if commit_note {
            self.apply(Intent::CommitNote, event_loop);
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
            self.workspace.board.view.message = error.to_string();
        }
        if let Err(e) = self.recover() {
            tracing::error!(%e,"renderer creation failed");
            self.workspace.board.view.message = e.to_string();
            self.startup_error = Some(e);
            event_loop.exit();
        }
        let startup = std::mem::take(&mut self.workspace.startup);
        if !startup.is_empty() {
            self.workspace.screen = Screen::Board;
        }
        for path in startup {
            self.import(path);
        }
        self.shell.redraw();
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.input.event(&WindowEvent::Focused(false), 1.);
        self.native_pressed = None;
        self.gesture.cancel();
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
                if self.workspace.board.document.discarded
                    && let Err(error) = self.finish_close(event_loop)
                {
                    self.workspace.board.view.message = error.to_string();
                }
                self.shell.redraw();
            }
            Wake::Repaint(delay) => {
                if delay.is_zero() {
                    self.shell.redraw();
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
                        self.workspace.board.view.message = error.to_string();
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
                        self.workspace.board.view.message = error;
                        overlay.click_through = false;
                    } else {
                        overlay.applied = (overlay.on_top, overlay.click_through);
                    }
                }
            }
            if self.overlay.as_ref().is_some_and(|overlay| overlay.exit) {
                self.toggle_overlay(event_loop);
            }
            self.shell.redraw();
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
                self.shell.redraw();
            }
        }
        self.input.wheel = self.tools.wheel;
        if let Some(input) = self.input.event(&event, window.scale_factor()) {
            self.pointer(input);
            self.shell.redraw();
        }
        match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.frame(event_loop) {
                    tracing::error!(%e,"frame failed");
                    self.workspace.board.view.message = e.to_string();
                    if self.overlay.is_some() {
                        self.toggle_overlay(event_loop);
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.shell.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.shell.redraw();
            }
            WindowEvent::CloseRequested => {
                self.apply(Intent::Close { window: true }, event_loop);
            }
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
                self.gesture.cancel();
                self.gestures.cancel();
                self.command = false;
            }
            WindowEvent::DroppedFile(path) => self.import(path),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.keyboard(&event, event_loop);
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.shell.deadline {
            if deadline <= Instant::now() {
                self.shell.deadline = None;
                self.shell.redraw();
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
/// Frame corners in screen space, so the canvas can outline a rotated selection.
fn screen_corners(camera: &Camera, rect: Rect, rotation: f32) -> [egui::Pos2; 4] {
    rect.corners(rotation).map(|corner| {
        let point = camera.screen(corner);
        egui::pos2(point[0], point[1])
    })
}
/// Draws a note's card and text in screen space. shortcut: notes render in the
/// UI layer, above images, ignoring z-interleave with images; upgrade with a
/// text pipeline in the renderer when stacking between the two matters.
fn draw_note(ui: &egui::Ui, camera: &Camera, item: &Item) {
    let Some(text) = item.note() else {
        return;
    };
    let zoom = camera.zoom;
    let rows = text.lines().count().max(1) as f32;
    // The frame is the note's text bounds; recover the font size from its height
    // so an imported note scaled by BeeRef's `scale` renders at that size.
    let font_size = (item.rect.height / (rows * 1.3)).max(4.0) * zoom;
    let font = egui::FontId::proportional(font_size);
    let color = egui::Color32::from_rgb(224, 226, 231).gamma_multiply(item.transform.opacity);
    let fill = egui::Color32::from_rgba_unmultiplied(39, 42, 50, 235)
        .gamma_multiply(item.transform.opacity);
    let stroke = egui::Stroke::new(
        1.,
        egui::Color32::from_rgb(48, 52, 60).gamma_multiply(item.transform.opacity),
    );
    let rotation = item.transform.rotation;

    // Upright notes take the simple rounded-card path.
    if rotation.abs() < 1e-4 {
        let p = camera.screen([item.rect.x, item.rect.y]);
        let card = egui::Rect::from_min_size(
            egui::pos2(p[0], p[1]),
            egui::vec2(item.rect.width * zoom, item.rect.height * zoom),
        );
        ui.painter().rect_filled(card, 4. * zoom, fill);
        ui.painter()
            .rect_stroke(card, 4. * zoom, stroke, egui::StrokeKind::Inside);
        let mut y = p[1];
        for line in text.lines() {
            ui.painter().text(
                egui::pos2(p[0], y),
                egui::Align2::LEFT_TOP,
                line,
                font.clone(),
                color,
            );
            y += font_size * 1.3;
        }
        return;
    }

    // Rotated notes: polygon card plus per-line text rotated around its own top-left.
    let corners = item.rect.corners(rotation).map(|corner| {
        let s = camera.screen(corner);
        egui::pos2(s[0], s[1])
    });
    ui.painter().add(egui::Shape::convex_polygon(
        vec![corners[0], corners[1], corners[3], corners[2]],
        fill,
        stroke,
    ));
    let line_offset = egui::vec2(
        (corners[2].x - corners[0].x) / rows,
        (corners[2].y - corners[0].y) / rows,
    );
    for (i, line) in text.lines().enumerate() {
        let pos = corners[0] + line_offset * i as f32;
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(line.to_owned(), font.clone(), color));
        ui.painter().add(egui::Shape::Text(
            egui::epaint::TextShape::new(pos, galley, color).with_angle(rotation),
        ));
    }
}
/// Measures a note's auto-sized frame in world units, using egui's bundled font.
fn measure_note(ctx: &egui::Context, text: &str) -> [f32; 2] {
    let font_size = moth_scene::NOTE_FONT_SIZE;
    let font = egui::FontId::proportional(font_size);
    let color = egui::Color32::WHITE;
    let mut width = 0.0f32;
    let mut lines = 0usize;
    for line in text.lines() {
        let galley = ctx.fonts_mut(|f| f.layout_no_wrap(line.to_owned(), font.clone(), color));
        width = width.max(galley.size().x);
        lines += 1;
    }
    if text.is_empty() || !text.ends_with('\n') {
        lines = lines.max(1);
    }
    [width, lines as f32 * font_size * 1.3]
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
fn canvas_clip(viewport: egui::Rect, scale: f32, size: [u32; 2]) -> [u32; 4] {
    if !viewport.is_finite() || !viewport.is_positive() {
        return [0, 0, size[0], size[1]];
    }
    let min = [
        (viewport.left() * scale).clamp(0., size[0].saturating_sub(1) as f32) as u32,
        (viewport.top() * scale).clamp(0., size[1].saturating_sub(1) as f32) as u32,
    ];
    let max = [
        (viewport.right() * scale).clamp((min[0] + 1) as f32, size[0] as f32) as u32,
        (viewport.bottom() * scale).clamp((min[1] + 1) as f32, size[1] as f32) as u32,
    ];
    [min[0], min[1], max[0] - min[0], max[1] - min[1]]
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
    fn library_and_resized_canvas_keep_scissors_inside_the_surface() {
        assert_eq!(
            canvas_clip(egui::Rect::NOTHING, 1., [1440, 900]),
            [0, 0, 1440, 900]
        );
        assert_eq!(
            canvas_clip(
                egui::Rect::from_min_max(egui::pos2(-10., 20.), egui::pos2(2000., 3000.)),
                2.,
                [800, 600]
            ),
            [0, 40, 800, 560]
        );
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
