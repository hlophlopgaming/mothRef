// SPDX-License-Identifier: GPL-3.0-or-later
//! Groups: create, dissolve, enter, rename and color.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn group_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
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
            Group => {
                let (id, commands) =
                    groups::create(&self.workspace.board.scene, &self.workspace.board.selection)?;
                let board = &mut self.workspace.board;
                board.group_edits.renaming = Some((id, String::new()));
                if let Some(Command::Insert(group)) = commands.first() {
                    board.group_edits.pending_group = Some((id, group.rect));
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
                    .unwrap_or(0) as u8;
                let board = &mut self.workspace.board;
                let commands: Vec<_> = groups::selected(&board.scene, &board.selection)
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
            _ => return None,
        };
        Some(intent)
    }
}
