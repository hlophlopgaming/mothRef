// SPDX-License-Identifier: GPL-3.0-or-later
//! Real GPU checks and measurements, shared by CI and xtask.
use crate::Registry;
use moth_io::{Decoded, Mip};
use moth_scene::{Camera, Command, Item, Rect, Scene};
use std::time::{Duration, Instant};
/// A measurement includes GPU completion, rather than only CPU submission time.
pub struct Measurement {
    /// Makes hardware and software rendering distinguishable.
    pub adapter: String,
    /// Includes GPU completion so submission speed cannot hide slow frames.
    pub p99_ms: f64,
    /// Reports allocated GPU pages rather than source image sizes.
    pub texture_bytes: u64,
    /// Records the sample size needed to interpret a percentile.
    pub frames: usize,
}
/// Generates deterministic colored references without relying on external images.
pub fn fixture(index: u32, side: u32) -> Decoded {
    let bytes = index.to_le_bytes();
    let color = [bytes[0], bytes[1], 127, 255];
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for _ in 0..side * side {
        rgba.extend_from_slice(&color);
    }
    let mut mips = vec![Mip {
        width: side,
        height: side,
        rgba,
    }];
    let mut s = side;
    while s > 1 {
        s /= 2;
        let mut rgba = Vec::with_capacity((s * s * 4) as usize);
        for _ in 0..s * s {
            rgba.extend_from_slice(&color);
        }
        mips.push(Mip {
            width: s,
            height: s,
            rgba,
        });
    }
    Decoded {
        hash: hash_fixture(index),
        original: [side, side],
        mips,
    }
}
fn hash_fixture(index: u32) -> moth_scene::BlobId {
    // Content identity is computed by the decoder for the deterministic PNG fixture.
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&index.to_le_bytes());
    moth_scene::BlobId::from_bytes(bytes)
}
/// A 100×100 board exercises distinct resident layers and spatial culling.
pub fn board() -> Result<Scene, String> {
    let mut scene = Scene::default();
    for i in 0..10_000u32 {
        let item = Item::image(
            hash_fixture(i),
            Rect {
                x: (i % 100) as f32 * 10.,
                y: (i / 100) as f32 * 10.,
                width: 9.,
                height: 9.,
            },
            scene.top(),
        );
        scene
            .apply(Command::Insert(item))
            .map_err(|e| e.to_string())?;
    }
    Ok(scene)
}
/// GPU objects are owned by one headless session and released together.
pub struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    registry: Registry,
    target: wgpu::Texture,
    scene: Scene,
    adapter: String,
}
impl Harness {
    /// Adapter failure is an error, never a silently skipped golden test.
    pub async fn new(software: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let (adapter, device, queue) = crate::request_device(&instance, None, software).await?;
        let mut registry = Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        for i in 0..10_000 {
            registry.upload(&device, &queue, &fixture(i, 16));
        }
        let target = target(&device, 1000, 1000);
        let scene = board()?;
        Ok(Self {
            device,
            queue,
            registry,
            target,
            scene,
            adapter: adapter.get_info().name,
        })
    }
    /// Moves the camera over a loaded board; no disk or decode is involved in panning.
    pub fn frame(&mut self, index: usize) -> Result<Duration, String> {
        let start = Instant::now();
        let camera = Camera {
            origin: [(index % 20) as f32 * 0.1, 0.],
            zoom: 1.,
        };
        let items = self.scene.query(Rect {
            x: camera.origin[0],
            y: 0.,
            width: 1000.,
            height: 1000.,
        });
        let view = self.target.create_view(&Default::default());
        self.registry.draw(
            &self.device,
            &self.queue,
            crate::Target {
                view: &view,
                screen: [1000., 1000.],
                clip: None,
                clear: wgpu::Color::BLACK,
            },
            &items,
            camera,
        );
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(10)),
            })
            .map_err(|e| e.to_string())?;
        Ok(start.elapsed())
    }
    /// Measures a newly uploaded paste through GPU completion on an otherwise blank board.
    pub fn show(&mut self, image: &Decoded) -> Result<(), String> {
        self.scene = Scene::default();
        self.registry.upload(&self.device, &self.queue, image);
        self.scene
            .apply(Command::Insert(Item::image(
                image.hash,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 600.,
                    height: 600. * image.original[1] as f32 / image.original[0] as f32,
                },
                None,
            )))
            .map_err(|e| e.to_string())?;
        self.frame(0)?;
        Ok(())
    }
    /// Warmup is excluded from the measured distribution.
    pub fn measure(&mut self, frames: usize) -> Result<Measurement, String> {
        for i in 0..10 {
            self.frame(i)?;
        }
        let mut times = Vec::with_capacity(frames);
        for i in 0..frames {
            times.push(self.frame(i)?.as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        Ok(Measurement {
            adapter: self.adapter.clone(),
            p99_ms: times[((frames * 99) / 100).min(frames - 1)],
            texture_bytes: self.registry.allocated_bytes(),
            frames,
        })
    }
}
fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
/// Compares every pixel against a two-image overlap golden with channel tolerance.
pub async fn golden(software: bool) -> Result<(), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let (_, device, queue) = crate::request_device(&instance, None, software).await?;
    let mut registry = Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm);
    let mut scene = Scene::default();
    for (i, color, offset) in [(0, [120, 90, 60, 255], 0.), (1, [0, 0, 255, 255], 16.)] {
        let mut image = fixture(i, 16);
        for mip in &mut image.mips {
            for pixel in mip.rgba.as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&color);
            }
        }
        registry.upload(&device, &queue, &image);
        scene
            .apply(Command::Insert(Item::image(
                image.hash,
                Rect {
                    x: offset,
                    y: offset,
                    width: 32.,
                    height: 32.,
                },
                scene.top(),
            )))
            .map_err(|e| e.to_string())?;
    }
    // The final green item must survive crossing the instance-batch boundary.
    let backdrop = scene
        .items()
        .min_by(|a, b| a.z.cmp(&b.z))
        .expect("the golden scene contains the two images inserted above")
        .clone();
    for _ in 0..10_000 {
        let mut item = backdrop.clone();
        item.id = moth_scene::Uuid::now_v7();
        scene
            .apply(Command::Insert(item))
            .map_err(|e| e.to_string())?;
    }
    let odd = moth_io::preview_rgba(31, 17, &[0, 255, 0, 255].repeat(31 * 17), 31)
        .map_err(|e| e.to_string())?;
    registry.upload(&device, &queue, &odd);
    scene
        .apply(Command::Insert(Item::image(
            odd.hash,
            Rect {
                x: 48.,
                y: 0.,
                width: 8.,
                height: 8.,
            },
            scene.top(),
        )))
        .map_err(|e| e.to_string())?;
    let texture = target(&device, 64, 64);
    let view = texture.create_view(&Default::default());
    let items = scene.query(Rect {
        x: 0.,
        y: 0.,
        width: 64.,
        height: 64.,
    });
    registry.draw(
        &device,
        &queue,
        crate::Target {
            view: &view,
            screen: [64., 64.],
            clip: None,
            clear: wgpu::Color::BLACK,
        },
        &items,
        Camera::default(),
    );
    let mut encoder = device.create_command_encoder(&Default::default());
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("golden readback"),
        size: 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
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
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    {
        let bytes = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        for y in 0..64 {
            for x in 0..64 {
                let expected: [u8; 4] = if (48..56).contains(&x) && y < 8 {
                    [0, 255, 0, 255]
                } else if (16..48).contains(&x) && (16..48).contains(&y) {
                    [0, 0, 255, 255]
                } else if x < 32 && y < 32 {
                    [120, 90, 60, 255]
                } else {
                    [0, 0, 0, 255]
                };
                let pixel = &bytes[(y * 256 + x * 4)..(y * 256 + x * 4 + 4)];
                if pixel.iter().zip(expected).any(|(&a, b)| a.abs_diff(b) > 2) {
                    return Err(format!(
                        "golden mismatch at ({x},{y}): {pixel:?} != {expected:?}"
                    ));
                }
            }
        }
    }
    buffer.unmap();
    registry.clear();
    if registry.allocated_bytes() != 0 {
        return Err("registry leaked textures".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn headless_golden() {
        pollster::block_on(super::golden(false)).unwrap();
    }
}
