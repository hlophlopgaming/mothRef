// SPDX-License-Identifier: GPL-3.0-or-later
//! Looking at the board: selection, camera, the View toggle, lock, grayscale
//! and the Layers drawer.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn view_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
            Layers => !self.workspace.library,
            Deselect => !self.workspace.board.selection.is_empty(),
            SelectAll => !self.workspace.board.scene.is_empty(),
            Lock | Grayscale => self.workspace.board.idle(),
            _ => true,
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_view(
        &mut self,
        action: Action,
        _event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        let intent = match action {
            ToggleView => {
                self.workspace.board.appearance.view = !self.workspace.board.appearance.view;
                self.workspace.board.appearance.over = false;
                self.remember_appearance();
                return None;
            }
            Select => return None,
            Deselect => {
                self.workspace.board.selection.clear();
                return None;
            }
            SelectAll => {
                // Everything at the current level: the board, or the entered group.
                let board = &mut self.workspace.board;
                board.selection = match board.entered {
                    Some(group) => board.scene.children(group).map(|item| item.id).collect(),
                    None => board
                        .scene
                        .items()
                        .filter(|item| item.parent.is_none())
                        .map(|item| item.id)
                        .collect(),
                };
                selection::with_members(&board.scene, &mut board.selection, None);
                return None;
            }
            Fit => Intent::Fit,
            Focus => Intent::FitSelection,
            Previous => Intent::Focus(-1),
            Next => Intent::Focus(1),
            ZoomIn => Intent::Zoom(1. + self.settings.zoom_increment / 100.),
            ZoomOut => Intent::Zoom(1. / (1. + self.settings.zoom_increment / 100.)),
            ZoomReset => Intent::ZoomTo(1.),
            PanLeft => Intent::Pan([-self.settings.keyboard_pan_step, 0.]),
            PanRight => Intent::Pan([self.settings.keyboard_pan_step, 0.]),
            PanUp => Intent::Pan([0., -self.settings.keyboard_pan_step]),
            PanDown => Intent::Pan([0., self.settings.keyboard_pan_step]),
            ResetCamera => Intent::ResetCamera,
            Layers => {
                let ctx = self.shell.context();
                let key = egui::Id::new("layers_open");
                ctx.data_mut(|data| {
                    let open = data.get_temp_mut_or_default::<bool>(key);
                    *open = !*open;
                });
                return None;
            }
            Lock => {
                self.workspace.board.locked = !self.workspace.board.locked;
                return None;
            }
            Grayscale => {
                self.workspace.board.grayscale = !self.workspace.board.grayscale;
                return None;
            }
            _ => return None,
        };
        Some(intent)
    }
}
