// SPDX-License-Identifier: GPL-3.0-or-later
//! Window chrome: the titlebar, tabs, status bar and the board catalog.
use crate::gpu::Gpu;
use moth_platform::Platform;
use winit::window::Window;

use super::{Document, Intent};

/// Read-only inputs the chrome draws from; bundled to keep the call small.
pub(super) struct Chrome<'a> {
    pub compact: bool,
    pub controls_right: bool,
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
    /// The View bar was asked for explicitly (touch, context menu); it stays
    /// until a press lands outside it.
    pub window_bar: &'a mut bool,
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
        view_bar(ui, &ctx, &tab_labels, &mut intents);
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

/// View and Over show images only. Their window bar floats at the top centre
/// and slides in when the pointer reaches the top edge, while a control in it
/// has focus or a menu is open, or after the explicit "Window controls" action
/// (context menu, palette), which is the touch path.
fn view_bar(
    ui: &mut egui::Ui,
    ctx: &Chrome<'_>,
    tabs: &[moth_ui::Tab<'_>],
    intents: &mut Vec<Intent>,
) {
    use moth_ui::{Text, text};
    let context = ui.ctx().clone();
    let tokens = moth_ui::tokens(&context);
    let id = egui::Id::new("view_window_bar");
    let content = context.content_rect();
    let gap = f32::from(tokens.space_s);
    let last = context.memory(|m| m.area_rect(id));
    let focused = context.data(|d| d.get_temp::<bool>(id.with("focus")).unwrap_or(false));
    let mut explicit = *ctx.window_bar;
    if explicit
        && !egui::Popup::is_any_open(&context)
        && context.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| last.is_none_or(|r| !r.contains(p)))
        })
    {
        explicit = false;
        intents.push(Intent::WindowBar(false));
    }
    let show = explicit || focused || reveal_window_bar(&context, last, f32::from(tokens.space_l));
    let reveal = context.animate_bool(id.with("reveal"), show);
    // Floating over the canvas, the bar never pushes the selection bar down.
    context.data_mut(|d| d.insert_temp(egui::Id::new("window_bar_bottom"), content.top()));
    if reveal <= 0. {
        return;
    }
    let height = last.map_or(f32::from(tokens.space_header), |r| r.height());
    let top = content.top() + gap * reveal - (height + gap) * (1. - reveal);
    let bar = egui::Area::new(id)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(content.center().x, top))
        .order(egui::Order::Foreground)
        .show(&context, |ui| {
            ui.set_max_width((content.width() - gap * 2.).max(ui.spacing().interact_size.x));
            ui.multiply_opacity(reveal);
            egui::Frame::popup(ui.style())
                .corner_radius(tokens.radius_panel)
                .inner_margin(f32::from(tokens.space_xs))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = f32::from(tokens.space_xs);
                        if !ctx.controls_right {
                            window_buttons(ui, ctx, intents);
                            ui.separator();
                        }
                        let grip = moth_ui::icons::button(
                            ui,
                            moth_ui::icons::Icon::Grip,
                            text(Text::MoveWindow),
                            false,
                        )
                        .interact(egui::Sense::click_and_drag());
                        if grip.drag_started() {
                            intents.push(Intent::Drag);
                        }
                        if grip.double_clicked() {
                            intents.push(Intent::Action(super::bindings::Action::Maximize));
                        }
                        // A narrow window keeps one row: the rarer controls move into "⋯".
                        let narrow = content.width() < 640.;
                        ui.menu_button(
                            format!("{} ▾", short_title(ctx.board.title(), narrow)),
                            |ui| {
                                for tab in tabs {
                                    if ui.selectable_label(tab.active, tab.title).clicked() {
                                        intents.push(Intent::Tab(tab.slot));
                                        ui.close();
                                    }
                                }
                            },
                        );
                        let mut appearance = ctx.board.appearance;
                        let view = |ui: &mut egui::Ui,
                                    appearance: &mut super::window_mode::Appearance,
                                    intents: &mut Vec<Intent>| {
                            super::window_mode::controls(
                                ui,
                                appearance,
                                moth_platform::Desktop.window_capabilities(ctx.window).blur,
                            );
                            super::surfaces::menu(
                                ui,
                                super::surfaces::VIEW,
                                ctx.enabled,
                                ctx.registry,
                                intents,
                            );
                        };
                        if narrow {
                            // Flat on purpose: nested egui submenus draw an arrow glyph
                            // our fonts lack.
                            ui.menu_button("⋯", |ui| {
                                view(ui, &mut appearance, intents);
                                ui.separator();
                                pin_choices(ui, ctx, intents);
                                super::surfaces::menu(
                                    ui,
                                    &[super::surfaces::Slot::Action(
                                        super::bindings::Action::Overlay,
                                    )],
                                    ctx.enabled,
                                    ctx.registry,
                                    intents,
                                );
                            })
                            .response
                            .on_hover_text(text(Text::More));
                        } else {
                            ui.menu_button(format!("{} ▾", text(Text::ViewAppearance)), |ui| {
                                view(ui, &mut appearance, intents)
                            });
                            ui.separator();
                            pin_menu(ui, ctx, intents);
                            if super::surfaces::button(
                                ui,
                                super::bindings::Action::Overlay,
                                ctx.enabled,
                                ctx.registry,
                                true,
                                false,
                            ) {
                                intents.push(Intent::Action(super::bindings::Action::Overlay));
                            }
                        }
                        if appearance != ctx.board.appearance {
                            intents.push(Intent::Appearance(appearance));
                        }
                        if ctx.controls_right {
                            ui.separator();
                            window_buttons(ui, ctx, intents);
                        }
                    });
                });
        });
    let rect = bar.response.rect;
    // Read the focus id first: `read_response` locks the context again.
    let has_focus = context
        .memory(|m| m.focused())
        .and_then(|f| context.read_response(f))
        .is_some_and(|r| rect.contains_rect(r.rect));
    context.data_mut(|d| d.insert_temp(id.with("focus"), has_focus));
}

