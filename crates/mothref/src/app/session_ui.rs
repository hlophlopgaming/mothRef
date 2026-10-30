// SPDX-License-Identifier: GPL-3.0-or-later
//! Screens of a drawing session (DESIGN.md screens 12–12д): setup, the running stage
//! in full screen or a mini window, and the summary.
//!
//! Images are never egui textures: each frame lists the slides to draw (`draws`) in
//! screen points, and the renderer draws them from the board's texture arrays below
//! this UI. That keeps thumbnails and the stage at the resolution already resident.
use super::session::{self, Cue, Order, Phase, Replay, Run, Setup, Source, Stage};
use moth_scene::{Item, Scene, Uuid};
use moth_ui::{Text, text};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// The panel hides after this long without pointer or key input.
const HIDE_AFTER: Duration = Duration::from_secs(2);

/// One drawing session attached to a board.
pub(super) struct State {
    pub screen: Screen,
    /// Slides to draw this frame: board item and screen rectangle.
    pub draws: Vec<(Uuid, egui::Rect)>,
    /// Clip of `draws`, such as the summary's scrolled list.
    pub clip: Option<egui::Rect>,
    /// Last pointer or key input, for hiding the panel.
    active: Instant,
    /// The last image an aspect ratio was requested for.
    fitted: Option<Uuid>,
}
pub(super) enum Screen {
    Setup(Setup),
    Running(Run),
    Summary(Run),
}
/// What the App must do for the session: window changes, sounds, the board.
pub(super) enum Event {
    /// Full screen, or the mini window sized to this aspect ratio.
    Window {
        mini: bool,
        aspect: f32,
    },
    /// The session ended: restore the window.
    Leave,
    Cue(Cue),
    /// Leave and select these items on the board.
    Select(Vec<Uuid>),
    /// Remember the setup for the next session.
    Remember(Setup),
    /// Keep the plan under "My plans".
    SavePlan(Vec<Stage>),
    /// Move the window (dragging the stage of the mini window).
    Drag,
}
impl State {
    pub fn new(setup: Setup) -> Self {
        Self {
            screen: Screen::Setup(setup),
            draws: Vec::new(),
            clip: None,
            active: Instant::now(),
            fitted: None,
        }
    }
    /// Setup draws over the board; the other screens replace it.
    pub fn covers_board(&self) -> bool {
        !matches!(self.screen, Screen::Setup(_))
    }
}
/// Board context the screens read.
pub(super) struct Board<'a> {
    pub scene: &'a Scene,
    pub selection: &'a HashSet<Uuid>,
    pub group: Option<Uuid>,
    pub plans: &'a [(String, Vec<Stage>)],
}
/// Images of a source in board reading order: rows top to bottom, then left to right.
pub(super) fn images(board: &Board<'_>, source: Source) -> Vec<Uuid> {
    let mut items: Vec<&Item> = match source {
        Source::Board => board.scene.items().collect(),
        Source::Selection => board
            .selection
            .iter()
            .filter_map(|id| board.scene.get(*id))
            .collect(),
        Source::Group(group) => board
            .scene
            .descendants(group)
            .into_iter()
            .filter_map(|id| board.scene.get(id))
            .collect(),
    };
    items.retain(|item| item.blob().is_some());
    items.sort_by(|a, b| {
        a.rect
            .y
            .total_cmp(&b.rect.y)
            .then(a.rect.x.total_cmp(&b.rect.x))
            .then(a.id.cmp(&b.id))
    });
    items.into_iter().map(|item| item.id).collect()
}
fn source_label(board: &Board<'_>, source: Source) -> String {
    match source {
        Source::Board => text(Text::SourceBoard).to_owned(),
        Source::Selection => text(Text::SourceSelection).to_owned(),
        Source::Group(group) => {
            let title = board
                .scene
                .get(group)
                .and_then(|item| item.group_style())
                .map(|style| style.title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| text(Text::UntitledGroup).to_owned());
            format!("{} «{title}»", text(Text::SourceGroup))
        }
    }
}
/// `30 s`, `2 min`, `1 min 30 s`.
pub(super) fn seconds(value: u64) -> String {
    let (minutes, rest) = (value / 60, value % 60);
    match (minutes, rest) {
        (0, s) => format!("{s} {}", text(Text::SecondsShort)),
        (m, 0) => format!("{m} {}", text(Text::MinutesShort)),
        (m, s) => format!(
            "{m} {} {s} {}",
            text(Text::MinutesShort),
            text(Text::SecondsShort)
        ),
    }
}
/// `0:18`, `12:05`.
fn clock(duration: Duration) -> String {
    let total = duration.as_secs_f32().ceil() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}
