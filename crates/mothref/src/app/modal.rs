// SPDX-License-Identifier: GPL-3.0-or-later
//! Save-as dialog, close confirmation, arrangement prompt and recovery browser.

use super::{Intent, arrange, bindings::Action, workspace::Workspace};

/// Modal state owned by the application, drawn as egui popups each frame.
#[derive(Default)]
pub(super) struct Modal {
    /// The system "Save as" dialog is open; the board waits for its answer.
    pub(super) dialog: Option<()>,
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
                                        let changed = moth_ui::ago(entry.modified);
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

        if board.notices.conversion_prompt && self.dialog.is_none() && !workspace.confirm_close {
            let tokens = moth_ui::tokens(ui.ctx());
            let modal = egui::Modal::new(egui::Id::new("conversion"))
                .frame(
                    egui::Frame::popup(&ui.ctx().global_style())
                        .corner_radius(tokens.radius_l)
                        .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
                )
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(tokens.space_xl as f32 * 18.);
                    moth_ui::icons::dialog_title(ui, moth_ui::text(moth_ui::Text::ConversionTitle));
                    ui.add_space(tokens.space_xs as f32);
                    conversion_report(ui, board);
                    ui.add_space(tokens.space_l as f32);
                    // Save is what Enter does; continuing without a file sits beside it.
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if moth_ui::icons::primary_button(
                                ui,
                                moth_ui::text(moth_ui::Text::SaveAsMoth),
                                true,
                            )
                            .clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                intents.push(Intent::Action(Action::SaveConversion));
                            }
                            if moth_ui::icons::dialog_button(
                                ui,
                                moth_ui::text(moth_ui::Text::ContinueConversion),
                            )
                            .clicked()
                            {
                                intents.push(Intent::Action(Action::ContinueConversion));
                            }
                        });
                    });
                });
            // Esc keeps working on the board without a file, like the second button.
            if modal.should_close() {
                intents.push(Intent::Action(Action::ContinueConversion));
            }
        }
        if workspace.confirm_close && self.dialog.is_none() {
            let tokens = moth_ui::tokens(ui.ctx());
            let ready = board.intake.in_flight == 0
                && board.intake.pasting == 0
                && board.intake.pending.is_empty()
                && board.intake.waiting.is_empty();
            let modal = egui::Modal::new(egui::Id::new("confirm_close"))
                .frame(
                    egui::Frame::popup(&ui.ctx().global_style())
                        .corner_radius(tokens.radius_l)
                        .inner_margin(egui::Margin::same(tokens.space_xl as i8)),
                )
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(tokens.space_xl as f32 * 15.6);
                    moth_ui::icons::dialog_title(
                        ui,
                        &moth_ui::text(moth_ui::Text::SaveQuestion)
                            .replace("{name}", board.title()),
                    );
                    ui.add_space(tokens.space_xs as f32);
                    ui.label(
                        egui::RichText::new(moth_ui::text(moth_ui::Text::CloseWarning))
                            .color(tokens.text_muted),
                    );
                    ui.add_space(tokens.space_l as f32);
                    // "Don't save" stands apart on the left; Cancel and Save sit together
                    // on the right, Save filled because Enter chooses it.
                    ui.horizontal(|ui| {
                        if moth_ui::icons::danger_button(ui, moth_ui::text(moth_ui::Text::Close))
                            .clicked()
                        {
                            intents.push(Intent::Action(Action::Discard));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if moth_ui::icons::primary_button(
                                ui,
                                moth_ui::text(moth_ui::Text::Save),
                                ready,
                            )
                            .clicked()
                                || (ready && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                intents.push(Intent::Action(Action::SaveAndClose));
                            }
                            if moth_ui::icons::dialog_button(
                                ui,
                                moth_ui::text(moth_ui::Text::Cancel),
                            )
                            .clicked()
                            {
                                intents.push(Intent::Action(Action::KeepOpen));
                            }
                        });
                    });
                });
            // Esc or a click beside the dialog keeps the board open.
            if modal.should_close() {
                intents.push(Intent::Action(Action::KeepOpen));
            }
        }
        let settled = board.settled();
        if let Err(wait) = settled
            && board.notices.arrange_prompt.is_some()
        {
            // Images are still arriving: the prompt (and auto-arrange) waits for all of them.
            ui.ctx().request_repaint_after(wait);
        }
        if let Some(ids) = board.notices.arrange_prompt.clone()
            && !workspace.receiving_drop
            && settled.is_ok()
        {
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
        if let Some(added) = &board.notices.added {
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
    let Some((source, skipped)) = &board.file.conversion else {
        return;
    };
    let tokens = moth_ui::tokens(ui.ctx());
    ui.label(moth_ui::text(moth_ui::Text::ConversionRetained));
    ui.add_space(tokens.space_xs as f32);
    // The source path in a quiet well, so a long path reads as one value.
    egui::Frame::new()
        .fill(tokens.surface_raised)
        .stroke(egui::Stroke::new(1., tokens.border))
        .corner_radius(tokens.radius_m)
        .inner_margin(egui::Margin::symmetric(
            tokens.space_s as i8,
            tokens.space_xs as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                egui::Label::new(
                    egui::RichText::new(source.to_string_lossy())
                        .monospace()
                        .small(),
                )
                .wrap(),
            );
        });
    ui.add_space(tokens.space_s as f32);
    let unsupported = if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pur"))
    {
        moth_ui::Text::ConversionPur
    } else {
        moth_ui::Text::ConversionBee
    };
    ui.label(egui::RichText::new(moth_ui::text(unsupported)).color(tokens.text_muted));
    if *skipped > 0 {
        ui.label(
            egui::RichText::new(
                moth_ui::Message::Count(moth_ui::Text::ImportSkippedCount, *skipped)
                    .render(moth_ui::locale()),
            )
            .color(tokens.danger),
        );
    }
}
