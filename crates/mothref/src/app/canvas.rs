// SPDX-License-Identifier: GPL-3.0-or-later
//! The infinite canvas: notes, selection frames, drag overlays and the tool rail.
use super::{Document, Editing, Intent};
use crate::gpu::Gpu;
use moth_scene::{Camera, Item, Rect, Uuid};
use moth_ui::Actions;

/// Read-only inputs the canvas draws from; bundled to keep the call small.
pub(super) struct Canvas<'a> {
    pub board: &'a Document,
    pub tools: &'a mut super::tools::Tools,
    pub settings: &'a mut super::settings::Settings,
    pub enabled: &'a [bool],
    pub registry: &'a super::bindings::Registry,
    pub editing: &'a mut Option<Editing>,
    pub drag: Option<([f32; 2], [f32; 2], bool)>,
    pub moving: Option<(Uuid, Rect)>,
    pub resizing: Option<&'a super::resize::Resize>,
    pub pointer: Option<[f32; 2]>,
    pub motion: &'a super::motion::DragMotion,
    pub duplicate: bool,
    pub gpu: &'a Gpu,
    pub compact: bool,
    pub overlay: bool,
    pub secondary_gesture: bool,
    pub activate: bool,
}

/// Draws the canvas panel and the tools window, emitting intents for board edits.
pub(super) fn canvas(ui: &mut egui::Ui, ctx: Canvas<'_>) -> Vec<Intent> {
    let mut actions = Actions::default();
    let mut intents = Vec::new();
    let Canvas {
        board,
        tools,
        settings,
        enabled,
        registry,
        editing,
        drag,
        moving,
        resizing,
        pointer,
        motion,
        duplicate,
        gpu,
        compact,
        overlay,
        secondary_gesture,
        activate,
    } = ctx;
    let mut view = board.view;
    if !compact {
        intents.extend(tools.show(ui, board, settings, enabled, registry));
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let hover = pointer
                .filter(|p| view.viewport.contains(egui::pos2(p[0], p[1])))
                .and_then(|p| {
                    let world = view.camera.world(p);
                    board
                        .scene
                        .query(Rect {
                            x: world[0],
                            y: world[1],
                            width: 1. / view.camera.zoom,
                            height: 1. / view.camera.zoom,
                        })
                        .into_iter()
                        .rev()
                        .find(|item| {
                            board.presented(item)
                                && item.rect.contains_rotated(item.transform.rotation, world)
                        })
                        .map(|item| item.id)
                });
            let over_canvas =
                pointer.is_some_and(|p| view.viewport.contains(egui::pos2(p[0], p[1])));
            if over_canvas || drag.is_some() {
                ui.ctx().set_cursor_icon(if resizing.is_some() {
                    egui::CursorIcon::ResizeNwSe
                } else if tools.picking || tools.coordinates {
                    egui::CursorIcon::Crosshair
                } else if drag.is_some_and(|(_, _, pan)| pan) || moving.is_some() {
                    egui::CursorIcon::Grabbing
                } else if hover.is_some_and(|id| board.selection.contains(&id)) {
                    egui::CursorIcon::Move
                } else if hover.is_some() {
                    egui::CursorIcon::Grab
                } else {
                    egui::CursorIcon::Default
                });
            }
            let hover_key = egui::Id::new("canvas_hover_item");
            if let Some(id) = hover {
                ui.ctx().data_mut(|data| data.insert_temp(hover_key, id));
            }
            let fade = ui.ctx().animate_bool_with_time(
                hover_key,
                settings.show_outlines && hover.is_some() && moving.is_none(),
                if settings.reduced_motion || !settings.show_outlines {
                    0.
                } else {
                    0.09
                },
            );
            let hover_id = ui.ctx().data(|data| data.get_temp::<Uuid>(hover_key));
            let previous = view.viewport;
            view.viewport = ui.max_rect();
            if !compact {
                intents.extend(super::surfaces::toolbar(
                    ui.ctx(),
                    view.viewport,
                    enabled,
                    registry,
                    view.camera.zoom,
                ));
            }
            view.safe_area = view.viewport;
            view.selection_bar_visible = !compact && !board.selection.is_empty();
            view.selection_bar_height = 0.;
            if !compact {
                let bar_bottom = ui
                    .ctx()
                    .data(|data| data.get_temp::<f32>(egui::Id::new("window_bar_bottom")));
                let (toolbar, selection) = ui.ctx().memory(|memory| {
                    (
                        memory.area_rect(egui::Id::new("bottom_toolbar")),
                        memory.area_rect(egui::Id::new("selection_bar")),
                    )
                });
                view.safe_area.min.y = bar_bottom
                    .unwrap_or(view.viewport.top())
                    .max(view.viewport.top());
                if let Some(toolbar) = toolbar {
                    view.safe_area.max.y = toolbar.top().min(view.viewport.bottom());
                }
                view.selection_bar_height =
                    selection.map_or(ui.spacing().interact_size.y, |rect| rect.height());
            }

            if settings.scale_with_window
                && previous.is_positive()
                && previous.size() != view.viewport.size()
            {
                let anchor = view
                    .camera
                    .world([previous.center().x, previous.center().y]);
                view.camera.zoom = (view.camera.zoom
                    * (view.viewport.width() / previous.width())
                        .min(view.viewport.height() / previous.height()))
                .clamp(0.01, 64.);
                view.camera.origin = [
                    anchor[0] - view.viewport.center().x / view.camera.zoom,
                    anchor[1] - view.viewport.center().y / view.camera.zoom,
                ];
            }
            if view.needs_fit && board.in_flight == 0 {
                view.fit(&board.scene);
            }
            if board.scene.is_empty() {
                let empty = moth_ui::empty_board(ui);
                actions.paste |= empty.paste;
                if empty.open {
                    intents.push(Intent::Action(super::bindings::Action::Images));
                }
            }
            let origin = view
                .camera
                .world([view.viewport.min.x, view.viewport.min.y]);
            let mut returning: std::collections::HashMap<_, _> = motion
                .returning(motion.now(), settings.reduced_motion)
                .into_iter()
                .map(|item| (item.id, item))
                .collect();
            if let Some(resizing) = resizing {
                returning.extend(resizing.previews().into_iter().map(|item| (item.id, item)));
            }
            let mut visible = board.scene.query(Rect {
                x: origin[0],
                y: origin[1],
                width: view.viewport.width() / view.camera.zoom,
                height: view.viewport.height() / view.camera.zoom,
            });

            let transitions = board.motion.copies(motion.now());
            if moving.is_some() || !transitions.is_empty() || !returning.is_empty() {
                let mut ids: std::collections::HashSet<_> =
                    visible.iter().map(|item| item.id).collect();
                for id in board
                    .selection
                    .iter()
                    .filter(|_| moving.is_some())
                    .copied()
                    .chain(transitions.iter().map(|item| item.id))
                    .chain(returning.keys().copied())
                {
                    if ids.insert(id)
                        && let Some(item) = board.scene.get(id)
                    {
                        visible.push(item);
                    }
                }
            }
            let mut groups = Vec::new();
            for item in visible.into_iter().filter(|item| board.presented(item)) {
                let mut presented = board
                    .motion
                    .get(item.id, motion.now())
                    .map(std::borrow::Cow::Owned)
                    .unwrap_or(std::borrow::Cow::Borrowed(item));
                if moving.is_some()
                    && board.selection.contains(&item.id)
                    && !tools.locked
                    && !board.loading
                {
                    presented = std::borrow::Cow::Owned(motion.preview(
                        item,
                        drag,
                        view.camera.zoom,
                        settings.reduced_motion,
                    ));
                    if duplicate && item.blob().is_none() {
                        draw_note(ui, &view.camera, item);
                    }
                }
                if moving.is_none()
                    && let Some(returning) = returning.get(&item.id).cloned()
                {
                    presented = std::borrow::Cow::Owned(returning);
                }
                let item = &presented;
                if item.is_group() {
                    groups.push(item.as_ref().clone());
                }
                match item.blob() {
                    // Non-resident images show a placeholder until decoded.
                    Some(blob) if !gpu.registry.resident(blob, 1) => {
                        let p = view.camera.screen([item.rect.x, item.rect.y]);
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(p[0], p[1]),
                            egui::vec2(
                                item.rect.width * view.camera.zoom,
                                item.rect.height * view.camera.zoom,
                            ),
                        );
                        ui.painter().rect_filled(
                            rect,
                            moth_ui::tokens(ui.ctx()).radius_placeholder as f32,
                            moth_ui::tokens(ui.ctx()).canvas,
                        );
                        ui.painter().rect_stroke(
                            rect,
                            moth_ui::tokens(ui.ctx()).radius_placeholder as f32,
                            egui::Stroke::new(1., moth_ui::tokens(ui.ctx()).accent),
                            egui::StrokeKind::Inside,
                        );
                    }
                    // Notes are drawn by the UI layer in screen space.
                    None => {
                        if editing.as_ref().is_none_or(|e| e.id != item.id) {
                            draw_note(ui, &view.camera, item);
                        }
                    }
                    Some(_) => {}
                }
                if !overlay {
                    outline(
                        ui,
                        &view.camera,
                        item,
                        board.selection.len() == 1
                            && board.selection.contains(&item.id)
                            && !(duplicate
                                && moving.is_none()
                                && !returning.is_empty()
                                && !returning.contains_key(&item.id)),
                        if hover_id == Some(item.id) { fade } else { 0. },
                        settings.show_outlines,
                    );
                }
            }
            if !overlay && !compact {
                intents.extend(super::groups::chips(ui, board, &view.camera, &mut groups));
                if !board.scene.is_empty() {
                    intents.extend(super::groups::layers(ui.ctx(), board, view.safe_area));
                }
            }
            for ghost in returning.values().filter(|item| board.presented(item)) {
                if ghost.blob().is_none() && board.scene.get(ghost.id).is_none() {
                    draw_note(ui, &view.camera, ghost);
                }
                if !overlay && board.scene.get(ghost.id).is_none() {
                    outline(ui, &view.camera, ghost, true, 0., settings.show_outlines);
                }
            }
            for ghost in transitions.into_iter().filter(|item| board.presented(item)) {
                if ghost.blob().is_none() && board.scene.get(ghost.id).is_none() {
                    draw_note(ui, &view.camera, &ghost);
                }
            }
            if !overlay {
                let marquee =
                    drag.filter(|(_, _, pan)| !pan && moving.is_none())
                        .map(|(a, b, _)| {
                            let a = view.camera.world(a);
                            let b = view.camera.world(b);
                            Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0]).abs(),
                                height: (a[1] - b[1]).abs(),
                            }
                        });
                let mut bounds = egui::Rect::NOTHING;
                let mut count = 0;
                for item in board
                    .selection
                    .iter()
                    .filter_map(|id| board.scene.get(*id))
                    .filter(|i| board.presented(i))
                {
                    let presented = if moving.is_some() && !tools.locked && !board.loading {
                        Some(motion.preview(item, drag, view.camera.zoom, settings.reduced_motion))
                    } else {
                        selection_presentation(
                            item,
                            board.motion.get(item.id, motion.now()),
                            &returning,
                        )
                    };
                    let item = presented.as_ref().unwrap_or(item);
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
                    moth_ui::canvas::selection_badge(ui.ctx(), view.viewport, count, bounds.size());
                    if marquee.is_none() && !board.selection.is_empty() {
                        let screen = |p: egui::Pos2| {
                            let p = view.camera.screen([p.x, p.y]);
                            egui::pos2(p[0], p[1])
                        };
                        let frame =
                            egui::Rect::from_min_max(screen(bounds.min), screen(bounds.max));
                        selection_frame(
                            ui,
                            frame,
                            board.selection.len() > 1,
                            settings.show_outlines,
                        );
                        if !compact {
                            let group =
                                super::selection::single_group(&board.scene, &board.selection);
                            let kind =
                                if let Some(group) = group {
                                    super::surfaces::SelectionKind::Group(
                                        board.scene.children(group).count(),
                                    )
                                } else if board.selection.len() > 1 {
                                    super::surfaces::SelectionKind::Several
                                } else if board.selection.iter().any(|id| {
                                    board.scene.get(*id).is_some_and(|i| i.note().is_some())
                                }) {
                                    super::surfaces::SelectionKind::Note
                                } else {
                                    super::surfaces::SelectionKind::Image
                                };
                            intents.extend(super::surfaces::selection(
                                ui.ctx(),
                                frame,
                                view.viewport,
                                kind,
                                enabled,
                                registry,
                            ));
                        }
                    }
                }
            }
            if !overlay {
                let response = ui.interact(
                    view.viewport,
                    egui::Id::new("canvas_context"),
                    egui::Sense::click(),
                );
                if !secondary_gesture {
                    response.context_menu(|ui| {
                        super::surfaces::menu(
                            ui,
                            super::surfaces::CONTEXT_MENU,
                            enabled,
                            registry,
                            &mut intents,
                        )
                    });
                }
                // Double-click edits a note in place, or creates one on empty canvas.
                if response.double_clicked()
                    && activate
                    && board.idle()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let world = view.camera.world([pos.x, pos.y]);
                    let hit = board
                        .scene
                        .query(Rect {
                            x: world[0],
                            y: world[1],
                            width: 1. / view.camera.zoom,
                            height: 1. / view.camera.zoom,
                        })
                        .into_iter()
                        .rev()
                        .find(|i| {
                            board.presented(i)
                                && i.rect.contains_rotated(i.transform.rotation, world)
                        });
                    let chip = super::groups::chip_rects(ui.ctx())
                        .into_iter()
                        .rev()
                        .find(|(_, rect)| rect.contains(pos))
                        .map(|(id, _)| id);
                    match hit {
                        _ if chip.is_some() => {
                            intents.push(Intent::Action(super::bindings::Action::RenameGroup))
                        }
                        Some(item)
                            if super::selection::target(&board.scene, board.entered, item.id)
                                != item.id =>
                        {
                            intents.push(Intent::EnterGroup(item.id))
                        }
                        Some(item) if item.is_group() => intents.push(Intent::EnterGroup(item.id)),
                        Some(item) if item.note().is_some() => {
                            intents.push(Intent::EditNote(item.id))
                        }
                        Some(item) => intents.push(Intent::FocusItem(item.id)),
                        None => intents.push(Intent::AddNote(world)),
                    }
                }
                // The inline note editor floats above the canvas.
                if let Some(editing) = editing {
                    let anchor = view.camera.screen(editing.anchor);
                    let zoom = view.camera.zoom;
                    egui::Area::new(egui::Id::new("note_editor"))
                        .fixed_pos(egui::pos2(anchor[0], anchor[1]))
                        .show(ui.ctx(), |ui| {
                            let response = note_editor(ui, editing, zoom);
                            if editing.focus {
                                response.response.request_focus();
                                intents.push(Intent::NoteFocused);
                            }
                            if response.response.lost_focus() {
                                intents.push(Intent::CommitNote);
                            }
                        });
                }
            }
            if !compact
                && settings.show_minimap
                && let Some(bounds) = board.scene.bounds()
            {
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
                let world_min = view
                    .camera
                    .world([view.viewport.left(), view.viewport.top()]);
                let world_max = view
                    .camera
                    .world([view.viewport.right(), view.viewport.bottom()]);
                if let Some(p) = moth_ui::canvas::minimap(
                    ui.ctx(),
                    view.safe_area,
                    &samples,
                    egui::vec2(bounds.width, bounds.height),
                    moth_ui::canvas::Minimap {
                        size: settings.minimap_size,
                        alignment: settings.minimap_alignment(),
                        opacity: settings.minimap_opacity / 100.,
                        distance: settings.minimap_distance / 100.,
                    },
                    egui::Rect::from_min_max(
                        normalize(world_min[0], world_min[1]),
                        normalize(world_max[0], world_max[1]),
                    ),
                ) {
                    view.camera.origin = [
                        bounds.x + p.x * bounds.width - view.viewport.center().x / view.camera.zoom,
                        bounds.y + p.y * bounds.height
                            - view.viewport.center().y / view.camera.zoom,
                    ];
                }
            }
            if let Some((a, b, false)) = drag
                && moving.is_none()
            {
                ui.painter().rect_stroke(
                    egui::Rect::from_two_pos(egui::pos2(a[0], a[1]), egui::pos2(b[0], b[1])),
                    moth_ui::tokens(ui.ctx()).radius_none as f32,
                    egui::Stroke::new(1., moth_ui::tokens(ui.ctx()).accent),
                    egui::StrokeKind::Inside,
                );
            }
        });
    intents.insert(0, Intent::View(view));
    intents.extend(super::intent::intents_from_actions(actions));
    intents
}