/// `10 × 30 s, then 5 × 2 min`.
fn plan_label(stages: &[Stage]) -> String {
    stages
        .iter()
        .map(|stage| format!("{} × {}", stage.count, seconds(stage.seconds as u64)))
        .collect::<Vec<_>>()
        .join(&format!(", {} ", text(Text::Then)))
}
/// Plans offered before the user saves any: about 15 and 30 minutes.
pub(super) fn builtin_plans() -> Vec<(String, Vec<Stage>)> {
    let stage = |count, seconds| Stage { count, seconds };
    vec![
        (
            format!(
                "{} · 15 {}",
                text(Text::PlanWarmup),
                text(Text::MinutesShort)
            ),
            vec![stage(10, 30), stage(5, 60), stage(1, 300)],
        ),
        (
            format!(
                "{} · 30 {}",
                text(Text::PlanPoses),
                text(Text::MinutesShort)
            ),
            vec![stage(10, 60), stage(4, 300)],
        ),
    ]
}
/// Draws the active screen and runs the clock. Returns what the App must do.
pub(super) fn show(ui: &mut egui::Ui, slot: &mut Option<State>, board: &Board<'_>) -> Vec<Event> {
    let mut events = Vec::new();
    let Some(state) = slot.as_mut() else {
        return events;
    };
    state.draws.clear();
    state.clip = None;
    let now = Instant::now();
    if ui.input(|i| {
        i.pointer.delta() != egui::Vec2::ZERO
            || i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { .. }
                        | egui::Event::PointerButton { .. }
                        | egui::Event::Touch { .. }
                )
            })
    }) {
        state.active = now;
    }
    let mut close = false;
    // The Esc that ends a run must not also close the summary it opens.
    let settled = matches!(state.screen, Screen::Summary(_));
    match &mut state.screen {
        Screen::Setup(setup) => {
            match setup_screen(ui, setup, board) {
                Some(true) => {
                    let setup = setup.clone();
                    events.push(Event::Remember(setup.clone()));
                    let images = images(board, setup.source);
                    let mut rng = session::Rng::new(Uuid::now_v7().as_u128() as u64);
                    let slides =
                        session::deal(&images, &setup.stages(images.len()), setup.order, &mut rng);
                    if !slides.is_empty() {
                        let label = source_label(board, setup.source);
                        let run = Run::new(setup, slides, label, now);
                        state.fitted = None;
                        state.screen = Screen::Running(run);
                    }
                }
                Some(false) => close = true,
                None => {}
            }
            if let Screen::Setup(setup) = &state.screen
                && ui
                    .ctx()
                    .data(|d| d.get_temp::<bool>(egui::Id::new("session_save_plan")))
                    .unwrap_or(false)
            {
                ui.ctx()
                    .data_mut(|d| d.remove::<bool>(egui::Id::new("session_save_plan")));
                events.push(Event::SavePlan(setup.plan.clone()));
            }
        }
        Screen::Running(run) => {
            let cues = run.update(now);
            if run.setup.sound {
                events.extend(cues.iter().copied().map(Event::Cue));
            }
            if !run.done() {
                running_screen(
                    ui,
                    run,
                    state.active,
                    now,
                    &mut state.draws,
                    &mut events,
                    board,
                );
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            if let Some(slide) = run.current()
                && state.fitted != Some(slide.item)
                && let Some(item) = board.scene.get(slide.item)
            {
                state.fitted = Some(slide.item);
                let envelope = item.rect.rotated_envelope(item.transform.rotation);
                let size = [
                    envelope.upper()[0] - envelope.lower()[0],
                    envelope.upper()[1] - envelope.lower()[1],
                ];
                events.push(Event::Window {
                    mini: run.setup.mini,
                    aspect: size[0] / size[1].max(1.),
                });
            }
        }
        Screen::Summary(_) => {}
    }
    if let Screen::Running(run) = &mut state.screen
        && run.done()
    {
        let finished = std::mem::replace(
            run,
            Run::new(Setup::default(), Vec::new(), String::new(), now),
        );
        state.screen = Screen::Summary(finished);
        events.push(Event::Leave);
    }
    if let Screen::Summary(run) = &mut state.screen {
        match summary_screen(ui, run, board, &mut state.draws, &mut state.clip, settled) {
            Some(SummaryChoice::Close) => close = true,
            Some(SummaryChoice::Select) => {
                let ids: Vec<_> = run
                    .slides
                    .iter()
                    .filter(|s| s.shown)
                    .map(|s| s.item)
                    .collect();
                events.push(Event::Select(ids));
                close = true;
            }
            Some(SummaryChoice::Show(id)) => {
                events.push(Event::Select(vec![id]));
                close = true;
            }
            Some(SummaryChoice::New) => {
                state.screen = Screen::Setup(run.setup.clone());
            }
            Some(SummaryChoice::Replay(kind)) => {
                let mut rng = session::Rng::new(Uuid::now_v7().as_u128() as u64);
                let slides = session::replay(
                    &run.slides,
                    kind,
                    |id| board.scene.get(id).is_some(),
                    &mut rng,
                );
                if !slides.is_empty() {
                    let next = Run::new(run.setup.clone(), slides, run.source.clone(), now);
                    state.fitted = None;
                    state.screen = Screen::Running(next);
                }
            }
            None => {}
        }
    }
    if close {
        *slot = None;
    }
    events
}

