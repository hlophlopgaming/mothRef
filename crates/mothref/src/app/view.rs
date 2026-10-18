// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns the camera, selection and transient presentation of a document.
use super::*;

/// Owns board presentation independently of its journal and loading workers.
pub(super) struct View {
    pub(super) camera: Camera,
    pub(super) focus_return: Option<(Uuid, Camera)>,
    pub(super) needs_fit: bool,
    pub(super) pan: bool,
    pub(super) locked: bool,
    pub(super) grayscale: bool,
    pub(super) selection: HashSet<Uuid>,
    pub(super) viewport: egui::Rect,
    pub(super) message: String,
}
impl Default for View {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            focus_return: None,
            needs_fit: false,
            pan: true,
            locked: false,
            grayscale: false,
            selection: HashSet::new(),
            viewport: egui::Rect::NOTHING,
            message: String::new(),
        }
    }
}
impl View {
    pub(super) fn fit(&mut self, scene: &Scene) {
        self.needs_fit = true;
        if !self.viewport.is_finite() || !self.viewport.is_positive() {
            return;
        }
        if let Some(bounds) = scene.bounds() {
            self.camera.zoom = ((self.viewport.width() - 48.) / bounds.width)
                .min((self.viewport.height() - 48.) / bounds.height)
                .clamp(0.01, 64.);
            self.camera.origin = [
                bounds.x + bounds.width / 2. - self.viewport.center().x / self.camera.zoom,
                bounds.y + bounds.height / 2. - self.viewport.center().y / self.camera.zoom,
            ];
        }
        self.needs_fit = false;
    }
}
