// SPDX-License-Identifier: GPL-3.0-or-later
//! Small vector icons stay crisp without a second icon font or image cache.
use egui::{Color32, Pos2, Rect, Response, Stroke, Ui, Vec2};
/// House icons use one stroke weight across touch and desktop controls.
#[derive(Clone, Copy)]
pub enum Icon {
    /// Rotate an item clockwise.
    RotateCw,
    /// Rotate an item counterclockwise.
    RotateCcw,
    /// Mirror an item across its vertical axis.
    FlipHorizontal,
    /// Mirror an item across its horizontal axis.
    FlipVertical,
    /// Edit a crop frame.
    Crop,
    /// Duplicate a selection.
    Duplicate,
    /// Delete selected items.
    Trash,
    /// Settings control.
    Settings,
    /// Palette control.
    Palette,
    /// ImagePlus control.
    ImagePlus,
    /// Gauge control.
    Gauge,
    /// Keyboard control.
    Keyboard,
    /// Plug control.
    Plug,
    /// Code control.
    Code,
    /// ChevronRight control.
    ChevronRight,
    /// An expanded tree row.
    ChevronDown,
    /// Start or resume a timer.
    Play,
    /// Pause a timer.
    Pause,
    /// Previous image of a sequence.
    Previous,
    /// Next image of a sequence.
    Next,
    /// Sound cue on or off.
    Sound,
    /// Grow to full screen.
    Expand,
    /// Shrink to a small window.
    Shrink,
    /// Mark an item for later.
    Star,
    /// Stacked layers: the board's tree.
    Layers,
    /// Done, confirmed.
    Check,

