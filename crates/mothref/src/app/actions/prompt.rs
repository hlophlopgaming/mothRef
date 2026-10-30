// SPDX-License-Identifier: GPL-3.0-or-later
//! Answers to prompts and dialogs: close confirmation, conversion, recovery,
//! save path, and the arrangement offered after an import.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn prompt_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
            ContinueConversion | SaveConversion => {
                self.workspace.board.notices.conversion_prompt && self.workspace.board.idle()
            }
            SaveAndClose => {
                self.workspace.confirm_close
                    && self.workspace.board.intake.in_flight == 0
                    && self.workspace.board.intake.pasting == 0
                    && self.workspace.board.intake.pending.is_empty()
                    && self.workspace.board.intake.waiting.is_empty()
            }
            Discard | KeepOpen => self.workspace.confirm_close,
            CancelDialog => self.modal.dialog.is_some(),
            SavePath => self.modal.dialog.is_some() && !self.modal.dialog_path.trim().is_empty(),
            DismissRecovery | RestoreRecovery => {
                !self.workspace.recovery_paths().is_empty() && self.workspace.board.idle()
            }
            CancelArrange => self.workspace.board.notices.arrange_prompt.is_some(),
            _ => {
                // The added-images toast: its layouts and undo.
                self.workspace.board.notices.added.is_some()
                    && self.workspace.board.idle()
                    && !self.tools.locked
            }
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_prompt(
        &mut self,
        action: Action,
        event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        if matches!(
            action,
            AddedHorizontal | AddedVertical | AddedGrid | AddedOptimal
        ) {
            if let Some(added) = &self.workspace.board.notices.added {
                let ids = added.ids.clone();
                let mode = match action {
                    AddedHorizontal => arrange::Arrange::Horizontal,
                    AddedVertical => arrange::Arrange::Vertical,
                    AddedGrid => arrange::Arrange::Grid,
                    _ => arrange::Arrange::Optimal,
                };
                self.apply(Intent::Arrange(mode, ids), event_loop);
            }
            return None;
        }
        let intent = match action {
            ContinueConversion => {
                self.workspace.board.notices.conversion_prompt = false;
                return None;
            }
            SaveConversion => {
                self.workspace.board.notices.conversion_prompt = false;
                Intent::Save
            }
            SaveAndClose => Intent::SaveAndClose,
            Discard => Intent::Discard,
            KeepOpen => Intent::KeepOpen,
            CancelDialog => Intent::CancelDialog,
            DismissRecovery => Intent::DismissRecovery,
            RestoreRecovery => Intent::RestoreRecovery,
            DismissAdded => {
                self.workspace.finish_added();
                return None;
            }
            UndoAdded => Intent::Undo,
            AddedHorizontal | AddedVertical | AddedGrid | AddedOptimal => return None,
            CancelArrange => Intent::DismissArrangePrompt,
            SavePath => Intent::SaveAs(std::path::PathBuf::from(&self.modal.dialog_path)),
            _ => return None,
        };
        Some(intent)
    }
}
