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
/// Ask how to arrange a burst of newly inserted images once it exceeds this count.
/// shortcut: fixed threshold until the settings store lands; expose `arrange_prompt_threshold` then.
const ARRANGE_PROMPT_THRESHOLD: usize = 5;
mod arrange;
mod overlay;
mod session;
use session::{MAX_TABS, Session};

pub struct App {
    /// Open boards in tab-bar order; `active` indexes the one in the window.
    tabs: Vec<Session>,
    active: usize,
    overlay: Option<overlay::Overlay>,
    pinned: bool,
    library_worker: moth_platform::library::Worker,
    cards: Vec<moth_ui::library::Card>,
    catalog: moth_ui::library::Library,
    refresh_library: bool,
    dialog: Option<()>,
    dialog_path: String,
    dialog_focus: bool,
    moving: Option<(Uuid, Rect)>,
    editing: Option<Editing>,
    library: bool,
    closing_window: bool,
    pub startup_error: Option<anyhow::Error>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    ctx: egui::Context,
    egui: Option<egui_winit::State>,
    wake: Arc<dyn Fn() + Send + Sync>,
    proxy: EventLoopProxy<Wake>,
    recovery_paths: Vec<std::path::PathBuf>,
    recent_paths: Vec<std::path::PathBuf>,
    input: Adapter,
    gestures: Gestures,
    cursor: Option<[f32; 2]>,
    command: bool,
    drag: Option<([f32; 2], [f32; 2], bool)>,
    drag_pointer: Option<(moth_platform::input::Kind, u64)>,
    native_pressed: Option<(moth_platform::input::Kind, u64)>,
    deadline: Option<Instant>,
    confirm_close: bool,
    software: bool,
    startup: Vec<std::path::PathBuf>,
}
impl App {
    /// Canvas texture format for decode requests; RGBA8 until the GPU exists.
    fn image_format(&self) -> moth_io::ImageFormat {
        self.gpu
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
        Ok(Self {
            tabs: vec![board],
            active: 0,
            overlay: None,
            pinned: false,
            library_worker,
            cards: Vec::new(),
            catalog: Default::default(),
            refresh_library: false,
            dialog: None,
            dialog_path: String::new(),
            dialog_focus: false,
            moving: None,
            editing: None,
            library: true,
            closing_window: false,
            startup_error: None,
            window: None,
            gpu: None,
            ctx,
            egui: None,
            wake,
            proxy,
            recovery_paths: Vec::new(),
            recent_paths: Vec::new(),
            input: Adapter::default(),
            gestures: Gestures::default(),
            cursor: None,
            command: false,
            drag: None,
            drag_pointer: None,
            native_pressed: None,
            deadline: None,
            confirm_close: false,
            software: std::env::args().any(|a| a == "--software"),
            startup: std::env::args_os()
                .skip(1)
                .filter(|a| a != "--software")
                .map(std::path::PathBuf::from)
                .collect(),
        })
    }
    fn toggle_overlay(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.overlay.is_some() {
            if let Err(error) =
                moth_platform::Desktop.window_effects(&window, false, self.pinned, false)
            {
                self.tabs[self.active].message = error;
            } else {
                self.overlay = None;
            }
        } else if self.gpu.as_ref().is_some_and(|gpu| gpu.transparent) {
            match overlay::Overlay::new(
                event_loop,
                self.wake.clone(),
                self.software,
                moth_platform::Desktop.window_capabilities(&window).on_top,
            ) {
                Ok(overlay) => {
                    if let Err(error) =
                        moth_platform::Desktop.window_effects(&window, true, overlay.on_top, false)
                    {
                        self.tabs[self.active].message = error;
                    }
                    self.overlay = Some(overlay);
                    self.library = false;
                    self.cards.clear();
                    self.catalog.dismiss_editor();
                }
                Err(error) => self.tabs[self.active].message = error.to_string(),
            }
        } else {
            self.tabs[self.active].message =
                moth_ui::text(moth_ui::Text::OverlayUnsupported).into();
        }
        self.redraw();
    }
    /// The board shown in the window; parked tabs keep their own sessions in `tabs`.
    fn board(&self) -> &Session {
        &self.tabs[self.active]
    }
    fn board_mut(&mut self) -> &mut Session {
        &mut self.tabs[self.active]
    }
    /// Shows `tabs[index]`; sessions never move, so every tab keeps its slot.
    fn activate(&mut self, index: usize) {
        if self.confirm_close || index >= self.tabs.len() {
            return;
        }
        self.active = index;
        self.library = false;
        self.cards.clear();
        self.catalog.dismiss_editor();
        self.editing = None;
        self.drag = None;
        self.moving = None;
        self.drag_pointer = None;
        self.gestures.cancel();
        if let Some(gpu) = &mut self.gpu {
            gpu.registry.clear();
        }
        self.redraw();
    }
    /// A blank, untitled and idle board is reused instead of opening another tab.
    fn reusable(board: &Session) -> bool {
        board.scene.is_empty() && board.path.is_empty() && board.idle()
    }
    fn new_board(&mut self) {
        if self.confirm_close {
            return;
        }
        if Self::reusable(self.board()) {
            self.activate(self.active);
            return;
        }
        if self.tabs.len() >= MAX_TABS {
            self.board_mut().message = moth_ui::text(moth_ui::Text::TabLimit).into();
            return;
        }
        match Session::new(self.wake.clone(), false) {
            Ok(board) => {
                self.tabs.push(board);
                self.activate(self.tabs.len() - 1);
            }
            Err(error) => self.board_mut().message = error.to_string(),
        }
    }
    /// Makes the active tab reusable for new content, opening a tab when it is not.
    /// Returns false when no tab could be provided.
    fn blank_board(&mut self) -> bool {
        if Self::reusable(self.board()) {
            return true;
        }
        let count = self.tabs.len();
        self.new_board();
        self.tabs.len() != count
    }
    fn open_in_tab(&mut self, path: std::path::PathBuf) {
        if self.confirm_close {
            return;
        }
        if let Some(index) = self
            .tabs
            .iter()
            .position(|b| std::path::Path::new(&b.path) == path)
        {
            self.activate(index);
            return;
        }
        if !self.blank_board() {
            return;
        }
        self.library = false;
        self.cards.clear();
        self.catalog.dismiss_editor();
        self.open_board(path);
    }
    fn request_close(&mut self, window: bool) {
        self.closing_window = window;
        self.library = false;
        if self.board().dirty || !self.board().idle() {
            self.confirm_close = true;
        } else {
            self.discard();
        }
        self.redraw();
    }
    /// Drops the discarded active session, or exits when it was the last one and the
    /// window is closing (the discarded session stays so `frame` can stop drawing).
    fn finish_close(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        self.confirm_close = false;
        if self.tabs.len() > 1 {
            self.tabs.remove(self.active);
            self.active = self.active.min(self.tabs.len() - 1);
            self.activate(self.active);
            if self.closing_window {
                self.request_close(true);
            }
        } else if self.closing_window {
            event_loop.exit();
        } else {
            self.tabs[0] = Session::new(self.wake.clone(), false)?;
            self.active = 0;
            self.library = true;
            self.refresh_library = true;
            if let Some(gpu) = &mut self.gpu {
                gpu.registry.clear();
            }
        }
        Ok(())
    }
    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
    fn recover(&mut self) -> anyhow::Result<()> {
        self.native_pressed = None;
        self.gpu = None;
        self.cards.clear();
        self.catalog.dismiss_editor();
        self.refresh_library = self.library;
        self.ctx = egui::Context::default();
        moth_ui::theme(&self.ctx);
        let proxy = self.proxy.clone();
        self.ctx.set_request_repaint_callback(move |info| {
            let _ = proxy.send_event(Wake::Repaint(info.delay));
        });
        if let Some(window) = &self.window {
            self.gpu = Some(pollster::block_on(Gpu::new(
                window.clone(),
                self.wake.clone(),
                self.software,
            ))?);
            self.egui = Some(egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                window.theme(),
                Some(2048),
            ));
        }
        Ok(())
    }
    fn paste(&mut self) {
        if self.tabs[self.active].discarding {
            return;
        }
        // A new paste supersedes a pending arrangement prompt.
        self.tabs[self.active].arrange_prompt = None;
        match self.tabs[self.active]
            .clipboard
            .requests
            .try_send(moth_platform::ClipboardRequest::Paste)
        {
            Ok(()) => self.tabs[self.active].pasting += 1,
            Err(_) => {
                self.tabs[self.active].message = moth_ui::text(moth_ui::Text::ClipboardBusy).into()
            }
        }
    }

    fn persist(&mut self, request: moth_platform::boards::Request) {
        match self.tabs[self.active].boards.requests.try_send(request) {
            Ok(()) => self.tabs[self.active].in_flight += 1,
            Err(_) => {
                self.tabs[self.active].message = moth_ui::text(moth_ui::Text::StorageBusy).into()
            }
        }
    }
    /// Sends a whole command list as one undo step.
    fn persist_batch(&mut self, commands: Vec<(Command, Option<Handle>)>) {
        self.persist(moth_platform::boards::Request::ApplyBatch(commands));
    }
    /// Opens an undo group for a burst of asynchronous inserts (drop/paste).
    fn begin_batch(&mut self) {
        if !self.tabs[self.active].batching
            && self.tabs[self.active]
                .boards
                .requests
                .try_send(moth_platform::boards::Request::BeginBatch)
                .is_ok()
        {
            self.tabs[self.active].batching = true;
        }
    }
    /// Closes the undo group once the burst has fully drained to an idle board.
    fn end_batch(&mut self) {
        if self.tabs[self.active].batching
            && self.tabs[self.active]
                .boards
                .requests
                .try_send(moth_platform::boards::Request::EndBatch)
                .is_ok()
        {
            self.tabs[self.active].batching = false;
        }
    }
    fn discard(&mut self) {
        if self.tabs[self.active].in_flight > 0
            || self.tabs[self.active].pasting > 0
            || !self.tabs[self.active].pending.is_empty()
            || !self.tabs[self.active].waiting.is_empty()
        {
            self.tabs[self.active].message = moth_ui::text(moth_ui::Text::StorageBusy).into();
            return;
        }
        if !self.tabs[self.active].discarding {
            match self.tabs[self.active]
                .clipboard
                .requests
                .try_send(moth_platform::ClipboardRequest::Discard)
            {
                Ok(()) => self.tabs[self.active].discarding = true,
                Err(_) => {
                    self.tabs[self.active].message =
                        moth_ui::text(moth_ui::Text::ClipboardBusy).into()
                }
            }
        }
    }
    fn open_board(&mut self, path: std::path::PathBuf) {
        if self.tabs[self.active].scene.is_empty()
            && self.tabs[self.active].in_flight == 0
            && self.tabs[self.active].pasting == 0
            && self.tabs[self.active].pending.is_empty()
            && self.tabs[self.active].waiting.is_empty()
            && !self.tabs[self.active].discarding
        {
            self.tabs[self.active].path = path.to_string_lossy().into_owned();
            self.persist(moth_platform::boards::Request::Open(path));
        } else {
            self.tabs[self.active].message = moth_ui::text(moth_ui::Text::OpenEmpty).into();
        }
    }
    fn import_bee(&mut self, path: std::path::PathBuf) {
        if self.confirm_close || !self.blank_board() {
            return;
        }
        self.library = false;
        self.cards.clear();
        self.catalog.dismiss_editor();
        self.board_mut().path = String::new();
        if self.board().idle() {
            self.persist(moth_platform::boards::Request::ImportBee(path));
        }
    }
    fn import(&mut self, path: std::path::PathBuf) {
        if self.tabs[self.active].discarding {
            return;
        }
        // A new drop supersedes a pending arrangement prompt.
        self.tabs[self.active].arrange_prompt = None;
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
            .is_some_and(|e| e.eq_ignore_ascii_case("bee"))
        {
            self.import_bee(path);
            return;
        }
        if self.tabs[self.active].scene.len()
            + self.tabs[self.active].waiting.len()
            + self.tabs[self.active].pending.len()
            >= 1_000_000
        {
            self.tabs[self.active].message = moth_ui::text(moth_ui::Text::BoardFull).into();
            return;
        }
        if self.tabs[self.active].waiting.len() >= 1024 {
            self.tabs[self.active].message = moth_ui::text(moth_ui::Text::ImportBusy).into();
            return;
        }
        match self.tabs[self.active].storage.register(path) {
            Ok(h) => self.tabs[self.active].waiting.push(h),
            Err(e) => self.tabs[self.active].message = e.to_string(),
        }
        self.pump();
    }
    fn open_files(&mut self) {
        if self.confirm_close {
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
                if self.library {
                    self.new_board();
                }
                self.import(path);
            }
        }
    }
    fn pump(&mut self) {
        while let Some(&handle) = self.tabs[self.active].waiting.last() {
            match self.tabs[self.active]
                .decode
                .requests
                .try_send(moth_io::Request {
                    handle,
                    side: 256,
                    format: self.image_format(),
                }) {
                Ok(()) => {
                    self.tabs[self.active].waiting.pop();
                    self.tabs[self.active]
                        .pending
                        .insert(handle, Pending::Import);
                }
                Err(_) => break,
            }
        }
    }
    fn insertion_point(&self) -> [f32; 2] {
        if self.tabs[self.active].viewport.is_finite() {
            let p = self.tabs[self.active].viewport.center();
            return self.tabs[self.active].camera.world([p.x, p.y]);
        }
        let point = self
            .window
            .as_ref()
            .map(|w| {
                let size = w.inner_size().to_logical::<f32>(w.scale_factor());
                [size.width / 2., size.height / 2.]
            })
            .unwrap_or([0., 0.]);
        self.tabs[self.active].camera.world(point)
    }
    /// The cursor in world space, falling back to the viewport centre when unknown or off-canvas.
    fn cursor_point(&self) -> [f32; 2] {
        if let Some(cursor) = self.cursor
            && self.tabs[self.active].viewport.is_finite()
            && self.tabs[self.active]
                .viewport
                .contains(egui::pos2(cursor[0], cursor[1]))
        {
            return self.tabs[self.active].camera.world(cursor);
        }
        self.insertion_point()
    }
    /// Creates an empty note at `world` and opens the inline editor for it.
    fn add_note(&mut self, world: [f32; 2]) {
        if self.tabs[self.active].discarding || !self.tabs[self.active].idle() {
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
            self.tabs[self.active].scene.top(),
        );
        let id = item.id;
        self.persist(moth_platform::boards::Request::Apply(
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
        let Some(item) = self.tabs[self.active].scene.get(id) else {
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
            let [width, height] = measure_note(&self.ctx, &editing.draft);
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
            if self.tabs[self.active]
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
        self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
    }
    fn complete(&mut self, done: moth_io::Completion) {
        let Some(pending) = self.tabs[self.active].pending.remove(&done.request.handle) else {
            return;
        };
        let paste = matches!(pending, Pending::Paste);
        let insert = paste || matches!(pending, Pending::Import);
        match done.result {
            Ok(image) => {
                if matches!(pending,Pending::Preview(hash) if hash!=image.hash) {
                    self.tabs[self.active].failed.insert(done.request.handle);
                    self.tabs[self.active].message =
                        moth_ui::text(moth_ui::Text::SourceChanged).into();
                    return;
                }
                if insert {
                    self.begin_batch();
                    // Both pastes and drops land under the cursor; drops cascade to stay visible.
                    let (p, offset) = if paste {
                        (self.cursor_point(), 0.)
                    } else {
                        (
                            self.cursor_point(),
                            (self.tabs[self.active].scene.len() % 10) as f32 * 24.,
                        )
                    };
                    let width = (image.original[0] as f32).min(600.);
                    let height = width * image.original[1] as f32 / image.original[0] as f32;
                    let item = Item::image(
                        image.hash,
                        Rect {
                            x: p[0] - width / 2. + offset,
                            y: p[1] - height / 2. + offset,
                            width,
                            height,
                        },
                        self.tabs[self.active].scene.top(),
                    );
                    self.tabs[self.active].burst.push(item.id);
                    self.persist(moth_platform::boards::Request::Apply(
                        Command::Insert(item),
                        Some(done.request.handle),
                    ));
                }
                if let Some(gpu) = &mut self.gpu {
                    gpu.registry.upload(&gpu.device, &gpu.queue, &image);
                }
            }
            Err(e) => {
                if insert && let Some(hash) = done.failure_hash {
                    self.begin_batch();
                    let p = self.cursor_point();
                    let item = Item::image(
                        hash,
                        Rect {
                            x: p[0] - 120.,
                            y: p[1] - 90.,
                            width: 240.,
                            height: 180.,
                        },
                        self.tabs[self.active].scene.top(),
                    );
                    self.tabs[self.active].burst.push(item.id);
                    self.persist(moth_platform::boards::Request::Apply(
                        Command::Insert(item),
                        Some(done.request.handle),
                    ));
                }
                self.tabs[self.active].failed.insert(done.request.handle);
                self.tabs[self.active].message = e.to_string();
                tracing::warn!(error=%e,"image load failed");
            }
        }
    }
    fn results(&mut self) {
        // Inactive tabs finish journal work, but share one GPU cache with the active tab.
        let gpu = self.gpu.take();
        let active = self.active;
        for index in 0..self.tabs.len() {
            if index != active {
                self.active = index;
                self.drain_results();
            }
        }
        // Draining may open a tab (a dropped .bee), but never removes one.
        self.active = active;
        self.gpu = gpu;
        self.drain_results();
        if self.library
            && self.refresh_library
            && self
                .library_worker
                .requests
                .try_send(moth_platform::library::Request::Refresh(
                    self.recent_paths.clone(),
                ))
                .is_ok()
        {
            self.refresh_library = false;
        }
        while let Ok(result) = self.library_worker.results.try_recv() {
            match result {
                Ok(entry) if self.library => {
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
                            self.ctx.load_texture(
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
                Err(error) => self.tabs[self.active].message = error,
                _ => {}
            }
        }
    }
    fn drain_results(&mut self) {
        while let Ok(done) = self.tabs[self.active].boards.results.try_recv() {
            use moth_platform::boards::Completion;
            match done {
                Completion::Recovery(paths) => self.recovery_paths = paths,
                Completion::Recent(paths) => {
                    if self.recent_paths.is_empty() {
                        self.recent_paths = paths;
                        self.refresh_library = true;
                    }
                }
                Completion::Settings(code) => {
                    if let Some(language) = moth_ui::Locale::from_code(&code) {
                        moth_ui::set_locale(language);
                    }
                }
                done => {
                    self.tabs[self.active].in_flight =
                        self.tabs[self.active].in_flight.saturating_sub(1);
                    match done {
                        Completion::Applied(command, source) => {
                            match self.tabs[self.active].scene.apply(command) {
                                Ok(_) => {
                                    if let Some((hash, handle)) = source {
                                        self.tabs[self.active].sources.insert(hash, handle);
                                    }
                                    self.tabs[self.active].dirty = true;
                                }
                                Err(e) => self.tabs[self.active].message = e.to_string(),
                            }
                            let board = self.board_mut();
                            board.selection.retain(|id| board.scene.get(*id).is_some());
                        }
                        Completion::AppliedBatch(commands, sources) => {
                            for command in commands {
                                if let Err(e) = self.tabs[self.active].scene.apply(command) {
                                    self.tabs[self.active].message = e.to_string();
                                }
                            }
                            for (hash, handle) in sources {
                                self.tabs[self.active].sources.insert(hash, handle);
                            }
                            self.tabs[self.active].dirty = true;
                            let board = self.board_mut();
                            board.selection.retain(|id| board.scene.get(*id).is_some());
                        }
                        Completion::Opened(scene, sources, recovered) => {
                            self.tabs[self.active].scene = scene;
                            self.tabs[self.active].sources = sources;
                            self.editing = None;
                            self.recovery_paths.clear();
                            self.tabs[self.active].dirty = recovered;
                            self.tabs[self.active].message = moth_ui::text(if recovered {
                                moth_ui::Text::Recovered
                            } else {
                                moth_ui::Text::Opened
                            })
                            .into();
                            self.fit();
                        }
                        Completion::Imported(commands, sources, skipped) => {
                            self.tabs[self.active].scene = Scene::default();
                            self.editing = None;
                            for command in commands {
                                if let Err(e) = self.tabs[self.active].scene.apply(command) {
                                    self.tabs[self.active].message = e.to_string();
                                }
                            }
                            self.tabs[self.active].sources = sources;
                            self.tabs[self.active].path = String::new();
                            self.tabs[self.active].dirty = true;
                            self.tabs[self.active].message = moth_ui::text(if skipped == 0 {
                                moth_ui::Text::Imported
                            } else {
                                moth_ui::Text::ImportSkipped
                            })
                            .into();
                            self.fit();
                        }
                        Completion::Saved => {
                            self.tabs[self.active].dirty = false;
                            let path = std::path::PathBuf::from(&self.tabs[self.active].path);
                            self.recent_paths.retain(|p| p != &path);
                            self.recent_paths.insert(0, path);
                            self.recent_paths
                                .truncate(moth_platform::library::MAX_BOARDS);
                            self.refresh_library = true;
                            self.tabs[self.active].message =
                                moth_ui::text(moth_ui::Text::Saved).into();
                            if self.tabs[self.active].close_after_save {
                                self.discard();
                            }
                        }
                        Completion::Failed(e) => {
                            self.tabs[self.active].message = e;
                            self.tabs[self.active].close_after_save = false;
                        }
                        _ => {}
                    }
                }
            }
        }
        while let Ok(done) = self.tabs[self.active].decode.results.try_recv() {
            self.complete(done);
        }
        while let Ok(result) = self.tabs[self.active].clipboard.results.try_recv() {
            match result {
                moth_platform::ClipboardCompletion::Discarded(result) => match result {
                    Ok(()) => self.tabs[self.active].discarded = true,
                    Err(error) => {
                        self.tabs[self.active].discarding = false;
                        self.tabs[self.active].message = error.to_string();
                    }
                },
                moth_platform::ClipboardCompletion::Pasted(Ok((handle, image))) => {
                    self.tabs[self.active].pasting =
                        self.tabs[self.active].pasting.saturating_sub(1);
                    if self.tabs[self.active].scene.len()
                        + self.tabs[self.active].waiting.len()
                        + self.tabs[self.active].pending.len()
                        < 1_000_000
                    {
                        self.tabs[self.active]
                            .pending
                            .insert(handle, Pending::Paste);
                        self.complete(moth_io::Completion {
                            request: moth_io::Request {
                                handle,
                                side: 256,
                                format: self.image_format(),
                            },
                            result: Ok(image),
                            failure_hash: None,
                        });
                    } else {
                        self.tabs[self.active].message =
                            moth_ui::text(moth_ui::Text::BoardFull).into();
                    }
                }
                moth_platform::ClipboardCompletion::Files(paths) => {
                    self.tabs[self.active].pasting =
                        self.tabs[self.active].pasting.saturating_sub(1);
                    for path in paths {
                        self.import(path);
                    }
                }
                moth_platform::ClipboardCompletion::Pasted(Err(
                    moth_platform::PlatformError::EmptyClipboard,
                )) => {
                    self.tabs[self.active].pasting =
                        self.tabs[self.active].pasting.saturating_sub(1);
                    self.tabs[self.active].message =
                        moth_ui::text(moth_ui::Text::ClipboardEmpty).into();
                }
                moth_platform::ClipboardCompletion::Pasted(Err(e)) => {
                    self.tabs[self.active].pasting =
                        self.tabs[self.active].pasting.saturating_sub(1);
                    self.tabs[self.active].message = e.to_string();
                }
            }
        }
        self.pump();
        if self.tabs[self.active].batching && self.tabs[self.active].idle() {
            self.end_batch();
            if self.tabs[self.active].burst.len() > ARRANGE_PROMPT_THRESHOLD {
                self.tabs[self.active].arrange_prompt =
                    Some(std::mem::take(&mut self.tabs[self.active].burst));
            } else {
                self.tabs[self.active].burst.clear();
            }
        }
    }
    fn fit(&mut self) {
        self.tabs[self.active].fit();
    }
    /// Rotation, mirroring and reset keys; they act on the selection and need an idle board.
    fn transform_keys(&self, event: &KeyEvent) -> Option<Vec<Command>> {
        if self.command
            || self.ctx.egui_wants_keyboard_input()
            || self.tabs[self.active].selection.is_empty()
            || !self.tabs[self.active].idle()
        {
            return None;
        }
        let shift = self.ctx.input(|input| input.modifiers.shift);
        let transform_selection = |change: fn(Transform) -> Transform| {
            transform_selection(
                &self.tabs[self.active].scene,
                &self.tabs[self.active].selection,
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
            (PhysicalKey::Code(KeyCode::Digit0), true) => {
                transform_selection(|_| Transform::default())
            }
            _ => return None,
        })
    }
    fn native_input(&mut self) {
        let events: Vec<_> = self.input.native().collect();
        for event in events {
            match event {
                NativeEvent::Pointer(p) => {
                    let modifiers = self.ctx.input(|input| input.modifiers);
                    if let Some(state) = &mut self.egui {
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
                    self.tabs[self.active].message = error;
                    self.redraw();
                }
            }
        }
    }
    fn pointer(&mut self, input: Input) {
        self.cursor = Some(match &input {
            Input::Zoom { position, .. } => *position,
            Input::Pointer(p) => p.position,
        });
        if self.confirm_close || self.library || self.dialog.is_some() {
            return;
        }
        match input {
            Input::Zoom { position, factor } => {
                if self.tabs[self.active]
                    .viewport
                    .contains(egui::pos2(position[0], position[1]))
                {
                    self.tabs[self.active].camera.zoom_at(position, factor);
                }
            }
            Input::Pointer(p) => {
                if p.phase == Phase::Down
                    && self
                        .ctx
                        .layer_id_at(egui::pos2(p.position[0], p.position[1]))
                        .is_some_and(|layer| layer.order != egui::Order::Background)
                {
                    return;
                }
                if p.phase == Phase::Down
                    && !self.tabs[self.active]
                        .viewport
                        .contains(egui::pos2(p.position[0], p.position[1]))
                {
                    return;
                }
                if let Some(g) = self.gestures.update(p) {
                    self.drag = None;
                    self.moving = None;
                    self.drag_pointer = None;
                    self.tabs[self.active].camera.zoom_at(g.anchor, g.zoom);
                    self.tabs[self.active].camera.origin[0] -=
                        g.pan[0] / self.tabs[self.active].camera.zoom;
                    self.tabs[self.active].camera.origin[1] -=
                        g.pan[1] / self.tabs[self.active].camera.zoom;
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
                        if self.tabs[self.active]
                            .viewport
                            .contains(egui::pos2(p.position[0], p.position[1])) =>
                    {
                        if !p.pan && !self.tabs[self.active].pan && self.tabs[self.active].idle() {
                            let world = self.tabs[self.active].camera.world(p.position);
                            if let Some(hit) = self.tabs[self.active]
                                .scene
                                .query(Rect {
                                    x: world[0],
                                    y: world[1],
                                    width: 1. / self.tabs[self.active].camera.zoom,
                                    height: 1. / self.tabs[self.active].camera.zoom,
                                })
                                .into_iter()
                                .rev()
                                // The index only guarantees the rotated envelope, so the
                                // click must still land inside the frame itself.
                                .find(|item| {
                                    item.rect.contains_rotated(item.transform.rotation, world)
                                })
                                .map(|item| (item.id, item.rect))
                            {
                                self.moving = Some(hit);
                                self.tabs[self.active].selection.clear();
                                self.tabs[self.active].selection.insert(hit.0);
                            }
                        }
                        self.drag =
                            Some((p.position, p.position, p.pan || self.tabs[self.active].pan));
                        self.drag_pointer = Some((p.kind, p.id));
                    }
                    Phase::Move => {
                        if let Some((_, last, pan)) = self.drag.as_mut() {
                            if *pan {
                                self.tabs[self.active].camera.origin[0] -=
                                    (p.position[0] - last[0]) / self.tabs[self.active].camera.zoom;
                                self.tabs[self.active].camera.origin[1] -=
                                    (p.position[1] - last[1]) / self.tabs[self.active].camera.zoom;
                            }
                            *last = p.position;
                        }
                    }
                    Phase::Up => {
                        self.drag_pointer = None;
                        if let Some((start, _, false)) = self.drag.take() {
                            if let Some((id, rect)) = self.moving.take() {
                                let dx =
                                    (p.position[0] - start[0]) / self.tabs[self.active].camera.zoom;
                                let dy =
                                    (p.position[1] - start[1]) / self.tabs[self.active].camera.zoom;
                                let moved = Rect {
                                    x: rect.x + dx,
                                    y: rect.y + dy,
                                    ..rect
                                };
                                if moved.valid()
                                    && dx.abs() + dy.abs()
                                        > 0.5 / self.tabs[self.active].camera.zoom
                                {
                                    self.persist(moth_platform::boards::Request::Apply(
                                        Command::SetRect { id, rect: moved },
                                        None,
                                    ));
                                }
                                return;
                            }
                            let a = self.tabs[self.active].camera.world(start);
                            let b = self.tabs[self.active].camera.world(p.position);
                            let rect = Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0])
                                    .abs()
                                    .max(1. / self.tabs[self.active].camera.zoom),
                                height: (a[1] - b[1])
                                    .abs()
                                    .max(1. / self.tabs[self.active].camera.zoom),
                            };
                            self.tabs[self.active].selection = self.tabs[self.active]
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
            tracing::trace_span!("frame", zoom = self.tabs[self.active].camera.zoom).entered();
        if self
            .gpu
            .as_ref()
            .is_some_and(|g| g.lost.load(Ordering::Acquire))
        {
            self.recover()?;
        }
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        self.results();
        if self.tabs[self.active].discarded {
            self.finish_close(event_loop)?;
            if self.closing_window && self.tabs.len() == 1 && self.board().discarded {
                return Ok(());
            }
        }
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let Some(state) = &mut self.egui else {
            return Ok(());
        };
        let texture = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.recover()?;
                self.redraw();
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
        let mut output = self.ctx.run_ui(state.take_egui_input(&window), |ui| {
            if self.overlay.is_none() {
                // Titles are copied so the toolbar can borrow the active board mutably.
                let titles: Vec<_> = self
                    .tabs
                    .iter()
                    .map(|tab| (tab.title().to_owned(), tab.dirty))
                    .collect();
                let tab_labels: Vec<_> = titles
                    .iter()
                    .enumerate()
                    .map(|(slot, (title, dirty))| moth_ui::Tab {
                        title,
                        dirty: *dirty,
                        active: slot == self.active,
                        slot,
                    })
                    .collect();
                let board = &mut self.tabs[self.active];
                actions = moth_ui::toolbar(
                    ui,
                    moth_ui::Status {
                        path: &board.path,
                        adapter: &gpu.adapter,
                        count: board.scene.len(),
                        selected: board.selection.len(),
                        zoom: board.camera.zoom,
                        texture_bytes: gpu.registry.allocated_bytes(),
                        message: &board.message,
                        idle: board.idle(),
                        dirty: board.dirty,
                        library: self.library,
                        pinned: self.pinned,
                        can_pin: moth_platform::Desktop.window_capabilities(&window).on_top,
                    },
                    &mut board.pan,
                    &tab_labels,
                );
                if self.library {
                    let intent = self.catalog.show(ui, &self.cards);
                    actions.new |= intent.new;
                    actions.open |= intent.browse;
                    if let Some(index) = intent.open {
                        open = Some(self.cards[index].path.clone());
                    }
                    if let Some((index, tags)) = intent.tags
                        && self
                            .library_worker
                            .requests
                            .try_send(moth_platform::library::Request::Tags(
                                self.cards[index].path.clone(),
                                tags,
                            ))
                            .is_err()
                    {
                        self.tabs[self.active].message =
                            moth_ui::text(moth_ui::Text::StorageBusy).into();
                    }
                    if let Some(index) = intent.pin {
                        let card = &self.cards[index];
                        if self
                            .library_worker
                            .requests
                            .try_send(moth_platform::library::Request::Pin(
                                card.path.clone(),
                                !card.pinned,
                            ))
                            .is_err()
                        {
                            self.tabs[self.active].message =
                                moth_ui::text(moth_ui::Text::StorageBusy).into();
                        }
                    }
                }
            }
            if !self.recovery_paths.is_empty() && self.tabs[self.active].scene.is_empty() {
                egui::Window::new(moth_ui::text(moth_ui::Text::Recovery)).show(ui.ctx(), |ui| {
                    for path in &self.recovery_paths {
                        if ui
                            .add_enabled(
                                self.tabs[self.active].in_flight == 0,
                                egui::Button::new(path.display().to_string()),
                            )
                            .clicked()
                        {
                            open = Some(path.clone());
                        }
                    }
                    if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                        self.recovery_paths.clear();
                    }
                });
            }
            if !self.library {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        self.tabs[self.active].viewport = ui.max_rect();
                        if self.tabs[self.active].needs_fit {
                            self.tabs[self.active].fit();
                        }
                        if self.tabs[self.active].scene.is_empty() {
                            let empty = moth_ui::empty_board(ui);
                            actions.paste |= empty.paste;
                            actions.open |= empty.open;
                        }
                        let origin = self.tabs[self.active].camera.world([
                            self.tabs[self.active].viewport.min.x,
                            self.tabs[self.active].viewport.min.y,
                        ]);
                        for item in self.tabs[self.active].scene.query(Rect {
                            x: origin[0],
                            y: origin[1],
                            width: self.tabs[self.active].viewport.width()
                                / self.tabs[self.active].camera.zoom,
                            height: self.tabs[self.active].viewport.height()
                                / self.tabs[self.active].camera.zoom,
                        }) {
                            match item.blob() {
                                // Non-resident images show a placeholder until decoded.
                                Some(blob) if !gpu.registry.resident(blob, 1) => {
                                    let p = self.tabs[self.active]
                                        .camera
                                        .screen([item.rect.x, item.rect.y]);
                                    let rect = egui::Rect::from_min_size(
                                        egui::pos2(p[0], p[1]),
                                        egui::vec2(
                                            item.rect.width * self.tabs[self.active].camera.zoom,
                                            item.rect.height * self.tabs[self.active].camera.zoom,
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
                                    if self.editing.as_ref().is_none_or(|e| e.id != item.id) {
                                        draw_note(ui, &self.tabs[self.active].camera, item);
                                    }
                                }
                                Some(_) => {}
                            }
                        }
                        for &id in &self.tabs[self.active].selection {
                            if self.overlay.is_some() {
                                break;
                            }
                            if let Some(item) = self.tabs[self.active].scene.get(id) {
                                let corners = screen_corners(
                                    &self.tabs[self.active].camera,
                                    item.rect,
                                    item.transform.rotation,
                                );
                                moth_ui::canvas::selection(ui, corners);
                            }
                        }
                        if self.overlay.is_none() {
                            let response = ui.interact(
                                self.tabs[self.active].viewport,
                                egui::Id::new("canvas_context"),
                                egui::Sense::click(),
                            );
                            response.context_menu(|ui| {
                                if ui
                                    .add_enabled(
                                        self.tabs[self.active].idle() && self.editing.is_none(),
                                        egui::Button::new(moth_ui::text(moth_ui::Text::AddNote)),
                                    )
                                    .clicked()
                                {
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        let world =
                                            self.tabs[self.active].camera.world([pos.x, pos.y]);
                                        add_note_at = Some(world);
                                    }
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(moth_ui::text(moth_ui::Text::Fit)).clicked() {
                                    actions.fit = true;
                                    ui.close();
                                }
                                ui.add_enabled_ui(self.tabs[self.active].idle(), |ui| {
                                    if ui.button(moth_ui::text(moth_ui::Text::Undo)).clicked() {
                                        undo = true;
                                        ui.close();
                                    }
                                    if ui.button(moth_ui::text(moth_ui::Text::Redo)).clicked() {
                                        redo = true;
                                        ui.close();
                                    }
                                    ui.separator();
                                    let enabled = !self.tabs[self.active].selection.is_empty();
                                    ui.add_enabled_ui(enabled, |ui| {
                                        let mut items: Vec<_> = self.tabs[self.active]
                                            .selection
                                            .iter()
                                            .filter_map(|id| self.tabs[self.active].scene.get(*id))
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
                                        let target_width = self.tabs[self.active].viewport.width()
                                            / self.tabs[self.active].camera.zoom;
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
                                                        commands.extend(arrange::arrange(
                                                            &items,
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
                                                    &self.tabs[self.active].scene,
                                                    &self.tabs[self.active].selection,
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
                                                    &self.tabs[self.active].scene,
                                                    &self.tabs[self.active].selection,
                                                    |transform| mirrored(transform, axis),
                                                ));
                                                ui.close();
                                            }
                                        }
                                        if ui
                                            .button(moth_ui::text(moth_ui::Text::ResetTransform))
                                            .clicked()
                                        {
                                            commands.extend(transform_selection(
                                                &self.tabs[self.active].scene,
                                                &self.tabs[self.active].selection,
                                                |_| Transform::default(),
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
                                && self.tabs[self.active].idle()
                                && let Some(pos) = response.interact_pointer_pos()
                            {
                                let world = self.tabs[self.active].camera.world([pos.x, pos.y]);
                                let hit = self.tabs[self.active]
                                    .scene
                                    .query(Rect {
                                        x: world[0],
                                        y: world[1],
                                        width: 1. / self.tabs[self.active].camera.zoom,
                                        height: 1. / self.tabs[self.active].camera.zoom,
                                    })
                                    .into_iter()
                                    .rev()
                                    .find(|i| i.rect.contains_rotated(i.transform.rotation, world));
                                match hit {
                                    Some(item) if item.note().is_some() => {
                                        edit_note_id = Some(item.id)
                                    }
                                    None => add_note_at = Some(world),
                                    _ => {}
                                }
                            }
                            // The inline note editor floats above the canvas.
                            if let Some(editing) = &mut self.editing {
                                let anchor = self.tabs[self.active].camera.screen(editing.anchor);
                                let zoom = self.tabs[self.active].camera.zoom;
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
                        if !self.tabs[self.active].scene.is_empty() && self.overlay.is_none() {
                            let tool = moth_ui::canvas::tools(
                                ui.ctx(),
                                self.tabs[self.active].viewport,
                                &mut self.tabs[self.active].pan,
                            );
                            actions.paste |= tool.paste;
                            actions.fit |= tool.fit;
                            actions.open |= tool.open;
                            actions.overlay |= tool.overlay;
                            if let Some(bounds) = self.tabs[self.active].scene.bounds() {
                                let normalize = |x: f32, y: f32| {
                                    egui::pos2(
                                        (x - bounds.x) / bounds.width,
                                        (y - bounds.y) / bounds.height,
                                    )
                                };
                                // The minimap samples at most 128 items; the indexed bounds still include every item.
                                let samples: Vec<_> = self.tabs[self.active]
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
                                let world_min = self.tabs[self.active].camera.world([
                                    self.tabs[self.active].viewport.left(),
                                    self.tabs[self.active].viewport.top(),
                                ]);
                                let world_max = self.tabs[self.active].camera.world([
                                    self.tabs[self.active].viewport.right(),
                                    self.tabs[self.active].viewport.bottom(),
                                ]);
                                if let Some(p) = moth_ui::canvas::minimap(
                                    ui.ctx(),
                                    self.tabs[self.active].viewport,
                                    &samples,
                                    egui::Rect::from_min_max(
                                        normalize(world_min[0], world_min[1]),
                                        normalize(world_max[0], world_max[1]),
                                    ),
                                ) {
                                    self.tabs[self.active].camera.origin = [
                                        bounds.x + p.x * bounds.width
                                            - self.tabs[self.active].viewport.center().x
                                                / self.tabs[self.active].camera.zoom,
                                        bounds.y + p.y * bounds.height
                                            - self.tabs[self.active].viewport.center().y
                                                / self.tabs[self.active].camera.zoom,
                                    ];
                                }
                            }
                        }
                        if let (Some((_, rect)), Some((a, b, false))) = (self.moving, self.drag) {
                            let moved = Rect {
                                x: rect.x + (b[0] - a[0]) / self.tabs[self.active].camera.zoom,
                                y: rect.y + (b[1] - a[1]) / self.tabs[self.active].camera.zoom,
                                ..rect
                            };
                            let rotation = self
                                .moving
                                .and_then(|(id, _)| self.tabs[self.active].scene.get(id))
                                .map_or(0., |item| item.transform.rotation);
                            let corners =
                                screen_corners(&self.tabs[self.active].camera, moved, rotation);
                            moth_ui::canvas::selection(ui, corners);
                        }
                        if let Some((a, b, false)) = self.drag
                            && self.moving.is_none()
                        {
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
            if self.dialog.is_some() {
                egui::Modal::new(egui::Id::new("board_path")).show(ui.ctx(), |ui| {
                    ui.set_width(ui.ctx().content_rect().width().min(460.) - 40.);
                    ui.heading(moth_ui::text(moth_ui::Text::PathDialog));
                    ui.label(moth_ui::text(moth_ui::Text::Path));
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.dialog_path)
                            .char_limit(4096)
                            .desired_width(f32::INFINITY),
                    );
                    if self.dialog_focus {
                        input.request_focus();
                        self.dialog_focus = false;
                    }
                    let submit =
                        input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                            self.dialog = None;
                        }
                        if ui
                            .add_enabled(
                                !self.dialog_path.trim().is_empty(),
                                egui::Button::new(moth_ui::text(moth_ui::Text::Save)),
                            )
                            .clicked()
                            || submit
                        {
                            let mut path = std::path::PathBuf::from(&self.dialog_path);
                            if path.extension().is_none() {
                                path.set_extension("moth");
                            }
                            self.tabs[self.active].path = path.to_string_lossy().into_owned();
                            save = true;
                            self.dialog = None;
                        }
                    });
                });
            }
            if self.confirm_close && self.dialog.is_none() {
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
                            self.confirm_close = false;
                            self.closing_window = false;
                            self.tabs[self.active].close_after_save = false;
                        }
                        if ui
                            .add_enabled(
                                self.tabs[self.active].in_flight == 0
                                    && self.tabs[self.active].pasting == 0
                                    && self.tabs[self.active].pending.is_empty()
                                    && self.tabs[self.active].waiting.is_empty(),
                                egui::Button::new(
                                    egui::RichText::new(moth_ui::text(moth_ui::Text::Save))
                                        .color(moth_ui::BACKGROUND),
                                )
                                .fill(moth_ui::ACCENT),
                            )
                            .clicked()
                        {
                            if self.tabs[self.active].path.is_empty() {
                                self.dialog_path.clear();
                                self.dialog = Some(());
                                self.dialog_focus = true;
                            } else {
                                save = true;
                            }
                            self.tabs[self.active].close_after_save = true;
                        }
                    });
                });
            }
            if let Some(ids) = self.tabs[self.active].arrange_prompt.clone() {
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
                                self.tabs[self.active].arrange_prompt = None;
                            }
                        }
                        if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                            self.tabs[self.active].arrange_prompt = None;
                        }
                    });
                });
            }
        });
        state.handle_platform_output(&window, output.platform_output);
        let board = &mut self.tabs[self.active];
        let p = board
            .camera
            .world([board.viewport.min.x, board.viewport.min.y]);
        let visible = board.scene.query(Rect {
            x: p[0],
            y: p[1],
            width: if self.library {
                0.
            } else {
                board.viewport.width() / board.camera.zoom
            },
            height: board.viewport.height() / board.camera.zoom,
        });
        for item in &visible {
            // Notes have no source bytes; only images request decode previews.
            let Some(blob) = item.blob() else {
                continue;
            };
            let side = moth_render::preview_side(
                item.rect.width.max(item.rect.height) * board.camera.zoom * output.pixels_per_point,
            );
            if !gpu.registry.resident(blob, side)
                && let Some(&handle) = board.sources.get(&blob)
                && !board.pending.contains_key(&handle)
                && !board.failed.contains(&handle)
                && board
                    .decode
                    .requests
                    .try_send(moth_io::Request {
                        handle,
                        side,
                        format: gpu.registry.image_format(),
                    })
                    .is_ok()
            {
                board.pending.insert(handle, Pending::Preview(blob));
            }
        }
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
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
                grid: !self.library
                    && !self.tabs[self.active].scene.is_empty()
                    && self.overlay.is_none(),
                opacity: self.overlay.as_ref().map_or(1., |overlay| overlay.opacity),
                view: &view,
                screen: [size.width as f32 / scale, size.height as f32 / scale],
                clip: Some(canvas_clip(
                    self.tabs[self.active].viewport,
                    scale,
                    [size.width, size.height],
                )),
                clear: if self.overlay.is_some() {
                    wgpu::Color::TRANSPARENT
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
            self.tabs[self.active].camera,
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
        self.deadline = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|o| Instant::now().checked_add(o.repaint_delay));
        if fit_selection {
            let selected: Vec<_> = self.tabs[self.active]
                .selection
                .iter()
                .filter_map(|id| self.tabs[self.active].scene.get(*id))
                .collect();
            if !selected.is_empty() {
                let min = selected.iter().fold([f32::INFINITY; 2], |a, i| {
                    [a[0].min(i.rect.x), a[1].min(i.rect.y)]
                });
                let max = selected.iter().fold([f32::NEG_INFINITY; 2], |a, i| {
                    [
                        a[0].max(i.rect.x + i.rect.width),
                        a[1].max(i.rect.y + i.rect.height),
                    ]
                });
                self.tabs[self.active].camera.zoom =
                    ((self.tabs[self.active].viewport.width() - 48.) / (max[0] - min[0]))
                        .min((self.tabs[self.active].viewport.height() - 48.) / (max[1] - min[1]))
                        .clamp(0.01, 64.);
                self.tabs[self.active].camera.origin = [
                    (min[0] + max[0]) / 2.
                        - self.tabs[self.active].viewport.center().x
                            / self.tabs[self.active].camera.zoom,
                    (min[1] + max[1]) / 2.
                        - self.tabs[self.active].viewport.center().y
                            / self.tabs[self.active].camera.zoom,
                ];
                self.redraw();
            }
        }
        if !commands.is_empty() {
            self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
        }
        if let Some((mode, ids)) = arranged {
            let mut items: Vec<_> = ids
                .iter()
                .filter_map(|id| self.tabs[self.active].scene.get(*id))
                .collect();
            items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
            let target_width =
                self.tabs[self.active].viewport.width() / self.tabs[self.active].camera.zoom;
            let commands = arrange::arrange(&items, mode, target_width);
            if !commands.is_empty() {
                self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
            }
        }
        if actions.paste && !self.tabs[self.active].discarding {
            self.paste();
        }
        if actions.fit {
            self.fit();
            self.redraw();
        }
        if actions.zoom > 0. {
            self.tabs[self.active].camera.zoom_at(
                [
                    self.tabs[self.active].viewport.center().x,
                    self.tabs[self.active].viewport.center().y,
                ],
                actions.zoom,
            );
            self.redraw();
        }
        if actions.overlay {
            self.toggle_overlay(event_loop);
        }
        if actions.pin {
            self.pinned = !self.pinned;
            if let Err(error) = moth_platform::Desktop.window_effects(
                &window,
                self.overlay.is_some(),
                self.pinned,
                false,
            ) {
                self.tabs[self.active].message = error;
            }
        }
        if actions.home {
            self.library = true;
            self.refresh_library = true;
            self.tabs[self.active].viewport = egui::Rect::NOTHING;
        }
        if actions.new {
            self.new_board();
        }
        if let Some(index) = actions.tab {
            self.activate(index);
        }
        if let Some(index) = actions.close_tab {
            self.activate(index);
            self.request_close(false);
        }
        if actions.close {
            self.request_close(true);
        }
        if actions.minimize {
            window.set_minimized(true);
        }
        if actions.maximize {
            window.set_maximized(!window.is_maximized());
        }
        if actions.drag {
            let _ = window.drag_window();
        }
        if let Some(language) = actions.language {
            moth_ui::set_locale(language);
            self.persist(moth_platform::boards::Request::SaveSettings(
                language.code().to_owned(),
            ));
        }
        if actions.open {
            self.open_files();
        }
        if actions.save {
            if self.tabs[self.active].path.is_empty() {
                self.dialog = Some(());
                self.dialog_focus = true;
                self.dialog_path.clear();
            } else {
                save = true;
            }
        }
        if let Some(path) = open {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
            {
                self.open_in_tab(path);
            } else {
                if self.library {
                    self.new_board();
                }
                self.import(path);
            }
        }
        undo |= actions.undo;
        redo |= actions.redo;
        if save {
            self.persist(moth_platform::boards::Request::Save(
                std::path::PathBuf::from(&self.tabs[self.active].path),
            ));
        }
        if undo {
            self.persist(moth_platform::boards::Request::Undo);
        }
        if redo {
            self.persist(moth_platform::boards::Request::Redo);
        }
        if close {
            self.discard();
        }
        if let Some(world) = add_note_at {
            self.add_note(world);
        }
        if let Some(id) = edit_note_id {
            self.edit_note(id);
        }
        if commit_note {
            self.finish_editing();
        }
        Ok(())
    }
}
impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match event_loop.create_window(
                Window::default_attributes()
                    .with_title(moth_ui::text(moth_ui::Text::Title))
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_inner_size(winit::dpi::LogicalSize::new(1440., 900.)),
            ) {
                Ok(w) => self.window = Some(Arc::new(w)),
                Err(e) => {
                    tracing::error!(%e,"window creation failed");
                    self.startup_error = Some(e.into());
                    event_loop.exit();
                    return;
                }
            }
        }
        if let Some(window) = &self.window
            && let Err(error) = self.input.attach(
                window.clone(),
                event_loop.owned_display_handle(),
                self.wake.clone(),
            )
        {
            tracing::warn!(%error, "native input startup failed");
            self.tabs[self.active].message = error.to_string();
        }
        if let Err(e) = self.recover() {
            tracing::error!(%e,"renderer creation failed");
            self.tabs[self.active].message = e.to_string();
            self.startup_error = Some(e);
            event_loop.exit();
        }
        let startup = std::mem::take(&mut self.startup);
        if !startup.is_empty() {
            self.library = false;
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
        self.gpu = None;
        self.egui = None;
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wake) {
        tracing::trace!(?event, "wake");
        match event {
            Wake::Shutdown => event_loop.exit(),
            Wake::Ready => {
                self.native_input();
                self.results();
                if self.tabs[self.active].discarded
                    && let Err(error) = self.finish_close(event_loop)
                {
                    self.tabs[self.active].message = error.to_string();
                }
                self.redraw();
            }
            Wake::Repaint(delay) => {
                if delay.is_zero() {
                    self.redraw();
                } else if let Some(deadline) = Instant::now().checked_add(delay) {
                    self.deadline = Some(self.deadline.map_or(deadline, |d| d.min(deadline)));
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
                        self.tabs[self.active].message = error.to_string();
                        overlay.exit = true;
                    }
                } else {
                    overlay.event(&event);
                }
                if overlay.applied != (overlay.on_top, overlay.click_through) {
                    if let Some(window) = &self.window
                        && let Err(error) = moth_platform::Desktop.window_effects(
                            window,
                            true,
                            overlay.on_top,
                            overlay.click_through,
                        )
                    {
                        self.tabs[self.active].message = error;
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
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Some(state) = &mut self.egui {
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
        if let Some(input) = self.input.event(&event, window.scale_factor()) {
            self.pointer(input);
            self.redraw();
        }
        match event {
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.frame(event_loop) {
                    tracing::error!(%e,"frame failed");
                    self.tabs[self.active].message = e.to_string();
                    if self.overlay.is_some() {
                        self.toggle_overlay(event_loop);
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::CloseRequested => self.request_close(true),
            WindowEvent::Focused(false) => {
                if let (Some((kind, id)), Some(state)) = (self.native_pressed, &mut self.egui) {
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
                if self.dialog.is_some()
                    || self.confirm_close
                    || self.tabs[self.active].arrange_prompt.is_some()
                {
                    return;
                }
                if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyN)
                {
                    self.new_board();
                } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyO) {
                    self.open_files();
                } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyL) {
                    self.library = true;
                    self.refresh_library = true;
                    self.redraw();
                } else if self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyW) {
                    self.request_close(false);
                } else if self.command && event.logical_key == Key::Named(NamedKey::Tab) {
                    self.activate((self.active + 1) % self.tabs.len());
                } else if (self.overlay.is_some()
                    && event.logical_key == Key::Named(NamedKey::Escape))
                    || (self.command && event.physical_key == PhysicalKey::Code(KeyCode::KeyT))
                {
                    self.toggle_overlay(event_loop);
                } else if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && (event.physical_key == PhysicalKey::Code(KeyCode::KeyV)
                        || matches!(event.logical_key,Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
                {
                    if !self.tabs[self.active].discarding {
                        self.paste();
                    }
                } else if self.command
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyS)
                    && self.tabs[self.active].idle()
                {
                    if self.tabs[self.active].path.is_empty() {
                        self.dialog = Some(());
                        self.dialog_focus = true;
                        self.dialog_path.clear();
                        self.redraw();
                    } else {
                        self.persist(moth_platform::boards::Request::Save(
                            std::path::PathBuf::from(&self.tabs[self.active].path),
                        ));
                    }
                } else if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyZ)
                    && self.tabs[self.active].in_flight == 0
                {
                    self.persist(moth_platform::boards::Request::Undo);
                } else if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyY)
                    && self.tabs[self.active].in_flight == 0
                {
                    self.persist(moth_platform::boards::Request::Redo);
                } else if let Some(commands) = self.transform_keys(&event) {
                    self.persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                } else if !self.ctx.egui_wants_keyboard_input()
                    && event.logical_key == Key::Character("v".into())
                {
                    self.tabs[self.active].pan = false;
                    self.redraw();
                } else if !self.ctx.egui_wants_keyboard_input()
                    && event.logical_key == Key::Character("h".into())
                {
                    self.tabs[self.active].pan = true;
                    self.redraw();
                } else if event.logical_key == Key::Named(NamedKey::Home) {
                    self.fit();
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.deadline {
            if deadline <= Instant::now() {
                self.deadline = None;
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
    let color = egui::Color32::from_rgb(224, 226, 231);
    let fill = egui::Color32::from_rgba_unmultiplied(39, 42, 50, 235);
    let stroke = egui::Stroke::new(1., egui::Color32::from_rgb(48, 52, 60));
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
