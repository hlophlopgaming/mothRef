// SPDX-License-Identifier: GPL-3.0-or-later
//! An independent interactive controller prevents click-through from trapping the user.
use super::*;
use moth_platform::Platform;
use winit::keyboard::{Key, NamedKey};
pub(super) struct Overlay {
    pub(super) window: Arc<Window>,
    gpu: Gpu,
    ctx: egui::Context,
    state: egui_winit::State,
    wake: Arc<dyn Fn() + Send + Sync>,
    software: bool,
    pub(super) opacity: f32,
    pub(super) click_through: bool,
    pub(super) on_top: bool,
    pub(super) exit: bool,
    pub(super) applied: (bool, bool),
}
impl Overlay {
    pub(super) fn new(
        event_loop: &ActiveEventLoop,
        wake: Arc<dyn Fn() + Send + Sync>,
        software: bool,
        on_top: bool,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(moth_ui::text(moth_ui::Text::Overlay))
                    .with_decorations(false)
                    .with_resizable(true)
                    .with_inner_size(winit::dpi::LogicalSize::new(530., 64.)),
            )?,
        );
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx);
        let repaint = wake.clone();
        ctx.set_request_repaint_callback(move |_| repaint());
        let gpu = pollster::block_on(Gpu::new(window.clone(), wake.clone(), software))?;
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(2048),
        );
        moth_platform::Desktop
            .window_effects(&window, false, true, false)
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            window,
            gpu,
            ctx,
            state,
            wake,
            software,
            opacity: 0.55,
            click_through: false,
            on_top,
            exit: false,
            applied: (on_top, false),
        })
    }
    pub(super) fn event(&mut self, event: &WindowEvent) {
        let _ = self.state.on_window_event(&self.window, event);
        match event {
            WindowEvent::CloseRequested => self.exit = true,
            WindowEvent::KeyboardInput { event, .. }
                if event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                self.exit = true
            }
            WindowEvent::Resized(size) => self.gpu.resize(size.width, size.height),
            _ => {}
        }
        self.window.request_redraw();
    }
    pub(super) fn frame(&mut self) -> anyhow::Result<()> {
        use moth_ui::{
            Text,
            icons::{self, Icon},
            text,
        };
        if self.gpu.lost.load(Ordering::Acquire) {
            self.gpu = pollster::block_on(Gpu::new(
                self.window.clone(),
                self.wake.clone(),
                self.software,
            ))?;
            self.ctx = egui::Context::default();
            moth_ui::theme(&self.ctx);
            let repaint = self.wake.clone();
            self.ctx.set_request_repaint_callback(move |_| repaint());
            self.state = egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                self.window.as_ref(),
                Some(self.window.scale_factor() as f32),
                self.window.theme(),
                Some(2048),
            );
        }
        let texture = match self.gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                let size = self.window.inner_size();
                self.gpu.resize(size.width, size.height);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let mut output = self
            .ctx
            .run_ui(self.state.take_egui_input(&self.window), |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(moth_ui::PANEL).inner_margin(8))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if icons::button(ui, Icon::Pan, text(Text::Pan), false).drag_started() {
                                let _ = self.window.drag_window();
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(text(Text::Opacity))
                                        .size(10.)
                                        .color(moth_ui::MUTED),
                                );
                                ui.add(
                                    egui::Slider::new(&mut self.opacity, 0.1..=1.)
                                        .show_value(false),
                                );
                            });
                            ui.monospace(format!("{:.0}%", self.opacity * 100.));
                            ui.toggle_value(&mut self.click_through, text(Text::ClickThrough));
                            let caps = moth_platform::Desktop.window_capabilities(&self.window);
                            ui.add_enabled_ui(caps.on_top, |ui| {
                                ui.toggle_value(&mut self.on_top, text(Text::OnTop));
                            });
                            self.exit |=
                                icons::button(ui, Icon::Close, text(Text::ExitOverlay), false)
                                    .clicked();
                        });
                    });
            });
        self.state
            .handle_platform_output(&self.window, output.platform_output);
        let size = self.window.inner_size();
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: output.pixels_per_point,
        };
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        let gpu = &mut self.gpu;
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                gpu.registry
                    .ui
                    .update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let buffers =
            gpu.registry
                .ui
                .update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
        let view = texture.texture.create_view(&Default::default());
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay controls"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            gpu.registry
                .ui
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        gpu.queue
            .submit(buffers.into_iter().chain([encoder.finish()]));
        self.window.pre_present_notify();
        gpu.queue.present(texture);
        for id in &output.textures_delta.free {
            gpu.registry.ui.free_texture(id);
        }
        output.textures_delta.clear();
        Ok(())
    }
}
