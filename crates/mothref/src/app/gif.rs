// SPDX-License-Identifier: GPL-3.0-or-later
//! An interactive player for the selected image, with one outstanding decode for the app.
use super::{App, Intent, bindings::Action};
use crossbeam_channel::{Receiver, Sender};
use moth_format::Handle;
use moth_io::{DecodeError, Decoded, ImageFormat, gif_animation};
use moth_platform::{ClipboardRequest, DesktopStorage};
use moth_scene::{BlobId, Item, Uuid};
use moth_ui::{Text, icons, icons::Icon, text};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::window::WindowId;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    window: WindowId,
    document: Uuid,
    item: Uuid,
    handle: Handle,
}

struct Work {
    source: Identity,
    storage: Arc<DesktopStorage>,
    frame: Option<(usize, BlobId, ImageFormat)>,
}

enum Output {
    Timeline(Option<Vec<Duration>>),
    Frame(usize, Decoded),
}

type Reply = (Identity, Result<Output, DecodeError>);

struct Worker {
    send: Sender<Option<Work>>,
    receive: Receiver<Reply>,
}

impl Worker {
    fn new(wake: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<Self> {
        let (send, jobs) = crossbeam_channel::bounded::<Option<Work>>(1);
        let (results, receive) = crossbeam_channel::bounded(1);
        std::thread::Builder::new()
            .name("gif-player".into())
            .spawn(move || {
                let mut reader = gif_animation::Reader::default();
                let mut current = None;
                while let Ok(work) = jobs.recv() {
                    let Some(work) = work else {
                        reader = gif_animation::Reader::default();
                        current = None;
                        continue;
                    };
                    if current != Some((work.source.document, work.source.handle)) {
                        reader = gif_animation::Reader::default();
                        current = Some((work.source.document, work.source.handle));
                    }
                    let result = match work.frame {
                        None => gif_animation::timeline(work.storage.as_ref(), work.source.handle)
                            .map(Output::Timeline),
                        Some((index, key, format)) => reader
                            .frame(work.storage.as_ref(), work.source.handle, index)
                            .map(|frame| {
                                Output::Frame(index, gif_animation::preview(frame, key, format))
                            }),
                    };
                    if results.send((work.source, result)).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self { send, receive })
    }
}

struct Playback {
    delays: Vec<Duration>,
    wanted: usize,
    displayed: Option<(usize, Decoded)>,
    playing: bool,
    repeat: bool,
    speed: f32,
    step: usize,
    deadline: Option<Instant>,
    upload: bool,
}

impl Playback {
    fn new(delays: Vec<Duration>) -> Self {
        Self {
            delays,
            wanted: 0,
            displayed: None,
            playing: false,
            repeat: true,
            speed: 1.,
            step: 1,
            deadline: None,
            upload: false,
        }
    }

    fn next(&mut self) {
        if self.wanted + 1 < self.delays.len() {
            self.wanted += 1;
        } else if self.repeat {
            self.wanted = 0;
        } else {
            self.playing = false;
        }
        self.deadline = None;
    }

    fn play_pause(&mut self) {
        self.playing = !self.playing;
        if self.playing && !self.repeat && self.wanted + 1 == self.delays.len() {
            self.wanted = 0;
        }
        self.deadline = None;
    }

    fn seek(&mut self, index: usize) {
        self.wanted = index.min(self.delays.len() - 1);
        self.deadline = None;
    }

    fn tick(&mut self, now: Instant) {
        if !self.playing
            || self
                .displayed
                .as_ref()
                .is_none_or(|(index, _)| *index != self.wanted)
        {
            return;
        }
        let deadline = *self
            .deadline
            .get_or_insert_with(|| now + self.delays[self.wanted].div_f32(self.speed));
        if deadline <= now {
            self.next();
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Still {
    id: Uuid,
    frame: usize,
}

/// Playback is view state; the original GIF and its journal references stay unchanged.
pub(super) struct Player {
    key: BlobId,
    worker: Option<Worker>,
    target: Option<Identity>,
    probed: bool,
    pending: bool,
    reset_reader: bool,
    playback: Option<Playback>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            key: moth_io::content_hash(Uuid::now_v7().as_bytes()),
            worker: None,
            target: None,
            probed: false,
            pending: false,
            reset_reader: false,
            playback: None,
        }
    }
}

impl Player {
    fn still(&self) -> Option<Still> {
        Some(Still {
            id: self.target?.item,
            frame: self.playback.as_ref()?.displayed.as_ref()?.0,
        })
    }

    pub(super) fn show(
        &self,
        ui: &egui::Ui,
        window: WindowId,
        item: &Item,
        camera: moth_scene::Camera,
        viewport: egui::Rect,
        enabled: &[bool],
    ) -> Vec<Intent> {
        let mut intents = Vec::new();
        let Some(playback) = &self.playback else {
            return intents;
        };
        if !self.belongs_to(window, item.id) || !viewport.is_finite() || !viewport.is_positive() {
            return intents;
        }
        let image = player_bounds(item, camera).intersect(viewport);
        if !image.is_finite() || !image.is_positive() {
            return intents;
        }
        let tokens = moth_ui::tokens(ui.ctx());
        let margin = f32::from(tokens.space_xs);
        let inset = image.shrink(f32::from(tokens.space_s));
        let button = if ui.ctx().content_rect().width() < 600. {
            44.
        } else {
            36.
        };
        let row_gap = ui.spacing().item_spacing.y;
        let horizontal_gap = ui.spacing().item_spacing.x;
        let available = inset.width() - margin * 2.;
        let full = available >= button * 5. + horizontal_gap * 5. + 60.;
        let compact = available >= button * 3. + horizontal_gap * 2.;
        let slider = inset.height() >= button * 1.8 + margin * 2.;
        let height = button
            + margin * 2.
            + if slider {
                ui.spacing().interact_size.y + row_gap
            } else {
                0.
            };
        if available < button || inset.height() < height {
            return intents;
        }
        let width = if full {
            available.min(320.)
        } else if compact {
            available.min(button * 3. + horizontal_gap * 2.)
        } else {
            button
        };
        egui::Area::new(egui::Id::unique("gif_player"))
            .fixed_pos(egui::pos2(inset.center().x, inset.bottom()))
            .pivot(egui::Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground)
            .constrain_to(inset)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE
                    .fill(tokens.theater.gamma_multiply(0.82))
                    .corner_radius(tokens.radius_m)
                    .inner_margin(egui::Margin::same(tokens.space_xs as i8))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        if slider && compact {
                            let mut frame = playback.wanted + 1;
                            ui.scope(|ui| {
                                ui.spacing_mut().slider_width = width;
                                ui.spacing_mut().interact_size.y *= 0.65;
                                ui.spacing_mut().slider_rail_height =
                                    f32::from(tokens.space_tight) * 2.;
                                let visuals = ui.visuals_mut();
                                visuals.widgets.inactive.bg_fill =
                                    tokens.on_theater.gamma_multiply(0.28);
                                for state in [
                                    &mut visuals.widgets.inactive,
                                    &mut visuals.widgets.hovered,
                                    &mut visuals.widgets.active,
                                ] {
                                    state.fg_stroke = egui::Stroke::new(1., tokens.on_theater);
                                }
                                visuals.widgets.hovered.bg_fill = tokens.on_theater;
                                visuals.widgets.active.bg_fill = tokens.on_theater;
                                visuals.selection.bg_fill = tokens.on_theater;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut frame, 1..=playback.delays.len())
                                            .show_value(false)
                                            .handle_shape(egui::style::HandleShape::Circle)
                                            .trailing_fill(true),
                                    )
                                    .on_hover_text(text(Text::GifFrame))
                                    .changed()
                                {
                                    intents.push(Intent::GifSeek(frame - 1));
                                }
                            });
                        }
                        ui.horizontal(|ui| {
                            if full {
                                self.transport_button(
                                    ui,
                                    Action::GifPrevious,
                                    Icon::Previous,
                                    enabled,
                                    &mut intents,
                                );
                            }
                            self.transport_button(
                                ui,
                                Action::GifPlay,
                                if playback.playing {
                                    Icon::Pause
                                } else {
                                    Icon::Play
                                },
                                enabled,
                                &mut intents,
                            );
                            if full {
                                self.transport_button(
                                    ui,
                                    Action::GifNext,
                                    Icon::Next,
                                    enabled,
                                    &mut intents,
                                );
                                ui.colored_label(
                                    tokens.on_theater,
                                    format!("{} / {}", playback.wanted + 1, playback.delays.len()),
                                );
                            }
                            if compact {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let more = icons::theater_button(
                                            ui,
                                            Icon::More,
                                            text(Text::More),
                                            false,
                                            true,
                                        );
                                        egui::Popup::menu(&more)
                                            .align(egui::RectAlign::TOP_END)
                                            .show(|ui| {
                                                self.options(ui, enabled, &mut intents, full)
                                            });
                                        self.transport_button(
                                            ui,
                                            Action::GifLoop,
                                            Icon::Repeat,
                                            enabled,
                                            &mut intents,
                                        );
                                    },
                                );
                            }
                        });
                    });
            });
        intents
    }

    fn transport_button(
        &self,
        ui: &mut egui::Ui,
        action: Action,
        icon: Icon,
        enabled: &[bool],
        intents: &mut Vec<Intent>,
    ) {
        let Some(playback) = &self.playback else {
            return;
        };
        let label = if action == Action::GifPlay && playback.playing {
            Text::GifPause
        } else {
            action.label()
        };
        if icons::theater_button(
            ui,
            icon,
            text(label),
            action == Action::GifLoop && playback.repeat,
            enabled[action as usize],
        )
        .clicked()
        {
            intents.push(Intent::Action(action));
        }
    }

    fn options(&self, ui: &mut egui::Ui, enabled: &[bool], intents: &mut Vec<Intent>, full: bool) {
        let Some(playback) = &self.playback else {
            return;
        };
        if !full {
            ui.horizontal(|ui| {
                self.transport_button(ui, Action::GifPrevious, Icon::Previous, enabled, intents);
                ui.label(format!(
                    "{} / {}",
                    playback.wanted + 1,
                    playback.delays.len()
                ));
                self.transport_button(ui, Action::GifNext, Icon::Next, enabled, intents);
            });
            ui.separator();
        }
        ui.label(text(Text::GifSpeed));
        ui.horizontal(|ui| {
            let mut speed = playback.speed;
            for value in [0.25, 0.5, 1., 1.5, 2.] {
                ui.selectable_value(&mut speed, value, format!("{value}×"));
            }
            if speed != playback.speed {
                intents.push(Intent::GifSpeed(speed));
            }
        });
        ui.label(text(Text::GifStep));
        ui.horizontal(|ui| {
            let mut step = playback.step;
            for value in [1, 2, 4, 5] {
                ui.selectable_value(&mut step, value, value.to_string());
            }
            if step != playback.step {
                intents.push(Intent::GifStep(step));
            }
        });
        ui.separator();
        for action in [Action::GifCopy, Action::GifAdd] {
            let mut button = egui::Button::new(text(action.label())).frame(false);
            if action == Action::GifAdd {
                button = button.sense(egui::Sense::click_and_drag());
            }
            let response = ui.add_enabled(enabled[action as usize], button);
            if response.clicked() {
                intents.push(Intent::Action(action));
                ui.close();
            }
            if action == Action::GifAdd
                && enabled[action as usize]
                && let Some(still) = self.still()
            {
                response.dnd_set_drag_payload(still);
                response.on_hover_text(text(Text::GifDragHint));
            }
        }
    }

    fn belongs_to(&self, window: WindowId, item: Uuid) -> bool {
        self.target
            .is_some_and(|target| target.window == window && target.item == item)
    }

    pub(super) fn draw_item(
        &self,
        window: WindowId,
        item: &Item,
        registry: &moth_render::Registry,
    ) -> Option<Item> {
        if !self.belongs_to(window, item.id)
            || self.playback.as_ref()?.displayed.as_ref()?.0 == 0
            || !registry.resident(self.key, 1)
        {
            return None;
        }
        let mut transient = item.clone();
        transient.content = moth_scene::Content::Image(self.key);
        Some(transient)
    }
}

