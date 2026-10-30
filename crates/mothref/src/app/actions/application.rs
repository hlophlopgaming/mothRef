// SPDX-License-Identifier: GPL-3.0-or-later
//! Application-wide actions: files and tabs, settings, help and the disk cache.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn application_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
            ShellIntegration => matches!(
                moth_platform::Desktop.shell_integration(),
                moth_platform::ShellIntegration::Missing | moth_platform::ShellIntegration::Pending
            ),
            ClearDiskCache => !self.tools.disk_cache_busy && !self.tools.clear_disk_cache,
            Save => self.workspace.board.idle(),
            _ => true,
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_application(
        &mut self,
        action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        let intent = match action {
            Palette => {
                self.cancel_drag();
                self.shortcuts.open_palette();
                return None;
            }
            Shortcuts => {
                self.cancel_drag();
                self.shortcuts.open_cheatsheet();
                return None;
            }
            BindingSettings => {
                self.cancel_drag();
                self.shortcuts.open_settings();
                return None;
            }
            Settings => {
                self.cancel_drag();
                self.settings_window.open = !self.settings_window.open;
                return None;
            }
            CloseWindow => Intent::CloseWindow,
            ClearDiskCache => {
                self.tools.clear_disk_cache = true;
                return None;
            }
            ShellIntegration => {
                moth_platform::Desktop.install_shell_integration();
                self.workspace.board.notices.message =
                    moth_ui::Text::ShellIntegrationInstalling.into();
                return None;
            }
            New => Intent::New,
            Open => Intent::Open,
            Images => Intent::Images,
            Save => Intent::Save,
            Library => Intent::Home,
            CloseTab => Intent::CloseTab(0),
            NextTab => {
                if !self.workspace.tabs.is_empty() {
                    self.activate(1);
                }
                return None;
            }
            _ => return None,
        };
        Some(intent)
    }
}
