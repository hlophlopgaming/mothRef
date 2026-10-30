// SPDX-License-Identifier: GPL-3.0-or-later
//! Renderer lifetime: the surface, egui state and one frame's draw+present.
use std::{sync::Arc, time::Instant};

use winit::{event_loop::EventLoopProxy, window::Window};

use super::{Wake, canvas};
use crate::gpu::Gpu;
use moth_scene::{Camera, Item};

/// The canvas draw parameters that do not depend on the per-frame surface view.
#[derive(Clone, Copy)]
pub(super) struct Frame {
    /// Draw the decorative grid when a board is visible.
    pub grid: bool,
    /// Global viewing opacity (overlay mode).
    pub opacity: f32,
    /// Apply whole-window opacity to every UI mesh, including floating areas.
    pub ui_opacity: f32,
    /// Clear colour for the canvas attachment.
    pub clear: wgpu::Color,
    /// Canvas viewport in logical pixels, for the clip rectangle.
    pub viewport: egui::Rect,
    /// View-only grayscale, never changes board content.
    pub grayscale: bool,
    /// sRGB color the grayscale view is toned with; `None` keeps it gray.
    pub tint: Option<[f32; 3]>,
    pub sampling: moth_scene::Sampling,
}

/// How a render attempt ended, so the caller can react without reaching into the GPU.
pub(super) enum Present {
    /// A frame was drawn and presented.
    Presented,
    /// The surface or device was lost; the renderer was rebuilt.
    Recovered,
    /// Nothing to draw: zero-sized, occluded or timed out.
    Skipped,
}

/// What a main window owns of the shell while another window is current; see
/// `panes.rs`. The device stays in the current shell and is shared.
pub(super) struct Parts {
    pub(super) window: Option<Arc<Window>>,
    pub(super) ctx: egui::Context,
    pub(super) egui: Option<egui_winit::State>,
    pub(super) deadline: Option<Instant>,
    pub(super) refinements: moth_ui::motion::Refinements<moth_scene::BlobId>,
    pub(super) gpu: Option<crate::gpu::WindowParts>,
}

/// Owns the window surface, GPU device, egui context and frame deadline.
pub(super) struct Shell {
    pub(super) window: Option<Arc<Window>>,
    pub(super) gpu: Option<Gpu>,
    ctx: egui::Context,
    pub(super) egui: Option<egui_winit::State>,
    software: bool,
    wake: Arc<dyn Fn() + Send + Sync>,
    proxy: EventLoopProxy<Wake>,
    pub(super) deadline: Option<Instant>,
    pub(super) refinements: moth_ui::motion::Refinements<moth_scene::BlobId>,
}

