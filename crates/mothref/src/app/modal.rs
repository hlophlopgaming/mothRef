// SPDX-License-Identifier: GPL-3.0-or-later
//! Save-as dialog, close confirmation, arrangement prompt and recovery browser.

use super::{Intent, arrange, bindings::Action, workspace::Workspace};

/// Modal state owned by the application, drawn as egui popups each frame.
#[derive(Default)]
pub(super) struct Modal {
    pub(super) dialog: Option<()>,
    pub(super) dialog_path: String,
    pub(super) dialog_focus: bool,
}

impl Modal {
    /// Draws every modal (recovery, save-as, close, arrange) and returns intents.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        workspace: &Workspace,
        auto_arrange: bool,
        default_arrange: u8,
    ) -> Vec<Intent> {
        let mut intents = Vec::new();
        let board = &workspace.board;
        let recovery_paths = workspace.recovery_paths();
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
                    intents.push(Intent::Action(Action::DismissRecovery));
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
                    intents.push(Intent::DialogFocused);
                }
                let submit = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                        intents.push(Intent::Action(Action::CancelDialog));
                    }
                    if ui
                        .add_enabled(
                            !self.dialog_path.trim().is_empty(),
                            egui::Button::new(moth_ui::text(moth_ui::Text::Save)),
                        )
                        .clicked()
                        || submit
                    {
                        intents.push(Intent::Action(Action::SavePath));
                    }
                });
            });
        }
        if workspace.confirm_close && self.dialog.is_none() {
            egui::Modal::new(egui::Id::new("confirm_close")).show(ui.ctx(), |ui| {
                ui.set_max_width(380.);
                ui.heading(moth_ui::text(moth_ui::Text::SaveQuestion));
                ui.add_space(moth_ui::tokens(ui.ctx()).space_s as f32);
                ui.label(moth_ui::text(moth_ui::Text::CloseWarning));
                ui.add_space(moth_ui::tokens(ui.ctx()).space_m as f32);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(moth_ui::text(moth_ui::Text::Close))
                                    .color(moth_ui::tokens(ui.ctx()).danger),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        intents.push(Intent::Action(Action::Discard));
                    }
                    if ui.button(moth_ui::text(moth_ui::Text::Keep)).clicked() {
                        intents.push(Intent::Action(Action::KeepOpen));
                    }
                    if ui
                        .add_enabled(
                            board.in_flight == 0
                                && board.pasting == 0
                                && board.pending.is_empty()
                                && board.waiting.is_empty(),
                            egui::Button::new(
                                egui::RichText::new(moth_ui::text(moth_ui::Text::Save))
                                    .color(moth_ui::tokens(ui.ctx()).canvas),
                            )
                            .fill(moth_ui::tokens(ui.ctx()).accent),
                        )
                        .clicked()
                    {
                        intents.push(Intent::Action(Action::SaveAndClose));
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
                    ui.add_space(moth_ui::tokens(ui.ctx()).space_m as f32);
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        for (label, mode) in [
                            (moth_ui::Text::Horizontal, arrange::Arrange::Horizontal),
                            (moth_ui::Text::Vertical, arrange::Arrange::Vertical),
                            (moth_ui::Text::Grid, arrange::Arrange::Grid),
                            (moth_ui::Text::Optimal, arrange::Arrange::Optimal),
                        ] {
                            if moth_ui::icons::labeled_button(
                                ui,
                                super::tools::layout_icon(mode),
                                moth_ui::text(label),
                                false,
                            )
                            .clicked()
                            {
                                intents.push(Intent::Action(match mode {
                                    arrange::Arrange::Horizontal => Action::Horizontal,
                                    arrange::Arrange::Vertical => Action::Vertical,
                                    arrange::Arrange::Grid => Action::Grid,
                                    arrange::Arrange::Optimal => Action::Optimal,
                                }));
                            }
                        }
                        if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                            intents.push(Intent::Action(Action::CancelArrange));
                        }
                    });
                });
            }
        }
        intents
    }
}
