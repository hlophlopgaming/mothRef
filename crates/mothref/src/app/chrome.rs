// SPDX-License-Identifier: GPL-3.0-or-later
//! Window chrome: the titlebar, tabs, status bar and the board catalog.
use crate::gpu::Gpu;
use moth_platform::Platform;
use winit::window::Window;

use super::{Document, Intent};

/// Read-only inputs the chrome draws from; bundled to keep the call small.
pub(super) struct Chrome<'a> {
    pub compact: bool,
    pub board: &'a mut Document,
    pub tabs: &'a [Document],
    pub gpu: &'a Gpu,
    pub library: bool,
    pub pinned: bool,
    pub target_pin: bool,
    pub zoom_increment: f32,
    pub catalog: &'a mut moth_ui::library::Library,
    pub cards: &'a [moth_ui::library::Card],
    pub window: &'a Window,
    pub enabled: &'a [bool],
    pub registry: &'a super::bindings::Registry,
}

/// Draws the titlebar, tab strip, status bar and (in library mode) the catalog.
pub(super) fn chrome(ui: &mut egui::Ui, ctx: Chrome<'_>) -> Vec<Intent> {
    let mut intents = Vec::new();
    let active_title = ctx.board.title().to_owned();
    let mut ordered = vec![(
        ctx.board.ui_id(),
        moth_ui::Tab {
            title: &active_title,
            dirty: ctx.board.dirty,
            active: true,
            slot: 0,
        },
    )];
    ordered.extend(ctx.tabs.iter().enumerate().map(|(i, tab)| {
        (
            tab.ui_id(),
            moth_ui::Tab {
                title: tab.title(),
                dirty: tab.dirty,
                active: false,
                slot: i + 1,
            },
        )
    }));
    ordered.sort_unstable_by_key(|(id, _)| *id);
    let tab_labels: Vec<_> = ordered.into_iter().map(|(_, tab)| tab).collect();
    if ctx.compact {
        use super::bindings::Action;
        use moth_ui::{Text, text};
        let context = ui.ctx().clone();
        let tokens = moth_ui::tokens(&context);
        let id = egui::Id::new("view_window_bar");
        let mut explicit = context.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
        let handle = egui::Area::new(id.with("handle"))
            .fixed_pos(context.content_rect().left_top())
            .order(egui::Order::Foreground)
            .show(&context, |ui| {
                egui::Frame::new()
                    .fill(tokens.surface)
                    .inner_margin(tokens.space_s as i8)
                    .show(ui, |ui| {
                        let button = moth_ui::icons::button(
                            ui,
                            moth_ui::icons::Icon::Maximize,
                            text(Text::WindowControls),
                            explicit,
                        )
                        .interact(egui::Sense::click_and_drag());
                        if button.clicked() {
                            explicit = !explicit;
                        }
                        if button.drag_started() {
                            intents.push(Intent::Drag);
                        }
                    });
            });
        let height = context
            .data(|d| d.get_temp::<f32>(id.with("height")))
            .unwrap_or(tokens.space_header as f32 * 2.);
        let show = reveal_window_bar(
            &context,
            explicit,
            height.max(tokens.space_header as f32 * 2.),
        );
        let reveal = context.animate_bool(id.with("reveal"), show);
        if reveal > 0. {
            let position = context.content_rect().left_top()
                + egui::vec2(handle.response.rect.width(), -height * (1. - reveal));
            let bar = egui::Area::new(id)
                .fixed_pos(position)
                .order(egui::Order::Foreground)
                .show(&context, |ui| {
                    ui.set_max_width(
                        (context.content_rect().width() - handle.response.rect.width())
                            .max(ui.spacing().interact_size.x),
                    );
                    egui::Frame::new()
                        .fill(tokens.surface)
                        .inner_margin(tokens.space_s as i8)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.menu_button(ctx.board.title(), |ui| {
                                    for tab in &tab_labels {
                                        if ui.selectable_label(tab.active, tab.title).clicked() {
                                            intents.push(Intent::Tab(tab.slot));
                                            ui.close();
                                        }
                                    }
                                });
                                let mut appearance = ctx.board.appearance;
                                ui.menu_button(text(Text::ViewAppearance), |ui| {
                                    super::window_mode::controls(
                                        ui,
                                        &mut appearance,
                                        moth_platform::Desktop.window_capabilities(ctx.window).blur,
                                    );
                                    super::surfaces::menu(
                                        ui,
                                        super::surfaces::VIEW,
                                        ctx.enabled,
                                        ctx.registry,
                                        &mut intents,
                                    );
                                });
                                if appearance != ctx.board.appearance {
                                    intents.push(Intent::Appearance(appearance));
                                }
                                for action in super::surfaces::VIEW_WINDOW {
                                    if super::surfaces::button(
                                        ui,
                                        *action,
                                        ctx.enabled,
                                        ctx.registry,
                                        true,
                                        *action == Action::Pin && ctx.pinned,
                                    ) {
                                        intents.push(Intent::Action(*action));
                                    }
                                }
                                let drag = window_drag_area(ui);
                                if drag.drag_started() {
                                    intents.push(Intent::Drag);
                                }
                                if drag.double_clicked() {
                                    intents.push(Intent::Action(Action::Maximize));
                                }
                            });
                        });
                });
            context.data_mut(|d| d.insert_temp(id.with("height"), bar.response.rect.height()));
        }
        context.data_mut(|d| d.insert_temp(id, explicit));
        return intents;
    }
    let mut actions = moth_ui::toolbar(
        ui,
        moth_ui::Status {
            path: &ctx.board.path,
            adapter: &ctx.gpu.adapter,
            count: ctx.board.scene.len(),
            images: ctx.board.image_count,
            selected: ctx.board.selection.len(),
            zoom: ctx.board.view.camera.zoom,
            zoom_increment: ctx.zoom_increment,
            texture_bytes: ctx.gpu.registry.allocated_bytes(),
            texture_budget: ctx.gpu.registry.budget_bytes(),
            message: &ctx.board.message.render(moth_ui::locale()),
            idle: ctx.board.idle(),
            dirty: ctx.board.dirty,
            library: ctx.library,
            pinned: ctx.pinned,
            target_pin: ctx.target_pin,
            maximized: ctx.window.is_maximized(),
            can_attach: moth_platform::Desktop
                .window_capabilities(ctx.window)
                .above_window,
            can_pin: moth_platform::Desktop
                .window_capabilities(ctx.window)
                .on_top,
        },
        &tab_labels,
        |ui| super::surfaces::window_menu(ui, ctx.enabled, ctx.registry, &mut intents),
    );
    let opacity = ui.ctx().animate_bool_with_time(
        egui::Id::new("library_visibility"),
        ctx.library,
        ui.style().animation_time * 1.4,
    );
    if ctx.library {
        let previous_opacity = ui.opacity();
        ui.multiply_opacity(opacity);
        let intent = ctx.catalog.show(ui, ctx.cards);
        ui.set_opacity(previous_opacity);
        actions.new |= intent.new;
        actions.open |= intent.browse;
        if let Some(index) = intent.open {
            intents.push(Intent::OpenPath(ctx.cards[index].path.clone()));
        }
        if let Some((index, tags)) = intent.tags {
            intents.push(Intent::Library(moth_platform::library::Request::Tags(
                ctx.cards[index].path.clone(),
                tags,
            )));
        }
        if let Some(index) = intent.pin {
            let card = &ctx.cards[index];
            intents.push(Intent::Library(moth_platform::library::Request::Pin(
                card.path.clone(),
                !card.pinned,
            )));
        }
    }
    intents.extend(super::intent::intents_from_actions(actions));
    intents
}