fn section(ui: &mut egui::Ui, label: Text) {
    ui.add_space(moth_ui::tokens(ui.ctx()).space_s as f32);
    ui.label(egui::RichText::new(text(label)).strong());
}
/// `Some(true)` starts, `Some(false)` cancels.
fn setup_screen(ui: &mut egui::Ui, setup: &mut Setup, board: &Board<'_>) -> Option<bool> {
    let tokens = moth_ui::tokens(ui.ctx());
    let mut outcome = None;
    let screen = ui.ctx().content_rect();
    // A dim layer keeps the board visible but out of reach.
    egui::Area::new(egui::Id::new("session_dim"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(screen, 0., tokens.shadow.gamma_multiply(0.45));
            ui.allocate_rect(screen, egui::Sense::click());
        });
    let counts = [
        (Source::Board, images(board, Source::Board).len()),
        (Source::Selection, images(board, Source::Selection).len()),
    ];
    let group = board.group.map(|group| {
        (
            Source::Group(group),
            images(board, Source::Group(group)).len(),
        )
    });
    let available = |source: Source| match source {
        Source::Group(_) => group.filter(|(s, _)| *s == source).map_or(0, |(_, n)| n),
        _ => counts
            .iter()
            .find(|(s, _)| *s == source)
            .map_or(0, |(_, n)| *n),
    };
    if available(setup.source) == 0 {
        setup.source = group
            .filter(|(_, n)| *n > 0)
            .map(|(s, _)| s)
            .unwrap_or(Source::Board);
    }
    let width = (screen.width() - tokens.space_xl as f32 * 4.).min(tokens.space_xl as f32 * 34.);
    egui::Area::new(egui::Id::new("session_setup"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(tokens.space_xl as f32)
                .show(ui, |ui| {
                    ui.set_width(width);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .max_height(screen.height() - tokens.space_xl as f32 * 5.)
                        .min_scrolled_height(screen.height() - tokens.space_xl as f32 * 5.)
                        .show(ui, |ui| {
                            ui.heading(text(Text::DrawingSession));
                            let images = available(setup.source);
                            // Two columns as in the design when there is room, one below
                            // the other on a narrow window.
                            let wide = width > tokens.space_xl as f32 * 24.;
                            let column = if wide {
                                (width - tokens.space_xl as f32) / 2.
                            } else {
                                width
                            };
                            let layout = if wide {
                                egui::Layout::left_to_right(egui::Align::Min)
                            } else {
                                egui::Layout::top_down(egui::Align::Min)
                            };
                            ui.with_layout(layout, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(column);
                                    section(ui, Text::SessionPlans);
                                    ui.horizontal_wrapped(|ui| {
                                        for (name, plan) in board.plans {
                                            if ui
                                                .selectable_label(setup.plan == *plan, name)
                                                .clicked()
                                            {
                                                setup.plan = plan.clone();
                                            }
                                        }
                                    });
                                    section(ui, Text::SessionImages);
                                    ui.horizontal_wrapped(|ui| {
                                        let mut sources = counts.to_vec();
                                        sources.extend(group);
                                        for (source, count) in sources {
                                            let label =
                                                format!("{}  {count}", source_label(board, source));
                                            ui.add_enabled_ui(count > 0, |ui| {
                                                if ui
                                                    .selectable_label(setup.source == source, label)
                                                    .clicked()
                                                {
                                                    setup.source = source;
                                                }
                                            });
                                        }
                                    });
                                    section(ui, Text::TimePerImage);
                                    ui.horizontal_wrapped(|ui| {
                                        for interval in session::INTERVALS {
                                            let chosen =
                                                setup.plan.is_empty() && setup.interval == interval;
                                            if ui
                                                .selectable_label(chosen, seconds(interval as u64))
                                                .clicked()
                                            {
                                                setup.interval = interval;
                                                setup.plan.clear();
                                            }
                                        }
                                    });
                                    section(ui, Text::SessionPlan);
                                    let mut remove = None;
                                    for (index, stage) in setup.plan.iter_mut().enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                egui::DragValue::new(&mut stage.count)
                                                    .range(1..=999),
                                            );
                                            ui.label("×");
                                            egui::ComboBox::from_id_salt(("session_stage", index))
                                                .selected_text(seconds(stage.seconds as u64))
                                                .show_ui(ui, |ui| {
                                                    for interval in session::INTERVALS {
                                                        ui.selectable_value(
                                                            &mut stage.seconds,
                                                            interval,
                                                            seconds(interval as u64),
                                                        );
                                                    }
                                                });
                                            if moth_ui::icons::button(
                                                ui,
                                                moth_ui::icons::Icon::Close,
                                                text(Text::Delete),
                                                false,
                                            )
                                            .clicked()
                                            {
                                                remove = Some(index);
                                            }
                                        });
                                    }
                                    if let Some(index) = remove {
                                        setup.plan.remove(index);
                                    }
                                    ui.horizontal(|ui| {
                                        if ui.button(text(Text::AddStage)).clicked() {
                                            let seconds = setup
                                                .plan
                                                .last()
                                                .map_or(setup.interval, |s| s.seconds);
                                            setup.plan.push(Stage { count: 5, seconds });
                                        }
                                        if !setup.plan.is_empty()
                                            && !board
                                                .plans
                                                .iter()
                                                .any(|(_, plan)| *plan == setup.plan)
                                            && ui.button(text(Text::SavePlan)).clicked()
                                        {
                                            ui.ctx().data_mut(|d| {
                                                d.insert_temp(
                                                    egui::Id::new("session_save_plan"),
                                                    true,
                                                )
                                            });
                                        }
                                    });
                                    let (count, time) = setup.total(images);
                                    ui.weak(if images == 0 {
                                        text(Text::NoSessionImages).to_owned()
                                    } else {
                                        format!(
                                            "{} · {}",
                                            moth_ui::item_counts(moth_ui::locale(), count, 0),
                                            seconds(time.as_secs())
                                        )
                                    });
                                });
                                if wide {
                                    ui.add_space(tokens.space_xl as f32);
                                }
                                ui.vertical(|ui| {
                                    ui.set_width(column);
                                    section(ui, Text::SessionOrder);
                                    for (order, label) in [
                                        (Order::Shuffle, Text::OrderShuffle),
                                        (Order::Board, Text::OrderBoard),
                                        (Order::Random, Text::OrderRandom),
                                    ] {
                                        ui.radio_value(&mut setup.order, order, text(label));
                                    }
                                    section(ui, Text::DuringSession);
                                    ui.checkbox(&mut setup.sound, text(Text::SessionSound));
                                    ui.checkbox(&mut setup.countdown, text(Text::SessionCountdown));
                                    ui.checkbox(&mut setup.warn, text(Text::SessionWarn));
                                    ui.checkbox(&mut setup.hide, text(Text::SessionHide));
                                    ui.add_space(tokens.space_s as f32);
                                    ui.horizontal(|ui| {
                                        ui.selectable_value(
                                            &mut setup.mini,
                                            false,
                                            text(Text::SessionFullscreen),
                                        );
                                        ui.selectable_value(
                                            &mut setup.mini,
                                            true,
                                            text(Text::SessionMini),
                                        );
                                    });
                                });
                            });
                            ui.add_space(tokens.space_m as f32);
                            ui.horizontal(|ui| {
                                if ui.button(text(Text::Cancel)).clicked() {
                                    outcome = Some(false);
                                }
                                ui.add_enabled_ui(images > 0, |ui| {
                                    let start = egui::Button::new(
                                        egui::RichText::new(text(Text::StartSession))
                                            .color(tokens.on_accent),
                                    )
                                    .fill(tokens.accent)
                                    .shortcut_text("Enter");
                                    if ui.add(start).clicked() {
                                        outcome = Some(true);
                                    }
                                });
                            });
                            let (enter, escape) = ui.input(|i| {
                                (
                                    i.key_pressed(egui::Key::Enter),
                                    i.key_pressed(egui::Key::Escape),
                                )
                            });
                            if escape {
                                outcome = Some(false);
                            } else if enter && images > 0 && !ui.ctx().egui_wants_keyboard_input() {
                                outcome = Some(true);
                            }
                        });
                });
        });
    outcome
}

