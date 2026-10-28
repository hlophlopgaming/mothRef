// SPDX-License-Identifier: GPL-3.0-or-later
//! Board chrome reports intent; all persistence stays with the application.
use super::{
    icons::{self, Icon},
    *,
};
/// Tab labels borrow session metadata instead of duplicating board state.
pub struct Tab<'a> {
    /// User-visible file stem.
    pub title: &'a str,
    /// A journaled change still needs an explicit file save.
    pub dirty: bool,
    /// Active state is independent of creation order.
    pub active: bool,
    /// Application session slot returned unchanged on activation.
    pub slot: usize,
}
/// Window and board controls share the same action path as shortcuts.
#[derive(Default)]
pub struct Actions {
    /// Open application preferences.
    pub settings: bool,
    /// Clipboard import.
    pub paste: bool,
    /// Frame content.
    pub fit: bool,
    /// Zoom multiplier, zero when unchanged.
    pub zoom: f32,
    /// Exact scale entered in the percentage field.
    pub zoom_to: Option<f32>,
    /// Return to the catalog without closing a board.
    pub home: bool,
    /// Create a bounded new session.
    pub new: bool,
    /// Open the path dialog.
    pub open: bool,
    /// Save through the storage worker.
    pub save: bool,
    /// Journal undo.
    pub undo: bool,
    /// Journal redo.
    pub redo: bool,
    /// Toggle transparent viewing.
    pub overlay: bool,
    /// Toggle native topmost support.
    pub pin: bool,
    /// Choose all windows (false) or select a target (true).
    pub pin_choice: Option<bool>,
    /// Activate an inactive session.
    pub tab: Option<usize>,
    /// Request safe closure of a session (zero is the active board).
    pub close_tab: Option<usize>,
    /// Close the window after all sessions are handled.
    pub close: bool,
    /// Native titlebar interaction.
    pub drag: bool,
    /// Native minimize.
    pub minimize: bool,
    /// Native maximize toggle.
    pub maximize: bool,
    /// User picked a different interface language.
    pub language: Option<Locale>,
}
/// Read-only status keeps UI independent of filesystem and GPU ownership.
pub struct Status<'a> {
    /// Current board path for a centered breadcrumb.
    pub path: &'a str,
    /// Renderer diagnostics.
    pub adapter: &'a str,
    /// Live item count.
    pub count: usize,
    /// Image count excludes notes.
    pub images: usize,
    /// Selected item count.
    pub selected: usize,
    /// Camera scale.
    pub zoom: f32,
    /// Percent zoom per toolbar step, shared with wheel preferences.
    pub zoom_increment: f32,
    /// Allocated texture pages.
    pub texture_bytes: u64,
    /// Adaptive image-cache ceiling for the active board.
    pub texture_budget: u64,
    /// Background worker feedback.
    pub message: &'a str,
    /// Avoid enqueueing conflicting persistence actions.
    pub idle: bool,
    /// Active session has unsaved edits.
    pub dirty: bool,
    /// The catalog is visible.
    pub library: bool,
    /// Reflect native topmost state.
    pub pinned: bool,
    /// Native topmost capability.
    pub can_pin: bool,
    /// Native target selection capability.
    pub can_attach: bool,
    /// Last chosen pin mode.
    pub target_pin: bool,
    /// Maximize uses a restore label while active.
    pub maximized: bool,
}
/// Draws a custom titlebar and the active workspace's toolbar.
pub fn toolbar(
    ui: &mut egui::Ui,
    status: Status<'_>,
    tabs: &[Tab<'_>],
    menus: impl FnOnce(&mut egui::Ui),
) -> Actions {
    let compact = ui.ctx().content_rect().width() < 600.;
    if compact {
        ui.spacing_mut().interact_size = egui::vec2(44., 44.);
    }
    let mut a = Actions::default();
    let brand = egui::Panel::top("brand")
        .frame(
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(egui::Margin::symmetric(
                    crate::tokens(ui.ctx()).space_m as i8,
                    crate::tokens(ui.ctx()).space_none as i8,
                )),
        )
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = crate::tokens(ui.ctx()).space_xs as f32;
                let (logo, _) = ui.allocate_exact_size(egui::vec2(14., 20.), egui::Sense::hover());
                ui.painter()
                    .circle_filled(logo.center(), 6., crate::tokens(ui.ctx()).accent);
                menus(ui);
                if !compact && status.library {
                    ui.label(
                        egui::RichText::new(text(Text::Library))
                            .color(crate::tokens(ui.ctx()).text_muted)
                            .monospace(),
                    );
                }
                let summary = format!(
                    "{} · {}",
                    text(if status.dirty || status.path.is_empty() {
                        Text::UnsavedChip
                    } else {
                        Text::SaveState
                    }),
                    item_counts(
                        locale(),
                        status.images,
                        status.count.saturating_sub(status.images)
                    )
                );
                let status_font = egui::TextStyle::Body.resolve(ui.style());
                let color = crate::tokens(ui.ctx()).text_muted;
                let status_width = ui
                    .fonts_mut(|fonts| {
                        fonts.layout_no_wrap(summary.clone(), status_font.clone(), color)
                    })
                    .size()
                    .x;
                // Below this the summary would crowd the tabs; it stays in the tooltip.
                let show_summary = !compact && status_width * 2.5 < ui.max_rect().width();
                let status_width = if show_summary { status_width } else { 0. };
                if !tabs.is_empty()
                    && (!status.library
                        || status.dirty
                        || status.count > 0
                        || !status.path.is_empty()
                        || tabs.len() > 1)
                {
                    let tab_width = (ui.available_size_before_wrap().x
                        - if status.library {
                            0.
                        } else {
                            status_width + ui.spacing().item_spacing.x
                        }
                        - if compact {
                            0.
                        } else {
                            // The window controls plus a little room to drag the window.
                            6. * (ui.spacing().interact_size.x.max(36.)
                                + ui.spacing().item_spacing.x)
                                + f32::from(crate::tokens(ui.ctx()).space_xl) * 2.
                        })
                    .max(0.);
                    egui::ScrollArea::horizontal()
                        .max_width(tab_width)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for tab in tabs {
                                    if let Some(action) = tab_chip(ui, tab, status.library) {
                                        match action {
                                            TabAction::Open => a.tab = Some(tab.slot),
                                            TabAction::Close => a.close_tab = Some(tab.slot),
                                        }
                                    }
                                }
                            });
                        });
                }
                if !status.library && show_summary {
                    let response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(summary).font(status_font).color(color),
                        )
                        .extend()
                        .sense(egui::Sense::click()),
                    );
                    if !status.message.is_empty() {
                        response.clone().on_hover_text(status.message);
                        egui::Popup::menu(&response).show(|ui| {
                            ui.label(status.message);
                        });
                    }
                }
                if compact {
                    ui.end_row();
                }
                window_controls(ui, &status, &mut a);
            });
        });
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new("window_bar_bottom"),
            brand.response.rect.bottom(),
        );
        d.insert_temp(
            egui::Id::new("collect_window_controls").with("bar"),
            brand.response.rect,
        );
    });

    a
}

