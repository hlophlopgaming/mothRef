// SPDX-License-Identifier: GPL-3.0-or-later
//! The infinite canvas: notes, selection frames, drag overlays and the tool rail.
use super::{Editing, Intent, Session, arrange};
use crate::gpu::Gpu;
use moth_scene::{Camera, Item, Rect, Uuid};
use moth_ui::Actions;

/// Read-only inputs the canvas draws from; bundled to keep the call small.
pub(super) struct Canvas<'a> {
    pub board: &'a mut Session,
    pub tools: &'a mut super::tools::Tools,
    pub editing: &'a mut Option<Editing>,
    pub drag: Option<([f32; 2], [f32; 2], bool)>,
    pub moving: Option<(Uuid, Rect)>,
    pub gpu: &'a Gpu,
    pub overlay: bool,
}

/// Draws the canvas panel and the tools window, emitting intents for board edits.
pub(super) fn canvas(
    ui: &mut egui::Ui,
    ctx: Canvas<'_>,
    actions: &mut Actions,
    intents: &mut Vec<Intent>,
) {
    let Canvas {
        board,
        tools,
        editing,
        drag,
        moving,
        gpu,
        overlay,
    } = ctx;
    if !overlay {
        egui::Panel::bottom("board_tools_button").show(ui, |ui| {
            if ui.button(moth_ui::text(moth_ui::Text::Tools)).clicked() {
                tools.open = !tools.open;
            }
        });
        let (edits, target, note, tool_intents) = tools.show(ui, board, editing.is_some());
        intents.extend(tool_intents);
        if !edits.is_empty() {
            intents.push(Intent::Commands(edits));
        }
        if let Some(target) = target {
            intents.push(Intent::Export(target));
        }
        if note {
            intents.push(Intent::AddNote(
                board
                    .camera
                    .world([board.viewport.center().x, board.viewport.center().y]),
            ));
        }
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let previous = board.viewport;
            board.viewport = ui.max_rect();
            if tools.scale_with_window
                && previous.is_positive()
                && previous.size() != board.viewport.size()
            {
                let anchor = board
                    .camera
                    .world([previous.center().x, previous.center().y]);
                board.camera.zoom = (board.camera.zoom
                    * (board.viewport.width() / previous.width())
                        .min(board.viewport.height() / previous.height()))
                .clamp(0.01, 64.);
                board.camera.origin = [
                    anchor[0] - board.viewport.center().x / board.camera.zoom,
                    anchor[1] - board.viewport.center().y / board.camera.zoom,
                ];
            }
            if board.needs_fit && board.in_flight == 0 {
                board.fit();
            }
            if board.scene.is_empty() {
                let empty = moth_ui::empty_board(ui);
                actions.paste |= empty.paste;
                actions.open |= empty.open;
            }
            let origin = board
                .camera
                .world([board.viewport.min.x, board.viewport.min.y]);
            for item in board.scene.query(Rect {
                x: origin[0],
                y: origin[1],
                width: board.viewport.width() / board.camera.zoom,
                height: board.viewport.height() / board.camera.zoom,
            }) {
                match item.blob() {
                    // Non-resident images show a placeholder until decoded.
                    Some(blob) if !gpu.registry.resident(blob, 1) => {
                        let p = board.camera.screen([item.rect.x, item.rect.y]);
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(p[0], p[1]),
                            egui::vec2(
                                item.rect.width * board.camera.zoom,
                                item.rect.height * board.camera.zoom,
                            ),
                        );
                        ui.painter().rect_filled(rect, 2., moth_ui::BACKGROUND);
                        ui.painter().rect_stroke(
                            rect,
                            2.,
                            egui::Stroke::new(1., moth_ui::ACCENT),
                            egui::StrokeKind::Inside,
                        );
                    }
                    // Notes are drawn by the UI layer in screen space.
                    None => {
                        if editing.as_ref().is_none_or(|e| e.id != item.id) {
                            draw_note(ui, &board.camera, item);
                        }
                    }
                    Some(_) => {}
                }
            }
            for &id in &board.selection {
                if overlay {
                    break;
                }
                if let Some(item) = board.scene.get(id) {
                    let corners = screen_corners(&board.camera, item.rect, item.transform.rotation);
                    moth_ui::canvas::selection(ui, corners);
                }
            }
            if !overlay {
                let marquee =
                    drag.filter(|(_, _, pan)| !pan && moving.is_none())
                        .map(|(a, b, _)| {
                            let a = board.camera.world(a);
                            let b = board.camera.world(b);
                            Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0]).abs(),
                                height: (a[1] - b[1]).abs(),
                            }
                        });
                let mut bounds = egui::Rect::NOTHING;
                let mut count = 0;
                for item in board.selection.iter().filter_map(|id| board.scene.get(*id)) {
                    let envelope = item.rect.rotated_envelope(item.transform.rotation);
                    let lo = envelope.lower();
                    let hi = envelope.upper();
                    bounds = bounds.union(egui::Rect::from_min_max(
                        egui::pos2(lo[0], lo[1]),
                        egui::pos2(hi[0], hi[1]),
                    ));
                    count += 1;
                }
                if let Some(rect) = marquee {
                    count = board.scene.query(rect).len();
                    bounds = egui::Rect::from_min_size(
                        egui::pos2(rect.x, rect.y),
                        egui::vec2(rect.width, rect.height),
                    );
                }
                if bounds.is_finite() {
                    moth_ui::canvas::selection_badge(
                        ui.ctx(),
                        board.viewport,
                        count,
                        bounds.size(),
                    );
                }
            }
            if !overlay {
                let response = ui.interact(
                    board.viewport,
                    egui::Id::new("canvas_context"),
                    egui::Sense::click(),
                );
                response.context_menu(|ui| {
                    if ui
                        .add_enabled(
                            board.idle() && editing.is_none(),
                            egui::Button::new(moth_ui::text(moth_ui::Text::AddNote)),
                        )
                        .clicked()
                    {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let world = board.camera.world([pos.x, pos.y]);
                            intents.push(Intent::AddNote(world));
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(moth_ui::text(moth_ui::Text::Fit)).clicked() {
                        actions.fit = true;
                        ui.close();
                    }
                    ui.add_enabled_ui(board.idle(), |ui| {
                        if ui.button(moth_ui::text(moth_ui::Text::Undo)).clicked() {
                            intents.push(Intent::Undo);
                            ui.close();
                        }
                        if ui.button(moth_ui::text(moth_ui::Text::Redo)).clicked() {
                            intents.push(Intent::Redo);
                            ui.close();
                        }
                        ui.separator();
                        let enabled = !board.scene.is_empty();
                        ui.add_enabled_ui(enabled, |ui| {
                            let mut items: Vec<_> = board
                                .scene
                                .items()
                                .filter(|i| {
                                    board.selection.is_empty() || board.selection.contains(&i.id)
                                })
                                .collect();
                            items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                            if ui
                                .button(moth_ui::text(moth_ui::Text::FitSelection))
                                .clicked()
                            {
                                intents.push(Intent::FitSelection);
                                ui.close();
                            }
                            let target_width = board.viewport.width() / board.camera.zoom;
                            ui.menu_button(moth_ui::text(moth_ui::Text::Arrange), |ui| {
                                for (label, mode) in [
                                    (moth_ui::Text::Horizontal, arrange::Arrange::Horizontal),
                                    (moth_ui::Text::Vertical, arrange::Arrange::Vertical),
                                    (moth_ui::Text::Grid, arrange::Arrange::Grid),
                                    (moth_ui::Text::Optimal, arrange::Arrange::Optimal),
                                ] {
                                    if ui.button(moth_ui::text(label)).clicked() {
                                        intents.push(Intent::Commands(tools.arrange(
                                            &mut items,
                                            mode,
                                            target_width,
                                        )));
                                        ui.close();
                                    }
                                }
                            });
                            ui.menu_button(moth_ui::text(moth_ui::Text::Normalize), |ui| {
                                for (label, mode) in [
                                    (moth_ui::Text::NormalizeWidth, arrange::Normalize::Width),
                                    (moth_ui::Text::NormalizeHeight, arrange::Normalize::Height),
                                    (moth_ui::Text::NormalizeSize, arrange::Normalize::Size),
                                    (moth_ui::Text::NormalizeArea, arrange::Normalize::Area),
                                ] {
                                    if ui.button(moth_ui::text(label)).clicked() {
                                        intents.push(Intent::Commands(arrange::normalize(
                                            &items, mode,
                                        )));
                                        ui.close();
                                    }
                                }
                            });
                            for (label, quarters) in [
                                (moth_ui::Text::RotateCw, 1.),
                                (moth_ui::Text::RotateCcw, -1.),
                            ] {
                                if ui.button(moth_ui::text(label)).clicked() {
                                    intents.push(Intent::Commands(super::transform_selection(
                                        &board.scene,
                                        &board.selection,
                                        |transform| super::rotated(transform, quarters),
                                    )));
                                    ui.close();
                                }
                            }
                            for (label, axis) in [
                                (moth_ui::Text::FlipHorizontal, 0),
                                (moth_ui::Text::FlipVertical, 1),
                            ] {
                                if ui.button(moth_ui::text(label)).clicked() {
                                    intents.push(Intent::Commands(super::transform_selection(
                                        &board.scene,
                                        &board.selection,
                                        |transform| super::mirrored(transform, axis),
                                    )));
                                    ui.close();
                                }
                            }
                            if ui
                                .button(moth_ui::text(moth_ui::Text::ResetTransform))
                                .clicked()
                            {
                                intents.push(Intent::OriginalGeometry(
                                    items.iter().map(|i| i.id).collect(),
                                    moth_platform::export::Geometry::All,
                                ));
                                ui.close();
                            }
                            if ui.button(moth_ui::text(moth_ui::Text::Delete)).clicked() {
                                intents.push(Intent::Commands(
                                    items
                                        .iter()
                                        .map(|i| moth_scene::Command::Remove(i.id))
                                        .collect(),
                                ));
                                ui.close();
                            }
                        });
                    });
                });
                // Double-click edits a note in place, or creates one on empty canvas.
                if response.double_clicked()
                    && board.idle()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let world = board.camera.world([pos.x, pos.y]);
                    let hit = board
                        .scene
                        .query(Rect {
                            x: world[0],
                            y: world[1],
                            width: 1. / board.camera.zoom,
                            height: 1. / board.camera.zoom,
                        })
                        .into_iter()
                        .rev()
                        .find(|i| i.rect.contains_rotated(i.transform.rotation, world));
                    match hit {
                        Some(item) if item.note().is_some() => {
                            intents.push(Intent::EditNote(item.id))
                        }
                        Some(item) => intents.push(Intent::FocusItem(item.id)),
                        None => intents.push(Intent::AddNote(world)),
                    }
                }
                // The inline note editor floats above the canvas.
                if let Some(editing) = editing {
                    let anchor = board.camera.screen(editing.anchor);
                    let zoom = board.camera.zoom;
                    egui::Area::new(egui::Id::new("note_editor"))
                        .fixed_pos(egui::pos2(anchor[0], anchor[1]))
                        .show(ui.ctx(), |ui| {
                            let response = egui::TextEdit::multiline(&mut editing.draft)
                                .desired_width((200. * zoom).max(120.))
                                .desired_rows(3)
                                .show(ui);
                            if editing.focus {
                                response.response.request_focus();
                                editing.focus = false;
                            }
                            if response.response.lost_focus() {
                                intents.push(Intent::CommitNote);
                            }
                        });
                }
            }
            if !board.scene.is_empty() && !overlay {
                let tool = moth_ui::canvas::tools(ui.ctx(), board.viewport, &mut board.pan);
                actions.paste |= tool.paste;
                actions.fit |= tool.fit;
                actions.open |= tool.open;
                actions.overlay |= tool.overlay;
                if let Some(bounds) = board.scene.bounds() {
                    let normalize = |x: f32, y: f32| {
                        egui::pos2(
                            (x - bounds.x) / bounds.width,
                            (y - bounds.y) / bounds.height,
                        )
                    };
                    // The minimap samples at most 128 items; the indexed bounds still include every item.
                    let samples: Vec<_> = board
                        .scene
                        .items()
                        .take(128)
                        .map(|item| {
                            let envelope = item.rect.rotated_envelope(item.transform.rotation);
                            let lower = envelope.lower();
                            let upper = envelope.upper();
                            egui::Rect::from_min_max(
                                normalize(lower[0], lower[1]),
                                normalize(upper[0], upper[1]),
                            )
                        })
                        .collect();
                    let world_min = board
                        .camera
                        .world([board.viewport.left(), board.viewport.top()]);
                    let world_max = board
                        .camera
                        .world([board.viewport.right(), board.viewport.bottom()]);
                    if let Some(p) = moth_ui::canvas::minimap(
                        ui.ctx(),
                        board.viewport,
                        &samples,
                        egui::Rect::from_min_max(
                            normalize(world_min[0], world_min[1]),
                            normalize(world_max[0], world_max[1]),
                        ),
                    ) {
                        board.camera.origin = [
                            bounds.x + p.x * bounds.width
                                - board.viewport.center().x / board.camera.zoom,
                            bounds.y + p.y * bounds.height
                                - board.viewport.center().y / board.camera.zoom,
                        ];
                    }
                }
            }
            if let (Some((_, rect)), Some((a, b, false))) = (moving, drag) {
                let moved = Rect {
                    x: rect.x + (b[0] - a[0]) / board.camera.zoom,
                    y: rect.y + (b[1] - a[1]) / board.camera.zoom,
                    ..rect
                };
                let rotation = moving
                    .and_then(|(id, _)| board.scene.get(id))
                    .map_or(0., |item| item.transform.rotation);
                let corners = screen_corners(&board.camera, moved, rotation);
                moth_ui::canvas::selection(ui, corners);
            }
            if let Some((a, b, false)) = drag
                && moving.is_none()
            {
                ui.painter().rect_stroke(
                    egui::Rect::from_two_pos(egui::pos2(a[0], a[1]), egui::pos2(b[0], b[1])),
                    0.,
                    egui::Stroke::new(1., moth_ui::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        });
}

