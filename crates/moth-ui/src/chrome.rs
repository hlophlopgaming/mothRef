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
    /// Clipboard import.
    pub paste: bool,
    /// Frame content.
    pub fit: bool,
    /// Zoom multiplier, zero when unchanged.
    pub zoom: f32,
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
    /// Allocated texture pages.
    pub texture_bytes: u64,
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
}
/// Draws a custom titlebar and the active workspace's toolbar.
pub fn toolbar(ui: &mut egui::Ui, status: Status<'_>, pan: &mut bool, tabs: &[Tab<'_>]) -> Actions {
    if ui.ctx().content_rect().width() < 600. {
        ui.spacing_mut().interact_size = egui::vec2(44., 44.);
    }
    let mut a = Actions::default();
    egui::Panel::top("brand")
        .frame(
            egui::Frame::new()
                .fill(BACKGROUND)
                .inner_margin(egui::Margin::symmetric(12, 0)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.;
                let (logo, _) = ui.allocate_exact_size(egui::vec2(14., 20.), egui::Sense::hover());
                ui.painter().circle_filled(logo.center(), 6., ACCENT);
                ui.label(egui::RichText::new(text(Text::Title)).monospace());
                ui.add_space(14.);
                if status.library {
                    ui.label(
                        egui::RichText::new(text(Text::Library))
                            .color(MUTED)
                            .monospace(),
                    );
                }
                if !status.library
                    || status.dirty
                    || status.count > 0
                    || !status.path.is_empty()
                    || tabs.len() > 1
                {
                    let tab_width = (ui.available_width()
                        - if ui.ctx().content_rect().width() < 600. {
                            95.
                        } else {
                            180.
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
                                    if tab.active && !status.library {
                                        ui.painter().line_segment(
                                            [response.rect.left_top(), response.rect.right_top()],
                                            egui::Stroke::new(2., ACCENT),
                                        );
                                    }
                                    if response.clicked() {
                                        a.tab = Some(tab.slot);
                                    }
                                    if icons::button(ui, Icon::Close, text(Text::CloseTab), false)
                                        .clicked()
                                    {
                                        a.close_tab = Some(tab.slot);
                                    }
                                }
                            });
                        });
                }
                a.new = icons::button(ui, Icon::Plus, text(Text::NewBoard), false).clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    a.close =
                        icons::button(ui, Icon::Close, text(Text::CloseWindow), false).clicked();
                    if ui.ctx().content_rect().width() >= 600. {
                        a.maximize = icons::button(ui, Icon::Maximize, text(Text::Maximize), false)
                            .clicked();
                        a.minimize =
                            icons::button(ui, Icon::Minus, text(Text::Minimize), false).clicked();
                    }
                    let drag = ui.allocate_response(ui.available_size(), egui::Sense::drag());
                    a.drag = drag.drag_started();
                    a.maximize |= drag.double_clicked();
                });
            });
        });
    egui::Panel::bottom("status")
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(16, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(status.adapter)
                        .monospace()
                        .size(10.)
                        .color(MUTED),
                );
                ui.add_space(16.);
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}  ·  {} {}  ·  {:.1} MiB",
                        status.count,
                        text(Text::Items),
                        status.selected,
                        text(Text::Selected),
                        status.texture_bytes as f64 / 1048576.
                    ))
                    .monospace()
                    .size(10.)
                    .color(MUTED),
                );
                if !status.message.is_empty() {
                    ui.colored_label(ACCENT, status.message);
                }
            });
        });
    if status.library {
        return a;
    }
    egui::Panel::top("toolbar")
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(12, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                a.home = icons::button(ui, Icon::Home, text(Text::Library), false).clicked();
                ui.separator();
                ui.add_enabled_ui(status.idle, |ui| {
                    a.undo = icons::button(ui, Icon::Undo, text(Text::Undo), false).clicked();
                    a.redo = icons::button(ui, Icon::Redo, text(Text::Redo), false).clicked();
                });
                ui.separator();
                ui.selectable_value(pan, false, text(Text::Select));
                ui.selectable_value(pan, true, text(Text::Pan));
                a.paste = ui.button(text(Text::PasteImage)).clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    a.save = ui
                        .add_enabled(
                            status.idle,
                            egui::Button::new(
                                egui::RichText::new(text(Text::Save)).color(BACKGROUND),
                            )
                            .fill(ACCENT),
                        )
                        .clicked();
                    ui.add_enabled_ui(status.can_pin, |ui| {
                        a.pin = icons::button(ui, Icon::Pin, text(Text::OnTop), status.pinned)
                            .clicked();
                    });
                    a.overlay =
                        icons::button(ui, Icon::Overlay, text(Text::Overlay), false).clicked();
                    a.fit = icons::button(ui, Icon::Fit, text(Text::Fit), false).clicked();
                    if ui.button("+").clicked() {
                        a.zoom = 1.25;
                    }
                    ui.monospace(format!("{:.0}%", status.zoom * 100.));
                    if ui.button("−").clicked() {
                        a.zoom = 0.8;
                    }
                    if ui.available_width() > 100. {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(if status.path.is_empty() {
                                    text(Text::NotSaved)
                                } else {
                                    status.path
                                })
                                .monospace()
                                .size(11.)
                                .color(MUTED),
                            )
                            .truncate(),
                        );
                    }
                });
            });
        });
    a
}
