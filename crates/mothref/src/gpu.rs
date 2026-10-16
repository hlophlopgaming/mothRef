// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::{Arc, atomic::AtomicBool};
use winit::window::Window;
pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub registry: moth_render::Registry,
    /// Makes hardware and software rendering distinguishable.
    pub adapter: String,
    pub lost: Arc<AtomicBool>,
}
impl Gpu {
    pub async fn new(
        window: Arc<Window>,
        wake: Arc<dyn Fn() + Send + Sync>,
        software: bool,
    ) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(window.clone()),
        ));
        let surface = instance.create_surface(window.clone())?;
        let (adapter, device, queue) =
            moth_render::request_device(&instance, Some(&surface), software)
                .await
                .map_err(anyhow::Error::msg)?;
        let info = adapter.get_info();
        let label = format!(
            "{} · {:?} · {:?}",
            info.name, info.device_type, info.backend
        );
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow::anyhow!("No compatible surface configuration"))?;
        config.format = surface
            .get_capabilities(&adapter)
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let registry = moth_render::Registry::new(&device, config.format);
        let lost = moth_render::watch_device(&device, wake);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            registry,
            adapter: label,
            lost,
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}
