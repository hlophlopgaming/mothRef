// SPDX-License-Identifier: GPL-3.0-or-later
//! Board edits from intents: clipboard, history, notes, groups, layout and export.
use super::*;
impl App {
    /// Applies an intent of this area. False when the camera must not animate
    /// to where the intent left it.
    pub(in crate::app) fn apply_board(
        &mut self,
        intent: Intent,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        match intent {
            Intent::NoteFocused => {
                if let Some(editing) = &mut self.editing {
                    editing.focus = false;
                }
            }
            Intent::Paste => {
                if !self.workspace.board.file.discarding {
                    self.paste();
                }
            }
            Intent::Undo => self.workspace.persist(moth_platform::boards::Request::Undo),
            Intent::Redo => self.workspace.persist(moth_platform::boards::Request::Redo),
            Intent::AddNote(world) => self.add_note(world),
            Intent::EditNote(id) => self.edit_note(id),
            Intent::CommitNote => self.finish_editing(),
            Intent::Commands(commands) => {
                self.workspace
                    .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
            }
            Intent::GroupTitle(title) => {
                let board = &mut self.workspace.board;
                let Some((id, _)) = board.group_edits.renaming.take() else {
                    return false;
                };
                let Some(style) = board.group_style(id) else {
                    return false;
                };
                let Some(mut title) = title.map(|title| title.trim().to_owned()) else {
                    return false;
                };
                while title.len() > moth_scene::MAX_GROUP_TITLE {
                    title.pop();
                }
                if title != style.title {
                    let group = moth_scene::Group { title, ..style };
                    board.group_edits.pending_styles.insert(id, group.clone());
                    self.workspace
                        .persist(moth_platform::boards::Request::Apply(
                            Command::SetGroup { id, group },
                            None,
                        ));
                }
            }
            Intent::MoveLayer(id, onto) => {
                if self.workspace.board.idle() && !self.tools.locked {
                    let commands = groups::move_layer(&self.workspace.board.scene, id, onto);
                    self.workspace
                        .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                }
            }
            Intent::SelectItem(id) => {
                let board = &mut self.workspace.board;
                if board.scene.get(id).is_none() {
                    return false;
                }
                board.entered = board.scene.get(id).and_then(|item| item.parent);
                board.selection = HashSet::from([id]);
                selection::with_members(&board.scene, &mut board.selection, None);
            }
            Intent::EnterGroup(hit) => {
                let board = &mut self.workspace.board;
                let group = selection::target(&board.scene, board.entered, hit);
                if !board.scene.get(group).is_some_and(|item| item.is_group()) {
                    return false;
                }
                board.entered = Some(group);
                board.selection.clear();
                if group != hit {
                    board
                        .selection
                        .insert(selection::target(&board.scene, board.entered, hit));
                    selection::with_members(&board.scene, &mut board.selection, None);
                }
            }
            Intent::Arrange(mode, ids) => {
                let mut items: Vec<_> = ids
                    .iter()
                    .filter_map(|id| self.workspace.board.scene.get(*id))
                    .collect();
                items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                let commands = if matches!(mode, arrange::Arrange::Grid | arrange::Arrange::Optimal)
                {
                    arrange::arrange(
                        &items,
                        mode,
                        self.settings.gap,
                        arrange::FreeRectChoice::from_index(self.settings.arrange_choice),
                        arrange::GuillotineSplit::from_index(self.settings.arrange_split),
                    )
                } else {
                    arrange::counted(
                        &items,
                        mode,
                        self.tools.layout_counts[mode as usize],
                        self.settings.gap,
                    )
                };
                // Layout changes belong to the still-open insertion group.
                let previous = self.workspace.board.notices.added.take();
                if !commands.is_empty() {
                    let pending = self.workspace.board.intake.in_flight;
                    self.workspace
                        .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                    if self.workspace.board.intake.in_flight == pending {
                        self.workspace.board.notices.added = previous;
                        return false;
                    }
                }
                self.workspace.board.notices.added = Some(document::Added {
                    ids,
                    mode,
                    needs_fit: true,
                });
                self.settings.default_arrange = mode as u8;
                self.apply(Intent::PreferencesDirty, event_loop);
            }
            Intent::Export(target) => self
                .workspace
                .persist(moth_platform::boards::Request::Export(target)),
            Intent::OriginalGeometry(ids, mode) => {
                self.workspace
                    .persist(moth_platform::boards::Request::OriginalGeometry { ids, mode });
            }
            Intent::DismissArrangePrompt => self.workspace.board.notices.arrange_prompt = None,
            _ => {}
        }
        true
    }
}