fn player_bounds(item: &Item, camera: moth_scene::Camera) -> egui::Rect {
    let [x, y] = camera.screen([
        item.rect.x + item.rect.width / 2.,
        item.rect.y + item.rect.height / 2.,
    ]);
    let (sin, cos) = item.transform.rotation.sin_cos();
    let (sin, cos) = (sin.abs(), cos.abs());
    let (w, h) = (item.rect.width, item.rect.height);
    let outer = egui::vec2(cos * w + sin * h, sin * w + cos * h);
    // Fit an upright rectangle inside the rotated image so the overlay never hangs off a corner.
    let fit = (w / (cos * outer.x + sin * outer.y)).min(h / (sin * outer.x + cos * outer.y));
    egui::Rect::from_center_size(egui::pos2(x, y), outer * (fit * camera.zoom))
}

impl App {
    pub(super) fn gif_action_enabled(&self, action: Action) -> bool {
        let ready = !self.workspace.library
            && !self.workspace.board.intake.loading
            && self.shell.window.as_ref().is_some_and(|window| {
                self.gif.target.is_some_and(|target| {
                    target.window == window.id()
                        && self.workspace.board.ui_id() == target.document
                        && self.workspace.board.selection.len() == 1
                        && self.workspace.board.selection.contains(&target.item)
                })
            })
            && self.gif.playback.is_some();
        ready
            && match action {
                Action::GifCopy => self.gif.still().is_some(),
                Action::GifAdd => {
                    self.gif.still().is_some()
                        && !self.workspace.board.locked
                        && self.workspace.board.idle()
                }
                _ => true,
            }
    }

