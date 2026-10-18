// SPDX-License-Identifier: GPL-3.0-or-later
//! Lightweight canvas furniture uses screen rectangles, without retaining scene items.
use super::{
    icons::{self, Icon},
    *,
};
/// Adds visible handles around a possibly rotated frame, without implying that
/// selection changed the source image.
///
/// Corners arrive in world-rotation order: top-left, top-right, bottom-left,
/// bottom-right. Handles land on the same 3x3 grid an unrotated frame has.
pub fn selection(ui: &egui::Ui, corners: [egui::Pos2; 4]) {
    let [top_left, top_right, bottom_left, bottom_right] = corners;
    ui.painter().add(egui::Shape::closed_line(
        vec![top_left, top_right, bottom_right, bottom_left],
        egui::Stroke::new(1., ACCENT),
    ));
    for across in 0..3u8 {
        for down in 0..3u8 {
            if across == 1 && down == 1 {
                continue;
            }
            let ratio = |step: u8| step as f32 / 2.;
            let top = top_left.lerp(top_right, ratio(across));
            let bottom = bottom_left.lerp(bottom_right, ratio(across));
            let handle =
                egui::Rect::from_center_size(top.lerp(bottom, ratio(down)), egui::vec2(6., 6.));
            ui.painter().rect_filled(handle, 0., BACKGROUND);
            ui.painter().rect_stroke(
                handle,
                0.,
                egui::Stroke::new(1., ACCENT),
                egui::StrokeKind::Inside,
            );
        }
    }
}
/// A floating tool rail leaves the whole canvas available for content.
pub fn tools(ctx: &Context, viewport: egui::Rect, pan: &mut bool) -> Actions {
    let mut a = Actions::default();
    egui::Area::new(egui::Id::new("canvas_tools"))
        .fixed_pos(viewport.left_top() + egui::vec2(12., 12.))
        .show(ctx, |ui| {
            // Separators fill the cross axis, so constrain it to the buttons.
            let horizontal = viewport.height() < 360.;
            let button_width = if ctx.content_rect().width() < 600. {
                44.
            } else {
                36.
            };
            ui.set_max_width(if horizontal {
                (viewport.width() - 24.).max(0.)
            } else {
                button_width + 10.
            });
            if horizontal {
                ui.set_max_height(button_width + 10.);
            }
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .corner_radius(12)
                .inner_margin(5)
                .stroke(egui::Stroke::new(1., Color32::from_rgb(47, 51, 59)))
                .show(ui, |ui| {
                    let layout = if horizontal {
                        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true)
                    } else {
                        egui::Layout::top_down(egui::Align::Min)
                    };
                    ui.with_layout(layout, |ui| {
                        if icons::button(ui, Icon::Select, &tooltip(Text::Select), !*pan).clicked()
                        {
                            *pan = false;
                        }
                        if icons::button(ui, Icon::Pan, &tooltip(Text::Pan), *pan).clicked() {
                            *pan = true;
                        }
                        a.paste =
                            icons::button(ui, Icon::Paste, &tooltip(Text::Paste), false).clicked();
                        a.open =
                            icons::button(ui, Icon::Folder, &tooltip(Text::Open), false).clicked();
                        ui.separator();
                        a.fit = icons::button(ui, Icon::Fit, &tooltip(Text::Fit), false).clicked();
                        a.overlay =
                            icons::button(ui, Icon::Overlay, &tooltip(Text::Overlay), false)
                                .clicked();
                    });
                });
        });
    a
}
/// Draws bounded metadata samples and returns a normalized click for camera navigation.
pub fn minimap(
    ctx: &Context,
    viewport: egui::Rect,
    items: &[egui::Rect],
    camera: egui::Rect,
) -> Option<egui::Pos2> {
    let mut click = None;
    if viewport.width() < 600. || viewport.height() < 300. {
        return None;
    }
    let size = egui::vec2(188., 120.);
    egui::Area::new(egui::Id::new("minimap"))
        .fixed_pos(viewport.right_bottom() - size - egui::vec2(16., 16.))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .corner_radius(10)
                .inner_margin(10)
                .show(ui, |ui| {
                    let (r, response) = ui.allocate_exact_size(
                        size - egui::vec2(20., 20.),
                        egui::Sense::click_and_drag(),
                    );
                    let transform =
                        |p: egui::Pos2| r.min + egui::vec2(p.x * r.width(), p.y * r.height());
                    for item in items {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_max(transform(item.min), transform(item.max))
                                .intersect(r),
                            0.,
                            Color32::from_rgb(69, 75, 82),
                        );
                    }
                    ui.painter().rect_stroke(
                        egui::Rect::from_min_max(transform(camera.min), transform(camera.max))
                            .intersect(r),
                        0.,
                        egui::Stroke::new(1., ACCENT),
                        egui::StrokeKind::Inside,
                    );
                    if (response.clicked() || response.dragged())
                        && let Some(p) = response.interact_pointer_pos()
                    {
                        click = Some(egui::pos2(
                            ((p.x - r.left()) / r.width()).clamp(0., 1.),
                            ((p.y - r.top()) / r.height()).clamp(0., 1.),
                        ));
                    }
                });
        });
    click
}

