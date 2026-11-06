// SPDX-License-Identifier: GPL-3.0-or-later
//! Drafts for specialised editors; all effects run registered actions.
use super::arrange;
use super::{
    Document,
    bindings::{Action, Registry},
    intent::Intent,
    settings::Settings,
    snap,
};
use moth_scene::{Rect, Transform, Uuid};
use moth_ui::{Text, text};
/// The small editor windows opened by actions, each with its own draft.
#[derive(Default)]
pub(super) struct Tools {
    /// Edit selection: exact frame and content values of one item.
    pub edit: ItemEditor,
    pub arrange: ArrangeOptions,
    pub export: ExportForm,
    pub pickers: Pickers,
    /// Open original: the short notice shown while the system viewer starts.
    pub viewer: Option<ViewerNotice>,
}
/// "Open original" hands the image to the system viewer; the board only shows a short
/// notice near the bottom toolbar, never a window of its own.
#[derive(Clone, Copy)]
pub(super) struct ViewerNotice {
    /// When the action ran; a request that never answers stops showing after a while.
    pub started: std::time::Instant,
    /// When the viewer answered (opened or failed); the notice fades after it.
    pub answered: Option<std::time::Instant>,
}
impl ViewerNotice {
    pub fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
            answered: None,
        }
    }
}
#[derive(Default)]
pub(super) struct ItemEditor {
    pub open: bool,
    /// Values being typed, applied by "Apply".
    pub draft: Option<(Uuid, Rect, Transform)>,
    /// The item as the draft last saw it, so outside edits refresh the draft.
    pub observed: Option<(Uuid, Rect, Transform)>,
}
/// Layout options, and the state that makes repeating a layout reverse it.
pub(super) struct ArrangeOptions {
    pub open: bool,
    pub layout_counts: [usize; 2],
    pub order: u8,
    pub last_arrange: Option<(u8, u8, u64)>,
    pub reversed: bool,
    pub random_salt: u128,
    /// Scale factor for "Scale selection".
    pub factor: f32,
}
impl Default for ArrangeOptions {
    fn default() -> Self {
        Self {
            open: false,
            layout_counts: [1, 1],
            order: 0,
            last_arrange: None,
            reversed: false,
            random_salt: 0,
            factor: 1.,
        }
    }
}
pub(super) struct ExportForm {
    pub open: bool,
    pub path: String,
    /// Longest side of a scene export, in pixels.
    pub side: u32,
    /// World region exported when `crop` is on.
    pub region: Rect,
    pub crop: bool,
}
impl Default for ExportForm {
    fn default() -> Self {
        Self {
            open: false,
            path: String::new(),
            side: 2048,
            region: Rect {
                x: 0.,
                y: 0.,
                width: 1024.,
                height: 1024.,
            },
            crop: false,
        }
    }
}
#[derive(Default)]
pub(super) struct Pickers {
    pub open: bool,
    /// HEX, RGB or HSV for sampled colors.
    pub color_format: u8,
}
const MIB: u64 = 1024 * 1024;
fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1024. * MIB as f64))
}
/// Adapter, its memory and the limit derived from it, for settings and the status line.
pub(super) fn video_summary(video: &moth_platform::gpu::VideoMemory) -> String {
    let memory = match video.dedicated {
        Some(bytes) => gib(bytes),
        None => format!(
            "{}{}",
            text(Text::VideoShared),
            video
                .system
                .map(|b| format!(" ({})", gib(b)))
                .unwrap_or_default()
        ),
    };
    format!(
        "{}: {} · {} — {}: {}",
        text(Text::VideoCard),
        video.name,
        memory,
        text(Text::VideoLimitIs),
        gib(video.limit)
    )
}
fn number(ui: &mut egui::Ui, key: Text, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.horizontal(|ui| {
        ui.label(text(key));
        ui.add(egui::DragValue::new(value).range(range));
    });
}
fn rect(ui: &mut egui::Ui, rect: &mut Rect) {
    number(ui, Text::PositionX, &mut rect.x, -1e9..=1e9);
    number(ui, Text::PositionY, &mut rect.y, -1e9..=1e9);
    number(ui, Text::Width, &mut rect.width, 0.001..=1e9);
    number(ui, Text::Height, &mut rect.height, 0.001..=1e9);
}
impl Tools {
    pub fn blocked(&self) -> bool {
        self.edit.open || self.arrange.open || self.export.open
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        board: &Document,
        settings: &mut Settings,
        tool: &mut super::gesture::Tool,
        enabled: &[bool],
        registry: &Registry,
    ) -> Vec<Intent> {
        if !self.edit.open && !self.arrange.open && !self.export.open && !self.pickers.open {
            viewer_notice(ui.ctx(), &mut self.viewer, board, settings.reduced_motion);
            return Vec::new();
        }
        viewer_notice(ui.ctx(), &mut self.viewer, board, settings.reduced_motion);
        let mut intents = Vec::new();
        let items: Vec<_> = board
            .selection
            .iter()
            .filter_map(|id| board.scene.get(*id))
            .collect();
        let mut open = self.edit.open;
        egui::Window::new(text(Text::EditItem))
            .open(&mut open)
            .vscroll(true)
            .max_height(ui.content_rect().height())
            .show(ui.ctx(), |ui| {
                if items.len() == 1 {
                    let item = items[0];
                    if self.edit.observed != Some((item.id, item.rect, item.transform)) {
                        self.edit.observed = Some((item.id, item.rect, item.transform));
                        self.edit.draft = Some((item.id, item.rect, item.transform));
                    }
                    let rotation_snap = settings.rotation_snap;
                    egui::CollapsingHeader::new(text(Text::EditSelection))
                        .default_open(true)
                        .show(ui, |ui| {
                            let Some((_, frame, t)) = &mut self.edit.draft else {
                                return;
                            };
                            ui.label(&item.name);
                            let previous_width = frame.width;
                            rect(ui, frame);
                            if item.note().is_some() {
                                if t.note_font_size == 0. {
                                    t.note_font_size = item.note_font_size();
                                }
                                if frame.width != previous_width {
                                    t.note_wrap = true;
                                }
                                ui.checkbox(&mut t.note_wrap, text(Text::NoteWrap));
                                number(ui, Text::NoteFontSize, &mut t.note_font_size, 0.001..=1e9);
                            }
                            let mut degrees = t.rotation.to_degrees();
                            number(ui, Text::Rotation, &mut degrees, -36000.0..=36000.0);
                            t.rotation = degrees.to_radians();
                            if rotation_snap {
                                t.rotation = snap::snap_angle(t.rotation, 15.0_f32.to_radians());
                            }
                            ui.add(
                                egui::Slider::new(&mut t.opacity, 0.0..=1.0)
                                    .text(text(Text::Opacity)),
                            );
                            ui.add_enabled(
                                item.blob().is_some(),
                                egui::Checkbox::new(&mut t.grayscale, text(Text::Grayscale)),
                            );
                            ui.add_enabled(
                                item.blob().is_some(),
                                egui::Checkbox::new(&mut t.flip[0], text(Text::FlipHorizontal)),
                            );
                            ui.add_enabled(
                                item.blob().is_some(),
                                egui::Checkbox::new(&mut t.flip[1], text(Text::FlipVertical)),
                            );
                            if item.blob().is_some() {
                                let labels = [
                                    Text::SamplingDefault,
                                    Text::SamplingSmooth,
                                    Text::SamplingNearest,
                                    Text::SamplingAutomatic,
                                ];
                                egui::ComboBox::from_id_salt("image_sampling")
                                    .selected_text(text(labels[t.sampling as usize]))
                                    .show_ui(ui, |ui| {
                                        for (index, label) in labels.into_iter().enumerate() {
                                            if let Some(sampling) =
                                                moth_scene::Sampling::from_code(index as u8)
                                            {
                                                ui.selectable_value(
                                                    &mut t.sampling,
                                                    sampling,
                                                    text(label),
                                                );
                                            }
                                        }
                                    });
                            }
                            ui.add_enabled_ui(item.blob().is_some(), |ui| {
                                ui.collapsing(text(Text::Content), |ui| {
                                    number(
                                        ui,
                                        Text::ContentScale,
                                        &mut t.content_scale,
                                        0.001..=1000.,
                                    );
                                    number(
                                        ui,
                                        Text::OffsetX,
                                        &mut t.content_offset[0],
                                        -1000.0..=1000.0,
                                    );
                                    number(
                                        ui,
                                        Text::OffsetY,
                                        &mut t.content_offset[1],
                                        -1000.0..=1000.0,
                                    );
                                    let mut degrees = t.content_rotation.to_degrees();
                                    number(ui, Text::Rotation, &mut degrees, -36000.0..=36000.0);
                                    t.content_rotation = degrees.to_radians();
                                });
                            });
                            super::surfaces::menu(
                                ui,
                                &[
                                    super::surfaces::Slot::Action(
                                        super::bindings::Action::ApplyItem,
                                    ),
                                    super::surfaces::Slot::Action(
                                        super::bindings::Action::CancelItem,
                                    ),
                                ],
                                enabled,
                                registry,
                                &mut intents,
                            );
                        });
                } else {
                    self.edit.draft = None;
                    self.edit.observed = None;
                }
            });
        self.edit.open = open;
        let mut open = self.arrange.open;
        egui::Window::new(text(Text::ArrangeOptions))
            .open(&mut open)
            .vscroll(true)
            .max_height(ui.content_rect().height())
            .show(ui.ctx(), |ui| {
                let before = settings.preferences();
                number(ui, Text::Gap, &mut settings.gap, 1.0..=10000.);
                egui::ComboBox::from_id_salt("arrange_order")
                    .selected_text(text(order_label(self.arrange.order)))
                    .show_ui(ui, |ui| {
                        for i in 0..4 {
                            ui.selectable_value(&mut self.arrange.order, i, text(order_label(i)));
                        }
                    });
                for (i, label) in [Text::LayoutRows, Text::LayoutColumns]
                    .into_iter()
                    .enumerate()
                {
                    ui.add(
                        egui::Slider::new(
                            &mut self.arrange.layout_counts[i],
                            1..=board.scene.len().max(1),
                        )
                        .text(text(label)),
                    );
                }
                super::surfaces::menu(
                    ui,
                    super::surfaces::ARRANGE,
                    enabled,
                    registry,
                    &mut intents,
                );
                number(
                    ui,
                    Text::ScaleSelection,
                    &mut self.arrange.factor,
                    0.001..=1000.,
                );
                super::surfaces::menu(
                    ui,
                    &[super::surfaces::Slot::Action(Action::ScaleApply)],
                    enabled,
                    registry,
                    &mut intents,
                );
                if before != settings.preferences() {
                    intents.push(Intent::PreferencesDirty);
                }
            });
        self.arrange.open = open;
        let mut open = self.export.open;
        egui::Window::new(text(Text::Export))
            .open(&mut open)
            .vscroll(true)
            .max_height(ui.content_rect().height())
            .show(ui.ctx(), |ui| {
                ui.label(text(Text::ExportLimit));
                export_feedback(ui, board);
                ui.label(text(Text::ExportPath));
                ui.text_edit_singleline(&mut self.export.path);
                ui.horizontal(|ui| {
                    ui.label(text(Text::ExportSide));
                    ui.add(egui::DragValue::new(&mut self.export.side).range(1..=4096));
                });
                ui.checkbox(&mut self.export.crop, text(Text::CropRegion));
                if self.export.crop {
                    rect(ui, &mut self.export.region);
                }
                super::surfaces::menu(ui, super::surfaces::EXPORT, enabled, registry, &mut intents);
            });
        self.export.open = open;
        let mut open = self.pickers.open;
        egui::Window::new(text(Text::ImagePickers))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (i, label) in ["HEX", "RGB", "HSV"].into_iter().enumerate() {
                        ui.selectable_value(&mut self.pickers.color_format, i as u8, label);
                    }
                });
                if tool.picks() {
                    ui.label(text(if *tool == super::gesture::Tool::PickInfo {
                        Text::PickCoordinatesHint
                    } else {
                        Text::PickColorHint
                    }));
                }
                export_feedback(ui, board);
                super::surfaces::menu(
                    ui,
                    &[
                        super::surfaces::Slot::Action(Action::SampleColor),
                        super::surfaces::Slot::Action(Action::CoordinatePicker),
                    ],
                    enabled,
                    registry,
                    &mut intents,
                );
            });
        self.pickers.open = open;
        if !open {
            *tool = super::gesture::Tool::Select;
        }
        intents
    }
}
/// Shows "Opening original…" until the viewer answers, then the result for a moment.
fn viewer_notice(
    ctx: &egui::Context,
    slot: &mut Option<ViewerNotice>,
    board: &Document,
    reduced_motion: bool,
) {
    const SHOWN: f32 = 2.4;
    const FADE: f32 = 0.35;
    const PATIENCE: f32 = 30.;
    let Some(notice) = slot else {
        return;
    };
    let answer = board
        .notices
        .export_message
        .as_ref()
        .filter(|_| !board.file.exporting);
    if answer.is_some() && notice.answered.is_none() {
        notice.answered = Some(std::time::Instant::now());
    }
    let alpha = match notice.answered {
        Some(at) => {
            let age = at.elapsed().as_secs_f32();
            if age < SHOWN {
                ctx.request_repaint_after(std::time::Duration::from_secs_f32(SHOWN - age));
                1.
            } else {
                ctx.request_repaint();
                ((SHOWN + FADE - age) / FADE).clamp(0., 1.)
            }
        }
        None if notice.started.elapsed().as_secs_f32() > PATIENCE => 0.,
        None => 1.,
    };
    if alpha == 0. {
        *slot = None;
        return;
    }
    let tokens = moth_ui::tokens(ctx);
    let gap = tokens.space_m as f32;
    let viewport = ctx.content_rect();
    // The import notice keeps its last rect after it closes; count it only while shown.
    let added = board.notices.added.is_some();
    let bottom = ctx.memory(|memory| {
        ["bottom_toolbar", "selection_badge", "added_feedback"]
            .into_iter()
            .filter(|id| added || *id != "added_feedback")
            .filter_map(|id| memory.area_rect(egui::Id::new(id)))
            .map(|rect| rect.top())
            .fold(viewport.bottom(), f32::min)
    });
    egui::Area::new(egui::Id::new("viewer_feedback"))
        .order(egui::Order::Foreground)
        .interactable(false)
        .constrain_to(viewport)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(viewport.center().x, bottom - gap))
        .show(ctx, |ui| {
            ui.set_opacity(if reduced_motion { alpha.ceil() } else { alpha });
            ui.set_max_width((viewport.width() - gap * 2.).max(1.));
            egui::Frame::popup(ui.style())
                .corner_radius(tokens.radius_l)
                .inner_margin(egui::Margin::symmetric(
                    tokens.space_m as i8,
                    tokens.space_xs as i8,
                ))
                .show(ui, |ui| {
                    ui.horizontal(|ui| match answer {
                        None => {
                            ui.spinner();
                            ui.label(text(Text::OpeningOriginal));
                        }
                        Some(message) => {
                            let (icon, _) = ui.allocate_exact_size(
                                egui::Vec2::splat(ui.text_style_height(&egui::TextStyle::Body)),
                                egui::Sense::hover(),
                            );
                            // Success gets a check; an error is plain text in the warning tone.
                            let opened =
                                matches!(message, moth_ui::Message::Text(Text::ViewerOpened));
                            let (glyph, color) = if opened {
                                (moth_ui::icons::Icon::Check, tokens.accent_text)
                            } else {
                                (moth_ui::icons::Icon::Close, tokens.danger)
                            };
                            moth_ui::icons::paint(ui, icon, glyph, color);
                            ui.label(message.render(moth_ui::locale()));
                        }
                    });
                });
        });
}
fn export_feedback(ui: &mut egui::Ui, board: &Document) {
    if board.file.exporting {
        ui.spinner();
    }
    if let Some(message) = &board.notices.export_message {
        ui.label(message.render(moth_ui::locale()));
    }
}
pub(super) fn layout(i: u8) -> (Text, arrange::Arrange) {
    match i {
        1 => (Text::Vertical, arrange::Arrange::Vertical),
        2 => (Text::Grid, arrange::Arrange::Grid),
        3 => (Text::Optimal, arrange::Arrange::Optimal),
        _ => (Text::Horizontal, arrange::Arrange::Horizontal),
    }
}
pub(super) fn layout_icon(mode: arrange::Arrange) -> moth_ui::icons::Icon {
    use moth_ui::icons::Icon;
    match mode {
        arrange::Arrange::Horizontal => Icon::ArrangeHorizontal,
        arrange::Arrange::Vertical => Icon::ArrangeVertical,
        arrange::Arrange::Grid => Icon::ArrangeGrid,
        arrange::Arrange::Optimal => Icon::ArrangeOptimal,
    }
}
fn order_label(i: u8) -> Text {
    match i {
        1 => Text::ByFilename,
        2 => Text::ByTime,
        3 => Text::Random,
        _ => Text::ByLayer,
    }
}
