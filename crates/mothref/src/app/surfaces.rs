// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered surfaces share labels, shortcuts, availability and effects.
use super::{
    Intent,
    bindings::{Action, Registry},
};
use Action::*;
use moth_ui::{Text, text};
pub(super) enum Slot {
    Action(Action),
    Separator,
    Group(Text, &'static [Slot]),
    Zoom,
}
use Slot::{Action as A, Group as G, Separator as S};
pub(super) const ARRANGE: &[Slot] = &[
    A(Horizontal),
    A(Vertical),
    A(Grid),
    A(Optimal),
    A(PackFit),
    S,
    A(ArrangeName),
    A(ArrangeTime),
    A(ArrangeOptions),
];
const NORMALIZE: &[Slot] = &[
    A(NormalizeWidth),
    A(NormalizeHeight),
    A(NormalizeSize),
    A(NormalizeArea),
    A(ResetScale),
    A(NormalizeScale),
];
const SAMPLING: &[Slot] = &[
    A(SamplingDefault),
    A(SamplingSmooth),
    A(SamplingNearest),
    A(SamplingAutomatic),
];
const TRANSFORM: &[Slot] = &[
    A(RotateCw),
    A(RotateCcw),
    A(FlipHorizontal),
    A(FlipVertical),
    A(ResetRotation),
    A(ResetFlip),
    A(ResetCrop),
    A(ResetTransform),
    A(EditItem),
];
const ORDER: &[Slot] = &[A(BringFront), A(SendBack), A(BringForward), A(SendBackward)];
const ALIGN: &[Slot] = &[
    A(AlignLeft),
    A(AlignRight),
    A(AlignTop),
    A(AlignBottom),
    A(DistributeHorizontal),
    A(DistributeVertical),
    A(Stack),
];
pub(super) const EXPORT: &[Slot] = &[
    A(ExportScene),
    A(ExportSelection),
    A(ExportOriginal),
    A(ExportOriginals),
    A(ExportZip),
    A(CopyImage),
    A(OpenOriginal),
];
const INSERT: &[Slot] = &[A(Images), A(Paste), A(Note)];
const FILE: &[Slot] = &[A(New), A(Open), A(Save), A(Library)];
pub(super) const VIEW_WINDOW: &[Action] = &[Minimize, Maximize, CloseWindow];
const WINDOW_MODES: &[Slot] = &[
    A(Bottom),
    A(LockWindow),
    A(ClickThrough),
    A(OverlaySelection),
    A(ResizeSelection),
    A(ResizeAll),
    A(OpacityUp),
    A(OpacityDown),
];
pub(super) const VIEW: &[Slot] = &[
    A(DrawingSession),
    A(WindowBar),
    A(Layers),
    A(ToggleView),
    A(Fit),
    A(Focus),
    A(Lock),
    A(Grayscale),
    A(Overlay),
    G(Text::WindowSettings, WINDOW_MODES),
    A(Shortcuts),
    A(Palette),
];
pub(super) const CONTEXT_MENU: &[Slot] = &[
    G(Text::CategoryView, VIEW),
    G(Text::Content, TRANSFORM),
    G(Text::ImageSampling, SAMPLING),
    G(Text::Arrange, ARRANGE),
    G(Text::Normalize, NORMALIZE),
    G(Text::AlignGroup, ALIGN),
    G(Text::OrderGroup, ORDER),
    S,
    A(SelectAll),
    A(Group),
    A(Ungroup),
    A(EnterGroup),
    A(RenameGroup),
    G(Text::GroupColor, GROUP_COLOR),
    S,
    A(Duplicate),
    A(CopyImage),
    A(OpenOriginal),
    A(SampleColor),
    A(CoordinatePicker),
    A(Export),
    S,
    A(Delete),
];
const TOOLBAR: &[Slot] = &[
    A(Select),
    S,
    G(Text::Insert, INSERT),
    S,
    A(Undo),
    A(Redo),
    S,
    A(Fit),
    Slot::Zoom,
    G(Text::CategoryView, VIEW),
];
pub(super) enum SelectionKind {
    Image,
    Several,
    Note,
    /// One group, with the number of its direct members.
    Group(usize),
}
const IMAGE_BAR: &[Slot] = &[
    G(Text::ImageSampling, SAMPLING),
    A(Focus),
    A(RotateCw),
    A(FlipHorizontal),
    A(EditItem),
    A(ArrangeOptions),
];
const MULTI_BAR: &[Slot] = &[
    A(Group),
    A(DrawingSession),
    A(Focus),
    G(Text::Arrange, ARRANGE),
    G(Text::Normalize, NORMALIZE),
    G(Text::AlignGroup, ALIGN),
    A(Duplicate),
];
const NOTE_BAR: &[Slot] = &[A(EditNote), A(EditItem), A(Duplicate), A(Delete)];
pub(super) const GROUP_COLOR: &[Slot] = &[
    A(GroupNeutral),
    A(GroupSage),
    A(GroupAmber),
    A(GroupSky),
    A(GroupWing),
    A(GroupLilac),
];
const GROUP_BAR: &[Slot] = &[
    A(DrawingSession),
    A(EnterGroup),
    A(Ungroup),
    G(Text::GroupColor, GROUP_COLOR),
];
fn selection_bar(kind: SelectionKind) -> &'static [Slot] {
    match kind {
        SelectionKind::Image => IMAGE_BAR,
        SelectionKind::Several => MULTI_BAR,
        SelectionKind::Note => NOTE_BAR,
        SelectionKind::Group(_) => GROUP_BAR,
    }
}
pub(super) fn button(
    ui: &mut egui::Ui,
    action: Action,
    enabled: &[bool],
    registry: &Registry,
    icon_only: bool,
    selected: bool,
) -> bool {
    let label = text(action.label());
    let enabled = enabled.get(action as usize).copied().unwrap_or(false);
    if icon_only && let Some(icon) = action.icon() {
        return moth_ui::icons::enabled_button(
            ui,
            icon,
            &format!("{label} {}", registry.label(action)),
            selected,
            enabled,
        )
        .clicked();
    }
    if icon_only {
        // Bars carry words without an icon slot or a shortcut; the tooltip has both.
        let shortcut = registry.label(action);
        let response = ui.add_enabled(enabled, egui::Button::new(label).selected(selected));
        let response = if shortcut.is_empty() {
            response
        } else {
            response.on_hover_text(format!("{label}  {shortcut}"))
        };
        return response.clicked();
    }
    let icon_id = ui.next_auto_id().with("menu_icon");
    let icon_size = egui::Vec2::splat(MENU_ICON);
    let button = egui::Button::new((egui::Atom::custom(icon_id, icon_size), label))
        .shortcut_text(registry.label(action))
        .selected(selected)
        .min_size(egui::vec2(0., ui.spacing().interact_size.y));
    let result = ui.add_enabled_ui(enabled, |ui| {
        let result = button.atom_ui(ui);
        if let (Some(icon), Some(rect)) = (action.icon(), result.rect(icon_id)) {
            moth_ui::icons::paint(ui, rect, icon, ui.visuals().text_color());
        }
        result.response
    });
    let response = result.inner;
    response.clicked()
}
/// Icon column of menus: one size for every row, so labels line up.
const MENU_ICON: f32 = 16.;
/// Menus are lists, not toolbars: rows a little taller than the text.
fn compact(ui: &mut egui::Ui) {
    let spacing = ui.spacing_mut();
    spacing.interact_size.y = 28.;
    spacing.button_padding.y = 4.;
    spacing.item_spacing.y = 1.;
}
/// Whether a slot has anything the user can run now.
fn available(slot: &Slot, enabled: &[bool]) -> bool {
    match slot {
        A(action) => enabled.get(*action as usize).copied().unwrap_or(false),
        G(_, children) => children.iter().any(|child| available(child, enabled)),
        S | Slot::Zoom => false,
    }
}
/// A context menu: only what applies to the click, without empty separators.
pub(super) fn context(
    ui: &mut egui::Ui,
    slots: &[Slot],
    enabled: &[bool],
    registry: &Registry,
    intents: &mut Vec<Intent>,
) {
    items(ui, slots, enabled, registry, intents, true);
}
/// A menu that shows every entry, greyed out when unavailable, so it can be learned.
pub(super) fn menu(
    ui: &mut egui::Ui,
    slots: &[Slot],
    enabled: &[bool],
    registry: &Registry,
    intents: &mut Vec<Intent>,
) {
    items(ui, slots, enabled, registry, intents, false);
}
fn items(
    ui: &mut egui::Ui,
    slots: &[Slot],
    enabled: &[bool],
    registry: &Registry,
    intents: &mut Vec<Intent>,
    only_available: bool,
) {
    compact(ui);
    let shown: Vec<&Slot> = if only_available {
        // Drop unavailable entries, then separators that no longer separate anything.
        let kept: Vec<&Slot> = slots
            .iter()
            .filter(|slot| matches!(slot, S) || available(slot, enabled))
            .collect();
        let mut out: Vec<&Slot> = Vec::new();
        for slot in kept {
            if matches!(slot, S) && out.last().is_none_or(|last| matches!(last, S)) {
                continue;
            }
            out.push(slot);
        }
        while out.last().is_some_and(|last| matches!(last, S)) {
            out.pop();
        }
        out
    } else {
        slots.iter().collect()
    };
    for slot in shown {
        match slot {
            A(action) => {
                if button(ui, *action, enabled, registry, false, false) {
                    intents.push(Intent::Action(*action));
                    if egui::containers::menu::is_in_menu(ui) {
                        ui.close();
                    }
                }
            }
            S => {
                ui.separator();
            }
            G(label, children) => {
                let button = egui::Button::new((
                    egui::Atom::custom(
                        ui.next_auto_id().with("group_icon"),
                        egui::Vec2::splat(MENU_ICON),
                    ),
                    text(*label),
                ))
                .right_text(egui::Atom::custom(
                    ui.next_auto_id().with("submenu_arrow"),
                    egui::Vec2::splat(MENU_ICON),
                ));
                let (response, _) = egui::containers::menu::SubMenuButton::from_button(button)
                    .ui(ui, |ui| {
                        items(ui, children, enabled, registry, intents, only_available)
                    });
                let size = MENU_ICON - 4.;
                let rect = egui::Rect::from_center_size(
                    egui::pos2(
                        response.rect.right() - ui.spacing().button_padding.x - size / 2.,
                        response.rect.center().y,
                    ),
                    egui::Vec2::splat(size),
                );
                moth_ui::icons::paint(
                    ui,
                    rect,
                    moth_ui::icons::Icon::ChevronRight,
                    ui.visuals().text_color(),
                );
            }
            Slot::Zoom => {}
        }
    }
}
fn row(
    ui: &mut egui::Ui,
    slots: &[Slot],
    enabled: &[bool],
    registry: &Registry,
    intents: &mut Vec<Intent>,
    zoom: f32,
) {
    for slot in slots {
        match slot {
            A(action) => {
                if button(
                    ui,
                    *action,
                    enabled,
                    registry,
                    true,
                    matches!(action, Select),
                ) {
                    intents.push(Intent::Action(*action));
                }
            }
            S => {
                ui.separator();
            }
            G(label, children) => {
                if *label == Text::CategoryView {
                    let response = moth_ui::icons::button(
                        ui,
                        moth_ui::icons::Icon::Compass,
                        text(*label),
                        false,
                    );
                    egui::Popup::menu(&response).show(|ui| {
                        menu(ui, children, enabled, registry, intents);
                    });
                } else {
                    ui.menu_button(format!("{} ▾", text(*label)), |ui| {
                        menu(ui, children, enabled, registry, intents)
                    });
                }
            }
            Slot::Zoom => {
                if button(ui, ZoomOut, enabled, registry, true, false) {
                    intents.push(Intent::Action(ZoomOut));
                }
                let mut percent = zoom * 100.;
                // Whole percents in a field as wide as the widest value, so the toolbar
                // keeps its width while zooming.
                let font = egui::TextStyle::Button.resolve(ui.style());
                let color = ui.visuals().text_color();
                let widest = ui
                    .fonts_mut(|fonts| fonts.layout_no_wrap("6400%".into(), font, color))
                    .size()
                    .x;
                let size = egui::vec2(
                    widest + ui.spacing().button_padding.x * 2.,
                    ui.spacing().interact_size.y,
                );
                if ui
                    .add_sized(
                        size,
                        egui::DragValue::new(&mut percent)
                            .range(1.0..=6400.)
                            .max_decimals(0)
                            .suffix("%"),
                    )
                    .changed()
                    && percent.is_finite()
                {
                    intents.push(Intent::ZoomTo(percent / 100.));
                }
                if button(ui, ZoomIn, enabled, registry, true, false) {
                    intents.push(Intent::Action(ZoomIn));
                }
            }
        }
    }
}
pub(super) fn window_menu(
    ui: &mut egui::Ui,
    enabled: &[bool],
    registry: &Registry,
    intents: &mut Vec<Intent>,
) {
    ui.menu_button(text(Text::Title), |ui| {
        menu(ui, FILE, enabled, registry, intents);
        ui.separator();
        menu(ui, &[A(Settings)], enabled, registry, intents);
    });
}

