// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns the window, GPU state and egui context; recreates them on device loss.
use super::*;

/// Owns the rendering surface and event-loop plumbing so device loss can rebuild
/// them without touching the scene or the workers.
pub(super) struct Shell {
    pub(super) window: Option<Arc<Window>>,
    pub(super) gpu: Option<Gpu>,
    pub(super) ctx: egui::Context,
    pub(super) egui: Option<egui_winit::State>,
    pub(super) proxy: EventLoopProxy<Wake>,
    pub(super) wake: Arc<dyn Fn() + Send + Sync>,
    pub(super) deadline: Option<Instant>,
    pub(super) software: bool,
}

impl Shell {
    /// Canvas texture format for decode requests; RGBA8 until the GPU exists.
    pub(super) fn image_format(&self) -> moth_io::ImageFormat {
        self.gpu
            .as_ref()
            .map(|g| g.registry.image_format())
            .unwrap_or_default()
    }
    /// Requests a redraw of the owned window, if one exists.
    pub(super) fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
    /// Rebuilds the egui context and renderer after device/surface loss.
    pub(super) fn recreate(&mut self) -> anyhow::Result<()> {
        self.gpu = None;
        self.ctx = egui::Context::default();
        moth_ui::theme(&self.ctx);
        let proxy = self.proxy.clone();
        self.ctx.set_request_repaint_callback(move |info| {
            let _ = proxy.send_event(Wake::Repaint(info.delay));
        });
        if let Some(window) = &self.window {
            self.gpu = Some(pollster::block_on(Gpu::new(
                window.clone(),
                self.wake.clone(),
                self.software,
            ))?);
            self.egui = Some(egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                window.theme(),
                Some(2048),
            ));
        }
        Ok(())
    }
}
