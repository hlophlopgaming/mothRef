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
    /// Open original: which external viewer to use.
    pub viewer_open: bool,
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
        if !self.edit.open
            && !self.arrange.open
            && !self.export.open
            && !self.pickers.open
            && !self.viewer_open
        {
            return Vec::new();
        }
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
        let mut open = self.viewer_open;
        egui::Window::new(text(Text::OpenOriginal))
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                export_feedback(ui, board);
            });
        self.viewer_open = open;
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