/// Keeps selection measurements visible without retaining scene metadata.
pub fn selection_badge(ctx: &Context, viewport: egui::Rect, count: usize, size: egui::Vec2) {
    egui::Area::new(egui::Id::new("selection_badge"))
        .fixed_pos(viewport.center_bottom() - egui::vec2(0., 8.))
        .pivot(egui::Align2::CENTER_BOTTOM)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(8)
                .corner_radius(6)
                .show(ui, |ui| {
                    ui.label(format!(
                        "{} {} · {:.0} × {:.0}",
                        count,
                        text(Text::Selected),
                        size.x,
                        size.y
                    ));
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_rail_stays_compact_across_canvas_sizes() {
        let ctx = Context::default();
        let mut pan = true;
        for size in [
            egui::vec2(1430., 720.),
            egui::vec2(800., 300.),
            egui::vec2(320., 240.),
            egui::vec2(320., 720.),
            egui::vec2(1430., 720.),
        ] {
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let input = || egui::RawInput {
                screen_rect: Some(viewport),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input(), |ui| {
                tools(ui.ctx(), viewport, &mut pan);
            });
            output.textures_delta.clear();
            let mut output = ctx.run_ui(input(), |ui| {
                tools(ui.ctx(), viewport, &mut pan);
            });
            output.textures_delta.clear();
            let rail = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.corner_radius == egui::CornerRadius::same(12) => {
                        Some(r.rect)
                    }
                    _ => None,
                })
                .expect("tool rail paints its frame");
            if size.y >= 360. {
                assert!(rail.width() < 80., "tool rail expanded to {rail:?}");
                assert!(rail.height() < 360., "tool rail expanded to {rail:?}");
            } else {
                assert!(rail.height() < 120., "horizontal rail expanded to {rail:?}");
                assert!(
                    rail.width() <= size.x - 24.,
                    "rail leaves viewport: {rail:?}"
                );
            }
        }
    }
    #[test]
    fn badge_reports_count_and_world_size() {
        let ctx = Context::default();
        let draw = |ui: &mut egui::Ui| {
            selection_badge(ui.ctx(), ui.max_rect(), 3, egui::vec2(120., 80.));
        };
        let mut warmup = ctx.run_ui(Default::default(), draw);
        warmup.textures_delta.clear();
        let mut output = ctx.run_ui(Default::default(), draw);
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text().contains("120 × 80") && t.galley.text().starts_with("3 "))
        }));
    }

    #[test]
    fn selection_outline_follows_frame_perimeter() {
        let ctx = Context::default();
        let corners = [
            egui::pos2(20., 20.),
            egui::pos2(120., 30.),
            egui::pos2(10., 80.),
            egui::pos2(110., 90.),
        ];
        let mut output = ctx.run_ui(Default::default(), |ui| selection(ui, corners));
        output.textures_delta.clear();
        let path = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Path(path) => Some(path),
                _ => None,
            })
            .expect("selection always paints an outline path");
        assert_eq!(
            path.points,
            [corners[0], corners[1], corners[3], corners[2]]
        );
        assert!(path.closed);
        assert_eq!(path.fill, Color32::TRANSPARENT);
        assert_eq!(path.stroke.color, egui::epaint::ColorMode::Solid(ACCENT));
    }
}
