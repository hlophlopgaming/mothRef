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
/// An export waiting for its destination from the system "Save as" dialog.
pub(super) enum PendingExport {
    /// The board, or the selection, rendered to PNG, SVG or PDF (chosen by extension).
    Scene {
        selection: bool,
        ids: Vec<Uuid>,
        region: Option<Rect>,
        side: u32,
    },
    /// One image's original file, with the item name offered as the file name.
    Original(Uuid, String),
    /// Originals of the selection (or the board when empty), each as its own file in a
    /// new folder named in the dialog.
    Originals(Vec<Uuid>),
    /// Originals of the selection (or the board when empty) in a ZIP archive.
    Archive(Vec<Uuid>),
}
impl PendingExport {
    /// The system dialog for this export: its title, file types and suggested name.
    pub fn dialog(&self, board: &str) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new();
        match self {
            Self::Scene { selection, .. } => dialog
                .set_title(text(if *selection {
                    Text::ExportSelection
                } else {
                    Text::ExportScene
                }))
                .add_filter("PNG", &["png"])
                .add_filter("SVG", &["svg"])
                .add_filter("PDF", &["pdf"])
                .set_file_name(format!("{board}.png")),
            Self::Original(_, name) => dialog
                .set_title(text(Text::ExportOriginal))
                .set_file_name(name.as_str()),
            Self::Originals(_) => dialog
                .set_title(text(Text::ExportOriginals))
                .set_file_name(format!("{board} {}", text(Text::OriginalsFolder))),
            Self::Archive(_) => dialog
                .set_title(text(Text::ExportZip))
                .add_filter("ZIP", &["zip"])
                .set_file_name(format!("{board}.zip")),
        }
    }
    /// The export to run at the chosen path. A file name typed without an extension
    /// gets the default one, since the scene format follows the extension.
    pub fn target(self, mut path: std::path::PathBuf) -> moth_platform::export::Target {
        use moth_platform::export::Target;
        match self {
            Self::Scene {
                ids, region, side, ..
            } => {
                if path.extension().is_none() {
                    path.set_extension("png");
                }
                Target::Scene {
                    path,
                    ids,
                    region,
                    side,
                }
            }
            Self::Original(id, _) => Target::Original { path, id },
            Self::Originals(ids) => Target::Originals {
                directory: path,
                ids,
            },
            Self::Archive(ids) => {
                if path.extension().is_none() {
                    path.set_extension("zip");
                }
                Target::Archive { path, ids }
            }
        }
    }
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
    /// The screen pixel under the held pointer outside the window.
    pub hover: Option<[u8; 3]>,
    /// The last color picked from the screen, shown as a sample.
    pub last_screen: Option<[u8; 3]>,
    /// The system lets the picker read the screen while dragging outside the window.
    pub screen_drag: bool,
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
            flash_notice(ui.ctx(), board, settings.reduced_motion);
            return Vec::new();
        }
        viewer_notice(ui.ctx(), &mut self.viewer, board, settings.reduced_motion);
        flash_notice(ui.ctx(), board, settings.reduced_motion);
        let mut intents = Vec::new();
        let items: Vec<_> = board
            .selection
            .iter()
            .filter_map(|id| board.scene.get(*id))
            .collect();
        // Side panels stack down the right edge, under the window bar.
        let mut top = None;
        let mut open = self.edit.open;
        side_panel(
            ui.ctx(),
            "edit_item",
            Text::EditItem,
            &mut open,
            &mut top,
            |ui| {
                if items.len() == 1 {
                    let item = items[0];
                    if self.edit.observed != Some((item.id, item.rect, item.transform)) {
                        self.edit.observed = Some((item.id, item.rect, item.transform));
                        self.edit.draft = Some((item.id, item.rect, item.transform));
                    }
                    let rotation_snap = settings.rotation_snap;
                    ui.scope(|ui| {
                        let Some((_, frame, t)) = &mut self.edit.draft else {
                            return;
                        };
                        let previous_width = frame.width;
                        let muted = moth_ui::tokens(ui.ctx()).text_muted;
                        ui.label(egui::RichText::new(&item.name).color(muted));
                        pairs(
                            ui,
                            &mut [
                                (Text::PositionX, &mut frame.x, -1e9..=1e9),
                                (Text::PositionY, &mut frame.y, -1e9..=1e9),
                                (Text::Width, &mut frame.width, 0.001..=1e9),
                                (Text::Height, &mut frame.height, 0.001..=1e9),
                            ],
                        );
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
                        ui.horizontal(|ui| {
                            ui.label(text(Text::Opacity));
                            ui.add(egui::Slider::new(&mut t.opacity, 0.0..=1.0));
                        });
                        ui.horizontal_wrapped(|ui| {
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
                        });
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
                                number(ui, Text::ContentScale, &mut t.content_scale, 0.001..=1000.);
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
                        footer(
                            ui,
                            enabled,
                            &mut intents,
                            Action::CancelItem,
                            Action::ApplyItem,
                        );
                    });
                } else {
                    self.edit.draft = None;
                    self.edit.observed = None;
                    ui.label(
                        egui::RichText::new(text(Text::SelectOneItem))
                            .color(moth_ui::tokens(ui.ctx()).text_muted),
                    );
                }
            },
        );
        self.edit.open = open;
        let mut open = self.arrange.open;
        side_panel(
            ui.ctx(),
            "arrange_options",
            Text::ArrangeOptions,
            &mut open,
            &mut top,
            |ui| {
                let before = settings.preferences();
                number(ui, Text::Gap, &mut settings.gap, 1.0..=10000.);
                ui.horizontal(|ui| {
                    ui.label(text(Text::ArrangeOrder));
                    egui::ComboBox::from_id_salt("arrange_order")
                        .selected_text(text(order_label(self.arrange.order)))
                        .show_ui(ui, |ui| {
                            for i in 0..4 {
                                ui.selectable_value(
                                    &mut self.arrange.order,
                                    i,
                                    text(order_label(i)),
                                );
                            }
                        });
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
                // Layouts two to a row; the last entry opens this very panel.
                let layouts: Vec<Action> = super::surfaces::ARRANGE
                    .iter()
                    .filter_map(|slot| match slot {
                        super::surfaces::Slot::Action(Action::ArrangeOptions) => None,
                        super::surfaces::Slot::Action(action) => Some(*action),
                        _ => None,
                    })
                    .collect();
                let column = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
                egui::Grid::new("arrange_layouts")
                    .num_columns(2)
                    .min_col_width(column)
                    .max_col_width(column)
                    .show(ui, |ui| {
                        for (index, action) in layouts.into_iter().enumerate() {
                            let on = enabled.get(action as usize).copied().unwrap_or(false);
                            let label = text(action.label());
                            let clicked = ui
                                .add_enabled_ui(on, |ui| match action.icon() {
                                    Some(icon) => {
                                        moth_ui::icons::labeled_button(ui, icon, label, false)
                                    }
                                    None => ui.button(label),
                                })
                                .inner
                                .clicked();
                            if clicked {
                                intents.push(Intent::Action(action));
                            }
                            if index % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
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
            },
        );
        self.arrange.open = open;
        if self.export.open {
            self.export_dialog(ui.ctx(), board, enabled, &mut intents);
        }
        let mut open = self.pickers.open;
        side_panel(
            ui.ctx(),
            "image_pickers",
            Text::ImagePickers,
            &mut open,
            &mut top,
            |ui| {
                let tokens = moth_ui::tokens(ui.ctx());
                for action in [
                    Action::SampleColor,
                    Action::PickScreen,
                    Action::CoordinatePicker,
                ] {
                    // The system picker only where the system has one.
                    if action == Action::PickScreen
                        && !enabled.get(action as usize).copied().unwrap_or(false)
                    {
                        continue;
                    }
                    let active = match action {
                        Action::SampleColor => *tool == super::gesture::Tool::PickColor,
                        Action::PickScreen => false,
                        _ => *tool == super::gesture::Tool::PickInfo,
                    };
                    let response = ui.add_enabled_ui(
                        enabled.get(action as usize).copied().unwrap_or(false),
                        |ui| {
                            moth_ui::icons::labeled_button(
                                ui,
                                action.icon().unwrap_or(moth_ui::icons::Icon::Pointer),
                                text(action.label()),
                                active,
                            )
                        },
                    );
                    if response.inner.clicked() {
                        intents.push(Intent::Action(action));
                    }
                }
                ui.add_space(tokens.space_s as f32);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(text(Text::ColorFormat)).color(tokens.text_muted));
                    for (i, label) in ["HEX", "RGB", "HSV"].into_iter().enumerate() {
                        ui.selectable_value(&mut self.pickers.color_format, i as u8, label);
                    }
                });
                // The color under the held pointer outside the window, else the last one
                // taken from the screen.
                let screen = self
                    .pickers
                    .hover
                    .map(|rgb| (Text::ScreenColorHover, rgb))
                    .or(self
                        .pickers
                        .last_screen
                        .map(|rgb| (Text::ScreenColorLast, rgb)));
                if let Some((label, rgb)) = screen {
                    ui.add_space(tokens.space_s as f32);
                    ui.horizontal(|ui| {
                        // The ring keeps white and canvas-colored samples visible.
                        moth_ui::icons::swatch(ui, rgb, true);
                        let (hex, _, _) = moth_platform::screen_color::texts(rgb);
                        ui.label(egui::RichText::new(text(label)).color(tokens.text_muted));
                        ui.label(egui::RichText::new(hex).monospace());
                    });
                }
                if let Some(info) = &board.notices.pixel_info {
                    ui.add_space(tokens.space_s as f32);
                    pixel_info(ui, info, board);
                }
                if tool.picks() {
                    ui.add_space(tokens.space_xs as f32);
                    ui.label(
                        egui::RichText::new(text(if *tool == super::gesture::Tool::PickInfo {
                            Text::PickCoordinatesHint
                        } else if self.pickers.screen_drag {
                            Text::PickColorDragHint
                        } else {
                            Text::PickColorHint
                        }))
                        .color(tokens.text_muted),
                    );
                }
                export_feedback(ui, board);
            },
        );
        self.pickers.open = open;
        if !open {
            *tool = super::gesture::Tool::Select;
        }
        intents
    }
}
/// A tool panel at the right edge, under the window bar: title, close button and
/// content in the dialog style. `top` stacks several open panels instead of overlapping.
fn side_panel(
    ctx: &egui::Context,
    id: &str,
    title: Text,
    open: &mut bool,
    top: &mut Option<f32>,
    add: impl FnOnce(&mut egui::Ui),
) {
    if !*open {
        return;
    }
    let tokens = moth_ui::tokens(ctx);
    let gap = tokens.space_m as f32;
    let viewport = ctx.content_rect();
    let start = top.unwrap_or_else(|| {
        ctx.data(|d| d.get_temp::<f32>(egui::Id::new("window_bar_bottom")))
            .unwrap_or(viewport.top())
            + gap
    });
    let width = (tokens.space_xl as f32 * 13.)
        .min(viewport.width() - gap * 2.)
        .max(1.);
    let shown = egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Middle)
        .constrain_to(viewport)
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(egui::pos2(viewport.right() - gap, start))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .corner_radius(tokens.radius_l)
                .inner_margin(egui::Margin::same(tokens.space_m as i8))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        moth_ui::icons::dialog_title(ui, text(title));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if moth_ui::icons::button(
                                ui,
                                moth_ui::icons::Icon::Close,
                                text(Text::ClosePanel),
                                false,
                            )
                            .clicked()
                            {
                                *open = false;
                            }
                        });
                    });
                    ui.add_space(tokens.space_xs as f32);
                    // Shows all the content when it fits below `start`, else scrolls; the
                    // minimum keeps egui from shrinking the scroll area to its first guess.
                    let room = (viewport.bottom() - start - gap * 6.).max(gap * 4.);
                    egui::ScrollArea::vertical()
                        .id_salt(id)
                        .max_height(room)
                        .min_scrolled_height(room)
                        .show(ui, add);
                });
        });
    *top = Some(shown.response.rect.bottom() + gap);
}
/// The Cancel / Apply pair at the end of a panel; Apply is the filled one.
fn footer(
    ui: &mut egui::Ui,
    enabled: &[bool],
    intents: &mut Vec<Intent>,
    cancel: Action,
    apply: Action,
) {
    let tokens = moth_ui::tokens(ui.ctx());
    let on = |action: Action| enabled.get(action as usize).copied().unwrap_or(false);
    ui.add_space(tokens.space_s as f32);
    // A horizontal row first, so the right-to-left layout takes one line, not the
    // whole remaining height of the panel.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if moth_ui::icons::primary_button(ui, text(Text::Apply), on(apply)).clicked() {
                intents.push(Intent::Action(apply));
            }
            if ui
                .add_enabled(on(cancel), egui::Button::new(text(Text::Cancel)))
                .clicked()
            {
                intents.push(Intent::Action(cancel));
            }
        })
    });
}
/// Label–value pairs two to a row (X and Y, width and height), labels muted.
fn pairs(ui: &mut egui::Ui, fields: &mut [(Text, &mut f32, std::ops::RangeInclusive<f32>)]) {
    let tokens = moth_ui::tokens(ui.ctx());
    egui::Grid::new(ui.next_auto_id())
        .num_columns(4)
        .spacing([tokens.space_s as f32, tokens.space_xs as f32])
        .show(ui, |ui| {
            for (index, (key, value, range)) in fields.iter_mut().enumerate() {
                ui.label(egui::RichText::new(text(*key)).color(tokens.text_muted));
                ui.add(egui::DragValue::new(&mut **value).range(range.clone()));
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}
/// The coordinate picker's answer as labelled rows, plus the current selection's size.
fn pixel_info(ui: &mut egui::Ui, info: &moth_platform::export::PixelInfo, board: &Document) {
    let tokens = moth_ui::tokens(ui.ctx());
    let selection = board
        .selection
        .iter()
        .filter_map(|id| board.scene.get(*id))
        .map(|item| {
            let corners = item.rect.rotated_envelope(item.transform.rotation);
            egui::Rect::from_min_max(
                egui::pos2(corners.lower()[0], corners.lower()[1]),
                egui::pos2(corners.upper()[0], corners.upper()[1]),
            )
        })
        .reduce(|a, b| a.union(b));
    let rows = [
        (Text::PixelInfoName, info.name.clone()),
        (
            Text::PixelInfoPixel,
            format!("X {} · Y {}", info.pixel[0], info.pixel[1]),
        ),
        (
            Text::PixelInfoOriginal,
            format!(
                "{} × {} {}",
                info.size[0],
                info.size[1],
                text(Text::PixelsShort)
            ),
        ),
        (
            Text::PixelInfoFile,
            format!("{} · {}", info.format, file_size(info.bytes)),
        ),
        (
            Text::PixelInfoFrame,
            format!("{:.0} × {:.0}", info.frame[0], info.frame[1]),
        ),
    ];
    egui::Grid::new("pixel_info")
        .num_columns(2)
        .spacing([tokens.space_m as f32, tokens.space_xs as f32])
        .show(ui, |ui| {
            for (label, value) in rows {
                ui.label(egui::RichText::new(text(label)).color(tokens.text_muted));
                ui.add(egui::Label::new(value).truncate());
                ui.end_row();
            }
            if let Some(bounds) = selection {
                ui.label(
                    egui::RichText::new(text(Text::PixelInfoSelection)).color(tokens.text_muted),
                );
                ui.label(format!("{:.0} × {:.0}", bounds.width(), bounds.height()));
                ui.end_row();
            }
        });
}
/// A file size in the unit people read: bytes, KB or MB (decimal, as file managers show).
fn file_size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.0} KB", bytes as f64 / 1e3),
        _ => format!("{:.1} MB", bytes as f64 / 1e6),
    }
}
/// What each export gives, shown under its label in the Export dialog.
fn export_hint(action: Action) -> Text {
    match action {
        Action::ExportSelection => Text::ExportSelectionHint,
        Action::ExportOriginal => Text::ExportOriginalHint,
        Action::ExportOriginals => Text::ExportOriginalsHint,
        Action::ExportZip => Text::ExportZipHint,
        _ => Text::ExportSceneHint,
    }
}
impl Tools {
    /// Longest side and the optional crop region of a board picture.
    fn picture_options(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(text(Text::ExportSide));
            ui.add(
                egui::DragValue::new(&mut self.export.side)
                    .range(1..=4096)
                    .suffix(format!(" {}", text(Text::PixelsShort))),
            );
        });
        ui.checkbox(&mut self.export.crop, text(Text::CropRegion));
        if self.export.crop {
            let region = &mut self.export.region;
            pairs(
                ui,
                &mut [
                    (Text::PositionX, &mut region.x, -1e9..=1e9),
                    (Text::PositionY, &mut region.y, -1e9..=1e9),
                    (Text::Width, &mut region.width, 0.001..=1e9),
                    (Text::Height, &mut region.height, 0.001..=1e9),
                ],
            );
        }
    }
    /// Export: a dialog card that says what each export gives and asks where to save
    /// it only after the choice (the system "Save as" dialog), like saving a board.
    fn export_dialog(
        &mut self,
        ctx: &egui::Context,
        board: &Document,
        enabled: &[bool],
        intents: &mut Vec<Intent>,
    ) {
        let tokens = moth_ui::tokens(ctx);
        let on = |action: Action| enabled.get(action as usize).copied().unwrap_or(false);
        let modal = egui::Modal::new(egui::Id::new("export_dialog"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .corner_radius(tokens.radius_l)
                    .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
            )
            .show(ctx, |ui| {
                let width = (tokens.space_xl as f32 * 17.)
                    .min(ctx.content_rect().width() - tokens.space_xl as f32 * 4.)
                    .max(1.);
                ui.set_width(width);
                moth_ui::icons::dialog_title(ui, text(Text::Export));
                ui.label(egui::RichText::new(text(Text::ExportLimit)).color(tokens.text_muted));
                ui.add_space(tokens.space_l as f32);
                egui::ScrollArea::vertical()
                    // Everything but the title, intro and footer; scrolls only on small windows.
                    .max_height(
                        (ctx.content_rect().height() - tokens.space_xl as f32 * 12.)
                            .max(tokens.space_xl as f32 * 4.),
                    )
                    .show(ui, |ui| {
                        for (index, slot) in super::surfaces::EXPORT.iter().enumerate() {
                            let super::surfaces::Slot::Group(title, actions) = slot else {
                                continue;
                            };
                            if index > 0 {
                                ui.add_space(tokens.space_m as f32);
                            }
                            moth_ui::icons::dialog_section(ui, text(*title));
                            for slot in actions.iter() {
                                let super::surfaces::Slot::Action(action) = slot else {
                                    continue;
                                };
                                if moth_ui::icons::option_row(
                                    ui,
                                    action.icon().unwrap_or(moth_ui::icons::Icon::Image),
                                    text(action.label()),
                                    text(export_hint(*action)),
                                    on(*action),
                                )
                                .clicked()
                                {
                                    intents.push(Intent::Action(*action));
                                }
                            }
                            // Size and crop belong to the picture of the board.
                            if index == 0 {
                                self.picture_options(ui);
                            }
                        }
                    });
                ui.add_space(tokens.space_m as f32);
                ui.horizontal(|ui| {
                    export_feedback(ui, board);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if moth_ui::icons::dialog_button(ui, text(Text::DoneButton)).clicked() {
                            self.export.open = false;
                        }
                    });
                });
            });
        // Esc or a click beside the card closes it; a running export keeps going.
        if modal.should_close() {
            self.export.open = false;
        }
    }
}
/// Shows "Opening original…" until the viewer answers, then the result for a moment.
fn viewer_notice(
    ctx: &egui::Context,
    slot: &mut Option<ViewerNotice>,
    board: &Document,
    reduced_motion: bool,
) {
    const SHOWN: f32 = NOTICE_SHOWN;
    const FADE: f32 = NOTICE_FADE;
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
    let content = match answer {
        None => NoticeContent::Busy(Text::OpeningOriginal),
        Some(message) => NoticeContent::Done {
            ok: matches!(message, moth_ui::Message::Text(Text::ViewerOpened)),
            message,
        },
    };
    bottom_notice(
        ctx,
        "viewer_feedback",
        &["bottom_toolbar", "selection_badge", "added_feedback"],
        alpha,
        reduced_motion,
        content,
    );
}
/// How long a finished notice stays, then how long it fades.
const NOTICE_SHOWN: f32 = 2.4;
const NOTICE_FADE: f32 = 0.35;
/// What a bottom notice says: work under way, or how it ended.
enum NoticeContent<'a> {
    Busy(Text),
    Done {
        message: &'a moth_ui::Message,
        ok: bool,
    },
}
/// A short-lived message about something that did not happen (for now: a paste with
/// nothing usable on the clipboard), so it is seen instead of hiding in the status line.
fn flash_notice(ctx: &egui::Context, board: &Document, reduced_motion: bool) {
    let Some((message, ok, at)) = &board.notices.flash else {
        return;
    };
    let age = at.elapsed().as_secs_f32();
    if age >= NOTICE_SHOWN + NOTICE_FADE {
        return;
    }
    let alpha = if age < NOTICE_SHOWN {
        ctx.request_repaint_after(std::time::Duration::from_secs_f32(NOTICE_SHOWN - age));
        1.
    } else {
        ctx.request_repaint();
        ((NOTICE_SHOWN + NOTICE_FADE - age) / NOTICE_FADE).clamp(0., 1.)
    };
    bottom_notice(
        ctx,
        "flash_feedback",
        &[
            "bottom_toolbar",
            "selection_badge",
            "added_feedback",
            "viewer_feedback",
        ],
        alpha,
        reduced_motion,
        NoticeContent::Done { message, ok: *ok },
    );
}
/// The pill above the bottom toolbar shared by the short notices.
fn bottom_notice(
    ctx: &egui::Context,
    id: &str,
    below: &[&str],
    alpha: f32,
    reduced_motion: bool,
    content: NoticeContent<'_>,
) {
    let tokens = moth_ui::tokens(ctx);
    let gap = tokens.space_m as f32;
    let viewport = ctx.content_rect();
    let bottom = moth_ui::canvas::notice_bottom(ctx, viewport, below);
    egui::Area::new(egui::Id::new(id))
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
                    ui.horizontal(|ui| match content {
                        NoticeContent::Busy(label) => {
                            ui.spinner();
                            ui.label(text(label));
                        }
                        NoticeContent::Done { message, ok } => {
                            let (icon, _) = ui.allocate_exact_size(
                                egui::Vec2::splat(ui.text_style_height(&egui::TextStyle::Body)),
                                egui::Sense::hover(),
                            );
                            // Success gets a check; an error a cross in the danger tone.
                            let (glyph, color) = if ok {
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_destinations_get_a_default_extension_only_when_typed_without_one() {
        use moth_platform::export::Target;
        let scene = || PendingExport::Scene {
            selection: false,
            ids: Vec::new(),
            region: None,
            side: 2048,
        };
        let Target::Scene { path, .. } = scene().target("/tmp/board".into()) else {
            panic!("scene export");
        };
        assert_eq!(path, std::path::Path::new("/tmp/board.png"));
        let Target::Scene { path, .. } = scene().target("/tmp/board.pdf".into()) else {
            panic!("scene export");
        };
        assert_eq!(path, std::path::Path::new("/tmp/board.pdf"));
        let Target::Archive { path, .. } =
            PendingExport::Archive(Vec::new()).target("/tmp/refs".into())
        else {
            panic!("archive export");
        };
        assert_eq!(path, std::path::Path::new("/tmp/refs.zip"));
        // A folder name is used as typed: the worker creates it fresh.
        let id = Uuid::from_u128(7);
        let Target::Originals { directory, ids } =
            PendingExport::Originals(vec![id]).target("/tmp/refs v2".into())
        else {
            panic!("originals export");
        };
        assert_eq!(directory, std::path::Path::new("/tmp/refs v2"));
        assert_eq!(ids, [id]);
    }
}
