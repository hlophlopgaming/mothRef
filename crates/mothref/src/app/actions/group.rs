// SPDX-License-Identifier: GPL-3.0-or-later
//! Groups: create, dissolve, enter, rename and color.
use super::*;
impl App {
    fn set_group_color(&mut self, ids: Vec<Uuid>, color: u32) -> Intent {
        let board = &mut self.workspace.board;
        let commands: Vec<_> = ids
            .into_iter()
            .filter_map(|id| {
                let style = board.group_style(id)?;
                (style.color != color).then_some(Command::SetGroup {
                    id,
                    group: moth_scene::Group { color, ..style },
                })
            })
            .collect();
        for command in &commands {
            if let Command::SetGroup { id, group } = command {
                board.group_edits.pending_styles.insert(*id, group.clone());
            }
        }
        Intent::Commands(commands)
    }
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn group_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
            ApplyGroupColor => self.modal.group_color.as_ref().is_some_and(|(ids, _)| {
                ids.iter().all(|id| {
                    self.workspace
                        .board
                        .scene
                        .get(*id)
                        .is_some_and(|item| item.is_group())
                })
            }),
            CancelGroupColor => self.modal.group_color.is_some(),
            EnterGroup | RenameGroup => selection::single_group(
                &self.workspace.board.scene,
                &self.workspace.board.selection,
            )
            .is_some(),
            Group => !self.workspace.board.selection.is_empty(),
            // Ungroup and the backdrop colors act on the selected groups.
            _ => !groups::selected(&self.workspace.board.scene, &self.workspace.board.selection)
                .is_empty(),
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_group(
        &mut self,
        action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        let intent = match action {
            CancelGroupColor => {
                self.modal.group_color = None;
                return None;
            }
            GroupCustom => {
                let board = &self.workspace.board;
                let ids = groups::selected(&board.scene, &board.selection);
                let style = board.group_style(*ids.first()?)?;
                let [r, g, b, _] = moth_ui::tokens(self.shell.context())
                    .group(style.color)
                    .to_array();
                self.modal.group_color = Some((ids, [r, g, b]));
                return None;
            }
            ApplyGroupColor => {
                let (ids, rgb) = self.modal.group_color.take()?;
                let color = moth_scene::Group::custom_color(rgb);
                return Some(self.set_group_color(ids, color));
            }
            Group => {
                let (id, commands) =
                    groups::create(&self.workspace.board.scene, &self.workspace.board.selection)?;
                let board = &mut self.workspace.board;
                board.group_edits.renaming = Some((id, String::new()));
                if let Some(Command::Insert(group)) = commands.first() {
                    board.group_edits.pending_group = Some((id, group.rect));
                    if let Some(style) = group.group_style() {
                        board.group_edits.pending_styles.insert(id, style.clone());
                    }
                }
                Intent::Commands(commands)
            }
            Ungroup => {
                let board = &mut self.workspace.board;
                let commands = groups::dissolve(&board.scene, &board.selection);
                board.entered = None;
                Intent::Commands(commands)
            }
            EnterGroup | RenameGroup => {
                let board = &mut self.workspace.board;
                let group = selection::single_group(&board.scene, &board.selection)?;
                if action == EnterGroup {
                    board.entered = Some(group);
                    board.selection = board.scene.children(group).map(|item| item.id).collect();
                    selection::with_members(&board.scene, &mut board.selection, None);
                } else if let Some(style) = board.group_style(group) {
                    board.group_edits.renaming = Some((group, style.title));
                }
                return None;
            }
            GroupNeutral | GroupSage | GroupAmber | GroupSky | GroupWing | GroupLilac => {
                let color = bindings::GROUP_COLORS
                    .iter()
                    .position(|candidate| *candidate == action)
                    .unwrap_or(0) as u32;
                let ids =
                    groups::selected(&self.workspace.board.scene, &self.workspace.board.selection);
                self.set_group_color(ids, color)
            }
            _ => return None,
        };
        Some(intent)
    }
}