/// Pin, minimize, maximize and close stay out of the way until the pointer
/// reaches the bar, a control has keyboard focus, or a tap on the bar asks for
/// them (touch has no hover). They keep their space while hidden, so the bar
/// never jumps; the free space beside them drags the window.
fn window_controls(ui: &mut egui::Ui, status: &Status<'_>, a: &mut Actions) {
    let size = ui.spacing().interact_size.x.max(36.) + ui.spacing().item_spacing.x;
    let width = (6. * size).min(ui.max_rect().width());
    ui.allocate_ui_with_layout(
        egui::vec2(
            ui.available_size_before_wrap().x.max(width),
            ui.spacing().interact_size.y,
        ),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            let id = egui::Id::new("collect_window_controls");
            let tapped = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
            let bar = ui
                .ctx()
                .data(|d| d.get_temp::<egui::Rect>(id.with("bar")))
                .unwrap_or(ui.max_rect());
            let focused = ui
                .ctx()
                .data(|d| d.get_temp::<bool>(id.with("focus")))
                .unwrap_or(false);
            // Resolve tokens outside `input`: reading them locks the context again.
            let near = bar.expand(f32::from(tokens(ui.ctx()).space_s));
            let show = tapped
                || focused
                || egui::Popup::is_any_open(ui.ctx())
                || ui.input(|i| {
                    i.key_pressed(egui::Key::Tab)
                        || i.pointer.hover_pos().is_some_and(|p| near.contains(p))
                });
            let reveal = ui.ctx().animate_bool(id.with("reveal"), show);
            let group = ui.scope(|ui| {
                if reveal <= 0. {
                    ui.set_invisible();
                } else {
                    ui.multiply_opacity(reveal);
                }
                a.close = icons::button(
                    ui,
                    Icon::Close,
                    &tooltip(ui.ctx(), Text::CloseWindow),
                    false,
                )
                .clicked();
                a.maximize = icons::button(
                    ui,
                    if status.maximized {
                        Icon::Restore
                    } else {
                        Icon::Maximize
                    },
                    &tooltip(
                        ui.ctx(),
                        if status.maximized {
                            Text::Restore
                        } else {
                            Text::Maximize
                        },
                    ),
                    false,
                )
                .clicked();
                a.minimize =
                    icons::button(ui, Icon::Minus, &tooltip(ui.ctx(), Text::Minimize), false)
                        .clicked();
                ui.separator();
                pin_controls(ui, status, a);
            });
            // Read the focus id first: `read_response` locks the context again.
            let has_focus = ui
                .ctx()
                .memory(|m| m.focused())
                .and_then(|f| ui.ctx().read_response(f))
                .is_some_and(|r| group.response.rect.contains_rect(r.rect));
            let drag = ui.allocate_response(
                ui.available_size_before_wrap(),
                egui::Sense::click_and_drag(),
            );
            a.drag = drag.drag_started();
            a.maximize |= drag.double_clicked();
            let mut tapped = tapped;
            if drag.clicked() {
                tapped = !tapped;
            } else if tapped
                && ui.input(|i| i.pointer.any_pressed())
                && !ui.rect_contains_pointer(group.response.rect.union(drag.rect))
            {
                tapped = false;
            }
            ui.ctx().data_mut(|d| {
                d.insert_temp(id, tapped);
                d.insert_temp(id.with("focus"), has_focus);
            });
        },
    );
}