pub(super) fn toolbar(
    ctx: &egui::Context,
    viewport: egui::Rect,
    enabled: &[bool],
    registry: &Registry,
    zoom: f32,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    egui::Area::new(egui::Id::new("bottom_toolbar"))
        .constrain_to(viewport)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(viewport.center_bottom() - egui::vec2(0., 12.))
        .show(ctx, |ui| {
            ui.set_max_width((viewport.width() - 24.).max(1.));
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    row(ui, TOOLBAR, enabled, registry, &mut intents, zoom);
                });
            });
        });
    intents
}
fn selection_position(
    bounds: egui::Rect,
    viewport: egui::Rect,
    size: egui::Vec2,
    gap: f32,
) -> egui::Pos2 {
    // Above the selection, else below it, else (a selection filling the window) at
    // the top of the free area, so the bar never sits on the toolbar.
    let y = if bounds.top() - gap - size.y >= viewport.top() {
        bounds.top() - gap - size.y
    } else if bounds.bottom() + gap + size.y <= viewport.bottom() {
        bounds.bottom() + gap
    } else {
        viewport.top() + gap
    };
    egui::pos2(
        (bounds.center().x - size.x / 2.).clamp(
            viewport.left(),
            (viewport.right() - size.x).max(viewport.left()),
        ),
        y.clamp(
            viewport.top(),
            (viewport.bottom() - size.y).max(viewport.top()),
        ),
    )
}