    pub(super) fn gif_action(&mut self, action: Action) {
        match action {
            Action::GifCopy => {
                if let Some(still) = self.gif.still() {
                    self.workspace
                        .persist(moth_platform::boards::Request::Export(
                            moth_platform::export::Target::GifClipboard(still.id, still.frame),
                        ));
                }
            }
            Action::GifAdd => {
                if let Some(still) = self.gif.still()
                    && let Some(item) = self.workspace.board.scene.get(still.id)
                {
                    let bounds = item.rect.rotated_envelope(item.transform.rotation);
                    self.add_gif_frame(
                        still,
                        [
                            bounds.upper()[0] + item.rect.width / 2. + 24.,
                            item.rect.y + item.rect.height / 2.,
                        ],
                    );
                }
            }
            _ => {
                let Some(playback) = &mut self.gif.playback else {
                    return;
                };
                match action {
                    Action::GifPrevious => {
                        playback.seek(playback.wanted.saturating_sub(playback.step))
                    }
                    Action::GifNext => playback.seek(playback.wanted.saturating_add(playback.step)),
                    Action::GifPlay => playback.play_pause(),
                    Action::GifLoop => playback.repeat = !playback.repeat,
                    _ => unreachable!("GIF action list"),
                }
            }
        }
        self.redraw();
    }