fn window_drag_area(ui: &mut egui::Ui) -> egui::Response {
    ui.allocate_response(
        egui::vec2(
            ui.available_size_before_wrap().x,
            ui.spacing().interact_size.y,
        ),
        egui::Sense::click_and_drag(),
    )
}

fn reveal_window_bar(context: &egui::Context, explicit: bool, height: f32) -> bool {
    explicit
        || egui::Popup::is_any_open(context)
        || context.memory(|m| m.focused().is_some())
        || context.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|p| p.y < i.content_rect().top() + height)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_drag_target_uses_the_free_space_on_the_same_row() {
        let context = egui::Context::default();
        let mut output = context.run_ui(Default::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let button = ui.button("Window controls");
                    let drag = window_drag_area(ui);
                    assert!(drag.rect.left() >= button.rect.right());
                    assert_eq!(drag.rect.top(), button.rect.top());
                });
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn view_bar_hover_does_not_deadlock_the_frame() {
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let context = egui::Context::default();
            for (pointer, explicit, expected) in [
                (egui::pos2(100., 10.), false, true),
                (egui::pos2(100., 50.), false, true),
                (egui::pos2(100., 200.), false, false),
                (egui::pos2(100., 200.), true, true),
            ] {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        events: vec![egui::Event::PointerMoved(pointer)],
                        ..Default::default()
                    },
                    |_| assert_eq!(reveal_window_bar(&context, explicit, 64.), expected),
                );
                output.textures_delta.clear();
            }
            let _ = send.send(());
        });
        receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("View bar hover must finish a frame without recursively locking egui");
    }
}
