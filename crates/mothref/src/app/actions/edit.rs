// SPDX-License-Identifier: GPL-3.0-or-later
//! Changing items: clipboard and history, notes, delete and duplicate,
//! transforms, crop, stacking order, nudging and image smoothing.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn edit_enabled(&self, action: Action) -> bool {
        use Action::*;
        let single = |kind: fn(&moth_scene::Item) -> bool| {
            self.workspace.board.selection.len() == 1
                && self
                    .workspace
                    .board
                    .selection
                    .iter()
                    .any(|id| self.workspace.board.scene.get(*id).is_some_and(kind))
        };
        match action {
            SamplingDefault | SamplingSmooth | SamplingNearest | SamplingAutomatic => {
                self.workspace.board.idle()
                    && !self.tools.locked
                    && !self.workspace.board.selection.is_empty()
                    && self.workspace.board.selection.iter().all(|id| {
                        self.workspace
                            .board
                            .scene
                            .get(*id)
                            .is_some_and(|item| item.blob().is_some())
                    })
            }
            EditNote => {
                self.workspace.board.idle()
                    && !self.tools.locked
                    && single(|item| item.note().is_some())
            }
            EditItem | CancelItem => self.workspace.board.selection.len() == 1,
            ApplyItem => {
                self.workspace.board.selection.len() == 1
                    && self.tools.draft.is_some_and(|(id, r, t)| {
                        self.workspace.board.selection.contains(&id) && r.valid() && t.valid()
                    })
            }
            Paste | Undo | Redo | Note => true,
            _ => !self.workspace.board.selection.is_empty(),
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_edit(
        &mut self,
        action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        let items = Self::selected_items(&self.workspace.board, action);
        let intent = match action {
            EditNote => {
                let [item] = items.as_slice() else {
                    return None;
                };
                Intent::EditNote(item.id)
            }
            EditItem => {
                self.tools.inspector = true;
                return None;
            }
            SamplingDefault | SamplingSmooth | SamplingNearest | SamplingAutomatic => {
                let sampling = match action {
                    SamplingSmooth => moth_scene::Sampling::Smooth,
                    SamplingNearest => moth_scene::Sampling::Nearest,
                    SamplingAutomatic => moth_scene::Sampling::Automatic,
                    _ => moth_scene::Sampling::Default,
                };
                Intent::Commands(
                    items
                        .iter()
                        .filter(|item| item.transform.sampling != sampling)
                        .map(|item| Command::SetTransform {
                            id: item.id,
                            transform: Transform {
                                sampling,
                                ..item.transform
                            },
                        })
                        .collect(),
                )
            }
            ApplyItem => {
                let (id, mut rect, transform) = self.tools.draft?;
                let item = self.workspace.board.scene.get(id)?;
                if let Some(text) = item.note() {
                    let [width, height] = canvas::measure_note_frame(
                        self.shell.context(),
                        text,
                        transform.note_font_size.max(0.001),
                        transform.note_wrap.then_some(rect.width),
                    );
                    rect.width = width;
                    rect.height = height;
                }
                if !rect.valid() || !transform.valid() {
                    return None;
                }
                let mut commands = Vec::new();
                if item.rect != rect {
                    commands.push(Command::SetRect { id, rect });
                }
                if item.transform != transform {
                    commands.push(Command::SetTransform { id, transform });
                }
                Intent::Commands(commands)
            }
            CancelItem => {
                self.tools.draft = None;
                self.tools.observed = None;
                return None;
            }
            ResetRotation | ResetFlip => Intent::Commands(transform_selection(
                &self.workspace.board.scene,
                &self.workspace.board.selection,
                |t| {
                    let mut t = t;
                    if action == ResetRotation {
                        t.rotation = 0.;
                    } else {
                        t.flip = [false; 2];
                    }
                    t
                },
            )),
            ResetScale | NormalizeScale => Intent::OriginalGeometry(
                items
                    .iter()
                    .filter(|i| i.blob().is_some())
                    .map(|i| i.id)
                    .collect(),
                if action == ResetScale {
                    moth_platform::export::Geometry::Scale
                } else {
                    moth_platform::export::Geometry::Normalize
                },
            ),
            Paste => Intent::Paste,
            Undo => Intent::Undo,
            Redo => Intent::Redo,
            Note => Intent::AddNote(self.workspace.board.view.camera.world([
                self.workspace.board.view.viewport.center().x,
                self.workspace.board.view.viewport.center().y,
            ])),
            Delete => Intent::Commands(groups::removal(
                &self.workspace.board.scene,
                items.iter().map(|i| i.id).collect(),
            )),
            Duplicate => Intent::Commands(arrange::translate(
                &items,
                [24., 24.],
                true,
                self.workspace.board.scene.top(),
            )),
            RotateCw | RotateCcw | FlipHorizontal | FlipVertical => {
                Intent::Commands(transform_selection(
                    &self.workspace.board.scene,
                    &self.workspace.board.selection,
                    |t| match action {
                        RotateCw => rotated(t, 1.),
                        RotateCcw => rotated(t, -1.),
                        FlipHorizontal => mirrored(t, 0),
                        _ => mirrored(t, 1),
                    },
                ))
            }
            ResetTransform | ResetCrop => Intent::OriginalGeometry(
                items.iter().map(|i| i.id).collect(),
                if action == ResetCrop {
                    moth_platform::export::Geometry::Crop
                } else {
                    moth_platform::export::Geometry::All
                },
            ),
            NudgeLeft | NudgeRight | NudgeUp | NudgeDown | NudgeLeftFast | NudgeRightFast
            | NudgeUpFast | NudgeDownFast => {
                let delta = match action {
                    NudgeLeft => [-1., 0.],
                    NudgeRight => [1., 0.],
                    NudgeUp => [0., -1.],
                    NudgeDown => [0., 1.],
                    NudgeLeftFast => [-10., 0.],
                    NudgeRightFast => [10., 0.],
                    NudgeUpFast => [0., -10.],
                    _ => [0., 10.],
                };
                Intent::Commands(arrange::translate(
                    &items,
                    delta,
                    false,
                    self.workspace.board.scene.top(),
                ))
            }
            BringFront | SendBack | BringForward | SendBackward => {
                Intent::Commands(arrange::reorder(
                    &self.workspace.board.scene,
                    &items.iter().map(|item| item.id).collect(),
                    matches!(action, BringFront | BringForward),
                    matches!(action, BringFront | SendBack),
                ))
            }
            _ => return None,
        };
        Some(intent)
    }
}