    pub(super) fn gif_seek(&mut self, frame: usize) {
        if let Some(playback) = &mut self.gif.playback {
            playback.seek(frame);
        }
        self.redraw();
    }

    pub(super) fn gif_speed(&mut self, speed: f32) {
        if let Some(playback) = &mut self.gif.playback {
            playback.speed = speed;
            playback.deadline = None;
        }
        self.redraw();
    }

    pub(super) fn gif_step(&mut self, step: usize) {
        if let Some(playback) = &mut self.gif.playback {
            playback.step = step;
        }
        self.redraw();
    }

    pub(super) fn add_gif_frame(&mut self, still: Still, point: [f32; 2]) {
        let board = &mut self.workspace.board;
        if board.locked || self.workspace.library || !board.idle() {
            return;
        }
        let Ok(handle) = moth_platform::export::gif_source(&board.scene, &board.sources, still.id)
        else {
            return;
        };
        if board
            .clipboard()
            .requests
            .try_send(ClipboardRequest::GifFrame(handle, still.frame, point))
            .is_ok()
        {
            board.intake.pasting += 1;
        } else {
            board.notices.message = Text::StorageBusy.into();
        }
    }

    pub(super) fn tick_gif(&mut self, window: WindowId) {
        let wake = self.shell.wake();
        let Some(gpu) = &mut self.shell.gpu else {
            return;
        };
        let board = &self.workspace.board;
        let format = gpu.registry.image_format();
        let target = if !self.workspace.library
            && !board.intake.loading
            && !board.file.discarding
            && !board.file.discarded
            && !self.workspace.confirm_close
            && board.session.is_none()
            && self.window.overlay.is_none()
            && board.selection.len() == 1
        {
            board.selection.iter().next().copied().and_then(|item| {
                let handle =
                    moth_platform::export::gif_source(&board.scene, &board.sources, item).ok()?;
                Some(Identity {
                    window,
                    document: board.ui_id(),
                    item,
                    handle,
                })
            })
        } else {
            None
        };
        // An unrelated redraw in another window may release its old texture, but
        // it must not reset the app-wide player while the user is watching it.
        if self
            .gif
            .target
            .is_some_and(|playing| playing.window != window)
            && (target.is_none()
                || !self
                    .shell
                    .window
                    .as_ref()
                    .is_some_and(|window| window.has_focus()))
        {
            if gpu.registry.resident(self.gif.key, 1) {
                gpu.registry.release_animation(self.gif.key);
            }
            return;
        }
        if self.gif.target != target {
            gpu.registry.release_animation(self.gif.key);
            self.gif.target = target;
            self.gif.reset_reader = target.is_none();
            self.gif.probed = false;
            self.gif.playback = None;
        }
        if target.is_some() && self.gif.worker.is_none() {
            match Worker::new(wake) {
                Ok(worker) => self.gif.worker = Some(worker),
                Err(error) => {
                    tracing::warn!(%error, "could not start GIF worker");
                    return;
                }
            }
        }
        let Some(worker) = &self.gif.worker else {
            return;
        };
        if let Ok((source, result)) = worker.receive.try_recv() {
            self.gif.pending = false;
            if Some(source) == target {
                match result {
                    Ok(Output::Timeline(timeline)) => {
                        self.gif.probed = true;
                        self.gif.playback = timeline.map(Playback::new);
                    }
                    Ok(Output::Frame(index, frame)) => {
                        if let Some(playback) = &mut self.gif.playback
                            && index == playback.wanted
                        {
                            playback.displayed = Some((index, frame));
                            playback.upload = true;
                        }
                    }
                    Err(error) => {
                        tracing::debug!(%error, "could not decode GIF");
                        self.gif.probed = true;
                        self.gif.playback = None;
                        self.workspace.board.notices.message = Text::GifFailed.into();
                    }
                }
            }
        }
        let Some(target) = target else {
            if self.gif.reset_reader && !self.gif.pending && worker.send.try_send(None).is_ok() {
                self.gif.reset_reader = false;
            }
            return;
        };
        let mut request = None;
        if let Some(playback) = &mut self.gif.playback {
            if let Some((_, frame)) = &playback.displayed
                && frame.format == format
                && (playback.upload || !gpu.registry.resident(self.gif.key, 1))
            {
                if !gpu
                    .registry
                    .upload_animation(&gpu.device, &gpu.queue, frame)
                {
                    playback.playing = false;
                    self.workspace.board.notices.message = Text::GifVideoMemory.into();
                }
                playback.upload = false;
            }
            playback.tick(Instant::now());
            if playback
                .displayed
                .as_ref()
                .is_none_or(|(index, frame)| *index != playback.wanted || frame.format != format)
            {
                request = Some(Some((playback.wanted, self.gif.key, format)));
            }
        } else if !self.gif.probed {
            request = Some(None);
        }
        if !self.gif.pending
            && let Some(frame) = request
        {
            self.gif.pending = worker
                .send
                .try_send(Some(Work {
                    source: target,
                    storage: self.workspace.board.storage().clone(),
                    frame,
                }))
                .is_ok();
        }
    }

