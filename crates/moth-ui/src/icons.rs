// SPDX-License-Identifier: GPL-3.0-or-later
//! Small vector icons stay crisp without a second icon font or image cache.
use egui::{Color32, Pos2, Rect, Response, Stroke, Ui, Vec2};
/// House icons use one stroke weight across touch and desktop controls.
#[derive(Clone, Copy)]
pub enum Icon {
    /// Return to the board catalog.
    Home,
    /// Select canvas items.
    Select,
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
    /// Click-through control.
    Pointer,
}
/// Paints into a reserved rectangle so icons cannot change text layout.
pub fn paint(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let r = Rect::from_center_size(rect.center(), Vec2::splat(18.));
    let p = |x: f32, y: f32| Pos2::new(r.left() + x, r.top() + y);
    let stroke = Stroke::new(1.5, color);
    let line = |points: &[(f32, f32)]| {
        ui.painter().add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    match icon {
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
                2.,
                stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter()
                .rect_filled(Rect::from_min_max(p(7., 2.), p(12., 5.)), 1., color);
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
                        Rect::from_min_size(p(x, y), Vec2::splat(5.)),
                        0.,
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
                    Rect::from_min_size(p(x, y), Vec2::new(w, h)),
                    0.,
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
            ui.painter().circle_stroke(p(7., 7.), 5., stroke);
            line(&[(11., 11.), (16., 16.)]);
        }
        Icon::Clock => {
            ui.painter().circle_stroke(p(9., 9.), 7., stroke);
            line(&[(9., 4.), (9., 9.), (13., 11.)]);
        }
        Icon::Image => {
            ui.painter()
                .rect_stroke(r, 2., stroke, egui::StrokeKind::Inside);
            ui.painter().circle_stroke(p(5., 5.), 2., stroke);
            line(&[(1., 16.), (10., 8.), (17., 14.)]);
        }
        Icon::Minus => line(&[(4., 9.), (14., 9.)]),
        Icon::Maximize => {
            ui.painter().rect_stroke(
                Rect::from_min_max(p(3., 3.), p(15., 15.)),
                1.,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
    }
}
/// Icon buttons retain native egui focus, tooltips and accessibility labels.
pub fn button(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let response = ui.add(
        egui::Button::new("")
            .min_size(Vec2::splat(if ui.ctx().content_rect().width() < 600. {
                44.
            } else {
                36.
            }))
            .selected(selected),
    );
    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered(),
        ui.style().animation_time,
    );
    let pressed = ui.ctx().animate_bool_with_time(
        response.id.with("pressed"),
        response.is_pointer_button_down_on(),
        ui.style().animation_time,
    );
    if hover > 0. || pressed > 0. {
        ui.painter().rect_stroke(
            response.rect.shrink(1.),
            5.,
            Stroke::new(
                1. + pressed,
                super::ACCENT.gamma_multiply(hover * 0.5 + pressed * 0.5),
            ),
            egui::StrokeKind::Inside,
        );
    }
    paint(
        ui,
        response.rect.translate(egui::vec2(0., -hover + pressed)),
        icon,
        if selected {
            super::ACCENT
        } else {
            ui.visuals().text_color()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
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
