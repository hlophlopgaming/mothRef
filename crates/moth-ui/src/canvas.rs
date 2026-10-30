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
/// Persisted presentation choices supplied by the application's settings model.
pub struct Minimap {
    /// Maximum side length before viewport limits apply.
    pub size: f32,
    /// Edge or corner toward which the navigator moves.
    pub alignment: egui::Align2,
    /// Visibility of the complete navigator, from zero to one.
    pub opacity: f32,
    /// Fraction of the way from the viewport center to the selected placement.
    pub distance: f32,
}
/// Draws bounded metadata samples and returns a normalized click for camera navigation.
pub fn minimap(
    ctx: &Context,
    viewport: egui::Rect,
    items: &[egui::Rect],
    board_size: egui::Vec2,
    options: Minimap,
    camera: egui::Rect,
) -> Option<egui::Pos2> {
    let mut click = None;
    let (mut panel, mut map) = (None, None);
    if viewport.width() < 600. || viewport.height() < 300. {
        ctx.data_mut(|d| {
            d.insert_temp::<Option<(egui::Rect, egui::Rect, f32)>>(
                egui::Id::new("minimap_layout"),
                None,
            )
        });
        return None;
    }
    let tokens = crate::tokens(ctx);
    let size = minimap_size(viewport.size(), board_size, options.size, &tokens);
    let gap = f32::from(tokens.space_m);
    let inset = f32::from(tokens.space_badge);
    let bottom = ctx.memory(|memory| {
        ["bottom_toolbar", "selection_badge"]
            .into_iter()
            .filter_map(|id| memory.area_rect(egui::Id::new(id)))
            .map(|rect| rect.top())
            .fold(viewport.bottom(), f32::min)
    });
    let mut placement = viewport.shrink(gap);
    if options.alignment.y() == egui::Align::Max {
        placement.max.y = placement.max.y.min(bottom - gap);
    }
    let target = options
        .alignment
        .anchor_size(options.alignment.pos_in_rect(&placement), size);
    let center = viewport.center().lerp(target.center(), options.distance);
    egui::Area::new(egui::Id::new("minimap"))
        .fixed_pos(center)
        .pivot(egui::Align2::CENTER_CENTER)
        .constrain_to(viewport)
        .show(ctx, |ui| {
            ui.set_opacity(options.opacity);
            // No fill here: the renderer draws the panel and the thumbnails below this
            // layer (see `minimap_layout`); egui only adds the viewport frame on top.
            let frame = egui::Frame::new()
                .inner_margin(crate::tokens(ui.ctx()).space_badge as i8)
                .show(ui, |ui| {
                    let (r, response) = ui.allocate_exact_size(
                        size - egui::Vec2::splat(2. * inset),
                        egui::Sense::click_and_drag(),
                    );
                    let transform =
                        |p: egui::Pos2| r.min + egui::vec2(p.x * r.width(), p.y * r.height());
                    let _ = items;
                    map = Some(r);
                    // What the window shows: a soft veil outside, a clear frame inside.
                    let view =
                        egui::Rect::from_min_max(transform(camera.min), transform(camera.max))
                            .intersect(r);
                    ui.painter().rect_stroke(
                        view,
                        crate::tokens(ui.ctx()).radius_s as f32,
                        egui::Stroke::new(1.5, crate::tokens(ui.ctx()).accent),
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
            panel = Some(frame.response.rect);
        });
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new("minimap_layout"),
            panel
                .zip(map)
                .map(|(panel, map)| (panel, map, options.opacity)),
        )
    });
    click
}
/// Forgets the minimap until it is drawn again, so a hidden one leaves no panel behind.
pub fn clear_minimap(ctx: &Context) {
    ctx.data_mut(|d| {
        d.insert_temp::<Option<(egui::Rect, egui::Rect, f32)>>(
            egui::Id::new("minimap_layout"),
            None,
        )
    });
}
/// Where the last frame's minimap sits: its panel, the map inside it, and opacity.
pub fn minimap_layout(ctx: &Context) -> Option<(egui::Rect, egui::Rect, f32)> {
    ctx.data(|d| {
        d.get_temp::<Option<(egui::Rect, egui::Rect, f32)>>(egui::Id::new("minimap_layout"))
    })
    .flatten()
}