impl Shell {
    pub(super) fn new(
        proxy: EventLoopProxy<Wake>,
        wake: Arc<dyn Fn() + Send + Sync>,
        software: bool,
    ) -> Self {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::LIGHT);
        let repaint = proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            let _ = repaint.send_event(Wake::Repaint(info.delay));
        });
        Self {
            window: None,
            gpu: None,
            ctx,
            egui: None,
            software,
            wake,
            proxy,
            deadline: None,
            refinements: Default::default(),
        }
    }
    pub(super) fn context(&self) -> &egui::Context {
        &self.ctx
    }
    /// A fresh egui context with the app's repaint callback and the given theme.
    pub(super) fn new_context(&self) -> egui::Context {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::tokens(&self.ctx));
        ctx.set_zoom_factor(self.ctx.zoom_factor());
        let proxy = self.proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            let _ = proxy.send_event(Wake::Repaint(info.delay));
        });
        ctx
    }
    /// Exchanges the window-specific parts with another main window's.
    pub(super) fn swap_parts(&mut self, parts: &mut Parts) {
        std::mem::swap(&mut self.window, &mut parts.window);
        std::mem::swap(&mut self.ctx, &mut parts.ctx);
        std::mem::swap(&mut self.egui, &mut parts.egui);
        std::mem::swap(&mut self.deadline, &mut parts.deadline);
        std::mem::swap(&mut self.refinements, &mut parts.refinements);
        if let (Some(gpu), Some((surface, config, registry))) = (&mut self.gpu, &mut parts.gpu) {
            std::mem::swap(&mut gpu.surface, surface);
            std::mem::swap(&mut gpu.config, config);
            std::mem::swap(&mut gpu.registry, registry);
        }
    }
    pub(super) fn shortcut_wake(
        &self,
    ) -> Arc<dyn Fn(Option<moth_platform::shortcuts::Shortcut>) + Send + Sync> {
        let proxy = self.proxy.clone();
        Arc::new(move |shortcut| {
            let _ = proxy.send_event(shortcut.map_or(Wake::Ready, Wake::Global));
        })
    }
    pub(super) fn wake(&self) -> Arc<dyn Fn() + Send + Sync> {
        self.wake.clone()
    }
    pub(super) fn software(&self) -> bool {
        self.software
    }
    pub(super) fn upload(&mut self, image: &moth_io::Decoded, reduced: bool) {
        if let Some(gpu) = &mut self.gpu {
            let before = gpu.registry.preview_detail(image.hash);
            gpu.registry.upload(&gpu.device, &gpu.queue, image);
            if let Some((side, _)) = gpu.registry.preview_detail(image.hash)
                && side > before.map_or(0., |(side, _)| side)
            {
                let previous = before.map_or(32., |(_, displayed)| displayed);
                let level = (side / previous).log2().max(0.);
                if self
                    .refinements
                    .start(image.hash, level, Instant::now(), reduced)
                {
                    gpu.registry.set_detail_lod(image.hash, level);
                }
            }
        }
    }

    /// Whether the GPU device was lost since the last frame.
    pub(super) fn lost(&self) -> bool {
        self.gpu
            .as_ref()
            .is_some_and(|g| g.lost.load(std::sync::atomic::Ordering::Acquire))
    }

    /// Recreates the egui context and the GPU/egui state, keeping scene data intact.
    pub(super) fn recover(&mut self) -> anyhow::Result<()> {
        self.refinements.clear();
        self.gpu = None;
        let tokens = moth_ui::tokens(&self.ctx);
        let zoom = self.ctx.zoom_factor();
        self.ctx = egui::Context::default();
        moth_ui::theme(&self.ctx, &tokens);
        self.ctx.set_zoom_factor(zoom);
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

    /// Tessellates, draws and presents one frame, updating the deadline.
    pub(super) fn render(
        &mut self,
        mut output: egui::FullOutput,
        items: &[&Item],
        overlay: &[&Item],
        camera: Camera,
        frame: Frame,
    ) -> anyhow::Result<Present> {
        let Some(window) = &self.window else {
            return Ok(Present::Skipped);
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(Present::Skipped);
        }
        if let Some(state) = &mut self.egui {
            state.handle_platform_output(window, output.platform_output);
        }
        let Some(gpu) = &mut self.gpu else {
            return Ok(Present::Skipped);
        };
        let texture = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.recover()?;
                return Ok(Present::Recovered);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(Present::Skipped);
            }
            _ => return Err(anyhow::anyhow!("Surface validation failed")),
        };
        // Resize events can lag window.inner_size(); clip to the acquired attachment.
        let size = texture.texture.size();
        gpu.registry.grayscale = frame.grayscale;
        gpu.registry.tint = frame.tint;
        gpu.registry.sampling = frame.sampling;
        let tokens = moth_ui::tokens(&self.ctx);
        gpu.registry.backdrops = backdrops(tokens);
        gpu.registry.panel = moth_render::Backdrop {
            fill: linear(tokens.surface),
            border: linear(tokens.border),
            radius: Some(tokens.radius_card as f32),
        };
        let mut jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        if frame.ui_opacity < 1. {
            for job in &mut jobs {
                if let egui::epaint::Primitive::Mesh(mesh) = &mut job.primitive {
                    for vertex in &mut mesh.vertices {
                        vertex.color = vertex.color.gamma_multiply(frame.ui_opacity);
                    }
                }
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: output.pixels_per_point,
        };
        for (id, delta) in &output.textures_delta.set {
            for part in delta {
                gpu.registry
                    .ui
                    .update_texture(&gpu.device, &gpu.queue, *id, part);
            }
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let buffers =
            gpu.registry
                .ui
                .update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
        let view = texture.texture.create_view(&Default::default());
        let scale = output.pixels_per_point;
        let draw_result = gpu.registry.draw(
            &gpu.device,
            &gpu.queue,
            moth_render::Target {
                grid: frame.grid,
                opacity: frame.opacity,
                view: &view,
                screen: [size.width as f32 / scale, size.height as f32 / scale],
                clip: Some(canvas::canvas_clip(
                    frame.viewport,
                    scale,
                    [size.width, size.height],
                )),
                clear: frame.clear,
            },
            items,
            camera,
        );
        if draw_result.is_ok() {
            gpu.registry.draw_overlay(
                &gpu.device,
                &gpu.queue,
                moth_render::Target {
                    grid: false,
                    opacity: 1.,
                    view: &view,
                    screen: [size.width as f32 / scale, size.height as f32 / scale],
                    clip: Some(canvas::canvas_clip(
                        frame.viewport,
                        scale,
                        [size.width, size.height],
                    )),
                    clear: frame.clear,
                },
                overlay,
                camera,
            );
        }
        if let Err(error) = draw_result {
            for id in &output.textures_delta.free {
                gpu.registry.ui.free_texture(id);
            }
            output.textures_delta.clear();
            return Err(error.into());
        }
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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
        window.pre_present_notify();
        gpu.queue.present(texture);
        for id in &output.textures_delta.free {
            gpu.registry.ui.free_texture(id);
        }
        output.textures_delta.clear();
        self.deadline = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|o| Instant::now().checked_add(o.repaint_delay));
        Ok(Present::Presented)
    }
}
/// Theme group colors in the renderer's straight-alpha linear form.
pub(super) fn backdrops(
    tokens: moth_ui::Tokens,
) -> [moth_render::Backdrop; moth_scene::GROUP_COLORS as usize] {
    std::array::from_fn(|code| {
        let [fill, border] = tokens.group_backdrop(code as u8);
        moth_render::Backdrop {
            fill: linear(fill),
            border: linear(border),
            radius: None,
        }
    })
}
/// A theme color in the renderer's straight-alpha linear form.
fn linear(color: egui::Color32) -> [f32; 4] {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    [
        egui::ecolor::linear_f32_from_gamma_u8(r),
        egui::ecolor::linear_f32_from_gamma_u8(g),
        egui::ecolor::linear_f32_from_gamma_u8(b),
        a as f32 / 255.,
    ]
}
