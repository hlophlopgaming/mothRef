// SPDX-License-Identifier: GPL-3.0-or-later
//! Preferences: language and the settings window.
use super::*;
impl App {
    /// Applies an intent of this area. False when the camera must not animate
    /// to where the intent left it.
    pub(in crate::app) fn apply_preferences(
        &mut self,
        intent: Intent,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        match intent {
            Intent::Language(language) => {
                self.settings.language = language;
                moth_ui::set_locale(language);
                self.workspace
                    .persist(moth_platform::boards::Request::SaveSettings(
                        language.code().to_owned(),
                    ));
            }
            Intent::SettingsRow(index) => self.settings_window.open_row(index),
            Intent::PreferencesDirty => {
                if !self.settings.lock_maximized
                    && self.shell.window.as_ref().is_some_and(|w| w.is_maximized())
                {
                    self.window.locked_position = None;
                }
                self.configure_global_shortcuts();
                self.adapt_gpu_budget();
                moth_ui::theme::apply(self.shell.context(), &self.settings.tokens());
                self.shell.context().set_zoom_factor(self.settings.ui_scale);
                self.restyle_windows();
                for tab in &mut self.workspace.tabs {
                    let values = std::collections::BTreeMap::from([(
                        "undo_limit".to_owned(),
                        self.settings.undo_limit.to_string(),
                    )]);
                    if tab
                        .boards()
                        .requests
                        .try_send(moth_platform::boards::Request::Preferences(values))
                        .is_ok()
                    {
                        tab.intake.in_flight += 1;
                    } else {
                        tab.notices.message = moth_ui::Text::StorageBusy.into();
                    }
                }
                self.workspace
                    .persist(moth_platform::boards::Request::Preferences(
                        self.settings
                            .preferences()
                            .into_iter()
                            .map(|(k, v)| (k.to_owned(), v))
                            .chain(self.shortcuts.registry.preferences())
                            .collect(),
                    ));
            }
            _ => {}
        }
        true
    }
}
