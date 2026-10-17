// SPDX-License-Identifier: GPL-3.0-or-later
//! Window opacity is applied after flattening, so overlapping references stay equally translucent.
use super::{RenderError, Target};
/// One rebuildable screen-sized texture has a separate, explicit budget.
pub(super) const BUDGET: u64 = 64 * 1024 * 1024;
struct Frame {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    bytes: u64,
}
pub(super) struct Pass {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    parameters: wgpu::Buffer,
    frame: Option<Frame>,
}
impl Pass {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("flattened opacity"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("flattened opacity"),
            source: wgpu::ShaderSource::Wgsl(include_str!("opacity.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("flattened opacity"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("flattened opacity"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let parameters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("view opacity"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            format,
            layout,
            pipeline,
            parameters,
            frame: None,
        }
    }
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        size: wgpu::Extent3d,
    ) -> Result<wgpu::TextureView, RenderError> {
        let bytes = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|pixels| {
                pixels.checked_mul(u64::from(self.format.block_copy_size(None).unwrap_or(16)))
            })
            .unwrap_or(u64::MAX);
        if bytes > BUDGET {
            return Err(RenderError::OpacityBudget);
        }
        if self.frame.as_ref().is_none_or(|f| f.texture.size() != size) {
            self.clear();
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("flattened reference board"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("flattened reference board"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.parameters.as_entire_binding(),
                    },
                ],
            });
            self.frame = Some(Frame {
                texture,
                view,
                bind,
                bytes,
            });
        }
        Ok(self
            .frame
            .as_ref()
            .expect("prepare creates a bounded frame above")
            .view
            .clone())
    }
    pub(super) fn present(&self, device: &wgpu::Device, queue: &wgpu::Queue, target: Target<'_>) {
        let Some(frame) = &self.frame else {
            return;
        };
        queue.write_buffer(
            &self.parameters,
            0,
            bytemuck::cast_slice(&[target.opacity, 0., 0., 0.]),
        );
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("view opacity"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(target.clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &frame.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
    pub(super) fn bytes(&self) -> u64 {
        self.frame.as_ref().map_or(0, |f| f.bytes)
    }
    pub(super) fn clear(&mut self) {
        self.frame = None;
    }
}
