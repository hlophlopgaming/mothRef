// SPDX-License-Identifier: GPL-3.0-or-later
//! Save-as dialog, close confirmation, arrangement prompt and recovery browser.
use std::path::PathBuf;

use super::{Intent, Session, arrange};

/// Modal state owned by the application, drawn as egui popups each frame.
#[derive(Default)]
pub(super) struct Modal {
    pub(super) dialog: Option<()>,
    pub(super) dialog_path: String,
    pub(super) dialog_focus: bool,
    pub(super) confirm_close: bool,
    pub(super) closing_window: bool,
}

impl Modal {
    /// Draws every modal (recovery, save-as, close, arrange) and returns intents.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        board: &Session,
        recovery_paths: &[PathBuf],
        auto_arrange: bool,
        default_arrange: u8,
        intents: &mut Vec<Intent>,
    ) {
        if !recovery_paths.is_empty() && board.scene.is_empty() {
            egui::Window::new(moth_ui::text(moth_ui::Text::Recovery)).show(ui.ctx(), |ui| {
                for path in recovery_paths {
                    if ui
                        .add_enabled(
                            board.in_flight == 0,
                            egui::Button::new(path.display().to_string()),
                        )
                        .clicked()
                    {
                        intents.push(Intent::OpenPath(path.clone()));
                    }
                }
                if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                    intents.push(Intent::DismissRecovery);
                }
            });
        }
        if self.dialog.is_some() {
            egui::Modal::new(egui::Id::new("board_path")).show(ui.ctx(), |ui| {
                ui.set_width(ui.ctx().content_rect().width().min(460.) - 40.);
                ui.heading(moth_ui::text(moth_ui::Text::PathDialog));
                ui.label(moth_ui::text(moth_ui::Text::Path));
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.dialog_path)
                        .char_limit(4096)
                        .desired_width(f32::INFINITY),
                );
                if self.dialog_focus {
                    input.request_focus();
                    self.dialog_focus = false;
                }
                let submit = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                        intents.push(Intent::CancelDialog);
                    }
                    if ui
                        .add_enabled(
                            !self.dialog_path.trim().is_empty(),
                            egui::Button::new(moth_ui::text(moth_ui::Text::Save)),
                        )
                        .clicked()
                        || submit
                    {
                        let mut path = std::path::PathBuf::from(&self.dialog_path);
                        if path.extension().is_none() {
                            path.set_extension("moth");
                        }
                        intents.push(Intent::SaveAs(path));
                    }
                });
            });
        }
        if self.confirm_close && self.dialog.is_none() {
            egui::Modal::new(egui::Id::new("confirm_close")).show(ui.ctx(), |ui| {
                ui.set_max_width(380.);
                ui.heading(moth_ui::text(moth_ui::Text::SaveQuestion));
                ui.add_space(8.);
                ui.label(moth_ui::text(moth_ui::Text::CloseWarning));
                ui.add_space(12.);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(moth_ui::text(moth_ui::Text::Close))
                                    .color(egui::Color32::from_rgb(209, 133, 123)),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        intents.push(Intent::Discard);
                    }
                    if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                        intents.push(Intent::KeepOpen);
                    }
                    if ui
                        .add_enabled(
                            board.in_flight == 0
                                && board.pasting == 0
                                && board.pending.is_empty()
                                && board.waiting.is_empty(),
                            egui::Button::new(
                                egui::RichText::new(moth_ui::text(moth_ui::Text::Save))
                                    .color(moth_ui::BACKGROUND),
                            )
                            .fill(moth_ui::ACCENT),
                        )
                        .clicked()
                    {
                        intents.push(Intent::SaveAndClose);
                    }
                });
            });
        }
        if let Some(ids) = board.arrange_prompt.clone() {
            if auto_arrange {
                intents.push(Intent::Arrange(
                    super::tools::layout(default_arrange).1,
                    ids.clone(),
                ));
                intents.push(Intent::DismissArrangePrompt);
            } else {
                egui::Modal::new(egui::Id::new("arrange_prompt")).show(ui.ctx(), |ui| {
                    ui.set_max_width(380.);
                    ui.heading(moth_ui::text(moth_ui::Text::ArrangePrompt));
                    ui.add_space(12.);
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        for (label, mode) in [
                            (moth_ui::Text::Horizontal, arrange::Arrange::Horizontal),
                            (moth_ui::Text::Vertical, arrange::Arrange::Vertical),
                            (moth_ui::Text::Grid, arrange::Arrange::Grid),
                            (moth_ui::Text::Optimal, arrange::Arrange::Optimal),
                        ] {
                            if ui.button(moth_ui::text(label)).clicked() {
                                intents.push(Intent::Arrange(mode, ids.clone()));
                                intents.push(Intent::DismissArrangePrompt);
                            }
                        }
                        if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                            intents.push(Intent::DismissArrangePrompt);
                        }
                    });
                });
            }
        }
    }
}
