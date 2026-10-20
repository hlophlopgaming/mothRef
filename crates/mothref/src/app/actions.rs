// SPDX-License-Identifier: GPL-3.0-or-later
//! All registered actions share availability and execution, from keys or command search.
use super::*;
use bindings::Action;
impl App {
    pub(super) fn action_enabled(&self, action: Action) -> bool {
        use Action::*;
        if self.editing.is_some() {
            return false;
        }
        if matches!(
            action,
            Palette
                | Shortcuts
                | Tools
                | New
                | Open
                | Library
                | CloseTab
                | NextTab
                | Overlay
                | Minimize
                | Maximize
        ) {
            return true;
        }
        if action == Pin {
            return self
                .shell
                .window
                .as_ref()
                .is_some_and(|w| moth_platform::Desktop.window_capabilities(w).on_top);
        }
        if self.workspace.library {
            return false;
        }
        if matches!(
            action,
            Fit | Focus
                | Previous
                | Next
                | ZoomIn
                | ZoomOut
                | ZoomReset
                | ResetCamera
                | Select
                | Hand
        ) {
            return true;
        }
        if matches!(action, Lock | Grayscale | Save) {
            return self.workspace.board.idle();
        }
        if !self.workspace.board.idle() || self.tools.locked {
            return false;
        }
        if matches!(action, Paste | Undo | Redo | Note) {
            return true;
        }
        if matches!(
            action,
            Horizontal
                | Vertical
                | Grid
                | Optimal
                | ArrangeName
                | ArrangeTime
                | PackFit
                | NormalizeWidth
                | NormalizeHeight
                | NormalizeSize
                | NormalizeArea
                | AlignLeft
                | AlignRight
                | AlignTop
                | AlignBottom
                | Stack
        ) {
            return !self.workspace.board.scene.is_empty();
        }
        !self.workspace.board.selection.is_empty()
    }
    pub(super) fn run_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        use Action::*;
        if !self.action_enabled(action) {
            return;
        }
        let mut items: Vec<_> = self
            .workspace
            .board
            .scene
            .items()
            .filter(|i| self.workspace.board.selection.contains(&i.id))
            .collect();
        items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        let intent = match action {
            Palette => {
                self.cancel_drag();
                self.shortcuts.open_palette();
                return;
            }
            Shortcuts => {
                self.cancel_drag();
                self.shortcuts.open_settings();
                return;
            }
            Tools => Intent::ToggleTools,
            New => Intent::New,
            Open => Intent::Open,
            Save => Intent::Save,
            Library => Intent::Home,
            CloseTab => Intent::CloseTab(0),
            NextTab => {
                if !self.workspace.tabs.is_empty() {
                    self.activate(1);
                }
                return;
            }
            Paste => Intent::Paste,
            Undo => Intent::Undo,
            Redo => Intent::Redo,
            Overlay => Intent::Overlay,
            Pin => Intent::Pin,
            Minimize => Intent::Minimize,
            Maximize => Intent::Maximize,
            Select => {
                self.workspace.board.view.pan = false;
                return;
            }
            Hand => {
                self.workspace.board.view.pan = true;
                return;
            }
            Fit => Intent::Fit,
            Focus => Intent::FitSelection,
            Previous => Intent::Focus(-1),
            Next => Intent::Focus(1),
            ZoomIn => Intent::Zoom(1. + self.tools.zoom_increment / 100.),
            ZoomOut => Intent::Zoom(1. / (1. + self.tools.zoom_increment / 100.)),
            ZoomReset => Intent::ZoomTo(1.),
            ResetCamera => Intent::ResetCamera,
            Note => Intent::AddNote(self.workspace.board.view.camera.world([
                self.workspace.board.view.viewport.center().x,
                self.workspace.board.view.viewport.center().y,
            ])),
            Delete => Intent::Commands(items.iter().map(|i| Command::Remove(i.id)).collect()),
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
                    &self.workspace.board.selection,
                    matches!(action, BringFront | BringForward),
                    matches!(action, BringFront | SendBack),
                ))
            }
            Horizontal | Vertical | Grid | Optimal | ArrangeName | ArrangeTime | PackFit
            | NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea | AlignLeft
            | AlignRight | AlignTop | AlignBottom | Stack => {
                if items.is_empty() {
                    items = self.workspace.board.scene.items().collect();
                    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                }
                let commands = match action {
                    NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea => {
                        arrange::normalize(
                            &items,
                            match action {
                                NormalizeWidth => arrange::Normalize::Width,
                                NormalizeHeight => arrange::Normalize::Height,
                                NormalizeSize => arrange::Normalize::Size,
                                _ => arrange::Normalize::Area,
                            },
                        )
                    }
                    AlignLeft | AlignRight | AlignTop | AlignBottom | Stack => arrange::align(
                        &items,
                        match action {
                            AlignLeft => arrange::Align::Left,
                            AlignRight => arrange::Align::Right,
                            AlignTop => arrange::Align::Top,
                            AlignBottom => arrange::Align::Bottom,
                            _ => arrange::Align::Stack,
                        },
                    ),
                    _ => {
                        if action == ArrangeName {
                            self.tools.order = 1;
                        }
                        if action == ArrangeTime {
                            self.tools.order = 2;
                        }
                        self.tools.arrange(
                            &mut items,
                            match action {
                                Horizontal => arrange::Arrange::Horizontal,
                                Vertical => arrange::Arrange::Vertical,
                                Grid => arrange::Arrange::Grid,
                                _ => arrange::Arrange::Optimal,
                            },
                            self.workspace.board.view.viewport.width()
                                / self.workspace.board.view.camera.zoom,
                        )
                    }
                };
                if action == PackFit {
                    self.workspace.board.view.needs_fit = true;
                }
                Intent::Commands(commands)
            }
            Lock => {
                self.tools.locked = !self.tools.locked;
                self.workspace.board.locked = self.tools.locked;
                return;
            }
            Grayscale => {
                self.tools.grayscale = !self.tools.grayscale;
                self.workspace.board.grayscale = self.tools.grayscale;
                return;
            }
            MoveDrag | ExtendDrag | DuplicateDrag | PanDrag | PanSpaceDrag | PanAltDrag => return,
        };
        self.apply(intent, event_loop);
    }
}
