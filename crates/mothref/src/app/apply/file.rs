// SPDX-License-Identifier: GPL-3.0-or-later
//! Files and tabs: the library, opening, saving, closing and recovery.
use super::*;
impl App {
    /// Applies an intent of this area. False when the camera must not animate
    /// to where the intent left it.
    pub(in crate::app) fn apply_file(
        &mut self,
        intent: Intent,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        match intent {
            Intent::DialogFocused => self.modal.dialog_focus = false,
            Intent::Library(request) => {
                if self
                    .library_screen
                    .worker
                    .requests
                    .try_send(request)
                    .is_err()
                {
                    self.workspace.board.notices.message = moth_ui::Text::StorageBusy.into();
                }
            }
            Intent::Home => {
                self.workspace.library = true;
                self.workspace.refresh_library = true;
                self.workspace.board.view.viewport = egui::Rect::NOTHING;
            }
            Intent::New => self.new_board(),
            Intent::Open => self.open_files(false),
            Intent::Images => self.open_files(true),
            Intent::Save => {
                self.workspace.board.notices.conversion_prompt = false;
                if self.workspace.board.file.path.is_empty() {
                    self.modal.dialog = Some(());
                    self.modal.dialog_focus = true;
                    self.modal.dialog_path.clear();
                } else {
                    self.workspace.board.save();
                }
            }
            Intent::Tab(index) => self.activate(index),
            Intent::CloseTab(index) => {
                self.activate(index);
                self.request_close(false);
            }
            Intent::OpenPath(path) => {
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
                {
                    self.open_in_tab(path);
                } else {
                    if self.workspace.library {
                        self.new_board();
                    }
                    self.import(path);
                }
            }
            Intent::SaveAs(mut path) => {
                path.set_extension("moth");
                self.modal.dialog = None;
                self.workspace.board.file.path = path.to_string_lossy().into_owned();
                self.workspace.board.save();
            }
            Intent::CancelDialog => {
                self.modal.dialog = None;
                self.workspace.board.file.close_after_save = false;
            }
            Intent::SaveAndClose => {
                self.workspace.board.notices.conversion_prompt = false;
                self.workspace.board.file.close_after_save = true;
                if self.workspace.board.file.path.is_empty() {
                    self.modal.dialog_path.clear();
                    self.modal.dialog = Some(());
                    self.modal.dialog_focus = true;
                } else {
                    self.workspace.board.save();
                }
            }
            Intent::Discard => self.workspace.discard(),
            Intent::KeepOpen => {
                self.workspace.confirm_close = false;
                self.workspace.closing_window = false;
                self.workspace.board.file.close_after_save = false;
            }
            Intent::DismissRecovery => self.workspace.dismiss_recovery(self.modal.recovery),
            Intent::RestoreRecovery => self.workspace.restore_recovery(self.modal.recovery),
            _ => {}
        }
        true
    }
}