fn minimap_size(
    viewport: egui::Vec2,
    board: egui::Vec2,
    max_side: f32,
    tokens: &crate::Tokens,
) -> egui::Vec2 {
    let side = max_side.min(viewport.x * 0.3).min(viewport.y * 0.3);
    let aspect = if board.x.is_finite() && board.y.is_finite() && board.x > 0. && board.y > 0. {
        (board.x / board.y).clamp(tokens.minimap_aspect_min, 1. / tokens.minimap_aspect_min)
    } else {
        1.
    };
    if aspect >= 1. {
        egui::vec2(side, side / aspect)
    } else {
        egui::vec2(side * aspect, side)
    }
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
    fn minimap_placement_keeps_every_anchor_inside_the_viewport() {
        use egui::Align2;
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200., 800.));
        for alignment in [
            Align2::LEFT_TOP,
            Align2::CENTER_TOP,
            Align2::RIGHT_TOP,
            Align2::LEFT_CENTER,
            Align2::CENTER_CENTER,
            Align2::RIGHT_CENTER,
            Align2::LEFT_BOTTOM,
            Align2::CENTER_BOTTOM,
            Align2::RIGHT_BOTTOM,
        ] {
            let ctx = Context::default();
            crate::theme(&ctx, &crate::Tokens::LIGHT);
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        ..Default::default()
                    },
                    |ui| {
                        minimap(
                            ui.ctx(),
                            viewport,
                            &[],
                            egui::vec2(100., 200.),
                            Minimap {
                                size: crate::Tokens::LIGHT.minimap_max,
                                alignment,
                                opacity: 1.,
                                distance: 1.,
                            },
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                        );
                    },
                );
                output.textures_delta.clear();
            }
            let rect = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("minimap")))
                .unwrap();
            assert!(viewport.contains_rect(rect), "{alignment:?}: {rect:?}");
            let expected =
                alignment.pos_in_rect(&viewport.shrink(f32::from(crate::Tokens::LIGHT.space_m)));
            assert!(
                alignment.pos_in_rect(&rect).distance(expected) < 2.,
                "{alignment:?}: {rect:?}"
            );
        }
    }
    #[test]
    fn minimap_distance_interpolates_its_center_toward_the_selected_corner() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200., 800.));
        let board = egui::vec2(100., 200.);
        let tokens = crate::Tokens::LIGHT;
        let size = minimap_size(viewport.size(), board, tokens.minimap_max, &tokens);
        let corner = egui::Align2::LEFT_BOTTOM
            .anchor_size(
                egui::Align2::LEFT_BOTTOM.pos_in_rect(&viewport.shrink(f32::from(tokens.space_m))),
                size,
            )
            .center();
        for distance in [0., 0.5, 1.] {
            let ctx = Context::default();
            crate::theme(&ctx, &tokens);
            for _ in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        ..Default::default()
                    },
                    |ui| {
                        minimap(
                            ui.ctx(),
                            viewport,
                            &[],
                            board,
                            Minimap {
                                size: tokens.minimap_max,
                                alignment: egui::Align2::LEFT_BOTTOM,
                                opacity: 0.5,
                                distance,
                            },
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                        );
                    },
                );
                output.textures_delta.clear();
            }
            let rect = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("minimap")))
                .unwrap();
            assert!(
                rect.center()
                    .distance(viewport.center().lerp(corner, distance))
                    < 2.
            );
            assert!(viewport.contains_rect(rect));
        }
    }
    #[test]
    fn minimap_size_preference_respects_viewport_limit() {
        let viewport = egui::vec2(1600., 1000.);
        let board = egui::vec2(100., 200.);
        let small = minimap_size(viewport, board, 120., &crate::Tokens::LIGHT);
        let large = minimap_size(viewport, board, 600., &crate::Tokens::LIGHT);
        assert_eq!(small, egui::vec2(60., 120.));
        assert_eq!(large, egui::vec2(150., 300.));
    }
    #[test]
    fn minimap_adapts_to_board_and_stays_bounded() {
        let viewport = egui::vec2(800., 600.);
        for board in [
            egui::vec2(100., 1000.),
            egui::vec2(1000., 100.),
            egui::vec2(100., 100.),
            egui::Vec2::ZERO,
        ] {
            let size = minimap_size(
                viewport,
                board,
                crate::Tokens::LIGHT.minimap_max,
                &crate::Tokens::LIGHT,
            );
            assert!(size.x > 0. && size.y > 0.);
            assert!(size.x <= viewport.x * 0.3 && size.y <= viewport.y * 0.3);
            if board.x < board.y {
                assert!(size.x < size.y);
            }
            if board.x > board.y {
                assert!(size.x > size.y);
            }
            assert!(
                size.min_elem() / size.max_elem()
                    >= crate::Tokens::LIGHT.minimap_aspect_min - f32::EPSILON
            );
        }
    }
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
