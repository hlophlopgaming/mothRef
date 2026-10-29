// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::{Arc, atomic::AtomicBool};
use winit::window::Window;
/// The per-window part of the renderer: surface, its configuration and textures.
pub type WindowParts = (
    wgpu::Surface<'static>,
    wgpu::SurfaceConfiguration,
    moth_render::Registry,
);
pub struct Gpu {
    instance: wgpu::Instance,
    native_adapter: wgpu::Adapter,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub registry: moth_render::Registry,
    /// Makes hardware and software rendering distinguishable.
    pub adapter: String,
    /// Adapter name and memory model, for video-memory detection.
    pub name: String,
    pub kind: moth_platform::gpu::AdapterKind,
    pub transparent: bool,
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
        let transparent = surface
            .get_capabilities(&adapter)
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied);
        if transparent {
            config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);
        let registry = moth_render::Registry::new(
            &device,
            config.format,
            moth_render::pick_image_format(&adapter),
        );
        let lost = moth_render::watch_device(&device, wake);
        Ok(Self {
            instance,
            native_adapter: adapter,
            surface,
            device,
            queue,
            config,
            registry,
            adapter: label,
            name: info.name.clone(),
            kind: match info.device_type {
                wgpu::DeviceType::DiscreteGpu => moth_platform::gpu::AdapterKind::Discrete,
                wgpu::DeviceType::IntegratedGpu => moth_platform::gpu::AdapterKind::Integrated,
                wgpu::DeviceType::Cpu => moth_platform::gpu::AdapterKind::Software,
                _ => moth_platform::gpu::AdapterKind::Other,
            },
            lost,
            transparent,
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

impl Gpu {
    /// Surface and texture registry for another main window on the same device.
    /// Each window keeps its own registry: switching boards clears textures, and a
    /// shared registry would empty the other window's board too.
    pub fn window_parts(&self, window: Arc<Window>) -> anyhow::Result<WindowParts> {
        let (surface, config) = self.image_surface(window)?;
        let registry = moth_render::Registry::new(
            &self.device,
            config.format,
            moth_render::pick_image_format(&self.native_adapter),
        );
        Ok((surface, config, registry))
    }
    pub fn image_surface(
        &self,
        window: Arc<Window>,
    ) -> anyhow::Result<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)> {
        let surface = self.instance.create_surface(window.clone())?;
        let caps = surface.get_capabilities(&self.native_adapter);
        anyhow::ensure!(
            caps.formats.contains(&self.config.format),
            "Incompatible image window surface"
        );
        let size = window.inner_size();
        let mut config = self.config.clone();
        config.width = size.width.max(1);
        config.height = size.height.max(1);
        config.alpha_mode = if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            wgpu::CompositeAlphaMode::Auto
        };
        surface.configure(&self.device, &config);
        Ok((surface, config))
    }
}