fn running_screen(
    ui: &mut egui::Ui,
    run: &mut Run,
    active: Instant,
    now: Instant,
    draws: &mut Vec<(Uuid, egui::Rect)>,
    events: &mut Vec<Event>,
    board: &Board<'_>,
) {
    let tokens = moth_ui::tokens(ui.ctx());
    let area = ui.ctx().content_rect();
    let mini = run.setup.mini;
    let gap = if mini {
        0.
    } else {
        tokens.space_xl as f32 * 2.
    };
    let paused = matches!(run.phase, Phase::Paused { .. });
    let counting = matches!(run.phase, Phase::Countdown(_));
    let hidden = run.setup.hide
        && !paused
        && !counting
        && now.saturating_duration_since(active) > HIDE_AFTER;
    let warning = run.warning(now);
    // Keys work whether or not the panel shows.
    let (space, left, right, escape) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::Space),
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if space {
        run.toggle_pause(now);
    }
    if left {
        run.step(false, now);
    }
    if right {
        run.step(true, now);
    }
    if escape {
        run.finish(now);
        return;
    }
    // The stage: the slide fitted into the window, drawn by the renderer.
    let slide = run.current().map(|s| s.item);
    if let Some(item) = slide.and_then(|id| board.scene.get(id)) {
        let envelope = item.rect.rotated_envelope(item.transform.rotation);
        let size = egui::vec2(
            envelope.upper()[0] - envelope.lower()[0],
            envelope.upper()[1] - envelope.lower()[1],
        );
        let room = area.shrink(gap);
        let scale = (room.width() / size.x).min(room.height() / size.y);
        draws.push((
            item.id,
            egui::Rect::from_center_size(room.center(), size * scale),
        ));
    }
    let painter = ui.painter().clone();
    let response = ui.allocate_rect(area, egui::Sense::click_and_drag());
    if mini && response.drag_started() {
        events.push(Event::Drag);
    }
    if counting || paused {
        painter.rect_filled(area, 0., tokens.theater.gamma_multiply(0.6));
        let (big, caption, hint) = if counting {
            (
                format!("{}", run.left(now).as_secs_f32().ceil() as u32),
                format!(
                    "{} {}",
                    text(Text::FirstImageIn),
                    seconds(run.left(now).as_secs_f32().ceil() as u64)
                ),
                text(Text::SpaceToStart),
            )
        } else {
            (
                String::new(),
                text(Text::Paused).to_owned(),
                text(Text::SpaceToResume),
            )
        };
        let center = area.center();
        let ink = tokens.on_theater;
        if !big.is_empty() {
            painter.text(
                center - egui::vec2(0., tokens.space_xl as f32),
                egui::Align2::CENTER_CENTER,
                big,
                egui::FontId::proportional(if mini { 40. } else { 96. }),
                ink,
            );
        }
        painter.text(
            center + egui::vec2(0., tokens.space_xl as f32 * 1.5),
            egui::Align2::CENTER_CENTER,
            caption,
            egui::FontId::proportional(16.),
            ink,
        );
        painter.text(
            center + egui::vec2(0., tokens.space_xl as f32 * 2.5),
            egui::Align2::CENTER_CENTER,
            hint,
            egui::FontId::proportional(13.),
            ink.gamma_multiply(0.7),
        );
    }
    // The timer never hides.
    let ring = if mini { 20. } else { 42. };
    let margin = if mini {
        tokens.space_s as f32
    } else {
        tokens.space_xl as f32
    };
    let center = egui::pos2(area.right() - margin - ring, area.top() + margin + ring);
    let accent = if warning {
        tokens.wing
    } else if paused {
        tokens.text_muted
    } else {
        tokens.accent
    };
    painter.circle_filled(center, ring, tokens.theater.gamma_multiply(0.85));
    painter.circle_stroke(
        center,
        ring - 2.5,
        egui::Stroke::new(if mini { 3. } else { 5. }, tokens.border),
    );
    let fraction = run.fraction(now);
    let points: Vec<_> = (0..=64)
        .map(|step| {
            let angle =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * fraction * step as f32 / 64.;
            center + egui::vec2(angle.cos(), angle.sin()) * (ring - 2.5)
        })
        .collect();
    if fraction > 0. {
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(if mini { 3. } else { 5. }, accent),
        ));
    }
    let left_text = if mini {
        format!("{}", run.left(now).as_secs_f32().ceil() as u64)
    } else {
        clock(run.left(now))
    };
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        left_text,
        egui::FontId::monospace(if mini { 11. } else { 18. }),
        if warning {
            tokens.wing
        } else {
            tokens.on_theater
        },
    );
    if mini && warning {
        painter.rect_stroke(
            area,
            0.,
            egui::Stroke::new(2., tokens.wing),
            egui::StrokeKind::Inside,
        );
    }
    // Progress: one segment per slide while they fit, else a thin bar.
    let total = run.slides.len();
    let done = run.index;
    if hidden || mini {
        let y = area.bottom() - 2.;
        painter.line_segment(
            [egui::pos2(area.left(), y), egui::pos2(area.right(), y)],
            egui::Stroke::new(2., tokens.border),
        );
        let x = area.left() + area.width() * (done as f32 + 1. - fraction) / total.max(1) as f32;
        painter.line_segment(
            [egui::pos2(area.left(), y), egui::pos2(x, y)],
            egui::Stroke::new(2., tokens.accent),
        );
    }
    if hidden {
        return;
    }
    if mini {
        let label = format!(
            "{} / {total} · {}",
            run.index + 1,
            seconds(run.current().map_or(0, |s| (s.seconds + s.extra) as u64))
        );
        painter.text(
            egui::pos2(area.left() + margin, area.top() + margin + ring),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.5),
            tokens.on_theater,
        );
    } else {
        let stages: Vec<_> = {
            let mut stages: Vec<Stage> = Vec::new();
            for slide in &run.slides {
                let same = stages.len() == slide.stage + 1;
                match stages.last_mut() {
                    Some(stage) if same && stage.seconds == slide.seconds => stage.count += 1,
                    _ => stages.push(Stage {
                        count: 1,
                        seconds: slide.seconds,
                    }),
                }
            }
            stages
        };
        let top = area.top() + margin;
        painter.text(
            egui::pos2(area.left() + margin, top),
            egui::Align2::LEFT_TOP,
            text(Text::DrawingSession),
            egui::FontId::proportional(14.),
            tokens.on_theater,
        );
        painter.text(
            egui::pos2(area.left() + margin, top + 22.),
            egui::Align2::LEFT_TOP,
            format!("{} · {}", plan_label(&stages), run.source),
            egui::FontId::proportional(12.5),
            tokens.text_muted,
        );
    }
    // Controls at the bottom centre.
    let id = egui::Id::new("session_controls");
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::CENTER_BOTTOM,
            egui::vec2(
                0.,
                -(if mini {
                    tokens.space_m
                } else {
                    tokens.space_xl
                } as f32),
            ),
        )
        .show(ui.ctx(), |ui| {
            ui.vertical_centered(|ui| {
                if !mini && total <= 60 {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2((total as f32 * 26.).min(area.width() - 32.), 4.),
                        egui::Sense::hover(),
                    );
                    let width = rect.width() / total.max(1) as f32;
                    for index in 0..total {
                        let x = rect.left() + width * index as f32;
                        let segment = egui::Rect::from_min_size(
                            egui::pos2(x, rect.top()),
                            egui::vec2(width - 4., 4.),
                        );
                        let color = if index == done {
                            tokens.accent
                        } else if index < done {
                            tokens.text_muted
                        } else {
                            tokens.border
                        };
                        ui.painter().rect_filled(segment, 2., color);
                    }
                    ui.add_space(tokens.space_m as f32);
                }
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if moth_ui::icons::button(
                            ui,
                            moth_ui::icons::Icon::Previous,
                            text(Text::PreviousImage),
                            false,
                        )
                        .clicked()
                        {
                            run.step(false, now);
                        }
                        let playing = matches!(run.phase, Phase::Running { .. });
                        let icon = if playing {
                            moth_ui::icons::Icon::Pause
                        } else {
                            moth_ui::icons::Icon::Play
                        };
                        if moth_ui::icons::button(
                            ui,
                            icon,
                            text(if playing { Text::Paused } else { Text::Resume }),
                            true,
                        )
                        .clicked()
                        {
                            run.toggle_pause(now);
                        }
                        if moth_ui::icons::button(
                            ui,
                            moth_ui::icons::Icon::Next,
                            text(Text::NextImage),
                            false,
                        )
                        .clicked()
                        {
                            run.step(true, now);
                        }
                        ui.separator();
                        if !mini
                            && ui
                                .button(format!("+{}", seconds(session::EXTRA_SECONDS as u64)))
                                .clicked()
                        {
                            run.extend();
                        }
                        if !mini
                            && moth_ui::icons::button(
                                ui,
                                moth_ui::icons::Icon::Sound,
                                text(Text::SessionSound),
                                run.setup.sound,
                            )
                            .clicked()
                        {
                            run.setup.sound = !run.setup.sound;
                        }
                        let switch = if mini {
                            Text::SessionFullscreen
                        } else {
                            Text::SessionMini
                        };
                        let icon = if mini {
                            moth_ui::icons::Icon::Expand
                        } else {
                            moth_ui::icons::Icon::Shrink
                        };
                        if moth_ui::icons::button(ui, icon, text(switch), false).clicked() {
                            run.setup.mini = !run.setup.mini;
                            if let Some(item) = run.current().and_then(|s| board.scene.get(s.item))
                            {
                                let envelope = item.rect.rotated_envelope(item.transform.rotation);
                                events.push(Event::Window {
                                    mini: run.setup.mini,
                                    aspect: (envelope.upper()[0] - envelope.lower()[0])
                                        / (envelope.upper()[1] - envelope.lower()[1]).max(1.),
                                });
                            }
                        }
                        let finish = if mini {
                            moth_ui::icons::button(
                                ui,
                                moth_ui::icons::Icon::Close,
                                text(Text::FinishSession),
                                false,
                            )
                        } else {
                            ui.add(
                                egui::Button::new(text(Text::FinishSession)).shortcut_text("Esc"),
                            )
                        };
                        if finish.clicked() {
                            run.finish(now);
                        }
                    });
                });
            });
        });
}