    /// Merge the GIF wake after rendering establishes egui's own deadline.
    pub(super) fn schedule_gif(&mut self, window: WindowId) {
        if self
            .gif
            .target
            .is_some_and(|target| target.window == window)
            && let Some(playback) = &self.gif.playback
            && playback.playing
            && let Some(deadline) = playback.deadline
        {
            self.shell.deadline = Some(
                self.shell
                    .deadline
                    .map_or(deadline, |next| next.min(deadline)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_stays_inside_a_rotated_image() {
        let rect = moth_scene::Rect {
            x: 10.,
            y: 20.,
            width: 240.,
            height: 130.,
        };
        let mut item = Item::image(moth_io::content_hash(b"image"), rect, None);
        item.transform.rotation = 0.7;
        let bounds = player_bounds(&item, moth_scene::Camera::default());
        let (sin, cos) = item.transform.rotation.sin_cos();
        for x in [bounds.left(), bounds.right()] {
            for y in [bounds.top(), bounds.bottom()] {
                let [dx, dy] = [x - bounds.center().x, y - bounds.center().y];
                assert!((cos * dx + sin * dy).abs() <= rect.width / 2. + 0.001);
                assert!((-sin * dx + cos * dy).abs() <= rect.height / 2. + 0.001);
            }
        }
    }

    #[test]
    fn loop_wraps_and_play_once_stops_on_the_last_frame() {
        let mut gif = Playback::new(vec![Duration::from_millis(50); 3]);
        gif.playing = true;
        gif.seek(2);
        gif.next();
        assert_eq!(gif.wanted, 0);
        assert!(gif.playing);

        gif.repeat = false;
        gif.seek(2);
        gif.next();
        assert_eq!(gif.wanted, 2);
        assert!(!gif.playing);

        gif.play_pause();
        assert_eq!(gif.wanted, 0);
        assert!(gif.playing);
    }
}