/// Frame corners in screen space, so the canvas can outline a rotated selection.
fn screen_corners(camera: &Camera, rect: Rect, rotation: f32) -> [egui::Pos2; 4] {
    rect.corners(rotation).map(|corner| {
        let point = camera.screen(corner);
        egui::pos2(point[0], point[1])
    })
}

/// Draws a note's card and text in screen space. shortcut: notes render in the
/// UI layer, above images, ignoring z-interleave with images; upgrade with a
/// text pipeline in the renderer when stacking between the two matters.
fn draw_note(ui: &egui::Ui, camera: &Camera, item: &Item) {
    let Some(text) = item.note() else {
        return;
    };
    let zoom = camera.zoom;
    let rows = text.lines().count().max(1) as f32;
    // The frame is the note's text bounds; recover the font size from its height
    // so an imported note scaled by BeeRef's `scale` renders at that size.
    let font_size = (item.rect.height / (rows * 1.3)).max(4.0) * zoom;
    let font = egui::FontId::proportional(font_size);
    let color = egui::Color32::from_rgb(224, 226, 231).gamma_multiply(item.transform.opacity);
    let fill = egui::Color32::from_rgba_unmultiplied(39, 42, 50, 235)
        .gamma_multiply(item.transform.opacity);
    let stroke = egui::Stroke::new(
        1.,
        egui::Color32::from_rgb(48, 52, 60).gamma_multiply(item.transform.opacity),
    );
    let rotation = item.transform.rotation;

    // Upright notes take the simple rounded-card path.
    if rotation.abs() < 1e-4 {
        let p = camera.screen([item.rect.x, item.rect.y]);
        let card = egui::Rect::from_min_size(
            egui::pos2(p[0], p[1]),
            egui::vec2(item.rect.width * zoom, item.rect.height * zoom),
        );
        ui.painter().rect_filled(card, 4. * zoom, fill);
        ui.painter()
            .rect_stroke(card, 4. * zoom, stroke, egui::StrokeKind::Inside);
        let mut y = p[1];
        for line in text.lines() {
            ui.painter().text(
                egui::pos2(p[0], y),
                egui::Align2::LEFT_TOP,
                line,
                font.clone(),
                color,
            );
            y += font_size * 1.3;
        }
        return;
    }

    // Rotated notes: polygon card plus per-line text rotated around its own top-left.
    let corners = item.rect.corners(rotation).map(|corner| {
        let s = camera.screen(corner);
        egui::pos2(s[0], s[1])
    });
    ui.painter().add(egui::Shape::convex_polygon(
        vec![corners[0], corners[1], corners[3], corners[2]],
        fill,
        stroke,
    ));
    let line_offset = egui::vec2(
        (corners[2].x - corners[0].x) / rows,
        (corners[2].y - corners[0].y) / rows,
    );
    for (i, line) in text.lines().enumerate() {
        let pos = corners[0] + line_offset * i as f32;
        let galley = ui.fonts_mut(|f| f.layout_no_wrap(line.to_owned(), font.clone(), color));
        ui.painter().add(egui::Shape::Text(
            egui::epaint::TextShape::new(pos, galley, color).with_angle(rotation),
        ));
    }
}