fn pin_controls(ui: &mut egui::Ui, status: &Status<'_>, a: &mut Actions) {
    ui.add_enabled_ui(status.can_pin, |ui| {
        ui.menu_button("▾", |ui| {
            if ui
                .selectable_label(status.pinned && !status.target_pin, text(Text::AboveAll))
                .clicked()
            {
                a.pin_choice = Some(false);
                ui.close();
            }
            if ui
                .add_enabled(
                    status.can_attach,
                    egui::Button::new(text(Text::AboveWindow))
                        .selected(status.pinned && status.target_pin),
                )
                .clicked()
            {
                a.pin_choice = Some(true);
                ui.close();
            }
        })
        .response
        .on_hover_text(text(Text::OnTop));
        a.pin = icons::button(
            ui,
            Icon::Pin,
            &tooltip(
                ui.ctx(),
                if status.target_pin {
                    Text::AboveWindow
                } else {
                    Text::AboveAll
                },
            ),
            status.pinned,
        )
        .clicked();
    });
}

enum TabAction {
    Open,
    Close,
}

/// One rounded chip per board: the title opens it, the small cross inside closes it.
fn tab_chip(ui: &mut egui::Ui, tab: &Tab<'_>, library: bool) -> Option<TabAction> {
    let t = tokens(ui.ctx());
    let active = tab.active && !library;
    let height = ui.spacing().interact_size.y;
    let title = if tab.dirty {
        format!("{}  •", tab.title)
    } else {
        tab.title.to_owned()
    };
    let mut action = None;
    let chip = egui::Frame::new()
        .fill(if active { t.selection } else { t.surface })
        .stroke(egui::Stroke::new(
            1.,
            if active { t.accent } else { t.border },
        ))
        .corner_radius(t.radius_s)
        .inner_margin(egui::Margin {
            left: t.space_m as i8,
            right: t.space_xs as i8,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_min_height(height - 4.);
            ui.spacing_mut().item_spacing.x = f32::from(t.space_xs);
            let label = ui.add(
                egui::Label::new(egui::RichText::new(title).color(t.text))
                    .selectable(false)
                    .sense(egui::Sense::click()),
            );
            if label.clicked() {
                action = Some(TabAction::Open);
            }
            let side = f32::from(t.space_l) + f32::from(t.space_xs) * 2.;
            let close = ui
                .add(
                    egui::Button::new("")
                        .frame(false)
                        .min_size(egui::Vec2::splat(side)),
                )
                .on_hover_text(tooltip(ui.ctx(), Text::CloseTab));
            let color = if close.hovered() {
                t.text
            } else {
                t.text_muted
            };
            icons::paint(
                ui,
                close.rect.shrink(f32::from(t.space_xs) / 2.),
                Icon::Close,
                color,
            );
            if close.clicked() {
                action = Some(TabAction::Close);
            }
        });
    let chip = chip.response.interact(egui::Sense::click());
    if chip.clicked() && action.is_none() {
        action = Some(TabAction::Open);
    }
    action
}
