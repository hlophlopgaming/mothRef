// SPDX-License-Identifier: GPL-3.0-or-later
//! Lightweight canvas furniture uses screen rectangles, without retaining scene items.
use super::{
    icons::{self, Icon},
    *,
};
/// Adds visible handles without implying that selection changed the source image.
pub fn selection(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect,
        0.,
        egui::Stroke::new(1., ACCENT),
        egui::StrokeKind::Outside,
    );
    for x in [rect.left(), rect.center().x, rect.right()] {
        for y in [rect.top(), rect.center().y, rect.bottom()] {
            if x == rect.center().x && y == rect.center().y {
                continue;
            }
            let handle = egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(6., 6.));
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
            egui::Frame::new()
                .fill(PANEL)
                .corner_radius(12)
                .inner_margin(5)
                .stroke(egui::Stroke::new(1., Color32::from_rgb(47, 51, 59)))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        if icons::button(ui, Icon::Select, text(Text::Select), !*pan).clicked() {
                            *pan = false;
                        }
                        if icons::button(ui, Icon::Pan, text(Text::Pan), *pan).clicked() {
                            *pan = true;
                        }
                        a.paste =
                            icons::button(ui, Icon::Paste, text(Text::Paste), false).clicked();
                        a.open = icons::button(ui, Icon::Folder, text(Text::Open), false).clicked();
                        ui.separator();
                        a.fit = icons::button(ui, Icon::Fit, text(Text::Fit), false).clicked();
                        a.overlay =
                            icons::button(ui, Icon::Overlay, text(Text::Overlay), false).clicked();
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
                .fill(PANEL)
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