/// Measures a note's auto-sized frame in world units, using egui's bundled font.
pub(super) fn measure_note(ctx: &egui::Context, text: &str) -> [f32; 2] {
    let font_size = moth_scene::NOTE_FONT_SIZE;
    let font = egui::FontId::proportional(font_size);
    let color = egui::Color32::WHITE;
    let mut width = 0.0f32;
    let mut lines = 0usize;
    for line in text.lines() {
        let galley = ctx.fonts_mut(|f| f.layout_no_wrap(line.to_owned(), font.clone(), color));
        width = width.max(galley.size().x);
        lines += 1;
    }
    if text.is_empty() || !text.ends_with('\n') {
        lines = lines.max(1);
    }
    [width, lines as f32 * font_size * 1.3]
}

/// The canvas viewport as a clip rectangle in physical pixels.
pub(super) fn canvas_clip(viewport: egui::Rect, scale: f32, size: [u32; 2]) -> [u32; 4] {
    if !viewport.is_finite() || !viewport.is_positive() {
        return [0, 0, size[0], size[1]];
    }
    let min = [
        (viewport.left() * scale).clamp(0., size[0].saturating_sub(1) as f32) as u32,
        (viewport.top() * scale).clamp(0., size[1].saturating_sub(1) as f32) as u32,
    ];
    let max = [
        (viewport.right() * scale).clamp((min[0] + 1) as f32, size[0] as f32) as u32,
        (viewport.bottom() * scale).clamp((min[1] + 1) as f32, size[1] as f32) as u32,
    ];
    [min[0], min[1], max[0] - min[0], max[1] - min[1]]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_and_resized_canvas_keep_scissors_inside_the_surface() {
        assert_eq!(
            canvas_clip(egui::Rect::NOTHING, 1., [1440, 900]),
            [0, 0, 1440, 900]
        );
        assert_eq!(
            canvas_clip(
                egui::Rect::from_min_max(egui::pos2(-10., 20.), egui::pos2(2000., 3000.)),
                2.,
                [800, 600]
            ),
            [0, 40, 800, 560]
        );
    }
}