fn window_buttons(ui: &mut egui::Ui, ctx: &Chrome<'_>, intents: &mut Vec<Intent>) {
    use super::bindings::Action;
    for action in super::surfaces::VIEW_WINDOW {
        let clicked = if *action == Action::Maximize && ctx.window.is_maximized() {
            moth_ui::icons::enabled_button(
                ui,
                moth_ui::icons::Icon::Restore,
                moth_ui::text(moth_ui::Text::Restore),
                false,
                ctx.enabled[*action as usize],
            )
            .clicked()
        } else {
            super::surfaces::button(ui, *action, ctx.enabled, ctx.registry, true, false)
        };
        if clicked {
            intents.push(Intent::Action(*action));
        }
    }
}

fn pin_menu(ui: &mut egui::Ui, ctx: &Chrome<'_>, intents: &mut Vec<Intent>) {
    let caps = moth_platform::Desktop.window_capabilities(ctx.window);
    ui.add_enabled_ui(caps.on_top || caps.above_window, |ui| {
        if super::surfaces::button(
            ui,
            super::bindings::Action::Pin,
            ctx.enabled,
            ctx.registry,
            true,
            ctx.pinned,
        ) {
            intents.push(Intent::Action(super::bindings::Action::Pin));
        }
        ui.menu_button("▾", |ui| pin_choices(ui, ctx, intents));
    });
}

/// Above all windows / above a chosen window, as menu items.
fn pin_choices(ui: &mut egui::Ui, ctx: &Chrome<'_>, intents: &mut Vec<Intent>) {
    use moth_ui::{Text, text};
    let caps = moth_platform::Desktop.window_capabilities(ctx.window);
    for (target, available, label) in [
        (false, caps.on_top, Text::AboveAll),
        (true, caps.above_window, Text::AboveWindow),
    ] {
        if ui
            .add_enabled(
                available,
                egui::Button::new(text(label)).selected(ctx.pinned && ctx.target_pin == target),
            )
            .clicked()
        {
            intents.push(Intent::PinChoice(target));
            ui.close();
        }
    }
}

/// Long board names would push the window buttons to a second row.
fn short_title(title: &str, narrow: bool) -> String {
    let limit = if narrow { 12 } else { 32 };
    if title.chars().count() <= limit {
        title.to_owned()
    } else {
        format!("{}…", title.chars().take(limit - 1).collect::<String>())
    }
}

fn reveal_window_bar(context: &egui::Context, bar: Option<egui::Rect>, edge: f32) -> bool {
    egui::Popup::is_any_open(context)
        || context.input(|i| {
            i.pointer.hover_pos().is_some_and(|p| {
                p.y < i.content_rect().top() + edge
                    || bar.is_some_and(|r| r.expand(edge).contains(p))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_board_titles_are_shortened_in_the_view_bar() {
        assert_eq!(short_title("Refs", true), "Refs");
        assert_eq!(short_title("Character references", true), "Character r…");
        assert_eq!(
            short_title("Персонажи и окружение", true).chars().count(),
            12
        );
        assert_eq!(
            short_title("Character references", false),
            "Character references"
        );
    }

    #[test]
    fn view_bar_reveals_at_the_top_edge_and_over_itself_without_deadlock() {
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let context = egui::Context::default();
            let bar = egui::Rect::from_min_size(egui::pos2(200., 8.), egui::vec2(400., 48.));
            for (pointer, shown, expected) in [
                (egui::pos2(100., 4.), None, true),
                (egui::pos2(100., 40.), None, false),
                (egui::pos2(300., 40.), Some(bar), true),
                (egui::pos2(300., 60.), Some(bar), true),
                (egui::pos2(100., 300.), Some(bar), false),
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
                    |_| {
                        assert_eq!(
                            reveal_window_bar(&context, shown, 16.),
                            expected,
                            "{pointer:?}"
                        )
                    },
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