fn selection_frame(ui: &mut egui::Ui, frame: egui::Rect, multiple: bool, show: bool) {
    if multiple && show {
        moth_ui::canvas::selection(
            ui,
            [
                frame.left_top(),
                frame.right_top(),
                frame.left_bottom(),
                frame.right_bottom(),
            ],
        );
    }
}

/// Outlines share the displayed geometry, including sway, lift and transitions.
fn outline(
    ui: &mut egui::Ui,
    camera: &Camera,
    item: &Item,
    selected: bool,
    hover: f32,
    enabled: bool,
) {
    if !enabled || (!selected && hover <= 0.) {
        return;
    }
    let opacity = ui.opacity();
    ui.multiply_opacity(item.transform.opacity);
    let corners = screen_corners(camera, item.rect, item.transform.rotation);
    if selected {
        moth_ui::canvas::selection(ui, corners);
    } else {
        ui.painter().add(egui::Shape::closed_line(
            vec![corners[0], corners[1], corners[3], corners[2]],
            egui::Stroke::new(
                1.,
                moth_ui::tokens(ui.ctx())
                    .accent
                    .gamma_multiply(hover * 0.65),
            ),
        ));
    }
    ui.set_opacity(opacity);
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
    let font_size = item.note_font_size() * zoom;
    let tokens = moth_ui::tokens(ui.ctx());
    let color = tokens.text.gamma_multiply(item.transform.opacity);
    let fill = tokens.note_fill().gamma_multiply(item.transform.opacity);
    let stroke = egui::Stroke::new(1., tokens.border.gamma_multiply(item.transform.opacity));
    let rotation = item.transform.rotation;
    let corners = item.rect.corners(rotation).map(|corner| {
        let s = camera.screen(corner);
        egui::pos2(s[0], s[1])
    });
    if rotation.abs() < 1e-4 {
        let card = egui::Rect::from_min_max(corners[0], corners[3]);
        ui.painter()
            .rect_filled(card, tokens.radius_s as f32 * zoom, fill);
        ui.painter().rect_stroke(
            card,
            tokens.radius_s as f32 * zoom,
            stroke,
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter().add(egui::Shape::convex_polygon(
            vec![corners[0], corners[1], corners[3], corners[2]],
            fill,
            stroke,
        ));
    }
    let galley = note_galley(
        ui.ctx(),
        text,
        font_size,
        item.transform.note_wrap.then_some(item.rect.width * zoom),
        color,
    );
    ui.painter().add(egui::Shape::Text(
        egui::epaint::TextShape::new(corners[0], galley, color).with_angle(rotation),
    ));
}

pub(super) fn new_note_rect(anchor: [f32; 2], zoom: f32) -> Rect {
    Rect {
        x: anchor[0],
        y: anchor[1],
        width: 200. / zoom,
        height: moth_scene::NOTE_FONT_SIZE * 1.3 / zoom,
    }
}

pub(super) fn note_font_size(rect: Rect, text: &str) -> f32 {
    rect.height / (text.lines().count().max(1) as f32 * 1.3)
}

fn note_editor(
    ui: &mut egui::Ui,
    editing: &mut Editing,
    zoom: f32,
) -> egui::text_edit::TextEditOutput {
    let font_size = editing.font_size * zoom;
    let wrap_width = editing.wrap.then_some(editing.width * zoom);
    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _: f32| {
        note_galley(
            ui.ctx(),
            text.as_str(),
            font_size,
            wrap_width,
            moth_ui::tokens(ui.ctx()).text,
        )
    };
    egui::TextEdit::multiline(&mut editing.draft)
        .font(egui::FontId::proportional(font_size))
        .layouter(&mut layouter)
        .desired_width(editing.width * zoom)
        .desired_rows(1)
        .margin(egui::Margin::ZERO)
        .show(ui)
}

fn note_galley(
    ctx: &egui::Context,
    text: &str,
    font_size: f32,
    width: Option<f32>,
    color: egui::Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_format(
        text.to_owned(),
        egui::TextFormat {
            font_id: egui::FontId::proportional(font_size),
            color,
            line_height: Some(font_size * 1.3),
            ..Default::default()
        },
    );
    job.wrap.max_width = width.unwrap_or(f32::INFINITY);
    ctx.fonts_mut(|fonts| fonts.layout_job(job))
}

pub(super) fn measure_note_frame(
    ctx: &egui::Context,
    text: &str,
    font_size: f32,
    width: Option<f32>,
) -> [f32; 2] {
    let galley = note_galley(ctx, text, font_size, width, moth_ui::tokens(ctx).text);
    [
        width.unwrap_or(galley.size().x.max(font_size * 0.6)),
        galley.size().y.max(font_size * 1.3),
    ]
}

/// The canvas viewport as a clip rectangle in physical pixels.
/// Covers the board while the full-resolution mode loads it into video memory, so the
/// user first sees the finished board rather than images appearing one by one.
pub(super) fn preload_cover(
    ui: &mut egui::Ui,
    viewport: egui::Rect,
    progress: moth_io::PreloadProgress,
) {
    // Called with the root UI after the canvas panel: the canvas images are drawn below
    // egui, so this opaque rect hides them while windows such as Settings stay above it.
    ui.painter().rect_filled(
        viewport,
        moth_ui::tokens(ui.ctx()).radius_none as f32,
        ui.visuals().panel_fill,
    );
    let width = viewport.width().clamp(1., 360.);
    egui::Area::new(egui::Id::new("preload_progress"))
        .order(egui::Order::Foreground)
        .fixed_pos(viewport.center() - egui::vec2(width / 2., 24.))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            ui.set_width(width);
            ui.vertical_centered(|ui| {
                ui.label(moth_ui::text(moth_ui::Text::PreloadProgress));
                let fraction = if progress.total == 0 {
                    0.
                } else {
                    progress.done as f32 / progress.total as f32
                };
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .text(format!("{} / {}", progress.done, progress.total)),
                );
            });
        });
}
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

