// SPDX-License-Identifier: GPL-3.0-or-later
//! Instanced image quads and a fixed-budget, LRU array-texture registry.
mod opacity;
use moth_io::Decoded;
use moth_scene::{BlobId, Camera, Item};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Includes every mip and layer; allocation does not scale with board size.
pub const TEXTURE_BUDGET: u64 = 96 * 1024 * 1024;
const CLASSES: [(u32, u32); 8] = [
    (16, 256),
    (32, 256),
    (64, 256),
    (128, 128),
    (256, 32),
    (512, 8),
    (1024, 2),
    (2048, 1),
];
const MAX_INSTANCES: usize = 10_000;
/// Corner order shared by the shader constants, the CPU geometry and hit testing.
const LOCAL: [[f32; 2]; 4] = [[0., 0.], [1., 0.], [0., 1.], [1., 1.]];
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Instance {
    /// Clip-space positions of the content quad, in [`LOCAL`] corner order.
    corners: [[f32; 2]; 4],
    /// Frame-local coordinates of the same corners, for clipping against the frame.
    frame: [[f32; 2]; 4],
    /// Resident source rect in pixels: start corner, then a signed span for mirrored content.
    region: [f32; 4],
    layer: u32,
    lod: f32,
}
/// Content geometry of one item, shared by rendering and the scene-level hit tests.
struct Content {
    corners: [[f32; 2]; 4],
    frame: [[f32; 2]; 4],
    region: [f32; 4],
}
/// Maps item content through its frame into clip space.
///
/// Content is fitted into the frame without distortion, then scaled, panned and rotated
/// inside it; the frame's own rotation turns both. Content outside the frame is clipped by
/// the fragment stage, so the item's visible footprint stays the rotated frame.
fn content(item: &Item, dims: [u32; 2], camera: Camera, screen: [f32; 2]) -> Content {
    let transform = item.transform;
    let rect = item.rect;
    let dims = [dims[0] as f32, dims[1] as f32];
    let center = [rect.x + rect.width / 2., rect.y + rect.height / 2.];
    // Cover: at scale 1 the content fills the frame without distortion, and the frame clips
    // whatever does not fit. A frame larger than the content magnifies it instead of leaving gaps.
    let fit = (rect.width / dims[0]).max(rect.height / dims[1]) * transform.content_scale;
    let size = [dims[0] * fit, dims[1] * fit];
    let pan = [
        transform.content_offset[0] * rect.width,
        transform.content_offset[1] * rect.height,
    ];
    let (spin_sin, spin_cos) = transform.content_rotation.sin_cos();
    let (turn_sin, turn_cos) = transform.rotation.sin_cos();
    let mut corners = [[0.; 2]; 4];
    let mut frame = [[0.; 2]; 4];
    for (index, corner) in LOCAL.iter().enumerate() {
        let pan = [
            pan[0] + (corner[0] - 0.5) * size[0],
            pan[1] + (corner[1] - 0.5) * size[1],
        ];
        let spun = [
            pan[0] * spin_cos - pan[1] * spin_sin,
            pan[0] * spin_sin + pan[1] * spin_cos,
        ];
        // Frame-local coordinates stay upright while the frame turns with the content.
        frame[index] = [spun[0] / rect.width + 0.5, spun[1] / rect.height + 0.5];
        let turned = [
            spun[0] * turn_cos - spun[1] * turn_sin,
            spun[0] * turn_sin + spun[1] * turn_cos,
        ];
        let point = camera.screen([center[0] + turned[0], center[1] + turned[1]]);
        corners[index] = [
            point[0] / screen[0] * 2. - 1.,
            1. - point[1] / screen[1] * 2.,
        ];
    }
    let flip = transform.flip;
    Content {
        corners,
        frame,
        region: [
            if flip[0] { dims[0] } else { 0. },
            if flip[1] { dims[1] } else { 0. },
            if flip[0] { -dims[0] } else { dims[0] },
            if flip[1] { -dims[1] } else { dims[1] },
        ],
    }
}
#[derive(Clone, Copy)]
struct Slot {
    hash: BlobId,
    side: u32,
    dimensions: [u32; 2],
    used: u64,
    original_side: u32,
}
struct Array {
    size: u32,
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    slots: Vec<Option<Slot>>,
}
/// Describes a frame destination without tying canvas batches to the UI render pass.
#[derive(Clone, Copy)]
pub struct Target<'a> {
    /// Batches preserve this attachment between ordered submissions.
    pub view: &'a wgpu::TextureView,
    /// Logical dimensions keep camera coordinates independent of display scaling.
    pub screen: [f32; 2],
    /// Physical clip bounds keep images inside the canvas viewport.
    pub clip: Option<[u32; 4]>,
    /// Only the first batch clears the attachment.
    pub clear: wgpu::Color,
    /// Global viewing opacity is transient and never changes board content.
    pub opacity: f32,
    /// The desktop board can opt into the decorative canvas grid.
    pub grid: bool,
}
/// A viewing effect must fail before exceeding its explicit texture budget.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Limit screen-sized temporary pixels separately from image residency.
    #[error("Overlay exceeds the 64 MiB surface budget; resize the window to continue.")]
    OpacityBudget,
    /// Non-finite opacity never reaches a GPU uniform.
    #[error("Invalid view opacity")]
    Opacity,
}
/// Registry owns all canvas GPU resources; dropping it frees every board texture.
pub struct Registry {
    /// UI GPU resources share the renderer lifetime and are freed with the registry.
    pub ui: egui_wgpu::Renderer,
    arrays: Vec<Option<Array>>,
    locations: std::collections::HashMap<BlobId, (usize, usize)>,
    pipeline: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    opacity: opacity::Pass,
    layout: wgpu::BindGroupLayout,
    instances: wgpu::Buffer,
    sampler: wgpu::Sampler,
    clock: u64,
    allocated: u64,
}
impl Registry {
    /// RGBA8 is the universally supported fallback, chosen from adapter capabilities.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("images"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image quads"),
            source: wgpu::ShaderSource::Wgsl(include_str!("quad.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("canvas"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("quads"),layout:Some(&pipeline_layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),buffers:&[Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Instance>() as u64,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Float32x2,4=>Float32x2,5=>Float32x2,6=>Float32x2,7=>Float32x2,8=>Float32x4,9=>Uint32,10=>Float32]})],compilation_options:Default::default()},fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs"),targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})],compilation_options:wgpu::PipelineCompilationOptions{constants:&[("OUTPUT_SRGB",if format.is_srgb(){1.}else{0.})],..Default::default()}}),primitive:Default::default(),depth_stencil:None,multisample:Default::default(),multiview_mask:None,cache:None});
        let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("canvas grid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grid.wgsl").into()),
        });
        let grid = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("canvas grid"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &grid_shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &grid_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("OUTPUT_SRGB", if format.is_srgb() { 1. } else { 0. })],
                    ..Default::default()
                },
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded instances"),
            size: (MAX_INSTANCES * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mip sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            ui: egui_wgpu::Renderer::new(device, format, Default::default()),
            arrays: Vec::new(),
            locations: std::collections::HashMap::new(),
            pipeline,
            grid,
            opacity: opacity::Pass::new(device, format),
            layout,
            instances,
            sampler,
            clock: 0,
            allocated: 0,
        }
    }
    /// Reports actual texture allocation, rather than the sum of resident source sizes.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated + self.opacity.bytes()
    }
    /// A matching or larger resident preview avoids repeated decoding during a frame.
    pub fn resident(&self, hash: BlobId, side: u32) -> bool {
        self.locations
            .get(&hash)
            .and_then(|&(p, l)| self.arrays[p].as_ref()?.slots[l])
            .is_some_and(|s| s.side >= side.min(s.original_side))
    }

    /// Uploading replaces the least recently drawn layer within a size class.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &Decoded) {
        let Some(first) = image.mips.first() else {
            return;
        };
        let side = first.width.max(first.height).next_power_of_two().max(16);
        let Some(&(size, layers)) = CLASSES.iter().find(|c| c.0 >= side) else {
            return;
        };
        if let Some((p, l)) = self.locations.remove(&image.hash)
            && let Some(a) = self.arrays[p].as_mut()
        {
            a.slots[l] = None;
        }
        let free = self.arrays.iter().position(|a| {
            a.as_ref()
                .is_some_and(|a| a.size == size && a.slots.iter().any(Option::is_none))
        });
        let index = if let Some(i) = free {
            i
        } else {
            let bytes = array_bytes(size, layers);
            while self.allocated + bytes > TEXTURE_BUDGET {
                let oldest = self
                    .arrays
                    .iter()
                    .enumerate()
                    .filter_map(|(i, a)| {
                        a.as_ref().map(|a| {
                            (
                                i,
                                a.slots.iter().flatten().map(|s| s.used).max().unwrap_or(0),
                            )
                        })
                    })
                    .min_by_key(|a| a.1)
                    .map(|a| a.0);
                let Some(i) = oldest else {
                    return;
                };
                if let Some(a) = self.arrays[i].take() {
                    for s in a.slots.iter().flatten() {
                        self.locations.remove(&s.hash);
                    }
                    self.allocated -= array_bytes(a.size, a.slots.len() as u32);
                }
            }
            let i = self
                .arrays
                .iter()
                .position(Option::is_none)
                .unwrap_or(self.arrays.len());
            if i == self.arrays.len() {
                self.arrays.push(None);
            }
            i
        };
        if self.arrays[index].is_none() {
            let bytes = array_bytes(size, layers);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("image size class"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: layers,
                },
                mip_level_count: size.ilog2() + 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("array"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.arrays[index] = Some(Array {
                size,
                texture,
                bind,
                slots: vec![None; layers as usize],
            });
            self.allocated += bytes;
        }
        let Some(array) = self.arrays[index].as_mut() else {
            return;
        };
        let layer = array
            .slots
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                array
                    .slots
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, s)| s.map(|s| s.used).unwrap_or(0))
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });
        if let Some(old) = array.slots[layer] {
            self.locations.remove(&old.hash);
        }
        self.locations.insert(image.hash, (index, layer));
        self.clock += 1;
        array.slots[layer] = Some(Slot {
            hash: image.hash,
            side: size,
            original_side: image.original[0].max(image.original[1]),
            dimensions: [first.width, first.height],
            used: self.clock,
        });
        for (level, mip) in image.mips.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &array.texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &mip.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip.width * 4),
                    rows_per_image: Some(mip.height),
                },
                wgpu::Extent3d {
                    width: mip.width,
                    height: mip.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    /// Submits bounded batches in z-order before reusing the instance buffer.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: Target<'_>,
        items: &[&Item],
        camera: Camera,
    ) -> Result<(), RenderError> {
        if !target.opacity.is_finite() || !(0. ..=1.).contains(&target.opacity) {
            return Err(RenderError::Opacity);
        }
        if target.opacity < 1. {
            let view = self.opacity.prepare(device, target.view.texture().size())?;
            self.draw_images(
                device,
                queue,
                Target {
                    view: &view,
                    clear: wgpu::Color::TRANSPARENT,
                    opacity: 1.,
                    ..target
                },
                items,
                camera,
            );
            self.opacity.present(device, queue, target);
        } else {
            self.opacity.clear();
            self.draw_images(device, queue, target, items, camera);
        }
        Ok(())
    }
    fn draw_images(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: Target<'_>,
        items: &[&Item],
        camera: Camera,
    ) {
        for batch in 0..items.len().div_ceil(MAX_INSTANCES).max(1) {
            let start = batch * MAX_INSTANCES;
            let end = (start + MAX_INSTANCES).min(items.len());
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("canvas batch"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: if batch == 0 {
                                wgpu::LoadOp::Clear(target.clear)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                if let Some([x, y, width, height]) = target.clip {
                    pass.set_scissor_rect(x, y, width, height);
                }
                if batch == 0 && target.grid {
                    pass.set_pipeline(&self.grid);
                    pass.draw(0..3, 0..1);
                }
                self.draw_batch(queue, &mut pass, &items[start..end], camera, target.screen);
            }
            queue.submit([encoder.finish()]);
        }
    }
    /// Draws consecutive array runs in scene z-order, preserving overlap semantics.
    fn draw_batch(
        &mut self,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        items: &[&Item],
        camera: Camera,
        screen: [f32; 2],
    ) {
        let _span = tracing::info_span!("canvas_draw", visible = items.len()).entered();
        self.clock += 1;
        let mut instances = Vec::with_capacity(items.len().min(MAX_INSTANCES));
        let mut classes = Vec::with_capacity(instances.capacity());
        debug_assert!(items.len() <= MAX_INSTANCES);
        for item in items {
            let wanted = (item.rect.width.max(item.rect.height) * camera.zoom).max(1.);
            let Some(&(class, layer)) = self.locations.get(&item.blob) else {
                continue;
            };
            let Some(slot) = self.arrays[class].as_ref().and_then(|a| a.slots[layer]) else {
                continue;
            };
            if let Some(a) = self.arrays[class].as_mut()
                && let Some(s) = a.slots[layer].as_mut()
            {
                s.used = self.clock;
            }
            let shape = content(item, slot.dimensions, camera, screen);
            instances.push(Instance {
                corners: shape.corners,
                frame: shape.frame,
                region: shape.region,
                layer: layer as u32,
                // shortcut: content rotation is ignored when choosing the mip, so a rotated
                // crop can alias by up to one level. Upgrade with a diagonal term if reported.
                lod: ((slot.dimensions[0].max(slot.dimensions[1]) as f32 / wanted).log2()
                    - item.transform.content_scale.log2())
                .max(0.)
                .min(slot.dimensions[0].max(slot.dimensions[1]).ilog2() as f32),
            });
            classes.push(class);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        let mut start = 0;
        while start < classes.len() {
            let class = classes[start];
            let mut end = start + 1;
            while end < classes.len() && classes[end] == class {
                end += 1;
            }
            if let Some(array) = &self.arrays[class] {
                pass.set_bind_group(0, &array.bind, &[]);
                pass.draw(0..6, start as u32..end as u32);
            }
            start = end;
        }
    }
    /// Board teardown explicitly releases arrays; accounting must return to zero.
    pub fn clear(&mut self) {
        self.opacity.clear();
        for array in &mut self.arrays {
            if let Some(a) = array.take() {
                self.allocated -= array_bytes(a.size, a.slots.len() as u32);
            }
        }
        self.locations.clear();
        debug_assert_eq!(self.allocated, 0);
    }
}
impl Drop for Registry {
    fn drop(&mut self) {
        self.clear();
    }
}
fn array_bytes(size: u32, layers: u32) -> u64 {
    (0..=size.ilog2())
        .map(|m| u64::from((size >> m).max(1)).pow(2) * 4 * u64::from(layers))
        .sum()
}
/// Preview requests use size classes to avoid decoding on every zoom tick.
pub fn preview_side(pixels: f32) -> u32 {
    (pixels.ceil().clamp(1., 2048.) as u32)
        .next_power_of_two()
        .clamp(16, 2048)
}

/// Shared native/headless initialization always retries with a software adapter.
pub async fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
    software: bool,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    let options = |fallback| wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: fallback,
        compatible_surface: surface,
        ..Default::default()
    };
    let adapter = match instance.request_adapter(&options(software)).await {
        Ok(a) => a,
        Err(_) if !software => instance
            .request_adapter(&options(true))
            .await
            .map_err(|e| e.to_string())?,
        Err(e) => return Err(e.to_string()),
    };
    match adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
    {
        Ok((device, queue)) => Ok((adapter, device, queue)),
        Err(_) if !software => {
            let a = instance
                .request_adapter(&options(true))
                .await
                .map_err(|e| e.to_string())?;
            let (d, q) = a
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .map_err(|e| e.to_string())?;
            Ok((a, d, q))
        }
        Err(e) => Err(e.to_string()),
    }
}
/// Device loss only invalidates GPU state; the caller retains scene and sources.
pub fn watch_device(device: &wgpu::Device, wake: Arc<dyn Fn() + Send + Sync>) -> Arc<AtomicBool> {
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    device.set_device_lost_callback(move |reason, message| {
        tracing::warn!(?reason, message, "device lost");
        flag.store(true, Ordering::Release);
        wake();
    });
    lost
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn texture_budget_covers_all_classes() {
        let bytes: u64 = CLASSES.iter().map(|&(s, l)| array_bytes(s, l)).sum();
        assert!(bytes <= TEXTURE_BUDGET);
        assert_eq!(preview_side(257.), 512);
    }
}

pub mod headless;
