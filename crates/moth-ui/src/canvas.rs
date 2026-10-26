// SPDX-License-Identifier: GPL-3.0-or-later
//! Lightweight canvas furniture uses screen rectangles, without retaining scene items.
use super::*;
/// Adds visible handles around a possibly rotated frame, without implying that
/// selection changed the source image.
///
/// Corners arrive in world-rotation order: top-left, top-right, bottom-left,
/// bottom-right. Handles land on the same 3x3 grid an unrotated frame has.
pub fn selection(ui: &egui::Ui, corners: [egui::Pos2; 4]) {
    let [top_left, top_right, bottom_left, bottom_right] = corners;
    ui.painter().add(egui::Shape::closed_line(
        vec![top_left, top_right, bottom_right, bottom_left],
        egui::Stroke::new(1., crate::tokens(ui.ctx()).accent),
    ));
    for across in [0, 2u8] {
        for down in [0, 2u8] {
            if across == 1 && down == 1 {
                continue;
            }
            let ratio = |step: u8| step as f32 / 2.;
            let top = top_left.lerp(top_right, ratio(across));
            let bottom = bottom_left.lerp(bottom_right, ratio(across));
            let handle =
                egui::Rect::from_center_size(top.lerp(bottom, ratio(down)), egui::vec2(6., 6.));
            ui.painter().rect_filled(
                handle,
                crate::tokens(ui.ctx()).radius_none as f32,
                crate::tokens(ui.ctx()).canvas,
            );
            ui.painter().rect_stroke(
                handle,
                crate::tokens(ui.ctx()).radius_none as f32,
                egui::Stroke::new(1., crate::tokens(ui.ctx()).accent),
                egui::StrokeKind::Inside,
            );
        }
    }
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
                .corner_radius(crate::tokens(ui.ctx()).radius_card)
                .inner_margin(crate::tokens(ui.ctx()).space_badge as i8)
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
                            crate::tokens(ui.ctx()).radius_none as f32,
                            crate::tokens(ui.ctx()).minimap_item,
                        );
                    }
                    ui.painter().rect_stroke(
                        egui::Rect::from_min_max(transform(camera.min), transform(camera.max))
                            .intersect(r),
                        crate::tokens(ui.ctx()).radius_none as f32,
                        egui::Stroke::new(1., crate::tokens(ui.ctx()).accent),
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
    let label = format!(
        "{} {} · {:.0} × {:.0}",
        count,
        text(Text::Selected),
        size.x,
        size.y
    );
    let mut font = egui::TextStyle::Body.resolve(&ctx.style_of(ctx.theme()));
    let color = crate::tokens(ctx).text;
    let measured = ctx
        .fonts_mut(|fonts| fonts.layout_no_wrap(label.clone(), font.clone(), color))
        .size()
        .x;
    let available = (viewport.width()
        - 2. * f32::from(crate::tokens(ctx).space_m + crate::tokens(ctx).space_s))
    .max(1.);
    font.size *= (available / measured).min(1.);
    let gap = crate::tokens(ctx).space_m as f32;
    let bottom = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("bottom_toolbar")))
        .map_or(viewport.bottom(), |rect| rect.top());
    egui::Area::new(egui::Id::new("selection_badge"))
        .constrain_to(viewport)
        .fixed_pos(egui::pos2(viewport.left() + gap, bottom - gap))
        .pivot(egui::Align2::LEFT_BOTTOM)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(crate::tokens(ui.ctx()).space_s as i8)
                .corner_radius(crate::tokens(ui.ctx()).radius_badge)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(label).font(font)).extend());
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_remains_one_line_inside_a_narrow_viewport() {
        let ctx = Context::default();
        crate::theme(&ctx, &crate::Tokens::LIGHT);
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320., 600.));
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |ui| {
                    selection_badge(
                        ui.ctx(),
                        viewport,
                        999999,
                        egui::vec2(999999999., 999999999.),
                    );
                },
            );
            for text in output.shapes.iter().filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            }) {
                assert_eq!(text.galley.rows.len(), 1);
            }
            output.textures_delta.clear();
        }
        let rect = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("selection_badge")))
            .unwrap();
        assert!(viewport.contains_rect(rect), "{rect:?}");
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
        assert_eq!(path.fill, egui::Color32::TRANSPARENT);
        assert_eq!(
            path.stroke.color,
            egui::epaint::ColorMode::Solid(Tokens::DARK.accent)
        );
    }
}
