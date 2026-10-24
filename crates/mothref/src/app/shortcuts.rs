// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding editor and command search consume the same action inventory as dispatch.
use super::bindings::{ACTIONS, Action, Chord, Registry};
use moth_ui::{Text, text};

struct Toast {
    action: Action,
    started: std::time::Instant,
    hovered: bool,
    focused: bool,
    held: bool,
}

#[derive(Default)]
pub(super) struct Shortcuts {
    pub registry: Registry,
    pub palette: bool,
    sheet: bool,
    toast: Option<Toast>,
    pub open: bool,
    query: String,
    selected: usize,
    focus: bool,
    draft: Option<Vec<String>>,
    filter: String,
}
impl Shortcuts {
    pub fn feedback(&mut self, action: Action, now: std::time::Instant) {
        // One slot also bounds feedback during rapid alternating shortcuts.
        self.toast = Some(Toast {
            action,
            started: now,
            hovered: false,
            focused: false,
            held: false,
        });
    }
    pub fn toast(&mut self, ctx: &egui::Context, enabled: bool, reduced_motion: bool) {
        if !enabled {
            self.toast = None;
        }
        let Some(toast) = &mut self.toast else {
            return;
        };
        let age = toast.started.elapsed().as_secs_f32();
        let alpha = if toast.held || toast.hovered || toast.focused || age <= 3.5 {
            1.
        } else {
            ((4. - age) / 0.5).clamp(0., 1.)
        };
        if alpha == 0. {
            self.toast = None;
            return;
        }
        let mut dismiss = false;
        let mut hovered = false;
        let mut focused = false;
        egui::Area::new(egui::Id::new("hotkey_feedback"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, [0., -16.])
            .show(ctx, |ui| {
                ui.set_opacity(if reduced_motion { 1. } else { alpha });
                let frame = egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width((ctx.content_rect().width() - 40.).clamp(1., 480.));
                    ui.label(format!(
                        "{}   {}",
                        text(toast.action.label()),
                        self.registry.label(toast.action)
                    ));
                    ui.horizontal(|ui| {
                        let hold = ui
                            .selectable_label(toast.held, text(Text::ToastHold))
                            .on_hover_text(text(Text::ToastHint));
                        focused |= hold.has_focus();
                        if hold.clicked() {
                            toast.held = !toast.held;
                            toast.started = std::time::Instant::now();
                        }
                        let close = ui.button(text(Text::ToastDismiss));
                        focused |= close.has_focus();
                        dismiss |= close.clicked();
                    });
                });
                hovered = frame.response.contains_pointer();
            });
        if focused {
            toast.started = std::time::Instant::now();
        }
        dismiss |= toast.hovered && !hovered && !toast.held && !focused;
        dismiss |= (focused || toast.held)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        dismiss |= toast.focused && !focused && !toast.held && !hovered;
        toast.hovered = hovered;
        toast.focused = focused;
        if dismiss {
            self.toast = None;
        } else if !hovered && !focused && !toast.held {
            if age < 3.5 {
                ctx.request_repaint_after(std::time::Duration::from_secs_f32(3.5 - age));
            } else if reduced_motion {
                ctx.request_repaint_after(std::time::Duration::from_secs_f32((4. - age).max(0.)));
            } else {
                ctx.request_repaint();
            }
        }
    }
    pub fn toast_focused(&self) -> bool {
        self.toast.as_ref().is_some_and(|t| t.focused)
    }
    pub fn blocked(&self) -> bool {
        self.open || self.palette || self.sheet
    }
    pub fn open_cheatsheet(&mut self) {
        self.sheet = true;
        self.open = false;
        self.palette = false;
        self.filter.clear();
        self.focus = true;
    }
    pub fn open_palette(&mut self) {
        self.sheet = false;
        self.palette = true;
        self.open = false;
        self.query.clear();
        self.selected = 0;
        self.focus = true;
    }
    pub fn open_settings(&mut self) {
        self.sheet = false;
        self.open = true;
        self.palette = false;
        self.filter.clear();
        self.preview(self.registry.clone());
    }
    fn preview(&mut self, registry: Registry) {
        self.draft = Some(
            ACTIONS
                .iter()
                .map(|a| registry.get(*a).map_or_else(String::new, Chord::encode))
                .collect(),
        );
    }
    fn candidate(&self) -> Option<Registry> {
        let mut registry = self.registry.clone();
        for (action, value) in ACTIONS.iter().zip(self.draft.as_ref()?) {
            let chord = if value.is_empty() {
                None
            } else {
                let chord = Chord::parse(value)?;
                if action.pointer() != matches!(chord.trigger, super::bindings::Trigger::Drag(..)) {
                    return None;
                }
                Some(chord)
            };
            registry.entries[*action as usize] = chord;
        }
        Some(registry)
    }
    pub fn results(&self) -> Vec<Action> {
        let query = self.query.to_lowercase();
        ACTIONS
            .iter()
            .copied()
            .filter(|a| {
                !a.pointer()
                    && *a != Action::Palette
                    && (text(a.label()).to_lowercase().contains(&query)
                        || a.id().to_lowercase().contains(&query)
                        || self.registry.label(*a).to_lowercase().contains(&query))
            })
            .collect()
    }
    /// Returns one action and whether preferences changed; board writes stay in App.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        enabled: &impl Fn(Action) -> bool,
    ) -> (Option<Action>, bool) {
        let mut action = None;
        let mut changed = false;
        if self.sheet {
            let response =
                egui::Modal::new(egui::Id::new("shortcut_cheatsheet")).show(ui.ctx(), |ui| {
                    ui.set_width((ui.ctx().content_rect().width() - 40.).clamp(1., 650.));
                    ui.heading(text(Text::CheatSheet));
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text(text(Text::SearchActions))
                            .char_limit(128),
                    );
                    if self.focus {
                        input.request_focus();
                        self.focus = false;
                    }
                    let query = self.filter.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(ui.ctx().content_rect().height() * 0.65)
                        .show(ui, |ui| {
                            for category in [
                                Text::CategoryBoard,
                                Text::CategoryView,
                                Text::CategoryEdit,
                                Text::CategoryPointer,
                            ] {
                                let actions: Vec<_> = ACTIONS
                                    .iter()
                                    .filter(|a| {
                                        a.category() == category
                                            && (text(a.label()).to_lowercase().contains(&query)
                                                || a.id().to_lowercase().contains(&query)
                                                || self
                                                    .registry
                                                    .label(**a)
                                                    .to_lowercase()
                                                    .contains(&query)
                                                || text(category).to_lowercase().contains(&query))
                                    })
                                    .collect();
                                if actions.is_empty() {
                                    continue;
                                }
                                ui.heading(text(category));
                                for action in actions {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(text(action.label()));
                                        ui.monospace(self.registry.label(*action));
                                    });
                                }
                            }
                        });
                    if ui.button(text(Text::Shortcuts)).clicked() {
                        self.open_settings();
                    }
                    if ui.button(text(Text::Cancel)).clicked() {
                        self.sheet = false;
                    }
                });
            if response.should_close() {
                self.sheet = false;
            }
        }
        if self.palette {
            let response =
                egui::Modal::new(egui::Id::new("command_palette")).show(ui.ctx(), |ui| {
                    ui.set_width((ui.ctx().content_rect().width() - 40.).clamp(1., 560.));
                    ui.heading(text(Text::CommandPalette));
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .char_limit(128)
                            .desired_width(f32::INFINITY),
                    );
                    if self.focus {
                        input.request_focus();
                        self.focus = false;
                    }
                    if input.changed() {
                        self.selected = 0;
                    }
                    let results = self.results();
                    self.selected = self.selected.min(results.len().saturating_sub(1));
                    let navigate = ui.input(|i| {
                        i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp)
                    });
                    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
                    {
                        self.selected = (self.selected + 1).min(results.len().saturating_sub(1));
                    }
                    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                        self.selected = self.selected.saturating_sub(1);
                    }
                    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                        action = results.get(self.selected).copied().filter(|a| enabled(*a));
                    }
                    if results.is_empty() {
                        ui.label(text(Text::NoActions));
                    }
                    egui::ScrollArea::vertical()
                        .max_height(ui.ctx().content_rect().height() * 0.55)
                        .show(ui, |ui| {
                            for (index, candidate) in results.iter().enumerate() {
                                let label = format!(
                                    "{}    {}",
                                    text(candidate.label()),
                                    self.registry.label(*candidate)
                                );
                                let response = ui.add_enabled(
                                    enabled(*candidate),
                                    egui::Button::new(label).selected(index == self.selected),
                                );
                                if index == self.selected && navigate {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    action = Some(*candidate);
                                }
                            }
                        });
                    if ui.button(text(Text::Cancel)).clicked() {
                        self.palette = false;
                    }
                });
            if response.should_close() || action.is_some() {
                self.palette = false;
            }
        }
        if self.open {
            let response =
                egui::Modal::new(egui::Id::new("shortcut_settings")).show(ui.ctx(), |ui| {
                    ui.set_width((ui.ctx().content_rect().width() - 40.).clamp(1., 700.));
                    egui::ScrollArea::vertical()
                        .id_salt("shortcut_modal_body")
                        .max_height((ui.ctx().content_rect().height() - 60.).max(1.))
                        .show(ui, |ui| {
                            ui.heading(text(Text::Shortcuts));
                            ui.label(text(Text::BindingHelp));
                            ui.label(text(Text::PresetHelp));
                            ui.horizontal_wrapped(|ui| {
                                if ui.button(text(Text::Defaults)).clicked() {
                                    self.preview(Registry::default());
                                }
                                if ui.button("BeeRef").clicked() {
                                    self.preview(self.registry.preset(false));
                                }
                                if ui.button("PureRef").clicked() {
                                    self.preview(self.registry.preset(true));
                                }
                            });
                            ui.add(
                                egui::TextEdit::singleline(&mut self.filter)
                                    .hint_text(text(Text::SearchActions))
                                    .char_limit(128),
                            );
                            let filter = self.filter.to_lowercase();
                            if let Some(draft) = &mut self.draft {
                                egui::ScrollArea::vertical()
                                    .max_height(ui.ctx().content_rect().height() * 0.45)
                                    .show(ui, |ui| {
                                        egui::Grid::new("bindings").striped(true).show(ui, |ui| {
                                            ui.label(text(Text::BindingAction));
                                            ui.label(text(Text::BindingProposed));
                                            ui.label(text(Text::BindingCurrent));
                                            ui.end_row();
                                            for (index, action) in ACTIONS.iter().enumerate() {
                                                if !text(action.label())
                                                    .to_lowercase()
                                                    .contains(&filter)
                                                    && !action.id().to_lowercase().contains(&filter)
                                                {
                                                    continue;
                                                }
                                                let label = ui.label(text(action.label()));
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut draft[index])
                                                        .char_limit(80)
                                                        .desired_width(
                                                            (ui.available_width() * 0.5)
                                                                .clamp(60., 170.),
                                                        ),
                                                )
                                                .labelled_by(label.id);
                                                ui.label(self.registry.label(*action));
                                                ui.end_row();
                                            }
                                        });
                                    });
                            }
                            let candidate = self.candidate();
                            let valid =
                                candidate.as_ref().is_some_and(|c| c.conflicts().is_empty());
                            if candidate.is_none() {
                                ui.colored_label(
                                    egui::Color32::LIGHT_RED,
                                    text(Text::InvalidBinding),
                                );
                            }
                            if let Some(candidate) = &candidate {
                                for (a, b) in candidate.conflicts() {
                                    ui.colored_label(
                                        egui::Color32::LIGHT_RED,
                                        format!(
                                            "{}: {} ↔ {} ({})",
                                            text(Text::BindingConflict),
                                            text(a.label()),
                                            text(b.label()),
                                            candidate.label(a)
                                        ),
                                    );
                                }
                                let changes: Vec<_> = ACTIONS
                                    .iter()
                                    .filter(|a| self.registry.get(**a) != candidate.get(**a))
                                    .collect();
                                if !changes.is_empty() {
                                    ui.label(text(Text::BindingPreview));
                                    egui::ScrollArea::vertical()
                                        .id_salt("binding_preview")
                                        .max_height(110.)
                                        .show(ui, |ui| {
                                            for a in changes {
                                                ui.label(format!(
                                                    "{}: {} → {}",
                                                    text(a.label()),
                                                    self.registry.label(*a),
                                                    candidate.label(*a)
                                                ));
                                            }
                                        });
                                }
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(valid, egui::Button::new(text(Text::Apply)))
                                    .clicked()
                                {
                                    if let Some(candidate) = candidate {
                                        self.registry = candidate;
                                        changed = true;
                                    }
                                    self.open = false;
                                }
                                if ui.button(text(Text::Cancel)).clicked() {
                                    self.open = false;
                                }
                            });
                        });
                });
            if response.should_close() {
                self.open = false;
            }
        }
        (action, changed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feedback_replaces_repeats_and_expires_or_disables() {
        let ctx = egui::Context::default();
        let mut state = Shortcuts::default();
        let now = std::time::Instant::now();
        state.feedback(Action::Save, now - std::time::Duration::from_secs(10));
        state.feedback(Action::Save, now);
        assert_eq!(state.toast.as_ref().unwrap().started, now);
        state.feedback(Action::Next, now);
        assert_eq!(state.toast.as_ref().unwrap().action, Action::Next);
        state.toast.as_mut().unwrap().started = now - std::time::Duration::from_secs(5);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            state.toast(ui.ctx(), true, false)
        })
        .textures_delta
        .clear();
        assert!(state.toast.is_none());
        state.feedback(Action::Next, now - std::time::Duration::from_secs(10));
        state.toast.as_mut().unwrap().held = true;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            state.toast(ui.ctx(), true, false)
        })
        .textures_delta
        .clear();
        assert!(state.toast.is_some());
        ctx.run_ui(egui::RawInput::default(), |ui| {
            state.toast(ui.ctx(), false, false)
        })
        .textures_delta
        .clear();
        assert!(state.toast.is_none());
    }
    #[test]
    fn cheat_sheet_and_image_navigation_use_the_registry() {
        let mut state = Shortcuts::default();
        assert_eq!(
            state.registry.key(egui::Key::Slash, Default::default()),
            Some(Action::Shortcuts)
        );
        assert_eq!(
            state.registry.key(egui::Key::PageUp, Default::default()),
            Some(Action::Previous)
        );
        assert_eq!(
            state.registry.key(egui::Key::PageDown, Default::default()),
            Some(Action::Next)
        );
        state.open_cheatsheet();
        assert!(state.blocked());
        state.open_settings();
        assert!(!state.sheet);
        state.open_palette();
        assert!(!state.open);
        for action in ACTIONS {
            assert!(
                [
                    Text::CategoryBoard,
                    Text::CategoryView,
                    Text::CategoryEdit,
                    Text::CategoryPointer
                ]
                .contains(&action.category())
            );
        }
    }
    #[test]
    fn palette_arrows_enter_and_escape_are_isolated_from_canvas_actions() {
        let ctx = egui::Context::default();
        let mut state = Shortcuts::default();
        state.open_palette();
        let frame = |state: &mut Shortcuts, keys: &[egui::Key], enabled: bool| {
            let mut action = None;
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 600.),
                )),
                events: keys
                    .iter()
                    .flat_map(|key| {
                        [true, false].map(|pressed| egui::Event::Key {
                            key: *key,
                            physical_key: Some(*key),
                            pressed,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        })
                    })
                    .collect(),
                ..Default::default()
            };
            let mut output = ctx.run_ui(raw, |ui| {
                action = state.show(ui, &|_| enabled).0;
            });
            output.textures_delta.clear();
            action
        };
        frame(&mut state, &[], true);
        frame(&mut state, &[egui::Key::ArrowDown], true);
        assert_eq!(state.selected, 1);
        let expected = state.results()[1];
        assert_eq!(frame(&mut state, &[egui::Key::Enter], true), Some(expected));
        assert!(!state.palette);
        state.open_palette();
        frame(&mut state, &[], true);
        assert_eq!(frame(&mut state, &[egui::Key::Enter], false), None);
        assert!(state.palette);
        frame(&mut state, &[egui::Key::Escape], true);
        assert!(!state.palette);
    }
    #[test]
    fn search_uses_the_active_binding_and_invalid_drafts_cannot_apply() {
        let mut state = Shortcuts::default();
        state.registry.set(Action::Save, "Alt+F2");
        state.query = "alt+f2".into();
        assert_eq!(state.results(), vec![Action::Save]);
        state.open_settings();
        state.draft.as_mut().unwrap()[Action::Save as usize] = "Mouse1".into();
        assert!(state.candidate().is_none());
        state.preview(state.registry.preset(true));
        assert!(state.candidate().unwrap().conflicts().is_empty());
        state.preview(Registry::default());
        assert_eq!(state.candidate().unwrap(), Registry::default());
    }
}
