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
];
const INSERT: &[Slot] = &[A(Open), A(Paste), A(Note)];
const VIEW: &[Slot] = &[
    A(Save),
    A(New),
    A(Open),
    A(Fit),
    A(Focus),
    A(Lock),
    A(Grayscale),
    A(Overlay),
    A(Settings),
    A(Library),
    A(Palette),
    A(Shortcuts),
];
pub(super) const CONTEXT_MENU: &[Slot] = &[
    G(Text::CategoryView, VIEW),
    G(Text::Content, TRANSFORM),
    G(Text::Arrange, ARRANGE),
    G(Text::Normalize, NORMALIZE),
    G(Text::AlignLeft, ALIGN),
    G(Text::BringFront, ORDER),
    S,
    A(Duplicate),
    A(CopyImage),
    A(SampleColor),
    A(CoordinatePicker),
    A(Export),
    S,
    A(Delete),
];
const TOOLBAR: &[Slot] = &[
    A(Select),
    A(Hand),
    S,
    G(Text::Insert, INSERT),
    A(Settings),
    G(Text::More, VIEW),
    S,
    A(Undo),
    A(Redo),
    S,
    A(Fit),
    Slot::Zoom,
];
pub(super) enum SelectionKind {
    Image,
    Several,
    Note,
}
const IMAGE_BAR: &[Slot] = &[
    A(Focus),
    A(RotateCw),
    A(FlipHorizontal),
    A(EditItem),
    A(ArrangeOptions),
];
const MULTI_BAR: &[Slot] = &[
    A(Focus),
    G(Text::Arrange, ARRANGE),
    G(Text::Normalize, NORMALIZE),
    G(Text::AlignLeft, ALIGN),
    A(Duplicate),
];
const NOTE_BAR: &[Slot] = &[A(EditNote), A(EditItem), A(Duplicate), A(Delete)];
fn selection_bar(kind: SelectionKind) -> &'static [Slot] {
    match kind {
        SelectionKind::Image => IMAGE_BAR,
        SelectionKind::Several => MULTI_BAR,
        SelectionKind::Note => NOTE_BAR,
    }
}
fn button(
    ui: &mut egui::Ui,
    action: Action,
    enabled: &[bool],
    registry: &Registry,
    icon_only: bool,
    selected: bool,
) -> bool {
    let label = text(action.label());
    ui.add_enabled_ui(
        enabled.get(action as usize).copied().unwrap_or(false),
        |ui| {
            if icon_only && let Some(icon) = action.icon() {
                return moth_ui::icons::button(
                    ui,
                    icon,
                    &format!("{label} {}", registry.label(action)),
                    selected,
                )
                .clicked();
            }
            let button = egui::Button::new(if action.icon().is_some() {
                format!("     {label}")
            } else {
                label.to_owned()
            })
            .shortcut_text(registry.label(action))
            .selected(selected);
            let response = ui.add_sized(
                [ui.available_width().min(360.), ui.spacing().interact_size.y],
                button,
            );
            if let Some(icon) = action.icon() {
                let rect = egui::Rect::from_center_size(
                    egui::pos2(response.rect.left() + 14., response.rect.center().y),
                    egui::vec2(18., 18.),
                );
                moth_ui::icons::paint(ui, rect, icon, ui.visuals().text_color());
            }
            response.clicked()
        },
    )
    .inner
}
pub(super) fn menu(
    ui: &mut egui::Ui,
    slots: &[Slot],
    enabled: &[bool],
    registry: &Registry,
    intents: &mut Vec<Intent>,
) {
    for slot in slots {
        match slot {
            A(action) => {
                if button(ui, *action, enabled, registry, false, false) {
                    intents.push(Intent::Action(*action));
                    ui.close();
                }
            }
            S => {
                ui.separator();
            }
            G(label, children) => {
                ui.menu_button(text(*label), |ui| {
                    menu(ui, children, enabled, registry, intents)
                });
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
    pan: bool,
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
                    matches!(action, Select) && !pan || matches!(action, Hand) && pan,
                ) {
                    intents.push(Intent::Action(*action));
                }
            }
            S => {
                ui.separator();
            }
            G(label, children) => {
                ui.menu_button(text(*label), |ui| {
                    menu(ui, children, enabled, registry, intents)
                });
            }
            Slot::Zoom => {
                if button(ui, ZoomOut, enabled, registry, true, false) {
                    intents.push(Intent::Action(ZoomOut));
                }
                let mut percent = zoom * 100.;
                if ui
                    .add(
                        egui::DragValue::new(&mut percent)
                            .range(1.0..=6400.)
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
pub(super) fn toolbar(
    ctx: &egui::Context,
    viewport: egui::Rect,
    enabled: &[bool],
    registry: &Registry,
    zoom: f32,
    pan: bool,
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
                    row(ui, TOOLBAR, enabled, registry, &mut intents, zoom, pan)
                });
            });
        });
    intents
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
    let position = egui::pos2(
        bounds.center().x.clamp(
            viewport.left() + 1.,
            viewport.right().max(viewport.left() + 1.),
        ),
        (bounds.top() - 8.).max(viewport.top() + 48.),
    );
    egui::Area::new(egui::Id::new("selection_bar"))
        .constrain_to(viewport)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(position)
        .show(ctx, |ui| {
            ui.set_max_width(viewport.width().max(1.));
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    row(
                        ui,
                        selection_bar(kind),
                        enabled,
                        registry,
                        &mut intents,
                        1.,
                        false,
                    );
                    ui.menu_button(text(Text::More), |ui| {
                        menu(ui, CONTEXT_MENU, enabled, registry, &mut intents)
                    });
                });
            });
        });
    intents
}
#[cfg(test)]
mod tests {
    use super::*;
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
                        toolbar(ui.ctx(), viewport, &enabled, &registry, 1., false);
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
                    toolbar(ui.ctx(), viewport, &enabled, &registry, 1., false);
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