enum SummaryChoice {
    Close,
    Select,
    Show(Uuid),
    New,
    Replay(Replay),
}
fn summary_screen(
    ui: &mut egui::Ui,
    run: &mut Run,
    board: &Board<'_>,
    draws: &mut Vec<(Uuid, egui::Rect)>,
    clip: &mut Option<egui::Rect>,
    settled: bool,
) -> Option<SummaryChoice> {
    let tokens = moth_ui::tokens(ui.ctx());
    let mut choice = None;
    let (shown, spent, skipped) = run.summary();
    let starred = run.slides.iter().filter(|s| s.shown && s.star).count();
    let area = ui.ctx().content_rect().shrink(tokens.space_xl as f32);
    // No frame fills: thumbnails are drawn by the renderer below this UI.
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let ui = &mut child;
    // Only the free text sits on the dark stage; buttons keep the theme's colors.
    ui.label(
        egui::RichText::new(text(Text::SessionDone))
            .heading()
            .color(tokens.on_theater),
    );
    let stages: Vec<Stage> = {
        let mut stages: Vec<Stage> = Vec::new();
        for slide in run.slides.iter().filter(|s| s.shown) {
            match stages.last_mut() {
                Some(stage) if stage.seconds == slide.seconds => stage.count += 1,
                _ => stages.push(Stage {
                    count: 1,
                    seconds: slide.seconds,
                }),
            }
        }
        stages
    };
    ui.label(
        egui::RichText::new(format!(
            "{} · {} · {} · {} {}{}",
            moth_ui::item_counts(moth_ui::locale(), shown, 0),
            run.source,
            plan_label(&stages),
            text(Text::TotalLabel),
            seconds(spent.as_secs_f32().ceil() as u64),
            if skipped > 0 {
                format!(", {}: {skipped}", text(Text::SkippedLabel))
            } else {
                String::new()
            }
        ))
        .color(tokens.text_muted),
    );
    ui.add_space(tokens.space_m as f32);
    ui.horizontal_wrapped(|ui| {
        if ui.button(text(Text::SelectOnBoard)).clicked() {
            choice = Some(SummaryChoice::Select);
        }
        if ui.button(text(Text::NewSession)).clicked() {
            choice = Some(SummaryChoice::New);
        }
        let again =
            egui::Button::new(egui::RichText::new(text(Text::ReplaySame)).color(tokens.on_accent))
                .fill(tokens.accent);
        if ui.add(again).clicked() {
            choice = Some(SummaryChoice::Replay(Replay::Same));
        }
        ui.menu_button("▾", |ui| {
            for (kind, label, hint, count) in [
                (Replay::Same, Text::ReplaySame, Text::ReplaySameHint, None),
                (
                    Replay::Reshuffle,
                    Text::ReplayShuffle,
                    Text::ReplayShuffleHint,
                    None,
                ),
                (
                    Replay::Skipped,
                    Text::ReplaySkipped,
                    Text::ReplaySkippedHint,
                    Some(skipped),
                ),
                (
                    Replay::Starred,
                    Text::ReplayStarred,
                    Text::ReplayStarredHint,
                    Some(starred),
                ),
            ] {
                let title = match count {
                    Some(count) => format!("{} · {count}", text(label)),
                    None => text(label).to_owned(),
                };
                let enabled = count.is_none_or(|count| count > 0);
                let response = ui
                    .add_enabled(enabled, egui::Button::new(title))
                    .on_hover_text(text(hint));
                if response.clicked() {
                    choice = Some(SummaryChoice::Replay(kind));
                    ui.close();
                }
            }
        });
        if ui
            .add(egui::Button::new(text(Text::CloseSummary)).shortcut_text("Esc"))
            .clicked()
        {
            choice = Some(SummaryChoice::Close);
        }
    });
    if settled && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        choice = Some(SummaryChoice::Close);
    }
    ui.add_space(tokens.space_m as f32);
    let cell = egui::vec2(132., 132.);
    let scroll = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut current_stage = usize::MAX;
            let mut number = 0;
            let slides: Vec<usize> = (0..run.slides.len())
                .filter(|i| run.slides[*i].shown)
                .collect();
            let mut index = 0;
            while index < slides.len() {
                let stage = run.slides[slides[index]].stage;
                if stage != current_stage {
                    current_stage = stage;
                    ui.add_space(tokens.space_s as f32);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} · {}",
                            text(Text::StageLabel),
                            stage + 1,
                            seconds(run.slides[slides[index]].seconds as u64)
                        ))
                        .strong()
                        .color(tokens.on_theater),
                    );
                }
                let end = slides[index..]
                    .iter()
                    .position(|i| run.slides[*i].stage != stage)
                    .map_or(slides.len(), |n| index + n);
                ui.horizontal_wrapped(|ui| {
                    for &slot in &slides[index..end] {
                        number += 1;
                        let (rect, response) = ui
                            .allocate_exact_size(cell + egui::vec2(0., 22.), egui::Sense::click());
                        let picture = egui::Rect::from_min_size(rect.min, cell);
                        let slide = &mut run.slides[slot];
                        if let Some(item) = board.scene.get(slide.item) {
                            let envelope = item.rect.rotated_envelope(item.transform.rotation);
                            let size = egui::vec2(
                                envelope.upper()[0] - envelope.lower()[0],
                                envelope.upper()[1] - envelope.lower()[1],
                            );
                            let scale = (cell.x / size.x).min(cell.y / size.y);
                            draws.push((
                                item.id,
                                egui::Rect::from_center_size(picture.center(), size * scale),
                            ));
                        } else {
                            ui.painter().rect_stroke(
                                picture,
                                tokens.radius_s,
                                egui::Stroke::new(1., tokens.border),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let star = egui::Rect::from_min_size(
                            picture.right_top() - egui::vec2(26., -4.),
                            egui::vec2(22., 22.),
                        );
                        let star_response =
                            ui.interact(star, response.id.with("star"), egui::Sense::click());
                        ui.painter().circle_filled(
                            star.center(),
                            11.,
                            tokens.theater.gamma_multiply(0.8),
                        );
                        moth_ui::icons::paint(
                            ui,
                            star.shrink(3.),
                            moth_ui::icons::Icon::Star,
                            if slide.star {
                                tokens.group(2)
                            } else {
                                tokens.on_theater
                            },
                        );
                        if star_response.clicked() {
                            slide.star = !slide.star;
                        } else if response.clicked() {
                            choice = Some(SummaryChoice::Show(slide.item));
                        }
                        let time = if slide.skipped {
                            format!(
                                "{number}  {} {}",
                                text(Text::SkippedLabel),
                                clock(slide.spent)
                            )
                        } else if slide.extra > 0 {
                            format!("{number}  {}  +{}", clock(slide.spent), slide.extra)
                        } else {
                            format!("{number}  {}", clock(slide.spent))
                        };
                        ui.painter().text(
                            egui::pos2(picture.left(), picture.bottom() + 4.),
                            egui::Align2::LEFT_TOP,
                            time,
                            egui::FontId::proportional(12.),
                            if slide.skipped {
                                tokens.wing
                            } else {
                                tokens.text_muted
                            },
                        );
                    }
                });
                index = end;
            }
            ui.add_space(tokens.space_m as f32);
            ui.label(egui::RichText::new(text(Text::SummaryHint)).color(tokens.text_muted));
        });
    *clip = Some(scroll.inner_rect);
    choice
}
