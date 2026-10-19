// SPDX-License-Identifier: GPL-3.0-or-later
//! Real GPU checks and measurements, shared by CI and xtask.
use crate::Registry;
use moth_io::{Decoded, ImageFormat, Mip};
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
    let raw = index.to_le_bytes();
    let color = [raw[0], raw[1], 127, 255];
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for _ in 0..side * side {
        rgba.extend_from_slice(&color);
    }
    let mut mips = vec![Mip {
        width: side,
        height: side,
        bytes: rgba,
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
            bytes: rgba,
        });
    }
    Decoded {
        hash: hash_fixture(index),
        original: [side, side],
        mips,
        format: ImageFormat::Rgba8,
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
        let mut registry =
            Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Rgba8);
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
        self.registry
            .draw(
                &self.device,
                &self.queue,
                crate::Target {
                    opacity: 1.,
                    grid: false,
                    view: &view,
                    screen: [1000., 1000.],
                    clip: None,
                    clear: wgpu::Color::BLACK,
                },
                &items,
                camera,
            )
            .map_err(|error| error.to_string())?;
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
/// Copies a rendered target back to CPU bytes so a golden can compare pixels.
fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    stride: u32,
) -> Result<Vec<u8>, String> {
    let size = texture.size();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("golden readback"),
        size: u64::from(stride) * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(size.height),
            },
        },
        wgpu::Extent3d {
            width: size.width,
            height: size.height,
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
    let bytes = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?
        .to_vec();
    buffer.unmap();
    Ok(bytes)
}
/// A block stripe keeps every sample a few pixels clear of a color boundary.
fn stripe(pixel: u32) -> u8 {
    ((pixel / 8) % 4 * 36) as u8
}
/// Golden for arbitrary rotation, content crop/zoom and mirroring.
///
/// One 32x32 source of 8-pixel stripe blocks is drawn at (16,16) under several transforms.
/// Each sample names the target pixel, the source pixel it must show, and `None` where the
/// frame must clip the content away; the arithmetic behind every pair is in the comments.
pub async fn golden_transform(software: bool) -> Result<(), String> {
    use moth_scene::Transform;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let (_, device, queue) = crate::request_device(&instance, None, software).await?;
    let mut registry = Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Rgba8);
    let mut image = fixture(0, 32);
    for mip in &mut image.mips {
        for (index, pixel) in mip.bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = (index as u32 % mip.width, index as u32 / mip.width);
            *pixel = [stripe(x), stripe(y), 0, 255];
        }
    }
    registry.upload(&device, &queue, &image);
    let square = Rect {
        x: 16.,
        y: 16.,
        width: 32.,
        height: 32.,
    };
    type Sample = ((u32, u32), Option<(u32, u32)>);
    let cases: [(&str, Rect, Transform, &[Sample]); 9] = [
        (
            "identity",
            square,
            Transform::default(),
            // At scale 1 the source exactly covers the frame, so world equals source.
            &[((19, 35), Some((4, 20))), ((43, 19), Some((28, 4)))],
        ),
        (
            "canvas grayscale",
            square,
            Transform::default(),
            &[((19, 35), Some((4, 20))), ((43, 19), Some((28, 4)))],
        ),
        (
            "half opacity",
            square,
            Transform {
                opacity: 0.5,
                ..Transform::default()
            },
            &[((19, 35), Some((4, 20))), ((43, 19), Some((28, 4)))],
        ),
        (
            "grayscale",
            square,
            Transform {
                grayscale: true,
                ..Transform::default()
            },
            &[((19, 35), Some((4, 20))), ((43, 19), Some((28, 4)))],
        ),
        (
            "content zoom",
            square,
            Transform {
                content_scale: 2.,
                ..Transform::default()
            },
            // Doubling the content shows the middle half: source = (world + 32) / 2.
            &[((19, 35), Some((12, 20))), ((43, 19), Some((20, 12)))],
        ),
        (
            "content flip",
            square,
            Transform {
                content_scale: 2.,
                flip: [true, false],
                ..Transform::default()
            },
            // Mirroring puts source x at 32 minus the unmirrored column.
            &[((19, 35), Some((20, 20))), ((43, 19), Some((8, 8)))],
        ),
        (
            "content rotation",
            square,
            Transform {
                content_scale: 2.,
                content_rotation: std::f32::consts::FRAC_PI_2,
                ..Transform::default()
            },
            // A quarter turn inside the frame maps source x to world y and source y to world -x.
            &[((19, 27), Some((8, 20))), ((43, 35), Some((20, 8)))],
        ),
        (
            "item rotation",
            square,
            Transform {
                rotation: std::f32::consts::FRAC_PI_2,
                ..Transform::default()
            },
            // Turning the whole item keeps the square frame in place and turns the content.
            &[((19, 27), Some((8, 28))), ((43, 35), Some((20, 4)))],
        ),
        (
            "rotated frame clips",
            Rect {
                x: 0.,
                y: 0.,
                width: 64.,
                height: 32.,
            },
            Transform {
                rotation: std::f32::consts::FRAC_PI_4,
                ..Transform::default()
            },
            // 45 degrees leaves the frame's corners empty, so the second sample sits inside
            // the content quad but outside the frame and must be discarded.
            &[((34, 24), Some((20, 20))), ((17, 30), None)],
        ),
    ];
    let texture = target(&device, 64, 64);
    let view = texture.create_view(&Default::default());
    for (name, rect, transform, samples) in cases {
        registry.grayscale = name == "canvas grayscale";
        let mut scene = Scene::default();
        let mut item = Item::image(image.hash, rect, None);
        item.transform = transform;
        scene
            .apply(Command::Insert(item))
            .map_err(|e| e.to_string())?;
        let items = scene.query(Rect {
            x: 0.,
            y: 0.,
            width: 64.,
            height: 64.,
        });
        registry
            .draw(
                &device,
                &queue,
                crate::Target {
                    opacity: 1.,
                    grid: false,
                    view: &view,
                    screen: [64., 64.],
                    clip: None,
                    clear: wgpu::Color::BLACK,
                },
                &items,
                Camera::default(),
            )
            .map_err(|error| error.to_string())?;
        let bytes = readback(&device, &queue, &texture, 256)?;
        for &((x, y), source) in samples {
            let expected = match source {
                Some((source_x, source_y)) => [stripe(source_x), stripe(source_y), 0, 255],
                None => [0, 0, 0, 255],
            };
            let mut expected = expected;
            if source.is_some() {
                if transform.grayscale || registry.grayscale {
                    let linear = |value: u8| {
                        let v = value as f32 / 255.;
                        if v <= 0.04045 {
                            v / 12.92
                        } else {
                            ((v + 0.055) / 1.055).powf(2.4)
                        }
                    };
                    let l = linear(expected[0]) * 0.2126
                        + linear(expected[1]) * 0.7152
                        + linear(expected[2]) * 0.0722;
                    let g = if l <= 0.0031308 {
                        12.92 * l
                    } else {
                        1.055 * l.powf(1. / 2.4) - 0.055
                    };
                    expected[..3].fill((g * 255.).round() as u8);
                }
                for channel in &mut expected[..3] {
                    *channel = (*channel as f32 * transform.opacity).round() as u8;
                }
            }
            let pixel = &bytes[(y as usize * 256 + x as usize * 4)..][..4];
            if pixel.iter().zip(expected).any(|(&a, b)| a.abs_diff(b) > 3) {
                return Err(format!(
                    "{name} mismatch at ({x},{y}): {pixel:?} != {expected:?}"
                ));
            }
        }
    }
    registry.clear();
    if registry.allocated_bytes() != 0 {
        return Err("registry leaked textures".into());
    }
    Ok(())
}
/// Compares every pixel against a two-image overlap golden with channel tolerance.
pub async fn golden(software: bool) -> Result<(), String> {
    golden_view(software, 1., false, false).await
}
async fn golden_view(
    software: bool,
    opacity: f32,
    transparent: bool,
    grid: bool,
) -> Result<(), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let (_, device, queue) = crate::request_device(&instance, None, software).await?;
    let mut registry = Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Rgba8);
    let mut scene = Scene::default();
    for (i, color, offset) in [(0, [120, 90, 60, 255], 0.), (1, [0, 0, 255, 255], 16.)] {
        let mut image = fixture(i, 16);
        for mip in &mut image.mips {
            for pixel in mip.bytes.as_chunks_mut::<4>().0 {
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
    registry
        .draw(
            &device,
            &queue,
            crate::Target {
                opacity,
                grid,
                view: &view,
                screen: [64., 64.],
                clip: None,
                clear: if transparent {
                    wgpu::Color::TRANSPARENT
                } else {
                    wgpu::Color::BLACK
                },
            },
            &items,
            Camera::default(),
        )
        .map_err(|error| error.to_string())?;
    let bytes = readback(&device, &queue, &texture, 256)?;
    for y in 0..64 {
        for x in 0..64 {
            let mut expected: [f32; 4] = if (48..56).contains(&x) && y < 8 {
                [0., 255., 0., 255.]
            } else if (16..48).contains(&x) && (16..48).contains(&y) {
                [0., 0., 255., 255.]
            } else if x < 32 && y < 32 {
                [120., 90., 60., 255.]
            } else if grid {
                let distance = (((x as f32 + 0.5) % 40. - 20.).powi(2)
                    + ((y as f32 + 0.5) % 40. - 20.).powi(2))
                .sqrt();
                let t = ((distance - 0.5) / 0.5).clamp(0., 1.);
                let dot = 1. - t * t * (3. - 2. * t);
                [17. + 26. * dot, 19. + 27. * dot, 23. + 29. * dot, 255.]
            } else {
                [0., 0., 0., if transparent { 0. } else { 255. }]
            };
            for channel in &mut expected {
                *channel *= opacity;
            }
            let expected = expected.map(|v| v.round() as u8);
            let pixel = &bytes[(y * 256 + x * 4)..(y * 256 + x * 4 + 4)];
            if pixel.iter().zip(expected).any(|(&a, b)| a.abs_diff(b) > 2) {
                return Err(format!(
                    "golden mismatch at ({x},{y}): {pixel:?} != {expected:?}"
                ));
            }
        }
    }
    registry.clear();
    if registry.allocated_bytes() != 0 {
        return Err("registry leaked textures".into());
    }
    Ok(())
}
/// BC7 is lossy, so a solid-color scene verifies the compressed upload path and
/// GPU decode with a tolerance that absorbs BC7 quantization. Skipped (logged)
/// on adapters that cannot bind BC7, where the RGBA8 golden is the fallback path.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub async fn golden_bc7(software: bool) -> Result<(), String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let (adapter, device, queue) = crate::request_device(&instance, None, software).await?;
    if crate::pick_image_format(&adapter) != ImageFormat::Bc7 {
        tracing::warn!("adapter lacks BC7 support; skipping the BC7 golden");
        return Ok(());
    }
    let mut registry = Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Bc7);
    let mut scene = Scene::default();
    for (i, color, offset) in [(0, [120, 90, 60, 255], 0.), (1, [0, 0, 255, 255], 16.)] {
        let mut image = fixture(i, 16);
        for mip in &mut image.mips {
            for pixel in mip.bytes.as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&color);
            }
        }
        let hash = image.hash;
        registry.upload(&device, &queue, &moth_io::to_bc7(image));
        scene
            .apply(Command::Insert(Item::image(
                hash,
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
    let texture = target(&device, 64, 64);
    let view = texture.create_view(&Default::default());
    let items = scene.query(Rect {
        x: 0.,
        y: 0.,
        width: 64.,
        height: 64.,
    });
    registry
        .draw(
            &device,
            &queue,
            crate::Target {
                opacity: 1.,
                grid: false,
                view: &view,
                screen: [64., 64.],
                clip: None,
                clear: wgpu::Color::BLACK,
            },
            &items,
            Camera::default(),
        )
        .map_err(|error| error.to_string())?;
    let bytes = readback(&device, &queue, &texture, 256)?;
    for y in 0..64 {
        for x in 0..64 {
            let expected: [i32; 3] = if (16..48).contains(&x) && (16..48).contains(&y) {
                [0, 0, 255]
            } else if x < 32 && y < 32 {
                [120, 90, 60]
            } else {
                [0, 0, 0]
            };
            let pixel = &bytes[(y * 256 + x * 4)..][..4];
            if pixel
                .iter()
                .take(3)
                .zip(expected)
                .any(|(&a, b)| (a as i32 - b).abs() > 8)
            {
                return Err(format!(
                    "BC7 golden mismatch at ({x},{y}): {pixel:?} != {expected:?}"
                ));
            }
        }
    }
    registry.clear();
    if registry.allocated_bytes() != 0 {
        return Err("registry leaked textures".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn detail_refinement_blends_mips_without_extra_textures_or_geometry_changes() {
        use super::*;
        pollster::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let (_, device, queue) = crate::request_device(&instance, None, false).await.unwrap();
            let mut registry =
                Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Rgba8);
            let mut image = fixture(42, 128);
            for (level, mip) in image.mips.iter_mut().enumerate() {
                for pixel in mip.bytes.as_chunks_mut::<4>().0 {
                    pixel.copy_from_slice(if level == 0 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0, 0, 255, 255]
                    });
                }
            }
            registry.upload(&device, &queue, &image);
            let allocated = registry.allocated_bytes();
            let item = Item::image(
                image.hash,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 128.,
                    height: 128.,
                },
                None,
            );
            let texture = target(&device, 32, 32);
            let view = texture.create_view(&Default::default());
            for (level, expected) in [
                (2., [0u8, 0, 255, 255]),
                (0.5, [188, 0, 188, 255]),
                (0., [255, 0, 0, 255]),
            ] {
                assert!(registry.set_detail_lod(image.hash, level));
                assert_eq!(
                    registry.preview_detail(image.hash),
                    Some((128., 128. / level.exp2()))
                );
                registry
                    .draw(
                        &device,
                        &queue,
                        crate::Target {
                            view: &view,
                            screen: [32., 32.],
                            clip: None,
                            clear: wgpu::Color::BLACK,
                            grid: false,
                            opacity: 1.,
                        },
                        &[&item],
                        Camera::default(),
                    )
                    .unwrap();
                let pixels = readback(&device, &queue, &texture, 256).unwrap();
                for y in 1..31 {
                    for x in 1..31 {
                        let offset = y * 256 + x * 4;
                        assert!(
                            pixels[offset..offset + 4]
                                .iter()
                                .zip(expected)
                                .all(|(&actual, expected)| actual.abs_diff(expected) <= 3),
                            "mip {level}: {:?}",
                            &pixels[offset..offset + 4]
                        );
                    }
                }
                assert_eq!(registry.allocated_bytes(), allocated);
            }
            registry.clear();
            assert!(!registry.set_detail_lod(image.hash, 2.));
        });
    }
    #[test]
    fn adaptive_budget_releases_allocated_pages_when_image_count_falls() {
        use super::*;
        pollster::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let (_, device, queue) = crate::request_device(&instance, None, false).await.unwrap();
            let mut registry =
                Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Rgba8);
            registry.adapt_budget(256, crate::MAX_TEXTURE_BUDGET);
            assert_eq!(
                registry.allocated_bytes(),
                0,
                "growth must not preallocate textures"
            );
            for index in 0..8 {
                registry.upload(&device, &queue, &fixture(index, 2048));
            }
            let before = registry.allocated_bytes();
            assert!(before > crate::TEXTURE_BUDGET);
            assert!(before <= registry.budget_bytes());
            registry.adapt_budget(1, crate::MAX_TEXTURE_BUDGET);
            assert_eq!(registry.budget_bytes(), crate::MIN_TEXTURE_BUDGET);
            assert!(registry.allocated_bytes() <= registry.budget_bytes());
            assert!(registry.allocated_bytes() < before);
            registry.adapt_budget(0, crate::MAX_TEXTURE_BUDGET);
            assert_eq!(registry.allocated_bytes(), 0);
            assert_eq!(registry.budget_bytes(), 0);
            for index in 0..8 {
                assert!(!registry.resident(fixture(index, 16).hash, 1));
            }
        });
    }
    #[test]
    fn zoom_detail_upgrades_never_blank_other_resident_images() {
        use super::*;
        pollster::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let (_, device, queue) = crate::request_device(&instance, None, false).await.unwrap();
            let mut registry =
                Registry::new(&device, wgpu::TextureFormat::Rgba8Unorm, ImageFormat::Rgba8);
            let mut scene = Scene::default();
            for index in 0..8 {
                let mut image = fixture(64 + index, 256);
                image.original = [2048, 2048];
                registry.upload(&device, &queue, &image);
                scene
                    .apply(Command::Insert(Item::image(
                        image.hash,
                        Rect {
                            x: (index % 4) as f32 * 8.,
                            y: (index / 4) as f32 * 8.,
                            width: 8.,
                            height: 8.,
                        },
                        scene.top(),
                    )))
                    .unwrap();
            }
            let texture = target(&device, 32, 16);
            let view = texture.create_view(&Default::default());
            for upgraded in 0..8 {
                registry.upload(&device, &queue, &fixture(64 + upgraded, 2048));
                let items: Vec<_> = scene.items().collect();
                registry
                    .draw(
                        &device,
                        &queue,
                        crate::Target {
                            opacity: 1.,
                            grid: false,
                            view: &view,
                            screen: [32., 16.],
                            clip: None,
                            clear: wgpu::Color::BLACK,
                        },
                        &items,
                        Camera::default(),
                    )
                    .unwrap();
                let pixels = readback(&device, &queue, &texture, 256).unwrap();
                for index in 0..8 {
                    let offset = (index / 4 * 8 + 4) * 256 + (index % 4 * 8 + 4) * 4;
                    let pixel = &pixels[offset..offset + 4];
                    let expected = [64 + index as u8, 0, 127, 255];
                    assert!(
                        pixel.iter().zip(expected).all(|(&a, b)| a.abs_diff(b) <= 3),
                        "image {index} disappeared while upgrading image {upgraded}: {pixel:?}"
                    );
                }
                assert!(registry.allocated_bytes() <= crate::TEXTURE_BUDGET);
            }
            // A delayed, smaller decode from an earlier zoom must not downgrade detail.
            registry.upload(&device, &queue, &fixture(71, 256));
            assert!(registry.resident(fixture(71, 16).hash, 2048));
            registry.clear();
            assert_eq!(registry.allocated_bytes(), 0);
        });
    }
    #[test]
    fn headless_golden() {
        pollster::block_on(super::golden(false)).unwrap();
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn headless_golden_bc7() {
        pollster::block_on(super::golden_bc7(false)).unwrap();
    }
    #[test]
    fn transparent_overlay_preserves_premultiplied_alpha_and_item_order() {
        pollster::block_on(super::golden_view(false, 0.55, true, false)).unwrap();
    }
    #[test]
    fn canvas_grid_stays_behind_opaque_images() {
        pollster::block_on(super::golden_view(false, 1., false, true)).unwrap();
    }
    #[test]
    fn rotated_cropped_and_mirrored_content_matches_the_scene() {
        pollster::block_on(super::golden_transform(false)).unwrap();
    }
}
