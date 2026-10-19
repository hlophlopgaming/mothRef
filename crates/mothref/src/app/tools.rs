// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit Apply keeps inspector drafts out of the journal until confirmed.
use super::{Document, arrange, intent::Intent};
use moth_platform::export::{Geometry, Target};
use moth_scene::{Command, Rect, Transform, Uuid};
use moth_ui::{Text, text};

pub(super) struct Tools {
    draft: Option<(Uuid, Rect, Transform)>,
    observed: Option<(Uuid, Rect, Transform)>,
    pub gap: f32,
    pub(super) order: u8,
    last_arrange: Option<(u8, u8, u64)>,
    reversed: bool,
    random_salt: u128,
    pub locked: bool,
    pub grayscale: bool,
    pub dark: bool,
    pub reduced_motion: bool,
    pub show_outlines: bool,
    pub drag_animation: bool,
    pub drag_opacity: f32,
    pub ui_scale: f32,
    pub threshold: usize,
    pub undo_limit: usize,
    pub wheel: u8,
    pub scale_with_window: bool,
    pub auto_arrange: bool,
    pub default_arrange: u8,
    pub coordinates: bool,
    factor: f32,
    path: String,
    side: u32,
    region: Rect,
    crop: bool,
    pub picking: bool,
    pub color_format: u8,
    pub open: bool,
}
impl Default for Tools {
    fn default() -> Self {
        Self {
            draft: None,
            observed: None,
            gap: 24.,
            order: 0,
            last_arrange: None,
            reversed: false,
            random_salt: 0,
            locked: false,
            grayscale: false,
            dark: true,
            reduced_motion: false,
            show_outlines: true,
            drag_animation: true,
            drag_opacity: super::motion::DEFAULT_DRAG_OPACITY,
            ui_scale: 1.,
            threshold: 5,
            undo_limit: 256,
            wheel: 0,
            scale_with_window: false,
            auto_arrange: false,
            default_arrange: 0,
            coordinates: false,
            factor: 1.,
            path: String::new(),
            side: 2048,
            region: Rect {
                x: 0.,
                y: 0.,
                width: 1024.,
                height: 1024.,
            },
            crop: false,
            picking: false,
            color_format: 0,
            open: false,
        }
    }
}
fn preference(key: Text, changed: bool) -> String {
    format!("{}{}", text(key), if changed { " *" } else { "" })
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
    pub fn arrange(
        &mut self,
        items: &mut Vec<&moth_scene::Item>,
        mode: arrange::Arrange,
        width: f32,
    ) -> Vec<Command> {
        use std::hash::{Hash, Hasher};
        let mut ids: Vec<_> = items.iter().map(|i| i.id).collect();
        ids.sort_unstable();
        let mut hash = std::hash::DefaultHasher::new();
        ids.hash(&mut hash);
        let key = (self.order, mode as u8, hash.finish());
        let repeat = self.last_arrange == Some(key);
        self.reversed = repeat && !self.reversed;
        self.last_arrange = Some(key);
        if !repeat {
            self.random_salt = Uuid::now_v7().as_u128();
        }
        match self.order {
            1 => items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
            2 => items.sort_by_key(|i| i.id),
            3 => items.sort_by_key(|i| {
                i.id.as_u128()
                    .wrapping_mul(self.random_salt | 1)
                    .rotate_left(43)
            }),
            _ => items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id))),
        }
        if self.reversed {
            items.reverse();
        }
        arrange::arrange(items, mode, width, self.gap)
    }
    pub fn preferences(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                moth_platform::settings::REDUCED_MOTION,
                self.reduced_motion.to_string(),
            ),
            ("show_outlines", self.show_outlines.to_string()),
            ("drag_animation", self.drag_animation.to_string()),
            ("drag_opacity", self.drag_opacity.to_string()),
            ("wheel", self.wheel.to_string()),
            ("scale_with_window", self.scale_with_window.to_string()),
            ("undo_limit", self.undo_limit.to_string()),
            ("dark", self.dark.to_string()),
            ("ui_scale", self.ui_scale.to_string()),
            ("arrange_gap", self.gap.to_string()),
            ("arrange_prompt_threshold", self.threshold.to_string()),
            ("auto_arrange", self.auto_arrange.to_string()),
            ("default_arrange", self.default_arrange.to_string()),
        ]
    }
    pub fn load(&mut self, values: &std::collections::BTreeMap<String, String>) {
        self.reduced_motion = values
            .get(moth_platform::settings::REDUCED_MOTION)
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.show_outlines = values
            .get("show_outlines")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.drag_animation = values
            .get("drag_animation")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.drag_opacity = values
            .get("drag_opacity")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.1..=1.).contains(v))
            .unwrap_or(super::motion::DEFAULT_DRAG_OPACITY);
        self.wheel = values
            .get("wheel")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 3)
            .unwrap_or(0);
        self.scale_with_window = values
            .get("scale_with_window")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.undo_limit = values
            .get("undo_limit")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v <= 4096)
            .unwrap_or(256);
        self.dark = values
            .get("dark")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        self.ui_scale = values
            .get("ui_scale")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.75..=2.).contains(v))
            .unwrap_or(1.);
        self.gap = values
            .get("arrange_gap")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && (0.0..=10000.).contains(v))
            .unwrap_or(24.);
        self.threshold = values
            .get("arrange_prompt_threshold")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v <= 10000)
            .unwrap_or(5);
        self.auto_arrange = values
            .get("auto_arrange")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false);
        self.default_arrange = values
            .get("default_arrange")
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 4)
            .unwrap_or(0);
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        board: &Document,
        editing: bool,
    ) -> (Vec<Command>, Option<Target>, bool, Vec<Intent>) {
        let mut commands = Vec::new();
        let mut export = None;
        let mut note = false;
        let mut intents = Vec::new();
        let mut open = self.open;
        egui::Window::new(text(Text::Tools))
            .open(&mut open)
            .fade_in(!self.reduced_motion)
            .fade_out(!self.reduced_motion)
            .default_width(280.)
            .max_width(ui.content_rect().width().max(1.))
            .max_height(ui.content_rect().height().max(1.))
            .vscroll(true)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().interact_size.y = ui.spacing().interact_size.y.max(36.);
                ui.collapsing(text(Text::Settings), |ui| {
                    let before = self.preferences();
                    let defaults = Self::default();
                    ui.checkbox(&mut self.reduced_motion, text(Text::ReducedMotion));
                    ui.checkbox(&mut self.show_outlines, text(Text::ShowOutlines));
                    ui.checkbox(&mut self.drag_animation, text(Text::DragAnimation));
                    ui.add_enabled(
                        !self.drag_animation || self.reduced_motion,
                        egui::Slider::new(&mut self.drag_opacity, 0.1..=1.)
                            .text(text(Text::DragOpacity)),
                    );

                    let label = preference(
                        Text::ScaleWithWindow,
                        self.scale_with_window != defaults.scale_with_window,
                    );
                    ui.checkbox(&mut self.scale_with_window, label);
                    egui::ComboBox::from_id_salt("wheel")
                        .selected_text(preference(
                            [Text::WheelZoom, Text::WheelPan, Text::WheelOff][self.wheel as usize],
                            self.wheel != defaults.wheel,
                        ))
                        .show_ui(ui, |ui| {
                            for (i, key) in [Text::WheelZoom, Text::WheelPan, Text::WheelOff]
                                .into_iter()
                                .enumerate()
                            {
                                ui.selectable_value(&mut self.wheel, i as u8, text(key));
                            }
                        });
                    let label = preference(Text::UndoLimit, self.undo_limit != defaults.undo_limit);
                    ui.add(egui::Slider::new(&mut self.undo_limit, 0..=4096).text(label));
                    let bytes = board
                        .scene
                        .items()
                        .take(128)
                        .map(|i| {
                            std::mem::size_of::<moth_scene::Command>()
                                + i.name.len()
                                + i.note().map_or(0, str::len)
                        })
                        .sum::<usize>()
                        / board.scene.len().clamp(1, 128);
                    ui.label(format!(
                        "≈ {} KiB",
                        bytes.saturating_mul(self.undo_limit) / 1024
                    ));
                    let label = preference(Text::DarkTheme, self.dark != defaults.dark);
                    ui.checkbox(&mut self.dark, label);
                    let label = preference(Text::UiScale, self.ui_scale != defaults.ui_scale);
                    ui.add(egui::Slider::new(&mut self.ui_scale, 0.75..=2.).text(label));
                    ui.horizontal(|ui| {
                        ui.label(preference(Text::Gap, self.gap != defaults.gap));
                        ui.add(egui::DragValue::new(&mut self.gap).range(0.0..=10000.));
                    });
                    ui.horizontal(|ui| {
                        ui.label(preference(
                            Text::ArrangeThreshold,
                            self.threshold != defaults.threshold,
                        ));
                        ui.add(egui::DragValue::new(&mut self.threshold).range(0..=10000));
                    });
                    let label = preference(
                        Text::AutoArrange,
                        self.auto_arrange != defaults.auto_arrange,
                    );
                    ui.checkbox(&mut self.auto_arrange, label);
                    egui::ComboBox::from_id_salt("default_arrange")
                        .selected_text(preference(
                            layout(self.default_arrange).0,
                            self.default_arrange != defaults.default_arrange,
                        ))
                        .show_ui(ui, |ui| {
                            for i in 0..4 {
                                ui.selectable_value(
                                    &mut self.default_arrange,
                                    i,
                                    text(layout(i).0),
                                );
                            }
                        });
                    if before != self.preferences() {
                        intents.push(Intent::PreferencesDirty);
                    }
                    if self.preferences() != defaults.preferences() {
                        ui.label(text(Text::ChangedSetting));
                    }
                    if self.preferences() != defaults.preferences()
                        && ui
                            .button(moth_ui::tooltip(ui.ctx(), Text::Defaults))
                            .clicked()
                    {
                        self.wheel = defaults.wheel;
                        self.scale_with_window = defaults.scale_with_window;
                        self.undo_limit = defaults.undo_limit;
                        self.dark = defaults.dark;
                        self.ui_scale = defaults.ui_scale;
                        self.gap = defaults.gap;
                        self.threshold = defaults.threshold;
                        self.auto_arrange = defaults.auto_arrange;
                        self.default_arrange = defaults.default_arrange;
                        intents.push(Intent::PreferencesDirty);
                    }
                });
                ui.add_enabled(
                    !editing && board.idle(),
                    egui::Checkbox::new(&mut self.locked, text(Text::LockCanvas)),
                );
                ui.checkbox(&mut self.grayscale, text(Text::CanvasGrayscale));
                intents.push(Intent::SetViewFlags {
                    locked: self.locked,
                    grayscale: self.grayscale,
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button(moth_ui::tooltip(ui.ctx(), Text::Fit)).clicked() {
                        intents.push(Intent::NeedsFit);
                    }
                    if ui
                        .button(moth_ui::tooltip(ui.ctx(), Text::ResetCamera))
                        .clicked()
                    {
                        intents.push(Intent::ResetCamera);
                    }
                    if ui
                        .button(moth_ui::tooltip(ui.ctx(), Text::PreviousImage))
                        .clicked()
                    {
                        intents.push(Intent::Focus(-1));
                    }
                    if ui
                        .button(moth_ui::tooltip(ui.ctx(), Text::NextImage))
                        .clicked()
                    {
                        intents.push(Intent::Focus(1));
                    }
                });
                ui.add_enabled_ui(board.idle(), |ui| {
                    let mut items: Vec<_> = board
                        .scene
                        .items()
                        .filter(|item| {
                            board.selection.is_empty() || board.selection.contains(&item.id)
                        })
                        .collect();
                    items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                    ui.add_enabled_ui(!self.locked, |ui| {
                        note = ui
                            .button(moth_ui::tooltip(ui.ctx(), Text::AddNote))
                            .clicked();
                        ui.label(format!(
                            "{}: {}",
                            text(Text::EditSelection),
                            board.selection.len()
                        ));
                        ui.add_enabled_ui(!board.selection.is_empty(), |ui| {
                            for (label, quarters) in [(Text::RotateCw, 1.), (Text::RotateCcw, -1.)]
                            {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(super::transform_selection(
                                        &board.scene,
                                        &board.selection,
                                        |t| super::rotated(t, quarters),
                                    ));
                                }
                            }
                            for (label, axis) in
                                [(Text::FlipHorizontal, 0), (Text::FlipVertical, 1)]
                            {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(super::transform_selection(
                                        &board.scene,
                                        &board.selection,
                                        |t| super::mirrored(t, axis),
                                    ));
                                }
                            }
                            if ui
                                .button(moth_ui::tooltip(ui.ctx(), Text::Delete))
                                .clicked()
                            {
                                commands
                                    .extend(board.selection.iter().copied().map(Command::Remove));
                            }
                        });
                        if items.len() == 1 {
                            let item = items[0];
                            if self.observed != Some((item.id, item.rect, item.transform)) {
                                self.observed = Some((item.id, item.rect, item.transform));
                                self.draft = Some((item.id, item.rect, item.transform));
                            }
                            ui.collapsing(text(Text::EditSelection), |ui| {
                                let Some((id, frame, t)) = &mut self.draft else {
                                    return;
                                };
                                ui.label(&item.name);
                                rect(ui, frame);
                                let mut degrees = t.rotation.to_degrees();
                                number(ui, Text::Rotation, &mut degrees, -36000.0..=36000.0);
                                t.rotation = degrees.to_radians();
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
                                        number(
                                            ui,
                                            Text::Rotation,
                                            &mut degrees,
                                            -36000.0..=36000.0,
                                        );
                                        t.content_rotation = degrees.to_radians();
                                    });
                                });
                                if ui
                                    .add_enabled(
                                        frame.valid() && t.valid(),
                                        egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::Apply)),
                                    )
                                    .clicked()
                                {
                                    if *frame != item.rect {
                                        commands.push(Command::SetRect {
                                            id: *id,
                                            rect: *frame,
                                        });
                                    }
                                    if *t != item.transform {
                                        commands.push(Command::SetTransform {
                                            id: *id,
                                            transform: *t,
                                        });
                                    }
                                }
                                if ui
                                    .button(moth_ui::tooltip(ui.ctx(), Text::Cancel))
                                    .clicked()
                                {
                                    *frame = item.rect;
                                    *t = item.transform;
                                }
                            });
                        } else {
                            self.draft = None;
                            self.observed = None;
                        }
                        ui.collapsing(text(Text::Arrange), |ui| {
                            let old_gap = self.gap;
                            number(ui, Text::Gap, &mut self.gap, 0.0..=10000.);
                            egui::ComboBox::from_id_salt("arrange_order")
                                .selected_text(text(order_label(self.order)))
                                .show_ui(ui, |ui| {
                                    for i in 0..4 {
                                        ui.selectable_value(
                                            &mut self.order,
                                            i,
                                            text(order_label(i)),
                                        );
                                    }
                                });
                            for i in 0..4 {
                                let (key, mode) = layout(i);
                                if ui
                                    .add_enabled(
                                        !items.is_empty(),
                                        egui::Button::new(moth_ui::tooltip(ui.ctx(), key)),
                                    )
                                    .clicked()
                                {
                                    commands.extend(self.arrange(
                                        &mut items,
                                        mode,
                                        board.view.viewport.width() / board.view.camera.zoom,
                                    ));
                                }
                            }
                            if old_gap != self.gap {
                                intents.push(Intent::PreferencesDirty);
                            }
                            if ui
                                .button(moth_ui::tooltip(ui.ctx(), Text::PackFit))
                                .clicked()
                            {
                                let mut all: Vec<_> = board.scene.items().collect();
                                all.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                                commands.extend(arrange::arrange(
                                    &all,
                                    arrange::Arrange::Optimal,
                                    board.view.viewport.width() / board.view.camera.zoom,
                                    self.gap,
                                ));
                                intents.push(Intent::NeedsFit);
                            }
                            for (label, mode) in [
                                (Text::NormalizeWidth, arrange::Normalize::Width),
                                (Text::NormalizeHeight, arrange::Normalize::Height),
                                (Text::NormalizeSize, arrange::Normalize::Size),
                                (Text::NormalizeArea, arrange::Normalize::Area),
                            ] {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(arrange::normalize(&items, mode));
                                }
                            }
                            for (label, mode) in [
                                (Text::AlignLeft, arrange::Align::Left),
                                (Text::AlignRight, arrange::Align::Right),
                                (Text::AlignTop, arrange::Align::Top),
                                (Text::AlignBottom, arrange::Align::Bottom),
                                (Text::DistributeHorizontal, arrange::Align::Horizontal),
                                (Text::DistributeVertical, arrange::Align::Vertical),
                                (Text::Stack, arrange::Align::Stack),
                            ] {
                                if ui.button(moth_ui::tooltip(ui.ctx(), label)).clicked() {
                                    commands.extend(arrange::align(&items, mode));
                                }
                            }
                            number(ui, Text::ScaleSelection, &mut self.factor, 0.001..=1000.);
                            if ui
                                .add_enabled(
                                    !items.is_empty(),
                                    egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::Apply)),
                                )
                                .clicked()
                            {
                                commands.extend(scale(&items, self.factor));
                            }
                        });
                        for (key, mode) in [
                            (Text::ResetScale, Geometry::Scale),
                            (Text::NormalizeScale, Geometry::Normalize),
                        ] {
                            if ui.button(moth_ui::tooltip(ui.ctx(), key)).clicked() {
                                intents.push(Intent::OriginalGeometry(
                                    items
                                        .iter()
                                        .filter(|i| i.blob().is_some())
                                        .map(|i| i.id)
                                        .collect(),
                                    mode,
                                ));
                            }
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (key, delta, duplicate) in [
                                (Text::Duplicate, [24., 24.], true),
                                (Text::NudgeLeft, [-1., 0.], false),
                                (Text::NudgeRight, [1., 0.], false),
                                (Text::NudgeUp, [0., -1.], false),
                                (Text::NudgeDown, [0., 1.], false),
                            ] {
                                if ui
                                    .add_enabled(
                                        !board.selection.is_empty(),
                                        egui::Button::new(moth_ui::tooltip(ui.ctx(), key)),
                                    )
                                    .clicked()
                                {
                                    commands.extend(arrange::translate(
                                        &items,
                                        delta,
                                        duplicate,
                                        board.scene.top(),
                                    ));
                                }
                            }
                        });
                        for (key, front, extreme) in [
                            (Text::BringFront, true, true),
                            (Text::SendBack, false, true),
                            (Text::BringForward, true, false),
                            (Text::SendBackward, false, false),
                        ] {
                            if ui.button(moth_ui::tooltip(ui.ctx(), key)).clicked() {
                                commands.extend(arrange::reorder(
                                    &board.scene,
                                    &items.iter().map(|i| i.id).collect(),
                                    front,
                                    extreme,
                                ));
                            }
                        }
                        for (key, kind) in [
                            (Text::ResetRotation, 0),
                            (Text::ResetFlip, 1),
                            (Text::ResetCrop, 2),
                            (Text::ResetTransform, 3),
                        ] {
                            if ui.button(moth_ui::tooltip(ui.ctx(), key)).clicked() {
                                if kind >= 2 {
                                    intents.push(Intent::OriginalGeometry(
                                        items.iter().map(|i| i.id).collect(),
                                        if kind == 2 {
                                            Geometry::Crop
                                        } else {
                                            Geometry::All
                                        },
                                    ));
                                    continue;
                                }
                                for item in &items {
                                    let mut transform = item.transform;
                                    match kind {
                                        0 => transform.rotation = 0.,
                                        1 => transform.flip = [false; 2],
                                        2 => {
                                            transform.content_scale = 1.;
                                            transform.content_offset = [0.; 2];
                                            transform.content_rotation = 0.;
                                        }
                                        _ => transform = transform.reset_geometry(),
                                    };
                                    commands.push(Command::SetTransform {
                                        id: item.id,
                                        transform,
                                    });
                                }
                            }
                        }
                    });
                    ui.checkbox(&mut self.picking, text(Text::SampleColor));
                    ui.horizontal(|ui| {
                        for (i, label) in ["HEX", "RGB", "HSV"].into_iter().enumerate() {
                            ui.selectable_value(&mut self.color_format, i as u8, label);
                        }
                    });
                    ui.checkbox(&mut self.coordinates, text(Text::CoordinatePicker));
                    ui.collapsing(text(Text::Export), |ui| {
                        ui.label(text(Text::ExportLimit));
                        ui.label(text(Text::Path));
                        ui.text_edit_singleline(&mut self.path);
                        ui.horizontal(|ui| {
                            ui.label(text(Text::ExportSide));
                            ui.add(egui::DragValue::new(&mut self.side).range(1..=4096));
                        });
                        ui.checkbox(&mut self.crop, text(Text::CropRegion));
                        if self.crop {
                            rect(ui, &mut self.region);
                        }
                        let path = std::path::PathBuf::from(&self.path);
                        let valid = !self.path.is_empty() && (!self.crop || self.region.valid());
                        if ui
                            .add_enabled(
                                valid && !board.scene.is_empty(),
                                egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::ExportScene)),
                            )
                            .clicked()
                        {
                            export = Some(Target::Scene {
                                path: path.clone(),
                                ids: Vec::new(),
                                region: self.crop.then_some(self.region),
                                side: self.side,
                            });
                        }
                        if ui
                            .add_enabled(
                                valid && !items.is_empty(),
                                egui::Button::new(moth_ui::tooltip(
                                    ui.ctx(),
                                    Text::ExportSelection,
                                )),
                            )
                            .clicked()
                        {
                            export = Some(Target::Scene {
                                path: path.clone(),
                                ids: items.iter().map(|i| i.id).collect(),
                                region: self.crop.then_some(self.region),
                                side: self.side,
                            });
                        }
                        if let [item] = items.as_slice()
                            && item.blob().is_some()
                        {
                            if ui
                                .add_enabled(
                                    valid,
                                    egui::Button::new(moth_ui::tooltip(
                                        ui.ctx(),
                                        Text::ExportOriginal,
                                    )),
                                )
                                .clicked()
                            {
                                export = Some(Target::Original {
                                    path: path.clone(),
                                    id: item.id,
                                });
                            }
                            if ui
                                .button(moth_ui::tooltip(ui.ctx(), Text::CopyImage))
                                .clicked()
                            {
                                export = Some(Target::Clipboard(item.id));
                            }
                        }
                        if ui
                            .add_enabled(
                                valid && !board.scene.is_empty(),
                                egui::Button::new(moth_ui::tooltip(
                                    ui.ctx(),
                                    Text::ExportOriginals,
                                )),
                            )
                            .clicked()
                        {
                            export = Some(Target::Originals(path.clone()));
                        }
                        if ui
                            .add_enabled(
                                valid && !board.scene.is_empty(),
                                egui::Button::new(moth_ui::tooltip(ui.ctx(), Text::ExportZip)),
                            )
                            .clicked()
                        {
                            export = Some(Target::Archive {
                                path,
                                ids: items.iter().map(|i| i.id).collect(),
                            });
                        }
                    });
                });
            });
        self.open = open;
        (commands, export, note, intents)
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
fn order_label(i: u8) -> Text {
    match i {
        1 => Text::ByFilename,
        2 => Text::ByTime,
        3 => Text::Random,
        _ => Text::ByLayer,
    }
}
fn scale(items: &[&moth_scene::Item], factor: f32) -> Vec<Command> {
    if !factor.is_finite() || factor <= 0. {
        return Vec::new();
    }
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let commands: Vec<_> = items
        .iter()
        .map(|item| Command::SetRect {
            id: item.id,
            rect: Rect {
                x: first.rect.x + (item.rect.x - first.rect.x) * factor,
                y: first.rect.y + (item.rect.y - first.rect.y) * factor,
                width: item.rect.width * factor,
                height: item.rect.height * factor,
            },
        })
        .collect();
    if commands
        .iter()
        .all(|c| matches!(c,Command::SetRect{rect,..} if rect.valid()))
    {
        commands
    } else {
        Vec::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_arrangement_reverses_the_same_order_without_moving_the_anchor() {
        let a = moth_scene::Item::text(
            "a",
            Rect {
                x: 10.,
                y: 20.,
                width: 10.,
                height: 10.,
            },
            None,
        );
        let b = moth_scene::Item::text(
            "b",
            Rect {
                x: 30.,
                y: 20.,
                width: 20.,
                height: 10.,
            },
            Some(&a.z),
        );
        for order in [0, 1, 2, 3] {
            let mut tools = Tools {
                order,
                ..Tools::default()
            };
            let mut items = vec![&a, &b];
            let first = tools.arrange(&mut items, arrange::Arrange::Horizontal, 100.);
            let reversed = tools.arrange(&mut items, arrange::Arrange::Horizontal, 100.);
            assert!(
                matches!((&first[0],&reversed[1]),(Command::SetRect{id:a,..},Command::SetRect{id:b,..}) if a==b)
            );
            assert!(
                matches!((&first[0],&reversed[0]),(Command::SetRect{rect:a,..},Command::SetRect{rect:b,..}) if a.x==b.x && a.y==b.y)
            );
        }
    }
    #[test]
    fn preferences_roundtrip_and_reject_nonfinite_values() {
        let source = Tools {
            dark: false,
            ui_scale: 1.5,
            gap: 17.,
            threshold: 9,
            auto_arrange: true,
            default_arrange: 3,
            undo_limit: 42,
            show_outlines: false,
            drag_animation: false,
            drag_opacity: 0.42,
            ..Tools::default()
        };
        let map = source
            .preferences()
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        let mut loaded = Tools::default();
        loaded.load(&map);
        assert_eq!(loaded.preferences(), source.preferences());
        loaded.load(
            &[
                ("ui_scale".into(), "NaN".into()),
                ("drag_opacity".into(), "NaN".into()),
                ("drag_animation".into(), "invalid".into()),
                ("show_outlines".into(), "invalid".into()),
                ("arrange_gap".into(), "inf".into()),
                ("undo_limit".into(), "99999".into()),
            ]
            .into_iter()
            .collect(),
        );
        assert!(loaded.show_outlines);
        assert!(loaded.drag_animation);
        assert_eq!(
            loaded.drag_opacity,
            super::super::motion::DEFAULT_DRAG_OPACITY
        );
        assert_eq!(loaded.ui_scale, 1.);
        assert_eq!(loaded.gap, 24.);
        assert_eq!(loaded.undo_limit, 256);
    }
    #[test]
    fn scaling_rejects_overflow_and_keeps_relative_positions() {
        let a = moth_scene::Item::text(
            "a",
            Rect {
                x: 10.,
                y: 20.,
                width: 10.,
                height: 10.,
            },
            None,
        );
        let mut b = a.clone();
        b.id = Uuid::now_v7();
        b.rect.x = 30.;
        let out = scale(&[&a, &b], 2.);
        assert!(matches!(out[1],Command::SetRect{rect,..} if rect.x==50.&&rect.width==20.));
        assert!(scale(&[&a], f32::INFINITY).is_empty());
        assert!(scale(&[&a], 1e9).is_empty());
    }
}
