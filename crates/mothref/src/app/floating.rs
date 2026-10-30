// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent surfaces share the board registry, pixels and video-memory budget.
use super::*;
pub(super) struct ImageWindow {
    pub window: Arc<Window>,
    pub item: Uuid,
    input: moth_platform::input::Adapter,
    pub surface: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
}
impl App {
    pub(super) fn float_selection(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .window
            .floating
            .iter()
            .any(|w| self.workspace.board.selection.contains(&w.item))
        {
            self.window
                .floating
                .retain(|w| !self.workspace.board.selection.contains(&w.item));
            return;
        }
        let ids: Vec<_> = self
            .workspace
            .board
            .selection
            .iter()
            .copied()
            .filter(|id| {
                self.workspace
                    .board
                    .scene
                    .get(*id)
                    .is_some_and(|i| i.blob().is_some())
            })
            .filter(|id| !self.window.floating.iter().any(|w| w.item == *id))
            .collect();
        if self.window.floating.len() + ids.len() > 32 {
            self.workspace.board.notices.message = moth_ui::Text::OverlayWindowLimit.into();
            return;
        }
        let Some(gpu) = &self.shell.gpu else {
            return;
        };
        for id in ids {
            let result = (|| -> anyhow::Result<ImageWindow> {
                let window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title(moth_ui::text(moth_ui::Text::OverlaySelection))
                            .with_decorations(false)
                            .with_transparent(true)
                            .with_inner_size(winit::dpi::LogicalSize::new(400., 400.)),
                    )?,
                );
                moth_platform::Desktop
                    .window_effects(&window, true, true, false)
                    .map_err(anyhow::Error::msg)?;
                let surface = Some(gpu.image_surface(window.clone())?);
                Ok(ImageWindow {
                    window,
                    item: id,
                    input: Default::default(),
                    surface,
                })
            })();
            match result {
                Ok(image) => self.window.floating.push(image),
                Err(error) => {
                    self.workspace.board.notices.message = error.to_string().into();
                    break;
                }
            }
        }
        self.redraw();
    }
    pub(super) fn floating_event(&mut self, id: WindowId, event: &WindowEvent) -> bool {
        let Some(index) = self
            .window
            .floating
            .iter()
            .position(|w| w.window.id() == id)
        else {
            return false;
        };
        if matches!(event, WindowEvent::CloseRequested)
            || matches!(event,WindowEvent::KeyboardInput {event,..} if event.logical_key == Key::Named(NamedKey::Escape))
        {
            self.window.floating.remove(index);
            return true;
        }
        // Native input is translated through the same mouse/touch/pen abstraction.
        let window = self.window.floating[index].window.clone();
        if let Some(Input::Pointer(pointer)) = self.window.floating[index]
            .input
            .event(event, window.scale_factor())
            && pointer.phase == Phase::Down
        {
            let size = window.inner_size();
            let edge = moth_ui::tokens(self.shell.context()).space_s as f32;
            let [x, y] = pointer.position;
            use winit::window::ResizeDirection::*;
            let direction = match (
                x < edge,
                x > size.width as f32 / window.scale_factor() as f32 - edge,
                y < edge,
                y > size.height as f32 / window.scale_factor() as f32 - edge,
            ) {
                (true, _, true, _) => Some(NorthWest),
                (_, true, true, _) => Some(NorthEast),
                (true, _, _, true) => Some(SouthWest),
                (_, true, _, true) => Some(SouthEast),
                (true, _, _, _) => Some(West),
                (_, true, _, _) => Some(East),
                (_, _, true, _) => Some(North),
                (_, _, _, true) => Some(South),
                _ => None,
            };
            let result = if let Some(edge) = direction {
                window.drag_resize_window(edge)
            } else {
                window.drag_window()
            };
            match result {
                Ok(()) => self.window.floating[index].input.release_mouse(),
                Err(error) => tracing::warn!(%error,"floating image move failed"),
            }
        }
        match event {
            WindowEvent::Resized(_)
            | WindowEvent::Occluded(false)
            | WindowEvent::ScaleFactorChanged { .. } => window.request_redraw(),
            WindowEvent::RedrawRequested => {
                let result = self.draw_floating(index);
                if let Err(error) = result {
                    self.workspace.board.notices.message = error.to_string().into();
                }
            }
            _ => {}
        }
        true
    }
    fn draw_floating(&mut self, index: usize) -> anyhow::Result<()> {
        let image = &mut self.window.floating[index];
        let Some(item) = self.workspace.board.scene.get(image.item) else {
            self.window.floating.remove(index);
            return Ok(());
        };
        let Some(gpu) = &mut self.shell.gpu else {
            return Ok(());
        };
        if gpu.lost.load(Ordering::Acquire) {
            self.redraw();
            return Ok(());
        }
        let size = image.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        if image.surface.is_none() {
            image.surface = Some(gpu.image_surface(image.window.clone())?);
        }
        let Some((surface, config)) = &mut image.surface else {
            return Ok(());
        };
        if config.width != size.width || config.height != size.height {
            config.width = size.width;
            config.height = size.height;
            surface.configure(&gpu.device, config);
        }
        let texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                image.surface = None;
                image.window.request_redraw();
                return Ok(());
            }
            _ => return Ok(()),
        };
        let screen = [size.width as f32, size.height as f32];
        let bounds = item.rect.rotated_envelope(item.transform.rotation);
        let camera = fit_image(
            Rect {
                x: bounds.lower()[0],
                y: bounds.lower()[1],
                width: bounds.upper()[0] - bounds.lower()[0],
                height: bounds.upper()[1] - bounds.lower()[1],
            },
            screen,
        );
        let view = texture.texture.create_view(&Default::default());
        gpu.registry.draw(
            &gpu.device,
            &gpu.queue,
            moth_render::Target {
                view: &view,
                screen,
                clip: None,
                clear: wgpu::Color::TRANSPARENT,
                grid: false,
                opacity: self.workspace.board.appearance.images
                    * self.workspace.board.appearance.window,
            },
            &[item],
            camera,
        )?;
        image.window.pre_present_notify();
        gpu.queue.present(texture);
        Ok(())
    }
}
pub(super) fn fit_image(bounds: Rect, screen: [f32; 2]) -> Camera {
    let zoom = (screen[0] / bounds.width)
        .min(screen[1] / bounds.height)
        .clamp(0.01, 64.);
    Camera {
        zoom,
        origin: [
            bounds.x + bounds.width / 2. - screen[0] / zoom / 2.,
            bounds.y + bounds.height / 2. - screen[1] / zoom / 2.,
        ],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn floating_camera_centers_the_rotated_envelope() {
        let bounds = Rect {
            x: -100.,
            y: 30.,
            width: 400.,
            height: 200.,
        };
        let camera = fit_image(bounds, [300., 300.]);
        assert_eq!(camera.zoom, 0.75);
        assert_eq!(camera.screen([100., 130.]), [150., 150.]);
    }
    #[test]
    fn floating_views_share_pixels_and_preserve_transparent_letterboxing() {
        pollster::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            // Match the production fallback: not every desktop installs a software adapter.
            let (_, device, queue) = moth_render::request_device(&instance, None, false)
                .await
                .unwrap();
            let mut registry = moth_render::Registry::new(
                &device,
                wgpu::TextureFormat::Rgba8Unorm,
                moth_io::ImageFormat::Rgba8,
            );
            let source = moth_render::headless::fixture(128, 32);
            registry.upload(&device, &queue, &source);
            let bytes_before = registry.allocated_bytes();
            let bounds = Rect {
                x: -5.,
                y: 10.,
                width: 32.,
                height: 16.,
            };
            let item = Item::image(source.hash, bounds, None);
            for (width, opacity) in [(64, 1.), (128, 0.5)] {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("floating golden"),
                    size: wgpu::Extent3d {
                        width,
                        height: 64,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_formats: &[],
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                });
                let screen = [width as f32, 64.];
                registry
                    .draw(
                        &device,
                        &queue,
                        moth_render::Target {
                            view: &texture.create_view(&Default::default()),
                            screen,
                            clip: None,
                            clear: wgpu::Color::TRANSPARENT,
                            grid: false,
                            opacity,
                        },
                        &[&item],
                        fit_image(bounds, screen),
                    )
                    .unwrap();
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("floating golden readback"),
                    size: 512 * 64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(512),
                            rows_per_image: Some(64),
                        },
                    },
                    texture.size(),
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::sync_channel(1);
                buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        let _ = tx.send(result);
                    });
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: Some(Duration::from_secs(10)),
                    })
                    .unwrap();
                rx.recv().unwrap().unwrap();
                let pixels = buffer.slice(..).get_mapped_range().unwrap();
                for (x, y, color) in [
                    (width / 2, 32, [128., 0., 127., 255.]),
                    (
                        width / 2,
                        0,
                        if width == 64 {
                            [0.; 4]
                        } else {
                            [128., 0., 127., 255.]
                        },
                    ),
                ] {
                    let offset = (y * 512 + x * 4) as usize;
                    for (actual, expected) in pixels[offset..offset + 4].iter().zip(color) {
                        assert!(actual.abs_diff((expected * opacity).round() as u8) <= 2);
                    }
                }
                drop(pixels);
                buffer.unmap();
                assert_eq!(
                    registry.allocated_bytes(),
                    bytes_before
                        + if opacity < 1. {
                            u64::from(width) * 64 * 4
                        } else {
                            0
                        }
                );
            }
        });
    }
}