// Released previews remain authoritative until the journal acknowledges the move.
fn selection_presentation(
    item: &Item,
    transition: Option<Item>,
    returning: &std::collections::HashMap<Uuid, Item>,
) -> Option<Item> {
    returning.get(&item.id).cloned().or(transition)
}

/// The same logical displacement is used for preview and the eventual command.
pub(super) fn preview_rect(
    rect: Rect,
    drag: Option<([f32; 2], [f32; 2], bool)>,
    zoom: f32,
) -> Rect {
    if let Some((a, b, false)) = drag {
        Rect {
            x: rect.x + (b[0] - a[0]) / zoom,
            y: rect.y + (b[1] - a[1]) / zoom,
            ..rect
        }
    } else {
        rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapped_note_uses_one_layout_for_editor_measurement_and_painting() {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::LIGHT);
        let text = "Reference image with several words\nРусская заметка";
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let auto = measure_note_frame(ui.ctx(), text, 24., None);
            let wrapped = measure_note_frame(ui.ctx(), text, 24., Some(100.));
            assert_eq!(wrapped[0], 100.);
            assert!(wrapped[1] > auto[1]);
            let mut item = Item::text(
                text,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 100.,
                    height: wrapped[1],
                },
                None,
            );
            item.transform.note_font_size = 24.;
            item.transform.note_wrap = true;
            let mut editing = Editing {
                id: item.id,
                original: text.into(),
                draft: text.into(),
                anchor: [0.; 2],
                font_size: 24.,
                width: 100.,
                wrap: true,
                focus: false,
            };
            let editor = note_editor(ui, &mut editing, 1.);
            assert!((editor.galley.size().y - wrapped[1]).abs() < 0.01);
            assert_eq!(
                editor.galley.rows.len(),
                note_galley(
                    ui.ctx(),
                    text,
                    24.,
                    Some(100.),
                    moth_ui::tokens(ui.ctx()).text
                )
                .rows
                .len()
            );
            draw_note(ui, &Camera::default(), &item);
            assert_eq!(item.note_font_size(), 24.);
            let larger = measure_note_frame(ui.ctx(), text, 48., Some(200.));
            assert!((larger[1] - wrapped[1] * 2.).abs() < 1.);
        });
        output.textures_delta.clear();
    }
    #[test]
    fn note_commit_at_35_percent_preserves_editor_text_height() {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::DARK);
        let zoom = 0.35;
        let rect = new_note_rect([20., 30.], zoom);
        let mut item = Item::text("", rect, None);
        item.transform.note_font_size = note_font_size(rect, "");
        let mut editing = Editing {
            id: item.id,
            original: String::new(),
            draft: "Reference\nSecond line".to_owned(),
            anchor: [rect.x, rect.y],
            font_size: item.note_font_size(),
            width: rect.width,
            wrap: false,
            focus: false,
        };
        let mut editor_height = 0.;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let editor = note_editor(ui, &mut editing, zoom);
            assert_eq!(
                editor.galley.text(),
                editing.draft,
                "the editor must lay out its actual draft"
            );
            editor_height = editor.galley.rows[0].glyphs[0].font_height;
            assert!((editing.font_size * zoom - moth_scene::NOTE_FONT_SIZE).abs() < 0.001);
            let [width, height] =
                measure_note_frame(ui.ctx(), &editing.draft, editing.font_size, None);
            item.rect.width = width;
            item.rect.height = height;
            item.content = moth_scene::Content::Text(editing.draft.clone());
            draw_note(
                ui,
                &Camera {
                    origin: [0., 0.],
                    zoom,
                },
                &item,
            );
            assert!((item.note_font_size() - editing.font_size).abs() < 0.001);
        });
        let drawn: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Reference\nSecond line" => {
                    Some(text.galley.rows[0].glyphs[0].font_height)
                }
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        assert_eq!(
            drawn.len(),
            2,
            "both the editor and committed note are painted"
        );
        for height in drawn {
            assert!((height - editor_height).abs() < 0.01);
        }
        output.textures_delta.clear();
    }

    #[test]
    fn selection_keeps_released_positions_until_storage_acknowledges() {
        for reduced in [false, true] {
            let mut scene = moth_scene::Scene::default();
            let items: Vec<_> = [10., 90.]
                .into_iter()
                .map(|x| {
                    Item::text(
                        "reference",
                        Rect {
                            x,
                            y: 20.,
                            width: 40.,
                            height: 60.,
                        },
                        None,
                    )
                })
                .collect();
            for item in &items {
                scene
                    .apply(moth_scene::Command::Insert(item.clone()))
                    .unwrap();
            }
            let commands = super::super::arrange::translate(
                &items.iter().collect::<Vec<_>>(),
                [100., 50.],
                false,
                scene.top(),
            );
            let mut motion = super::super::motion::DragMotion::default();
            motion.start([10., 20.], 140.);
            motion.release(&commands, &scene, [100., 50.], reduced);
            let now = std::time::Instant::now() + std::time::Duration::from_secs(1);
            motion.step(now, reduced, false);
            let returning = motion
                .returning(now, reduced)
                .into_iter()
                .map(|item| (item.id, item))
                .collect();
            for source in &items {
                // Both stale scene geometry and an old transition must lose to the release preview.
                let presented =
                    selection_presentation(source, Some(source.clone()), &returning).unwrap();
                assert_eq!(presented.rect.x, source.rect.x + 100.);
                assert_eq!(presented.rect.y, source.rect.y + 50.);
                assert_eq!(scene.get(source.id).unwrap().rect, source.rect);
            }
        }
    }

    #[test]
    fn view_mode_keeps_multiple_selection_visible() {
        let context = egui::Context::default();
        moth_ui::theme(&context, &moth_ui::Tokens::DARK);
        let frame = egui::Rect::from_min_size(egui::pos2(20., 30.), egui::vec2(100., 80.));
        let mut output = context.run_ui(Default::default(), |ui| {
            selection_frame(ui, frame, true, true);
        });
        output.textures_delta.clear();
        assert!(
            output
                .shapes
                .iter()
                .any(|shape| matches!(shape.shape, egui::Shape::Path(_))),
            "Tab mode must paint a selection frame"
        );
    }

    #[test]
    fn hidden_outlines_paint_neither_selection_handles_nor_hover() {
        let item = Item::text(
            "reference",
            Rect {
                x: 20.,
                y: 30.,
                width: 60.,
                height: 40.,
            },
            None,
        );
        for selected in [false, true] {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                outline(ui, &Camera::default(), &item, selected, 1., false)
            });
            output.textures_delta.clear();
            assert!(
                output
                    .shapes
                    .iter()
                    .all(|shape| matches!(shape.shape, egui::Shape::Noop))
            );
        }
    }
    #[test]
    fn selection_and_hover_follow_the_animated_image_corners() {
        let camera = Camera {
            origin: [-20., 10.],
            zoom: 2.,
        };
        let source = Item::text(
            "reference",
            Rect {
                x: 20.,
                y: 30.,
                width: 60.,
                height: 40.,
            },
            None,
        );
        let mut motion = super::super::motion::DragMotion::default();
        motion.start([20., 30.], 72.);
        let start = std::time::Instant::now();
        motion.sample(0., start);
        motion.sample(100., start + std::time::Duration::from_millis(50));
        motion.step(start + std::time::Duration::from_millis(90), false, true);
        motion.step(start + std::time::Duration::from_millis(100), false, true);
        let presented = motion.preview(
            &source,
            Some(([0., 0.], [80., 60.], false)),
            camera.zoom,
            false,
        );
        assert_ne!(presented.transform.rotation, source.transform.rotation);
        assert_ne!(presented.rect.width, source.rect.width);
        let corners = screen_corners(&camera, presented.rect, presented.transform.rotation);
        for selected in [true, false] {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                outline(ui, &camera, &presented, selected, 1., true)
            });
            output.textures_delta.clear();
            let path = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Path(path) => Some(path),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                path.points,
                [corners[0], corners[1], corners[3], corners[2]]
            );
        }
    }
    #[test]
    fn preview_preserves_source_and_matches_translation() {
        let rect = Rect {
            x: 10.,
            y: 20.,
            width: 30.,
            height: 40.,
        };
        let moved = preview_rect(rect, Some(([100., 200.], [120., 180.], false)), 2.);
        assert_eq!(
            moved,
            Rect {
                x: 20.,
                y: 10.,
                ..rect
            }
        );
        assert_eq!(rect.x, 10.);
        assert_eq!(preview_rect(rect, None, 2.), rect);
    }
    #[test]
    fn zoom_during_drag_keeps_the_grabbed_point_under_the_cursor() {
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.,
        };
        let mut camera = Camera {
            origin: [0., 0.],
            zoom: 1.,
        };
        // Grab the image at its centre, then wheel-zoom 2x around the cursor.
        let grab = [50., 50.];
        let anchor = camera.world(grab);
        let cursor = [80., 60.];
        camera.zoom_at(cursor, 2.);
        // App::rebase_drag re-projects the start through the new camera.
        let start = camera.screen(anchor);
        let moved = preview_rect(rect, Some((start, cursor, false)), camera.zoom);
        let grabbed = camera.screen([moved.x + 50., moved.y + 50.]);
        assert!((grabbed[0] - cursor[0]).abs() < 0.001);
        assert!((grabbed[1] - cursor[1]).abs() < 0.001);
    }
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