    /// Return to the board catalog.
    Home,
    /// Select canvas items.
    Select,
    /// Open navigation and view controls.
    Compass,
    /// Move the camera.
    Pan,
    /// Clipboard import.
    Paste,
    /// Browse a board path.
    Folder,
    /// Durable checkpoint.
    Save,
    /// Undo the last command.
    Undo,
    /// Redo the last command.
    Redo,
    /// Zoom to content.
    Fit,
    /// Keep a window visible.
    Pin,
    /// Transparent reference view.
    Overlay,
    /// Add a board.
    Plus,
    /// Close a tab or window.
    Close,
    /// Grid library view.
    Grid,
    /// Unequal rectangles in a row.
    ArrangeHorizontal,
    /// Unequal rectangles in a column.
    ArrangeVertical,
    /// Rectangles filling a square with gutters.
    ArrangeGrid,
    /// Rectangles filling a square without gutters.
    ArrangeOptimal,
    /// List library view.
    List,
    /// Find boards.
    Search,
    /// Recently used boards.
    Clock,
    /// Board preview placeholder.
    Image,
    /// Minimize a window.
    Minus,
    /// Maximize a window.
    Maximize,
    /// Restore a maximized window.
    Restore,
    /// Drag handle that moves the window.
    Grip,
    /// Click-through control.
    Pointer,
}
/// Paints into a reserved rectangle so icons cannot change text layout.
pub fn paint(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    // Drawn on an 18-unit grid; a smaller slot (menus) shrinks the icon to fit, a
    // larger one (buttons) keeps it at its natural size in the middle.
    let scale = (rect.width().min(rect.height()) / 18.).min(1.);
    let r = Rect::from_center_size(rect.center(), Vec2::splat(18. * scale));
    let p = |x: f32, y: f32| Pos2::new(r.left() + x * scale, r.top() + y * scale);
    let stroke = Stroke::new(1.5 * scale.max(0.85), color);
    let line = |points: &[(f32, f32)]| {
        ui.painter().add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    match icon {
        Icon::RotateCw | Icon::RotateCcw => {
            let points = [
                (4., 4.),
                (9., 2.),
                (14., 5.),
                (15., 11.),
                (11., 15.),
                (5., 14.),
                (2., 10.),
            ];
            if matches!(icon, Icon::RotateCw) {
                line(&points);
                line(&[(1., 13.), (2., 10.), (5., 11.)]);
            } else {
                line(&points.map(|(x, y)| (18. - x, y)));
                line(&[(17., 13.), (16., 10.), (13., 11.)]);
            }
        }
        Icon::FlipHorizontal => {
            line(&[(9., 1.), (9., 17.)]);
            line(&[(6., 4.), (1., 14.), (6., 14.), (6., 4.)]);
            line(&[(12., 4.), (17., 14.), (12., 14.), (12., 4.)]);
        }
        Icon::FlipVertical => {
            line(&[(1., 9.), (17., 9.)]);
            line(&[(4., 6.), (14., 1.), (14., 6.), (4., 6.)]);
            line(&[(4., 12.), (14., 17.), (14., 12.), (4., 12.)]);
        }
        Icon::Crop => {
            line(&[(5., 1.), (5., 13.), (17., 13.)]);
            line(&[(1., 5.), (13., 5.), (13., 17.)]);
        }
        Icon::Duplicate => {
            line(&[(4., 12.), (2., 12.), (2., 2.), (12., 2.), (12., 4.)]);
            line(&[(6., 6.), (16., 6.), (16., 16.), (6., 16.), (6., 6.)]);
        }
        Icon::Trash => {
            line(&[(2., 5.), (16., 5.)]);
            line(&[(6., 5.), (6., 2.), (12., 2.), (12., 5.)]);
            line(&[(4., 5.), (5., 16.), (13., 16.), (14., 5.)]);
            line(&[(7., 8.), (7., 13.)]);
            line(&[(11., 8.), (11., 13.)]);
        }
        Icon::Settings => {
            ui.painter().circle_stroke(p(9., 9.), 5. * scale, stroke);
            ui.painter().circle_stroke(p(9., 9.), 2. * scale, stroke);
            for (a, b) in [
                ((9., 0.), (9., 4.)),
                ((9., 14.), (9., 18.)),
                ((0., 9.), (4., 9.)),
                ((14., 9.), (18., 9.)),
                ((2., 2.), (5., 5.)),
                ((13., 13.), (16., 16.)),
                ((2., 16.), (5., 13.)),
                ((13., 5.), (16., 2.)),
            ] {
                line(&[a, b]);
            }
        }
        Icon::Palette => {
            line(&[
                (16., 12.),
                (11., 16.),
                (4., 15.),
                (1., 10.),
                (3., 4.),
                (9., 1.),
                (15., 4.),
                (16., 8.),
                (11., 8.),
                (11., 11.),
                (16., 12.),
            ]);
            for (x, y) in [(5., 6.), (9., 4.), (5., 10.)] {
                ui.painter().circle_filled(p(x, y), 1. * scale, color);
            }
        }
        Icon::ImagePlus => {
            line(&[
                (2., 3.),
                (12., 3.),
                (12., 8.),
                (8., 8.),
                (2., 14.),
                (2., 3.),
            ]);
            line(&[(13., 10.), (13., 17.)]);
            line(&[(10., 13.), (17., 13.)]);
        }
        Icon::Gauge => {
            line(&[
                (2., 14.),
                (1., 9.),
                (3., 4.),
                (9., 1.),
                (15., 4.),
                (17., 9.),
                (16., 14.),
            ]);
            line(&[(9., 11.), (13., 5.)]);
            line(&[(5., 15.), (13., 15.)]);
        }
        Icon::Keyboard => {
            line(&[(1., 4.), (17., 4.), (17., 15.), (1., 15.), (1., 4.)]);
            for x in [4., 7., 10., 13.] {
                line(&[(x, 7.), (x, 9.)]);
            }
            line(&[(5., 12.), (13., 12.)]);
        }
        Icon::Plug => {
            line(&[(5., 1.), (5., 6.), (13., 6.), (13., 1.)]);
            line(&[
                (3., 6.),
                (15., 6.),
                (14., 11.),
                (9., 14.),
                (4., 11.),
                (3., 6.),
            ]);
            line(&[(9., 14.), (9., 18.)]);
        }
        Icon::Code => {
            line(&[(6., 4.), (1., 9.), (6., 14.)]);
            line(&[(12., 4.), (17., 9.), (12., 14.)]);
            line(&[(10., 2.), (8., 16.)]);
        }
        Icon::ChevronRight => line(&[(6., 3.), (12., 9.), (6., 15.)]),
        Icon::ChevronDown => line(&[(3., 6.), (9., 12.), (15., 6.)]),
        Icon::Play => line(&[(5., 3.), (15., 9.), (5., 15.), (5., 3.)]),
        Icon::Pause => {
            line(&[(6., 3.), (6., 15.)]);
            line(&[(12., 3.), (12., 15.)]);
        }
        Icon::Previous => {
            line(&[(4., 3.), (4., 15.)]);
            line(&[(15., 3.), (6., 9.), (15., 15.), (15., 3.)]);
        }
        Icon::Next => {
            line(&[(14., 3.), (14., 15.)]);
            line(&[(3., 3.), (12., 9.), (3., 15.), (3., 3.)]);
        }
        Icon::Sound => {
            line(&[
                (2., 7.),
                (5., 7.),
                (9., 3.),
                (9., 15.),
                (5., 11.),
                (2., 11.),
                (2., 7.),
            ]);
            line(&[(12., 6.), (13.5, 9.), (12., 12.)]);
            line(&[(14.5, 4.), (17., 9.), (14.5, 14.)]);
        }
        Icon::Expand => {
            line(&[(2., 6.), (2., 2.), (6., 2.)]);
            line(&[(12., 2.), (16., 2.), (16., 6.)]);
            line(&[(2., 12.), (2., 16.), (6., 16.)]);
            line(&[(12., 16.), (16., 16.), (16., 12.)]);
        }
        Icon::Shrink => {
            line(&[(6., 2.), (6., 6.), (2., 6.)]);
            line(&[(12., 2.), (12., 6.), (16., 6.)]);
            line(&[(6., 16.), (6., 12.), (2., 12.)]);
            line(&[(12., 16.), (12., 12.), (16., 12.)]);
        }
        Icon::Check => line(&[(3., 9.5), (7.5, 14.), (15., 4.5)]),
        Icon::Layers => {
            line(&[(2., 6.), (9., 2.5), (16., 6.), (9., 9.5), (2., 6.)]);
            line(&[(2., 9.5), (9., 13.), (16., 9.5)]);
            line(&[(2., 13.), (9., 16.5), (16., 13.)]);
        }
        Icon::Star => line(&[
            (9., 1.5),
            (11.2, 6.5),
            (16.5, 7.),
            (12.5, 10.5),
            (13.7, 16.),
            (9., 13.2),
            (4.3, 16.),
            (5.5, 10.5),
            (1.5, 7.),
            (6.8, 6.5),
            (9., 1.5),
        ]),
        Icon::Home => {
            line(&[
                (2., 8.),
                (9., 2.),
                (16., 8.),
                (16., 16.),
                (2., 16.),
                (2., 8.),
            ]);
            line(&[(7., 16.), (7., 10.), (11., 10.), (11., 16.)]);
        }
        Icon::Select | Icon::Pointer => {
            line(&[(3., 2.), (15., 8.), (10., 10.), (8., 16.), (3., 2.)])
        }
        Icon::Compass => {
            ui.painter().circle_stroke(p(9., 9.), 8. * scale, stroke);
            line(&[(12., 5.), (10., 11.), (6., 13.), (8., 7.), (12., 5.)]);
        }
        Icon::Pan => {
            line(&[
                (3., 10.),
                (3., 8.),
                (6., 10.),
                (6., 3.),
                (8., 3.),
                (8., 9.),
                (8., 2.),
                (10., 2.),
                (10., 9.),
                (10., 3.),
                (12., 3.),
                (12., 10.),
                (12., 5.),
                (14., 5.),
                (14., 12.),
                (11., 16.),
                (7., 16.),
                (3., 10.),
            ]);
        }
        Icon::Paste => {
            ui.painter().rect_stroke(
                Rect::from_min_max(p(4., 4.), p(15., 16.)),
                crate::tokens(ui.ctx()).radius_placeholder as f32,
                stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_filled(
                Rect::from_min_max(p(7., 2.), p(12., 5.)),
                crate::tokens(ui.ctx()).radius_icon as f32,
                color,
            );
        }
        Icon::Folder => line(&[
            (2., 5.),
            (7., 5.),
            (9., 7.),
            (16., 7.),
            (16., 15.),
            (2., 15.),
            (2., 5.),
        ]),
        Icon::Save => {
            line(&[
                (3., 2.),
                (13., 2.),
                (16., 5.),
                (16., 16.),
                (3., 16.),
                (3., 2.),
            ]);
            line(&[(6., 2.), (6., 7.), (12., 7.), (12., 2.)]);
            line(&[(6., 16.), (6., 11.), (13., 11.), (13., 16.)]);
        }
        Icon::Undo => {
            line(&[(6., 3.), (2., 7.), (6., 11.)]);
            line(&[(2., 7.), (10., 7.), (14., 9.), (15., 14.)]);
        }
        Icon::Redo => {
            line(&[(12., 3.), (16., 7.), (12., 11.)]);
            line(&[(16., 7.), (8., 7.), (4., 9.), (3., 14.)]);
        }
        Icon::Fit => {
            for (x, y, sx, sy) in [
                (2., 2., 1., 1.),
                (16., 2., -1., 1.),
                (2., 16., 1., -1.),
                (16., 16., -1., -1.),
            ] {
                line(&[(x + 4. * sx, y), (x, y), (x, y + 4. * sy)]);
            }
        }
        Icon::Pin => {
            line(&[
                (6., 2.),
                (13., 2.),
                (12., 7.),
                (15., 10.),
                (4., 10.),
                (7., 7.),
                (6., 2.),
            ]);
            line(&[(9., 10.), (9., 17.)]);
        }
        Icon::Overlay => {
            line(&[(9., 2.), (17., 7.), (9., 12.), (1., 7.), (9., 2.)]);
            line(&[(2., 11.), (9., 15.), (16., 11.)]);
        }
        Icon::Plus => {
            line(&[(9., 3.), (9., 15.)]);
            line(&[(3., 9.), (15., 9.)]);
        }
        Icon::Close => {
            line(&[(4., 4.), (14., 14.)]);
            line(&[(14., 4.), (4., 14.)]);
        }
        Icon::Grid => {
            for x in [2., 10.] {
                for y in [2., 10.] {
                    ui.painter().rect_stroke(
                        Rect::from_min_size(p(x, y), Vec2::splat(5. * scale)),
                        crate::tokens(ui.ctx()).radius_none as f32,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        Icon::ArrangeHorizontal
        | Icon::ArrangeVertical
        | Icon::ArrangeGrid
        | Icon::ArrangeOptimal => {
            let boxes: &[[f32; 4]] = match icon {
                Icon::ArrangeHorizontal => {
                    &[[1., 5., 4., 9.], [7., 3., 4., 11.], [13., 7., 4., 7.]]
                }
                Icon::ArrangeVertical => &[[3., 1., 11., 4.], [5., 7., 9., 4.], [1., 13., 13., 4.]],
                Icon::ArrangeGrid => &[[1., 1., 7., 16.], [10., 1., 7., 7.], [10., 10., 7., 7.]],
                _ => &[[2., 2., 8., 14.], [10., 2., 6., 8.], [10., 10., 6., 6.]],
            };
            for &[x, y, w, h] in boxes {
                ui.painter().rect_stroke(
                    Rect::from_min_size(p(x, y), Vec2::new(w, h) * scale),
                    crate::tokens(ui.ctx()).radius_none as f32,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Icon::List => {
            for y in [4., 9., 14.] {
                line(&[(3., y), (15., y)]);
            }
        }
        Icon::Search => {
            ui.painter().circle_stroke(p(7., 7.), 5. * scale, stroke);
            line(&[(11., 11.), (16., 16.)]);
        }
        Icon::Clock => {
            ui.painter().circle_stroke(p(9., 9.), 7. * scale, stroke);
            line(&[(9., 4.), (9., 9.), (13., 11.)]);
        }
        Icon::Image => {
            ui.painter().rect_stroke(
                r,
                crate::tokens(ui.ctx()).radius_placeholder as f32,
                stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter().circle_stroke(p(5., 5.), 2. * scale, stroke);
            line(&[(1., 16.), (10., 8.), (17., 14.)]);
        }
        Icon::Minus => line(&[(4., 9.), (14., 9.)]),
        Icon::Maximize => {
            ui.painter().rect_stroke(
                Rect::from_min_max(p(3., 3.), p(15., 15.)),
                crate::tokens(ui.ctx()).radius_icon as f32,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Restore => {
            let radius = crate::tokens(ui.ctx()).radius_icon as f32;
            ui.painter().rect_stroke(
                Rect::from_min_max(p(3., 6.), p(12., 15.)),
                radius,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(&[(6., 6.), (6., 3.), (15., 3.), (15., 12.), (12., 12.)]);
        }
        Icon::Grip => {
            for x in [7., 11.] {
                for y in [5., 9., 13.] {
                    ui.painter().circle_filled(p(x, y), 1.2 * scale, color);
                }
            }
        }
    }
}
/// Icon buttons retain native egui focus, tooltips and accessibility labels.
pub fn button(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    enabled_button(ui, icon, label, selected, true)
}
/// Uses native disabled-button layout so wrapped surfaces measure the button itself.
pub fn enabled_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    // Ghost buttons: no idle fill, so bars of icons read as one calm surface; hover
    // and the selected state paint a soft background instead of an outline.
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").frame(false).min_size(Vec2::splat(
            if ui.ctx().content_rect().width() < 600. {
                44.
            } else {
                36.
            },
        )),
    );
    let tokens = crate::tokens(ui.ctx());
    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered() && enabled,
        ui.style().animation_time,
    );
    let pressed = response.is_pointer_button_down_on();
    let radius = tokens.radius_m as f32;
    let area = response.rect.shrink(2.);
    if selected {
        ui.painter()
            .rect_filled(area, radius, tokens.accent.gamma_multiply(0.22));
    }
    if hover > 0. {
        let fill = if pressed {
            tokens.selection
        } else {
            tokens.hover
        };
        ui.painter().rect_filled(
            area,
            radius,
            fill.gamma_multiply(hover * if selected { 0.5 } else { 1. }),
        );
    }
    let color = if !enabled {
        tokens.text_muted.gamma_multiply(0.6)
    } else if selected {
        tokens.accent_text
    } else if hover > 0. {
        tokens.text
    } else {
        tokens.text.gamma_multiply(0.85)
    };
    paint(ui, response.rect, icon, color);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
}

/// A square choice: icon above its label, all tiles of a row the same size.
pub fn tile(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let tokens = crate::tokens(ui.ctx());
    let size = Vec2::new(tokens.space_xl as f32 * 3.6, tokens.space_xl as f32 * 3.);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered(),
        ui.style().animation_time,
    );
    ui.painter().rect(
        rect,
        tokens.radius_m,
        tokens.surface_raised,
        Stroke::new(1., tokens.border.lerp_to_gamma(tokens.accent, hover)),
        egui::StrokeKind::Inside,
    );
    let icon_rect = Rect::from_center_size(rect.center() - Vec2::new(0., 10.), Vec2::splat(22.));
    paint(
        ui,
        icon_rect,
        icon,
        if hover > 0.5 {
            tokens.accent_text
        } else {
            tokens.text
        },
    );
    ui.painter().text(
        rect.center() + Vec2::new(0., 16.),
        egui::Align2::CENTER_CENTER,
        label,
        egui::TextStyle::Button.resolve(ui.style()),
        tokens.text,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response
}
/// Dialog titles: one step above body text, not the page heading.
pub fn dialog_title(ui: &mut Ui, title: &str) {
    ui.label(egui::RichText::new(title).size(17.).strong());
}
/// A reserved vector slot keeps labels, focus and touch targets in the native button layout.
pub fn labeled_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let id = ui.next_auto_id().with("icon");
    let result = egui::Button::new((egui::Atom::custom(id, Vec2::splat(18.)), label))
        .selected(selected)
        .atom_ui(ui);
    if let Some(rect) = result.rect(id) {
        paint(ui, rect, icon, ui.visuals().text_color());
    }
    result.response
}
