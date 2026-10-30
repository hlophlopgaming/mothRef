// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera: fitting, panning, zooming and stepping through items.
use super::*;
impl App {
    /// Applies an intent of this area. False when the camera must not animate
    /// to where the intent left it.
    pub(in crate::app) fn apply_camera(
        &mut self,
        intent: Intent,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        match intent {
            Intent::View(mut view) => {
                if let Some(bounds) = self.window.resize_bounds.take() {
                    view.fit_bounds(Some(bounds));
                }
                self.workspace.board.view = view;
            }
            Intent::Fit => {
                self.fit();
                self.redraw();
            }
            Intent::Pan(delta) => {
                self.camera_motion.stop();
                let camera = &mut self.workspace.board.view.camera;
                camera.origin[0] += delta[0] / camera.zoom;
                camera.origin[1] += delta[1] / camera.zoom;
                self.redraw();
            }
            Intent::Zoom(mut factor) | Intent::ZoomTo(mut factor) => {
                if matches!(intent, Intent::ZoomTo(_)) {
                    self.camera_motion.stop();
                    factor /= self.workspace.board.view.camera.zoom;
                }
                self.camera_motion.zoom(
                    &mut self.workspace.board.view.camera,
                    [
                        self.workspace.board.view.viewport.center().x,
                        self.workspace.board.view.viewport.center().y,
                    ],
                    factor,
                    self.settings.reduced_motion || !self.settings.smooth_zoom,
                );
                self.redraw();
            }
            Intent::FitSelection => self.fit_selection(),
            Intent::ResetCamera => self.workspace.board.view.camera = Camera::default(),
            Intent::Focus(step) => {
                let mut items: Vec<_> = self
                    .workspace
                    .board
                    .scene
                    .items()
                    .filter(|i| i.blob().is_some())
                    .collect();
                items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                if !items.is_empty() {
                    let current = items
                        .iter()
                        .position(|i| self.workspace.board.selection.contains(&i.id));
                    let next = current.map_or(if step > 0 { 0 } else { items.len() - 1 }, |i| {
                        (i as i32 + step).rem_euclid(items.len() as i32) as usize
                    });
                    self.workspace.board.selection = HashSet::from([items[next].id]);
                    self.fit_selection();
                }
            }
            Intent::FocusItem(id) => {
                if self
                    .workspace
                    .board
                    .view
                    .focus_return
                    .as_ref()
                    .is_some_and(|(rid, _)| *rid == id)
                {
                    if let Some((_, camera)) = self.workspace.board.view.focus_return.take() {
                        self.workspace.board.view.camera = camera;
                    }
                } else {
                    self.workspace.board.view.focus_return =
                        Some((id, self.workspace.board.view.camera));
                    focus_selection(&mut self.workspace.board.selection, id);
                    self.fit_selection();
                }
            }
            _ => {}
        }
        true
    }
}
