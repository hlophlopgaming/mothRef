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
pub fn toolbar(ui: &mut egui::Ui, status: Status<'_>, tabs: &[Tab<'_>]) -> Actions {
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
                if !compact {
                    let (logo, _) =
                        ui.allocate_exact_size(egui::vec2(14., 20.), egui::Sense::hover());
                    ui.painter()
                        .circle_filled(logo.center(), 6., crate::tokens(ui.ctx()).accent);
                    ui.label(egui::RichText::new(text(Text::Title)).monospace());
                    ui.add_space(crate::tokens(ui.ctx()).space_header as f32);
                    if status.library {
                        ui.label(
                            egui::RichText::new(text(Text::Library))
                                .color(crate::tokens(ui.ctx()).text_muted)
                                .monospace(),
                        );
                    }
                }
                if !tabs.is_empty()
                    && (!status.library
                        || status.dirty
                        || status.count > 0
                        || !status.path.is_empty()
                        || tabs.len() > 1)
                {
                    let tab_width = (ui.available_size_before_wrap().x
                        - if compact {
                            160.
                        } else {
                            7. * (ui.spacing().interact_size.x.max(36.) + 4.) + 44.
                        })
                    .max(0.);
                    egui::ScrollArea::horizontal()
                        .max_width(tab_width)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for tab in tabs {
                                    let label = if tab.dirty {
                                        format!("{}  •", tab.title)
                                    } else {
                                        tab.title.to_owned()
                                    };
                                    let response =
                                        ui.selectable_label(tab.active && !status.library, label);
                                    let emphasis = ui.ctx().animate_bool_with_time(
                                        response.id.with("emphasis"),
                                        response.hovered() || (tab.active && !status.library),
                                        ui.style().animation_time,
                                    );
                                    if emphasis > 0. {
                                        ui.painter().line_segment(
                                            [response.rect.left_top(), response.rect.right_top()],
                                            egui::Stroke::new(
                                                2.,
                                                crate::tokens(ui.ctx())
                                                    .accent
                                                    .gamma_multiply(emphasis),
                                            ),
                                        );
                                    }
                                    if response.clicked() {
                                        a.tab = Some(tab.slot);
                                    }
                                    if icons::button(
                                        ui,
                                        Icon::Close,
                                        &tooltip(ui.ctx(), Text::CloseTab),
                                        false,
                                    )
                                    .clicked()
                                    {
                                        a.close_tab = Some(tab.slot);
                                    }
                                }
                            });
                        });
                }
                a.settings = icons::button(ui, Icon::List, text(Text::Settings), false).clicked();
                a.new = icons::button(ui, Icon::Plus, &tooltip(ui.ctx(), Text::NewBoard), false)
                    .clicked();
                if compact {
                    ui.end_row();
                }
                let controls_width = ((if compact { 6. } else { 7. })
                    * (ui.spacing().interact_size.x.max(36.) + ui.spacing().item_spacing.x))
                    .min(ui.max_rect().width());
                ui.allocate_ui_with_layout(
                    egui::vec2(
                        ui.available_size_before_wrap().x.max(controls_width),
                        ui.spacing().interact_size.y,
                    ),
                    egui::Layout::right_to_left(egui::Align::Center).with_main_wrap(true),
                    |ui| {
                        let id = ui.id().with("window_controls");
                        let mut explicit = ui
                            .ctx()
                            .data_mut(|d| d.get_temp::<bool>(id).unwrap_or(false));
                        let reveal =
                            icons::button(ui, Icon::Maximize, text(Text::WindowControls), explicit);
                        if reveal.clicked() {
                            explicit = !explicit;
                            ui.ctx().data_mut(|d| d.insert_temp(id, explicit));
                        }
                        let bar_bottom = ui
                            .ctx()
                            .data(|d| d.get_temp::<f32>(egui::Id::new("window_bar_bottom")))
                            .unwrap_or(ui.min_rect().top() + 56.);
                        let show = explicit
                            || reveal.has_focus()
                            || ui.input(|i| {
                                i.pointer
                                    .hover_pos()
                                    .is_some_and(|p| p.y < bar_bottom + 24.)
                                    || i.key_pressed(egui::Key::Tab)
                            })
                            || ui.memory(|m| m.focused().is_some())
                            || egui::Popup::is_any_open(ui.ctx());
                        ui.scope(|ui| {
                            if !show {
                                ui.set_invisible();
                            }
                            a.close = icons::button(
                                ui,
                                Icon::Close,
                                &tooltip(ui.ctx(), Text::CloseWindow),
                                false,
                            )
                            .clicked();
                            {
                                a.maximize = icons::button(
                                    ui,
                                    Icon::Maximize,
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
                                a.minimize = icons::button(
                                    ui,
                                    Icon::Minus,
                                    &tooltip(ui.ctx(), Text::Minimize),
                                    false,
                                )
                                .clicked();
                            }
                            ui.add_enabled_ui(status.can_pin, |ui| {
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
                                ui.menu_button("▾", |ui| {
                                    if ui
                                        .selectable_label(!status.target_pin, text(Text::AboveAll))
                                        .clicked()
                                    {
                                        a.pin_choice = Some(false);
                                        ui.close();
                                    }
                                    if ui
                                        .add_enabled(
                                            status.can_attach,
                                            egui::Button::new(text(Text::AboveWindow)),
                                        )
                                        .clicked()
                                    {
                                        a.pin_choice = Some(true);
                                        ui.close();
                                    }
                                })
                                .response
                                .on_hover_text(text(Text::OnTop));
                            });
                        });
                        let drag = ui.allocate_response(
                            ui.available_size_before_wrap(),
                            egui::Sense::drag(),
                        );
                        a.drag = drag.drag_started();
                        a.maximize |= drag.double_clicked();
                    },
                );
            });
        });
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            egui::Id::new("window_bar_bottom"),
            brand.response.rect.bottom(),
        )
    });
    if !status.library {
        egui::Area::new(egui::Id::new("save_chip"))
            .fixed_pos(egui::pos2(
                ui.max_rect().left() + 12.,
                ui.max_rect().top() + 12.,
            ))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(format!(
                        "{} · {} {}",
                        text(if status.dirty || status.path.is_empty() {
                            Text::UnsavedChip
                        } else {
                            Text::SaveState
                        }),
                        status.count,
                        text(Text::Images)
                    ));
                    if !status.message.is_empty() {
                        ui.label(status.message);
                    }
                });
            });
    }

    a
}