pub(super) fn selection(
    ctx: &egui::Context,
    bounds: egui::Rect,
    viewport: egui::Rect,
    kind: SelectionKind,
    enabled: &[bool],
    registry: &Registry,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    let gap = moth_ui::tokens(ctx).space_s as f32;
    let top = ctx
        .data(|data| data.get_temp::<f32>(egui::Id::new("window_bar_bottom")))
        .unwrap_or(viewport.top());
    let bottom = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("bottom_toolbar")))
        .map_or(viewport.bottom(), |toolbar| toolbar.top() - gap);
    let viewport = egui::Rect::from_min_max(
        egui::pos2(viewport.left(), top.max(viewport.top())),
        egui::pos2(viewport.right(), bottom.max(top.max(viewport.top()))),
    );
    let size = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("selection_bar")))
        .map_or(
            egui::vec2(
                viewport.width(),
                ctx.style_of(ctx.theme()).spacing.interact_size.y + gap * 2.,
            ),
            |rect| rect.size(),
        )
        .min(viewport.size());
    let position = selection_position(bounds, viewport, size, gap);
    egui::Area::new(egui::Id::new("selection_bar"))
        .constrain_to(viewport)
        .pivot(egui::Align2::LEFT_TOP)
        .fixed_pos(position)
        .show(ctx, |ui| {
            ui.set_max_width(viewport.width().max(1.));
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if let SelectionKind::Group(count) = kind {
                        ui.weak(format!("{} · {count}", text(Text::GroupLabel)));
                    }
                    row(ui, selection_bar(kind), enabled, registry, &mut intents, 1.);
                    ui.menu_button(text(Text::More), |ui| {
                        context(ui, CONTEXT_MENU, enabled, registry, &mut intents)
                    });
                });
            });
        });
    intents
}
pub(super) fn added(
    ctx: &egui::Context,
    viewport: egui::Rect,
    added: &super::document::Added,
    enabled: &[bool],
    registry: &Registry,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    let gap = moth_ui::tokens(ctx).space_m as f32;
    let bottom = ctx.memory(|memory| {
        ["bottom_toolbar", "selection_badge"]
            .into_iter()
            .filter_map(|id| memory.area_rect(egui::Id::new(id)))
            .map(|rect| rect.top())
            .fold(viewport.bottom(), f32::min)
    });
    egui::Area::new(egui::Id::new("added_feedback"))
        .order(egui::Order::Foreground)
        .constrain_to(viewport)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(viewport.center().x, bottom - gap))
        .show(ctx, |ui| {
            ui.set_max_width((viewport.width() - gap * 2.).max(1.));
            let tokens = moth_ui::tokens(ui.ctx());
            egui::Frame::popup(ui.style())
                .corner_radius(tokens.radius_l)
                .inner_margin(egui::Margin {
                    left: tokens.space_m as i8,
                    right: tokens.space_xs as i8,
                    top: tokens.space_xs as i8,
                    bottom: tokens.space_xs as i8,
                })
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let (check, _) = ui.allocate_exact_size(
                            egui::Vec2::splat(MENU_ICON),
                            egui::Sense::hover(),
                        );
                        moth_ui::icons::paint(
                            ui,
                            check,
                            moth_ui::icons::Icon::Check,
                            tokens.accent_text,
                        );
                        ui.label(
                            moth_ui::Message::Count(Text::AddedImages, added.ids.len())
                                .render(moth_ui::locale()),
                        );
                        let (label, _) = super::tools::layout(added.mode as u8);
                        ui.menu_button(format!("{} ▾", text(label)), |ui| {
                            menu(
                                ui,
                                &[
                                    A(AddedHorizontal),
                                    A(AddedVertical),
                                    A(AddedGrid),
                                    A(AddedOptimal),
                                ],
                                enabled,
                                registry,
                                &mut intents,
                            );
                        });
                        // Undo reads as a link-like action in the accent, as in the design.
                        let undo = ui.add_enabled(
                            enabled.get(UndoAdded as usize).copied().unwrap_or(false),
                            egui::Button::new(
                                egui::RichText::new(text(UndoAdded.label()))
                                    .strong()
                                    .color(tokens.accent_text),
                            )
                            .frame(false),
                        );
                        if undo.clicked() {
                            intents.push(Intent::Action(UndoAdded));
                        }
                        if button(ui, DismissAdded, enabled, registry, true, false) {
                            intents.push(Intent::Action(DismissAdded));
                        }
                    });
                });
        });
    intents
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_menu_replaces_top_level_file_and_view_buttons() {
        let context = egui::Context::default();
        moth_ui::theme(&context, &moth_ui::Tokens::DARK);
        let registry = Registry::default();
        let enabled = vec![true; super::super::bindings::ACTIONS.len()];
        let mut output = context.run_ui(Default::default(), |ui| {
            window_menu(ui, &enabled, &registry, &mut Vec::new());
        });
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(shape) => Some(shape.galley.text()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&text(Text::Title)));
        assert!(!labels.contains(&text(Text::FileMenu)));
        assert!(!labels.contains(&text(Text::CategoryView)));
        assert!(
            TOOLBAR
                .iter()
                .any(|slot| matches!(slot, G(Text::CategoryView, _)))
        );
        assert_eq!(&VIEW_WINDOW[..3], &[Minimize, Maximize, CloseWindow]);
        assert!(!VIEW_WINDOW.contains(&MoveWindow));
        output.textures_delta.clear();
    }

    #[test]
    fn editor_action_rows_leave_their_window_open_for_feedback() {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::LIGHT);
        let registry = Registry::default();
        let enabled = vec![true; super::super::bindings::ACTIONS.len()];
        let mut intents = Vec::new();
        let mut expanded = false;
        let mut draw = |events| {
            ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    expanded = egui::Window::new("export regression")
                        .fixed_pos(egui::pos2(100., 100.))
                        .show(ui.ctx(), |ui| {
                            menu(ui, &[A(ExportScene)], &enabled, &registry, &mut intents)
                        })
                        .is_some_and(|window| window.inner.is_some());
                },
            )
        };
        for _ in 0..3 {
            draw(Vec::new()).textures_delta.clear();
        }
        let mut output = draw(Vec::new());
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text() == text(ExportScene.label()) => {
                    Some(t.pos + t.galley.size() / 2.)
                }
                _ => None,
            })
            .unwrap();
        output.textures_delta.clear();
        for pressed in [true, false] {
            draw(vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ])
            .textures_delta
            .clear();
        }
        draw(Vec::new()).textures_delta.clear();
        assert!(expanded);
        assert!(
            intents
                .iter()
                .any(|intent| matches!(intent, Intent::Action(ExportScene)))
        );
    }

    #[test]
    fn selection_bar_fits_above_or_below_without_crossing_window_edges() {
        let viewport = egui::Rect::from_min_max(egui::pos2(0., 60.), egui::pos2(900., 600.));
        let size = egui::vec2(360., 48.);
        let bounds = egui::Rect::from_min_max(egui::pos2(800., 200.), egui::pos2(880., 300.));
        let position = selection_position(bounds, viewport, size, 8.);
        assert_eq!(position, egui::pos2(540., 144.));
        let near_top = bounds.translate(egui::vec2(-790., -130.));
        let position = selection_position(near_top, viewport, size, 8.);
        assert_eq!(position, egui::pos2(0., near_top.bottom() + 8.));
        assert!(viewport.contains_rect(egui::Rect::from_min_size(position, size)));
        // A selection filling the window puts the bar at the top of the free area.
        let position = selection_position(viewport.expand(20.), viewport, size, 8.);
        assert_eq!(position.y, viewport.top() + 8.);
    }

    #[test]
    fn menu_icons_reserve_their_own_column_at_each_scale() {
        for scale in [0.75, 1., 2.] {
            let ctx = egui::Context::default();
            moth_ui::theme(&ctx, &moth_ui::Tokens::DARK);
            ctx.set_zoom_factor(scale);
            let enabled = vec![true; super::super::bindings::ACTIONS.len()];
            let registry = Registry::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.vertical(|ui| {
                        button(ui, Save, &enabled, &registry, false, false);
                        button(ui, CopyImage, &enabled, &registry, false, false);
                    });
                });
            });
            for label in [text(Save.label()), text(CopyImage.label())] {
                let text_shape = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == label => Some(text),
                        _ => None,
                    })
                    .unwrap();
                assert!(text_shape.pos.x >= f32::from(moth_ui::Tokens::DARK.space_m) + 18.);
            }
            output.textures_delta.clear();
        }
    }
    #[test]
    fn bottom_toolbar_stays_inside_touch_and_desktop_viewports() {
        let registry = Registry::default();
        let enabled = vec![true; super::super::bindings::ACTIONS.len()];
        for size in [
            egui::vec2(320., 640.),
            egui::vec2(390., 844.),
            egui::vec2(844., 390.),
            egui::vec2(1440., 900.),
        ] {
            let ctx = egui::Context::default();
            moth_ui::theme(&ctx, &moth_ui::Tokens::DARK);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            for _ in 0..3 {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(viewport),
                        ..Default::default()
                    },
                    |ui| {
                        toolbar(ui.ctx(), viewport, &enabled, &registry, 1.);
                    },
                )
                .textures_delta
                .clear();
            }
            let rect = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("bottom_toolbar")))
                .expect("toolbar owns a visible area");
            assert!(rect.width() <= viewport.width(), "{size:?}: {rect:?}");
            assert!(rect.height() < viewport.height() / 2., "{size:?}: {rect:?}");
        }
    }
    #[test]
    fn idle_toolbar_stops_repainting() {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::DARK);
        let registry = Registry::default();
        let enabled = vec![true; super::super::bindings::ACTIONS.len()];
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440., 900.));
        let mut delay = std::time::Duration::ZERO;
        for frame in 0..30 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(viewport),
                    time: Some(frame as f64 / 60.),
                    ..Default::default()
                },
                |ui| {
                    toolbar(ui.ctx(), viewport, &enabled, &registry, 1.);
                },
            );
            delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            output.textures_delta.clear();
        }
        assert!(delay >= std::time::Duration::from_secs(1), "{delay:?}");
    }
    #[test]
    fn surfaces_reference_unique_registered_actions() {
        fn walk(slots: &[Slot]) {
            for slot in slots {
                match slot {
                    A(action) => assert!(super::super::bindings::ACTIONS.contains(action)),
                    G(_, children) => walk(children),
                    _ => {}
                }
            }
        }
        for slots in [
            TOOLBAR,
            CONTEXT_MENU,
            IMAGE_BAR,
            MULTI_BAR,
            NOTE_BAR,
            EXPORT,
        ] {
            walk(slots);
        }
    }
}
