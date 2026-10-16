// SPDX-License-Identifier: GPL-3.0-or-later
use crate::gpu::Gpu;
use moth_format::Handle;
use moth_platform::input::{Adapter, Gestures, Input, Phase};
use moth_scene::{BlobId, Camera, Command, Item, Rect, Scene, Uuid};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
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
    Preview(BlobId),
}
pub struct App {
    pub startup_error: Option<anyhow::Error>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    ctx: egui::Context,
    egui: Option<egui_winit::State>,
    wake: Arc<dyn Fn() + Send + Sync>,
    proxy: EventLoopProxy<Wake>,
    storage: Arc<moth_platform::DesktopStorage>,
    decode: moth_io::DecodePool,
    clipboard: moth_platform::ClipboardWorker,
    scene: Scene,
    boards: moth_platform::boards::Worker,
    in_flight: usize,
    pasting: usize,
    path: String,
    recovery_paths: Vec<std::path::PathBuf>,
    recent_paths: Vec<std::path::PathBuf>,
    dirty: bool,
    close_after_save: bool,
    camera: Camera,
    input: Adapter,
    gestures: Gestures,
    pan: bool,
    command: bool,
    drag: Option<([f32; 2], [f32; 2], bool)>,
    drag_pointer: Option<(moth_platform::input::Kind, u64)>,
    selection: HashSet<Uuid>,
    sources: HashMap<BlobId, Handle>,
    pending: HashMap<Handle, Pending>,
    failed: HashSet<Handle>,
    waiting: Vec<Handle>,
    viewport: egui::Rect,
    message: String,
    deadline: Option<Instant>,
    confirm_close: bool,
    discarding: bool,
    discarded: bool,
    software: bool,
    startup: Vec<std::path::PathBuf>,
}
impl App {
    pub fn new(proxy: EventLoopProxy<Wake>) -> anyhow::Result<Self> {
        let ready = proxy.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = ready.send_event(Wake::Ready);
        });
        let storage = Arc::new(moth_platform::DesktopStorage::default());
        let decode = moth_io::DecodePool::new(storage.clone(), wake.clone())?;
        let clipboard = moth_platform::ClipboardWorker::new(storage.clone(), wake.clone())?;
        let boards = moth_platform::boards::Worker::new(storage.clone(), wake.clone())?;
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx);
        let repaint = proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            let _ = repaint.send_event(Wake::Repaint(info.delay));
        });
        Ok(Self {
            startup_error: None,
            window: None,
            gpu: None,
            ctx,
            egui: None,
            wake,
            proxy,
            storage,
            decode,
            clipboard,
            scene: Scene::default(),
            boards,
            in_flight: 0,
            pasting: 0,
            path: String::new(),
            recovery_paths: Vec::new(),
            recent_paths: Vec::new(),
            dirty: false,
            close_after_save: false,
            camera: Camera::default(),
            input: Adapter::default(),
            gestures: Gestures::default(),
            pan: true,
            command: false,
            drag: None,
            drag_pointer: None,
            selection: HashSet::new(),
            sources: HashMap::new(),
            pending: HashMap::new(),
            failed: HashSet::new(),
            waiting: Vec::new(),
            viewport: egui::Rect::NOTHING,
            message: String::new(),
            deadline: None,
            confirm_close: false,
            discarding: false,
            discarded: false,
            software: std::env::args().any(|a| a == "--software"),
            startup: std::env::args_os()
                .skip(1)
                .filter(|a| a != "--software")
                .map(std::path::PathBuf::from)
                .collect(),
        })
    }
    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
    fn recover(&mut self) -> anyhow::Result<()> {
        self.gpu = None;
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
        if self.discarding {
            return;
        }
        match self
            .clipboard
            .requests
            .try_send(moth_platform::ClipboardRequest::Paste)
        {
            Ok(()) => self.pasting += 1,
            Err(_) => self.message = moth_ui::text(moth_ui::Text::ClipboardBusy).into(),
        }
    }

    fn persist(&mut self, request: moth_platform::boards::Request) {
        match self.boards.requests.try_send(request) {
            Ok(()) => self.in_flight += 1,
            Err(_) => self.message = moth_ui::text(moth_ui::Text::StorageBusy).into(),
        }
    }
    fn discard(&mut self) {
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
    fn open_board(&mut self, path: std::path::PathBuf) {
        if self.scene.is_empty()
            && self.in_flight == 0
            && self.pasting == 0
            && self.pending.is_empty()
            && self.waiting.is_empty()
            && !self.discarding
        {
            self.path = path.to_string_lossy().into_owned();
            self.persist(moth_platform::boards::Request::Open(path));
        } else {
            self.message = moth_ui::text(moth_ui::Text::OpenEmpty).into();
        }
    }
    fn import(&mut self, path: std::path::PathBuf) {
        if self.discarding {
            return;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
        {
            self.open_board(path);
            return;
        }
        if self.scene.len() + self.waiting.len() + self.pending.len() >= 1_000_000 {
            self.message = moth_ui::text(moth_ui::Text::BoardFull).into();
            return;
        }
        if self.waiting.len() >= 1024 {
            self.message = moth_ui::text(moth_ui::Text::ImportBusy).into();
            return;
        }
        match self.storage.register(path) {
            Ok(h) => self.waiting.push(h),
            Err(e) => self.message = e.to_string(),
        }
        self.pump();
    }
    fn pump(&mut self) {
        while let Some(&handle) = self.waiting.last() {
            match self
                .decode
                .requests
                .try_send(moth_io::Request { handle, side: 256 })
            {
                Ok(()) => {
                    self.waiting.pop();
                    self.pending.insert(handle, Pending::Import);
                }
                Err(_) => break,
            }
        }
    }
    fn insertion_point(&self) -> [f32; 2] {
        if self.viewport.is_finite() {
            let p = self.viewport.center();
            return self.camera.world([p.x, p.y]);
        }
        let point = self
            .window
            .as_ref()
            .map(|w| {
                let size = w.inner_size().to_logical::<f32>(w.scale_factor());
                [size.width / 2., size.height / 2.]
            })
            .unwrap_or([0., 0.]);
        self.camera.world(point)
    }
    fn complete(&mut self, done: moth_io::Completion) {
        let Some(pending) = self.pending.remove(&done.request.handle) else {
            return;
        };
        let insert = matches!(pending, Pending::Import);
        match done.result {
            Ok(image) => {
                if matches!(pending,Pending::Preview(hash) if hash!=image.hash) {
                    self.failed.insert(done.request.handle);
                    self.message = moth_ui::text(moth_ui::Text::SourceChanged).into();
                    return;
                }
                if insert {
                    let p = self.insertion_point();
                    let width = (image.original[0] as f32).min(600.);
                    let height = width * image.original[1] as f32 / image.original[0] as f32;
                    let offset = (self.scene.len() % 10) as f32 * 24.;
                    let item = Item::image(
                        image.hash,
                        Rect {
                            x: p[0] - width / 2. + offset,
                            y: p[1] - height / 2. + offset,
                            width,
                            height,
                        },
                        self.scene.top(),
                    );
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
                    let p = self.insertion_point();
                    let item = Item::image(
                        hash,
                        Rect {
                            x: p[0] - 120.,
                            y: p[1] - 90.,
                            width: 240.,
                            height: 180.,
                        },
                        self.scene.top(),
                    );
                    self.persist(moth_platform::boards::Request::Apply(
                        Command::Insert(item),
                        Some(done.request.handle),
                    ));
                }
                self.failed.insert(done.request.handle);
                self.message = e.to_string();
                tracing::warn!(error=%e,"image load failed");
            }
        }
    }
    fn results(&mut self) {
        while let Ok(done) = self.boards.results.try_recv() {
            use moth_platform::boards::Completion;
            match done {
                Completion::Recovery(paths) => self.recovery_paths = paths,
                Completion::Recent(paths) => self.recent_paths = paths,
                done => {
                    self.in_flight = self.in_flight.saturating_sub(1);
                    match done {
                        Completion::Applied(command, source) => {
                            match self.scene.apply(command) {
                                Ok(_) => {
                                    if let Some((hash, handle)) = source {
                                        self.sources.insert(hash, handle);
                                    }
                                    self.dirty = true;
                                }
                                Err(e) => self.message = e.to_string(),
                            }
                            self.selection.retain(|id| self.scene.get(*id).is_some());
                        }
                        Completion::Opened(scene, sources, recovered) => {
                            self.scene = scene;
                            self.sources = sources;
                            self.recovery_paths.clear();
                            self.dirty = recovered;
                            self.message = moth_ui::text(if recovered {
                                moth_ui::Text::Recovered
                            } else {
                                moth_ui::Text::Opened
                            })
                            .into();
                            self.fit();
                        }
                        Completion::Saved => {
                            self.dirty = false;
                            self.message = moth_ui::text(moth_ui::Text::Saved).into();
                            if self.close_after_save {
                                self.discard();
                            }
                        }
                        Completion::Failed(e) => {
                            self.message = e;
                            self.close_after_save = false;
                        }
                        _ => {}
                    }
                }
            }
        }
        while let Ok(done) = self.decode.results.try_recv() {
            self.complete(done);
        }
        while let Ok(result) = self.clipboard.results.try_recv() {
            match result {
                moth_platform::ClipboardCompletion::Discarded(result) => match result {
                    Ok(()) => self.discarded = true,
                    Err(error) => {
                        self.discarding = false;
                        self.message = error.to_string();
                    }
                },
                moth_platform::ClipboardCompletion::Pasted(Ok((handle, image))) => {
                    self.pasting = self.pasting.saturating_sub(1);
                    if self.scene.len() + self.waiting.len() + self.pending.len() < 1_000_000 {
                        self.pending.insert(handle, Pending::Import);
                        self.complete(moth_io::Completion {
                            request: moth_io::Request { handle, side: 256 },
                            result: Ok(image),
                            failure_hash: None,
                        });
                    } else {
                        self.message = moth_ui::text(moth_ui::Text::BoardFull).into();
                    }
                }
                moth_platform::ClipboardCompletion::Files(paths) => {
                    self.pasting = self.pasting.saturating_sub(1);
                    for path in paths {
                        self.import(path);
                    }
                }
                moth_platform::ClipboardCompletion::Pasted(Err(
                    moth_platform::PlatformError::EmptyClipboard,
                )) => {
                    self.pasting = self.pasting.saturating_sub(1);
                    self.message = moth_ui::text(moth_ui::Text::ClipboardEmpty).into();
                }
                moth_platform::ClipboardCompletion::Pasted(Err(e)) => {
                    self.pasting = self.pasting.saturating_sub(1);
                    self.message = e.to_string();
                }
            }
        }
        self.pump();
    }
    fn fit(&mut self) {
        let items: Vec<_> = self.scene.items().collect();
        if items.is_empty() {
            return;
        }
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for item in items {
            min[0] = min[0].min(item.rect.x);
            min[1] = min[1].min(item.rect.y);
            max[0] = max[0].max(item.rect.x + item.rect.width);
            max[1] = max[1].max(item.rect.y + item.rect.height);
        }
        self.camera.zoom = ((self.viewport.width() - 48.) / (max[0] - min[0]))
            .min((self.viewport.height() - 48.) / (max[1] - min[1]))
            .clamp(0.01, 64.);
        self.camera.origin = [
            (min[0] + max[0]) / 2. - self.viewport.center().x / self.camera.zoom,
            (min[1] + max[1]) / 2. - self.viewport.center().y / self.camera.zoom,
        ];
    }
    fn pointer(&mut self, input: Input) {
        if self.confirm_close {
            return;
        }
        match input {
            Input::Zoom { position, factor } => {
                if self.viewport.contains(egui::pos2(position[0], position[1])) {
                    self.camera.zoom_at(position, factor);
                }
            }
            Input::Pointer(p) => {
                if p.phase == Phase::Down
                    && !self
                        .viewport
                        .contains(egui::pos2(p.position[0], p.position[1]))
                {
                    return;
                }
                if let Some(g) = self.gestures.update(p) {
                    self.drag = None;
                    self.drag_pointer = None;
                    self.camera.zoom_at(g.anchor, g.zoom);
                    self.camera.origin[0] -= g.pan[0] / self.camera.zoom;
                    self.camera.origin[1] -= g.pan[1] / self.camera.zoom;
                    return;
                }
                if self.gestures.active() {
                    self.drag = None;
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
                            .viewport
                            .contains(egui::pos2(p.position[0], p.position[1])) =>
                    {
                        self.drag = Some((p.position, p.position, p.pan || self.pan));
                        self.drag_pointer = Some((p.kind, p.id));
                    }
                    Phase::Move => {
                        if let Some((_, last, pan)) = self.drag.as_mut() {
                            if *pan {
                                self.camera.origin[0] -=
                                    (p.position[0] - last[0]) / self.camera.zoom;
                                self.camera.origin[1] -=
                                    (p.position[1] - last[1]) / self.camera.zoom;
                            }
                            *last = p.position;
                        }
                    }
                    Phase::Up => {
                        self.drag_pointer = None;
                        if let Some((start, _, false)) = self.drag.take() {
                            let a = self.camera.world(start);
                            let b = self.camera.world(p.position);
                            let rect = Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0]).abs().max(1. / self.camera.zoom),
                                height: (a[1] - b[1]).abs().max(1. / self.camera.zoom),
                            };
                            self.selection = self.scene.query(rect).iter().map(|i| i.id).collect();
                        }
                    }
                    Phase::Cancel => {
                        self.drag = None;
                        self.drag_pointer = None;
                    }
                    _ => {}
                }
            }
        }
    }
    fn frame(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let _span = tracing::trace_span!("frame", zoom = self.camera.zoom).entered();
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
        if self.discarded {
            event_loop.exit();
            return Ok(());
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
        let mut output = self.ctx.run_ui(state.take_egui_input(&window), |ui| {
            actions = moth_ui::toolbar(
                ui,
                moth_ui::Status {
                    adapter: &gpu.adapter,
                    count: self.scene.len(),
                    selected: self.selection.len(),
                    zoom: self.camera.zoom,
                    texture_bytes: gpu.registry.allocated_bytes(),
                    message: &self.message,
                },
                &mut self.pan,
            );
            egui::Panel::top("files").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(moth_ui::text(moth_ui::Text::Path));
                    ui.text_edit_singleline(&mut self.path);
                    let idle = self.pasting == 0
                        && self.in_flight == 0
                        && self.pending.is_empty()
                        && self.waiting.is_empty()
                        && !self.discarding;
                    save = ui
                        .add_enabled(
                            idle && !self.path.is_empty(),
                            egui::Button::new(moth_ui::text(moth_ui::Text::Save)),
                        )
                        .clicked();
                    if ui
                        .add_enabled(
                            idle && self.scene.is_empty() && !self.path.is_empty(),
                            egui::Button::new(moth_ui::text(moth_ui::Text::Open)),
                        )
                        .clicked()
                    {
                        open = Some(std::path::PathBuf::from(&self.path));
                    }
                    undo = ui
                        .add_enabled(idle, egui::Button::new(moth_ui::text(moth_ui::Text::Undo)))
                        .clicked();
                    redo = ui
                        .add_enabled(idle, egui::Button::new(moth_ui::text(moth_ui::Text::Redo)))
                        .clicked();
                });
                if self.scene.is_empty() && self.in_flight == 0 {
                    for path in &self.recent_paths {
                        if ui.button(path.display().to_string()).clicked() {
                            open = Some(path.clone());
                        }
                    }
                }
            });
            if !self.recovery_paths.is_empty() && self.scene.is_empty() {
                egui::Window::new(moth_ui::text(moth_ui::Text::Recovery)).show(ui.ctx(), |ui| {
                    for path in &self.recovery_paths {
                        if ui
                            .add_enabled(
                                self.in_flight == 0,
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
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    self.viewport = ui.max_rect();
                    if self.scene.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label(moth_ui::text(moth_ui::Text::Empty));
                        });
                    }
                    let origin = self
                        .camera
                        .world([self.viewport.min.x, self.viewport.min.y]);
                    for item in self.scene.query(Rect {
                        x: origin[0],
                        y: origin[1],
                        width: self.viewport.width() / self.camera.zoom,
                        height: self.viewport.height() / self.camera.zoom,
                    }) {
                        if !gpu.registry.resident(item.blob, 1) {
                            let p = self.camera.screen([item.rect.x, item.rect.y]);
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(p[0], p[1]),
                                egui::vec2(
                                    item.rect.width * self.camera.zoom,
                                    item.rect.height * self.camera.zoom,
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
                    }
                    for &id in &self.selection {
                        if let Some(item) = self.scene.get(id) {
                            let p = self.camera.screen([item.rect.x, item.rect.y]);
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(p[0], p[1]),
                                egui::vec2(
                                    item.rect.width * self.camera.zoom,
                                    item.rect.height * self.camera.zoom,
                                ),
                            );
                            ui.painter().rect_stroke(
                                rect,
                                2.,
                                egui::Stroke::new(2., moth_ui::ACCENT),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }
                    if let Some((a, b, false)) = self.drag {
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
            if self.confirm_close {
                egui::Window::new(moth_ui::text(moth_ui::Text::Title))
                    .collapsible(false)
                    .show(ui.ctx(), |ui| {
                        ui.label(moth_ui::text(moth_ui::Text::CloseWarning));
                        if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                            self.confirm_close = false;
                        }
                        if ui
                            .add_enabled(
                                self.in_flight == 0
                                    && self.pasting == 0
                                    && self.pending.is_empty()
                                    && self.waiting.is_empty()
                                    && !self.path.is_empty(),
                                egui::Button::new(moth_ui::text(moth_ui::Text::SaveClose)),
                            )
                            .clicked()
                        {
                            save = true;
                            self.close_after_save = true;
                        }
                        if ui.button(moth_ui::text(moth_ui::Text::Close)).clicked() {
                            close = true;
                        }
                    });
            }
        });
        state.handle_platform_output(&window, output.platform_output);
        let p = self
            .camera
            .world([self.viewport.min.x, self.viewport.min.y]);
        let visible = self.scene.query(Rect {
            x: p[0],
            y: p[1],
            width: self.viewport.width() / self.camera.zoom,
            height: self.viewport.height() / self.camera.zoom,
        });
        for item in &visible {
            let side = moth_render::preview_side(
                item.rect.width.max(item.rect.height) * self.camera.zoom * output.pixels_per_point,
            );
            if !gpu.registry.resident(item.blob, side)
                && let Some(&handle) = self.sources.get(&item.blob)
                && !self.pending.contains_key(&handle)
                && !self.failed.contains(&handle)
                && self
                    .decode
                    .requests
                    .try_send(moth_io::Request { handle, side })
                    .is_ok()
            {
                self.pending.insert(handle, Pending::Preview(item.blob));
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
        let min = self.viewport.min;
        let max = self.viewport.max;
        gpu.registry.draw(
            &gpu.device,
            &gpu.queue,
            moth_render::Target {
                view: &view,
                screen: [size.width as f32 / scale, size.height as f32 / scale],
                clip: Some([
                    (min.x * scale).max(0.) as u32,
                    (min.y * scale).max(0.) as u32,
                    ((max.x - min.x) * scale).max(1.) as u32,
                    ((max.y - min.y) * scale).max(1.) as u32,
                ]),
                clear: wgpu::Color {
                    r: 0.008,
                    g: 0.009,
                    b: 0.012,
                    a: 1.,
                },
            },
            &visible,
            self.camera,
        );
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
        if actions.paste && !self.discarding {
            self.paste();
        }
        if actions.fit {
            self.fit();
            self.redraw();
        }
        if actions.zoom > 0. {
            self.camera.zoom_at(
                [self.viewport.center().x, self.viewport.center().y],
                actions.zoom,
            );
            self.redraw();
        }
        if let Some(path) = open {
            self.open_board(path);
        }
        if save {
            self.persist(moth_platform::boards::Request::Save(
                std::path::PathBuf::from(&self.path),
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
        Ok(())
    }
}
impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match event_loop.create_window(
                Window::default_attributes().with_title(moth_ui::text(moth_ui::Text::Title)),
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
        if let Err(e) = self.recover() {
            tracing::error!(%e,"renderer creation failed");
            self.message = e.to_string();
            self.startup_error = Some(e);
            event_loop.exit();
        }
        for path in std::mem::take(&mut self.startup) {
            self.import(path);
        }
        self.redraw();
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.input = Adapter::default();
        self.drag = None;
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
                self.results();
                if self.discarded {
                    event_loop.exit();
                } else {
                    self.redraw();
                }
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
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
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
            if response.repaint {
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
                    self.message = e.to_string();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::CloseRequested => {
                if !self.dirty
                    && self.in_flight == 0
                    && self.pending.is_empty()
                    && self.waiting.is_empty()
                {
                    self.discard();
                } else {
                    self.confirm_close = true;
                    self.redraw();
                }
            }
            WindowEvent::Focused(false) => {
                self.drag = None;
                self.drag_pointer = None;
                self.gestures.cancel();
                self.command = false;
            }
            WindowEvent::DroppedFile(path) => self.import(path),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && (event.physical_key == PhysicalKey::Code(KeyCode::KeyV)
                        || matches!(event.logical_key,Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
                {
                    if !self.discarding {
                        self.paste();
                    }
                } else if self.command
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyS)
                    && !self.path.is_empty()
                    && self.in_flight == 0
                {
                    self.persist(moth_platform::boards::Request::Save(
                        std::path::PathBuf::from(&self.path),
                    ));
                } else if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyZ)
                    && self.in_flight == 0
                {
                    self.persist(moth_platform::boards::Request::Undo);
                } else if self.command
                    && !self.ctx.egui_wants_keyboard_input()
                    && event.physical_key == PhysicalKey::Code(KeyCode::KeyY)
                    && self.in_flight == 0
                {
                    self.persist(moth_platform::boards::Request::Redo);
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
