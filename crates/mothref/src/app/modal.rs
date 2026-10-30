// SPDX-License-Identifier: GPL-3.0-or-later
//! Save-as dialog, close confirmation, arrangement prompt and recovery browser.

use super::{Intent, arrange, bindings::Action, workspace::Workspace};

/// Modal state owned by the application, drawn as egui popups each frame.
#[derive(Default)]
pub(super) struct Modal {
    pub(super) dialog: Option<()>,
    pub(super) dialog_path: String,
    pub(super) dialog_focus: bool,
    pub(super) recovery: usize,
}

impl Modal {
    /// Draws every modal (recovery, save-as, close, arrange) and returns intents.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        workspace: &Workspace,
        settings: &super::settings::Settings,
        enabled: &[bool],
        registry: &super::bindings::Registry,
    ) -> Vec<Intent> {
        let mut intents = Vec::new();
        let board = &workspace.board;
        let recovery_paths = workspace.recovery_paths();
        if workspace.library && !recovery_paths.is_empty() && board.scene.is_empty() {
            self.recovery = self.recovery.min(recovery_paths.len() - 1);
            // A card in the corner, not a modal: the library stays usable, and each
            // board left in a journal can be reopened or thrown away on its own row.
            let tokens = moth_ui::tokens(ui.ctx());
            egui::Area::new(egui::Id::new("recovery"))
                .order(egui::Order::Foreground)
                .anchor(
                    egui::Align2::RIGHT_BOTTOM,
                    egui::vec2(-(tokens.space_l as f32), -(tokens.space_l as f32)),
                )
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .corner_radius(tokens.radius_l)
                        .inner_margin(egui::Margin::same(tokens.space_m as i8))
                        .show(ui, |ui| {
                            ui.set_width(tokens.space_xl as f32 * 13.);
                            moth_ui::icons::dialog_title(
                                ui,
                                moth_ui::text(moth_ui::Text::Recovery),
                            );
                            ui.add_space(tokens.space_xs as f32);
                            egui::ScrollArea::vertical()
                                .max_height(ui.ctx().content_rect().height() / 3.)
                                .show(ui, |ui| {
                                    for (index, entry) in recovery_paths.iter().enumerate() {
                                        let age =
                                            entry.modified.elapsed().unwrap_or_default().as_secs();
                                        let (key, count) = if age < 60 {
                                            (moth_ui::Text::ChangedNow, 0)
                                        } else if age < 3600 {
                                            (moth_ui::Text::ChangedMinutes, age / 60)
                                        } else if age < 86400 {
                                            (moth_ui::Text::ChangedHours, age / 3600)
                                        } else {
                                            (moth_ui::Text::ChangedDays, age / 86400)
                                        };
                                        let changed = moth_ui::Message::Count(key, count as usize)
                                            .render(moth_ui::locale());
                                        let title = entry
                                            .title
                                            .as_deref()
                                            .unwrap_or(moth_ui::text(moth_ui::Text::Untitled));
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 0.;
                                                ui.label(title);
                                                ui.label(
                                                    egui::RichText::new(changed)
                                                        .small()
                                                        .color(tokens.text_muted),
                                                );
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.add_enabled_ui(board.idle(), |ui| {
                                                        if moth_ui::icons::button(
                                                            ui,
                                                            moth_ui::icons::Icon::Close,
                                                            moth_ui::text(
                                                                moth_ui::Text::DiscardRecovery,
                                                            ),
                                                            false,
                                                        )
                                                        .clicked()
                                                        {
                                                            self.recovery = index;
                                                            intents.push(Intent::Action(
                                                                Action::DismissRecovery,
                                                            ));
                                                        }
                                                        if ui
                                                            .button(moth_ui::text(
                                                                moth_ui::Text::RestoreRecovery,
                                                            ))
                                                            .clicked()
                                                        {
                                                            self.recovery = index;
                                                            intents.push(Intent::Action(
                                                                Action::RestoreRecovery,
                                                            ));
                                                        }
                                                    });
                                                },
                                            );
                                        });
                                    }
                                });
                        });
                });
        }

        if board.conversion_prompt && self.dialog.is_none() && !workspace.confirm_close {
            egui::Modal::new(egui::Id::new("conversion")).show(ui.ctx(), |ui| {
                moth_ui::icons::dialog_title(ui, moth_ui::text(moth_ui::Text::ConversionTitle));
                conversion_report(ui, board);
                ui.horizontal_wrapped(|ui| {
                    for (label, action) in [
                        (moth_ui::Text::SaveAsMoth, Action::SaveConversion),
                        (
                            moth_ui::Text::ContinueConversion,
                            Action::ContinueConversion,
                        ),
                    ] {
                        if ui.button(moth_ui::text(label)).clicked() {
                            intents.push(Intent::Action(action));
                        }
                    }
                });
            });
        }
        if self.dialog.is_some() {
            egui::Modal::new(egui::Id::new("board_path")).show(ui.ctx(), |ui| {
                ui.set_width(ui.ctx().content_rect().width().min(460.) - 40.);
                moth_ui::icons::dialog_title(ui, moth_ui::text(moth_ui::Text::PathDialog));
                conversion_report(ui, board);
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
                moth_ui::icons::dialog_title(ui, moth_ui::text(moth_ui::Text::SaveQuestion));
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
                                    .color(moth_ui::tokens(ui.ctx()).on_accent),
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
            if settings.auto_arrange || ids.len() <= settings.threshold {
                intents.push(Intent::Arrange(
                    super::tools::layout(settings.default_arrange).1,
                    ids.clone(),
                ));
                intents.push(Intent::DismissArrangePrompt);
            } else {
                let tokens = moth_ui::tokens(ui.ctx());
                let modal = egui::Modal::new(egui::Id::new("arrange_prompt"))
                    .frame(
                        egui::Frame::popup(&ui.ctx().global_style())
                            .corner_radius(tokens.radius_l)
                            .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
                    )
                    .show(ui.ctx(), |ui| {
                        ui.set_max_width(tokens.space_xl as f32 * 15.6);
                        moth_ui::icons::dialog_title(
                            ui,
                            moth_ui::text(moth_ui::Text::ArrangePrompt),
                        );
                        ui.label(
                            egui::RichText::new(moth_ui::item_counts(
                                moth_ui::locale(),
                                ids.len(),
                                0,
                            ))
                            .color(tokens.text_muted),
                        );
                        ui.add_space(tokens.space_m as f32);
                        ui.horizontal_wrapped(|ui| {
                            for (label, mode) in [
                                (moth_ui::Text::Horizontal, arrange::Arrange::Horizontal),
                                (moth_ui::Text::Vertical, arrange::Arrange::Vertical),
                                (moth_ui::Text::Grid, arrange::Arrange::Grid),
                                (moth_ui::Text::Optimal, arrange::Arrange::Optimal),
                            ] {
                                if moth_ui::icons::tile(
                                    ui,
                                    super::tools::layout_icon(mode),
                                    moth_ui::text(label),
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
                        });
                        ui.add_space(tokens.space_m as f32);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(moth_ui::text(moth_ui::Text::Cancel)).clicked() {
                                intents.push(Intent::Action(Action::CancelArrange));
                            }
                        });
                    });
                // Esc or a click beside the dialog means "leave them as they are".
                if modal.should_close() {
                    intents.push(Intent::Action(Action::CancelArrange));
                }
            }
        }
        if let Some(added) = &board.added {
            intents.extend(super::surfaces::added(
                ui.ctx(),
                board.view.viewport,
                added,
                enabled,
                registry,
            ));
        }
        intents
    }
}

fn conversion_report(ui: &mut egui::Ui, board: &super::document::Document) {
    if let Some((source, skipped)) = &board.conversion {
        ui.label(moth_ui::text(moth_ui::Text::ConversionRetained));
        ui.label(source.to_string_lossy());
        ui.label(moth_ui::text(
            if source
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pur"))
            {
                moth_ui::Text::ConversionPur
            } else {
                moth_ui::Text::ConversionBee
            },
        ));
        ui.label(
            moth_ui::Message::Count(moth_ui::Text::ImportSkippedCount, *skipped)
                .render(moth_ui::locale()),
        );
    }
}
