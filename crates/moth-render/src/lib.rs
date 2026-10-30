// SPDX-License-Identifier: GPL-3.0-or-later
//! Instanced image quads and a bounded adaptive LRU array-texture registry.
mod opacity;
mod tiles;
use moth_io::{Decoded, ImageFormat};
use moth_scene::{BlobId, Camera, Item};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Default cache ceiling, also used for software and shared-memory adapters.
pub const TEXTURE_BUDGET: u64 = 96 * 1024 * 1024;
/// A nonempty board can retain a full-detail texture and its fallback page.
pub const MIN_TEXTURE_BUDGET: u64 = 32 * 1024 * 1024;
/// Default hard ceiling until the app sets one from the detected video memory
/// ([`Registry::set_hard_limit`]); growth stays bounded even on million-item boards.
pub const MAX_TEXTURE_BUDGET: u64 = 512 * 1024 * 1024;
const FALLBACK_SIDE: u32 = 32;
/// While a detail tile loads, a resident tile up to this many levels coarser draws its
/// region instead of the whole-image preview, so a fast zoom stays sharp-ish rather
/// than dropping to the 256-pixel fallback. Callers keep these ancestors resident.
pub const PARENT_LEVELS: u32 = 4;
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
/// Instance class of group backdrops; drawn by their own pipeline with no texture.
const BACKDROP: usize = usize::MAX;
/// Backdrop corner radius in world units, so it scales with the board like the frame.
const BACKDROP_RADIUS: f32 = 14.;
/// Fill and border of one group color, as straight-alpha linear RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    /// Interior tint; usually a low alpha so the canvas shows through.
    pub fill: [f32; 4],
    /// One-pixel edge.
    pub border: [f32; 4],
    /// Corner radius in pixels whatever the zoom; `None` scales with the board.
    pub radius: Option<f32>,
}
impl Default for Backdrop {
    fn default() -> Self {
        Self {
            fill: [0.5, 0.5, 0.5, 0.08],
            border: [0.5, 0.5, 0.5, 0.35],
            radius: None,
        }
    }
}
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
    appearance: [f32; 4],
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
    Content {
        corners,
        frame,
        region: source_region(dims, transform.flip),
    }
}
/// A group backdrop as an image instance: `frame` carries pixel offsets inside the
/// rectangle (its last corner is the size), `region` the fill, `appearance` the border
/// and `lod` the corner radius in pixels.
fn backdrop_instance(
    rect: moth_scene::Rect,
    style: Backdrop,
    camera: Camera,
    screen: [f32; 2],
    scale: f32,
) -> Instance {
    let size = [
        rect.width * camera.zoom * scale,
        rect.height * camera.zoom * scale,
    ];
    let mut corners = [[0.; 2]; 4];
    let mut frame = [[0.; 2]; 4];
    for (index, corner) in LOCAL.iter().enumerate() {
        let point = camera.screen([
            rect.x + corner[0] * rect.width,
            rect.y + corner[1] * rect.height,
        ]);
        corners[index] = [
            point[0] / screen[0] * 2. - 1.,
            1. - point[1] / screen[1] * 2.,
        ];
        frame[index] = [corner[0] * size[0], corner[1] * size[1]];
    }
    Instance {
        corners,
        frame,
        region: style.fill,
        layer: 0,
        lod: style.radius.unwrap_or(BACKDROP_RADIUS * camera.zoom) * scale,
        appearance: style.border,
    }
}
fn source_region(dims: [f32; 2], flip: [bool; 2]) -> [f32; 4] {
    [
        if flip[0] { dims[0] } else { 0. },
        if flip[1] { dims[1] } else { 0. },
        if flip[0] { -dims[0] } else { dims[0] },
        if flip[1] { -dims[1] } else { dims[1] },
    ]
}

#[derive(Clone, Copy)]
struct Slot {
    hash: BlobId,
    side: u32,
    dimensions: [u32; 2],
    used: u64,
    original: [u32; 2],
    tile: bool,
    /// Number of mips written for this image; the shader's lod clamps to this.
    mips: u32,
    detail_lod: f32,
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
    /// Optional view-only grayscale leaves image metadata unchanged.
    pub grayscale: bool,
    /// Default interpolation for items without a persisted override.
    pub sampling: moth_scene::Sampling,
    /// Low-quality mode keeps its preview ceiling instead of requesting original tiles.
    pub tile_detail: bool,
    /// Images whose longer side exceeds this are drawn from tiles
    /// ([`moth_io::CacheMode::tile_threshold`]).
    pub tile_threshold: u32,
    /// Full-resolution mode: everything uploaded stays resident. Tiles leaving the
    /// viewport and previews shown smaller are kept instead of released.
    pub keep_all: bool,
    /// Group backdrop colors by palette code; the app sets them from theme tokens.
    pub backdrops: [Backdrop; moth_scene::GROUP_COLORS as usize],
    /// Opaque panels the app draws through the backdrop pipeline (the minimap), marked
    /// by a group color code past the palette.
    pub panel: Backdrop,
    /// UI GPU resources share the renderer lifetime and are freed with the registry.
    pub ui: egui_wgpu::Renderer,
    arrays: Vec<Option<Array>>,
    locations: std::collections::HashMap<BlobId, (usize, usize)>,
    // At most one fallback per resident blob, bounded by the same texture budget.
    fallbacks: std::collections::HashMap<BlobId, (usize, usize)>,
    pipeline: wgpu::RenderPipeline,
    backdrop: wgpu::RenderPipeline,
    grid: wgpu::RenderPipeline,
    opacity: opacity::Pass,
    layout: wgpu::BindGroupLayout,
    instances: wgpu::Buffer,
    sampler: wgpu::Sampler,
    clock: u64,
    allocated: u64,
    budget: u64,
    ceiling: u64,
    /// Texture format for image mips, chosen at runtime from adapter capabilities.
    image_format: ImageFormat,
    tile_lod_bias: u32,
    hard_limit: u64,
}
impl Registry {
    /// RGBA8 is the universally supported fallback; BC7 halves residency on adapters that support it.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        image_format: ImageFormat,
    ) -> Self {
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
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{label:Some("quads"),layout:Some(&pipeline_layout),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),buffers:&[Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Instance>() as u64,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Float32x2,4=>Float32x2,5=>Float32x2,6=>Float32x2,7=>Float32x2,8=>Float32x4,9=>Uint32,10=>Float32,11=>Float32x4]})],compilation_options:Default::default()},fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs"),targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})],compilation_options:wgpu::PipelineCompilationOptions{constants:&[("OUTPUT_SRGB",if format.is_srgb(){1.}else{0.})],..Default::default()}}),primitive:Default::default(),depth_stencil:None,multisample:Default::default(),multiview_mask:None,cache:None});
        let backdrop_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("group backdrops"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });
        let backdrop = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("group backdrops"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &backdrop_shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Float32x2,4=>Float32x2,5=>Float32x2,6=>Float32x2,7=>Float32x2,8=>Float32x4,9=>Uint32,10=>Float32,11=>Float32x4],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &backdrop_shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            grayscale: false,
            sampling: moth_scene::Sampling::Smooth,
            tile_detail: true,
            tile_threshold: moth_io::MAX_SIDE,
            keep_all: false,
            backdrops: [Backdrop::default(); moth_scene::GROUP_COLORS as usize],
            panel: Backdrop::default(),
            ui: egui_wgpu::Renderer::new(device, format, Default::default()),
            arrays: Vec::new(),
            locations: std::collections::HashMap::new(),
            fallbacks: std::collections::HashMap::new(),
            pipeline,
            backdrop,
            grid,
            opacity: opacity::Pass::new(device, format),
            layout,
            instances,
            sampler,
            clock: 0,
            allocated: 0,
            budget: TEXTURE_BUDGET,
            ceiling: TEXTURE_BUDGET,
            image_format,
            tile_lod_bias: 0,
            hard_limit: MAX_TEXTURE_BUDGET,
        }
    }
    /// Highest texture ceiling [`Registry::adapt_budget`] accepts, from the detected
    /// video memory or the user's setting.
    pub fn set_hard_limit(&mut self, bytes: u64) {
        self.hard_limit = bytes.max(MIN_TEXTURE_BUDGET);
    }
    /// Whether `image` can become resident without evicting anything or exceeding
    /// the current ceiling: it already is, a page of its size class has a free layer,
    /// or a new page still fits.
    pub fn fits(&self, image: &Decoded) -> bool {
        let hash = image.tile.map_or(image.hash, |tile| tile.key(image.hash));
        let Some(first) = image.mips.first() else {
            return true;
        };
        let side = first.width.max(first.height).next_power_of_two().max(16);
        let Some(&(size, layers)) = CLASSES.iter().find(|c| c.0 >= side) else {
            return true;
        };
        if self.resident(hash, side)
            || self
                .arrays
                .iter()
                .rev()
                .flatten()
                .any(|a| a.size == size && a.slots.iter().any(Option::is_none))
        {
            return true;
        }
        self.allocated + array_bytes(size, layers, self.image_format) <= self.ceiling
    }
    /// Reports actual texture allocation, rather than the sum of resident source sizes.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated + self.opacity.bytes()
    }
    /// Current image-cache ceiling; raising it does not preallocate textures.
    pub fn budget_bytes(&self) -> u64 {
        self.budget
    }

    /// Reclaimable image pages exclude independent overlay and UI allocations.
    pub fn image_bytes(&self) -> u64 {
        self.allocated
    }

    /// Grows with page demand and immediately evicts pages when available RAM falls.
    pub fn adapt_budget(&mut self, active: bool, ceiling: u64) {
        self.ceiling = if active {
            ceiling.min(self.hard_limit)
        } else {
            0
        };
        let budget = budget_for(self.allocated, self.ceiling);
        if self.budget == budget {
            return;
        }
        self.budget = budget;
        if budget == 0 {
            self.clear();
            return;
        }
        let mut evicted = false;
        while self.allocated > budget {
            if !self.evict_oldest() {
                break;
            }
            evicted = true;
        }
        if evicted {
            self.locations.shrink_to_fit();
            self.fallbacks.shrink_to_fit();
            self.arrays.shrink_to_fit();
        }
    }

    /// Format decode requests must produce; driven by adapter capabilities.
    pub fn image_format(&self) -> ImageFormat {
        self.image_format
    }
    /// A matching or larger resident preview avoids repeated decoding during a frame.
    pub fn resident(&self, hash: BlobId, side: u32) -> bool {
        self.locations
            .get(&hash)
            .and_then(|&(p, l)| self.arrays[p].as_ref()?.slots[l])
            .is_some_and(|s| s.side >= side.min(s.original[0].max(s.original[1])))
    }

    /// Original dimensions survive preview downsizing, so tile requests never open a file on the UI thread.
    pub fn original_dimensions(&self, hash: BlobId) -> Option<[u32; 2]> {
        let &(page, layer) = self.locations.get(&hash)?;
        Some(self.arrays[page].as_ref()?.slots[layer]?.original)
    }
    /// Affine coverage respects crop, content rotation, mirroring and physical pixel density.
    pub fn visible_tiles(
        &self,
        item: &Item,
        camera: Camera,
        screen: [f32; 2],
        clip: [f32; 4],
        scale: f32,
    ) -> Vec<moth_io::tiles::Tile> {
        if !self.tile_detail {
            return Vec::new();
        }
        let Some(original) = item.blob().and_then(|hash| self.original_dimensions(hash)) else {
            return Vec::new();
        };
        tiles::visible(
            item,
            original,
            camera,
            screen,
            clip,
            scale,
            self.detail(self.tile_lod_bias),
        )
    }
    fn detail(&self, bias: u32) -> tiles::Detail {
        tiles::Detail {
            bias,
            threshold: self.tile_threshold,
        }
    }
    /// Coarsens an oversized viewport working set instead of repeatedly evicting its own detail.
    pub fn fit_tile_budget(
        &mut self,
        items: &[&Item],
        cameras: [Camera; 2],
        screen: [f32; 2],
        clip: [f32; 4],
        scale: f32,
    ) {
        self.tile_lod_bias = 0;
        if !self.tile_detail {
            return;
        }
        let &(side, layers) = CLASSES
            .iter()
            .find(|c| c.0 == moth_io::tiles::SIDE)
            .expect("the fixed tile side has a configured size class");
        let mut preview_slots = std::collections::HashSet::new();
        let mut reserved_pages = std::collections::HashSet::new();
        for item in items {
            let Some(hash) = item.blob() else { continue };
            if let Some(&(page, layer)) = self.locations.get(&hash)
                && let Some(array) = self.arrays[page].as_ref()
            {
                if array.size == side {
                    preview_slots.insert((page, layer));
                } else {
                    reserved_pages.insert(page);
                }
            }
            if let Some(&(page, _)) = self.fallbacks.get(&hash) {
                reserved_pages.insert(page);
            }
        }
        let reserved: u64 = reserved_pages
            .into_iter()
            .filter_map(|p| self.arrays[p].as_ref())
            .map(|a| array_bytes(a.size, a.slots.len() as u32, self.image_format))
            .sum();
        let pages =
            self.ceiling.saturating_sub(reserved) / array_bytes(side, layers, self.image_format);
        let limit = (pages as usize * layers as usize).saturating_sub(preview_slots.len());
        'levels: for bias in 0..=15 {
            let mut wanted = std::collections::HashSet::new();
            for camera in cameras {
                for item in items {
                    let Some(hash) = item.blob() else { continue };
                    let Some(original) = self.original_dimensions(hash) else {
                        continue;
                    };
                    for tile in tiles::visible(
                        item,
                        original,
                        camera,
                        screen,
                        clip,
                        scale,
                        self.detail(bias),
                    ) {
                        wanted.insert(tile.key(hash));
                        if wanted.len() > limit {
                            continue 'levels;
                        }
                    }
                }
            }
            self.tile_lod_bias = bias;
            return;
        }
        self.tile_lod_bias = 15;
    }
    /// Tile envelopes let the scheduler prefer visible pixels nearest the viewport center.
    pub fn tile_screen_bounds(
        &self,
        item: &Item,
        tile: moth_io::tiles::Tile,
        camera: Camera,
        screen: [f32; 2],
    ) -> Option<[f32; 4]> {
        let original = self.original_dimensions(item.blob()?)?;
        let shape = content(item, original, camera, screen);
        let piece = tiles::piece(&shape, tiles::range(tile, original), item.transform.flip);
        let mut bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for point in piece.corners {
            let point = [
                (point[0] + 1.) * screen[0] / 2.,
                (1. - point[1]) * screen[1] / 2.,
            ];
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
        }
        Some(bounds)
    }
    /// Unneeded tile layers are released on pan/zoom, including in No cache mode.
    pub fn retain_tiles(&mut self, wanted: &std::collections::HashSet<BlobId>) {
        if self.keep_all {
            return;
        }
        for array in self.arrays.iter_mut().flatten() {
            for slot in &mut array.slots {
                if slot.is_some_and(|s| s.tile && !wanted.contains(&s.hash)) {
                    self.locations
                        .remove(&slot.take().expect("tile was just checked").hash);
                }
            }
        }
        self.release_empty_pages();
    }
    fn release_empty_pages(&mut self) {
        for array in &mut self.arrays {
            if array
                .as_ref()
                .is_some_and(|a| a.slots.iter().all(Option::is_none))
            {
                let a = array.take().expect("empty page was just checked");
                self.allocated -= array_bytes(a.size, a.slots.len() as u32, self.image_format);
            }
        }
    }
    /// Uploading replaces the least recently drawn layer within a size class.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, image: &Decoded) {
        self.upload_mip(device, queue, image, 0);
    }

    /// Resident resolution and currently displayed detail, for view-only refinement.
    pub fn preview_detail(&self, hash: BlobId) -> Option<(f32, f32)> {
        let &(page, layer) = self.locations.get(&hash)?;
        let slot = self.arrays[page].as_ref()?.slots[layer]?;
        let side = slot.dimensions[0].max(slot.dimensions[1]) as f32;
        Some((side, side / slot.detail_lod.exp2()))
    }

    /// A transient mip floor changes sharpness without changing image opacity or geometry.
    pub fn set_detail_lod(&mut self, hash: BlobId, level: f32) -> bool {
        let Some(&(page, layer)) = self.locations.get(&hash) else {
            return false;
        };
        let Some(slot) = self.arrays[page]
            .as_mut()
            .and_then(|a| a.slots[layer].as_mut())
        else {
            return false;
        };
        slot.detail_lod = if level.is_finite() {
            level.clamp(0., slot.mips.saturating_sub(1) as f32)
        } else {
            0.
        };
        true
    }

    fn upload_mip(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &Decoded,
        first_mip: usize,
    ) {
        let hash = image.tile.map_or(image.hash, |tile| tile.key(image.hash));
        // A device-loss recreation can change the chosen format; skip stale results.
        // The preview path re-decodes with the current format on the next miss.
        if image.format != self.image_format {
            return;
        }
        let Some(first) = image.mips.get(first_mip) else {
            return;
        };
        let side = first.width.max(first.height).next_power_of_two().max(16);
        let Some(&(size, layers)) = CLASSES.iter().find(|c| c.0 >= side) else {
            return;
        };
        if self.resident(hash, side) {
            return;
        }
        if first_mip == 0
            && side > FALLBACK_SIDE
            && let Some(base) = image
                .mips
                .iter()
                .position(|mip| mip.width.max(mip.height) <= FALLBACK_SIDE)
        {
            self.upload_mip(device, queue, image, base);
        }
        if let Some((p, l)) = self.locations.remove(&hash)
            && let Some(a) = self.arrays[p].as_mut()
        {
            if a.size <= FALLBACK_SIDE && size > a.size {
                self.fallbacks.insert(hash, (p, l));
            } else {
                a.slots[l] = None;
            }
        }
        let Some(index) = self.page(device, size, layers, true) else {
            return;
        };
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
            if self.locations.get(&old.hash) == Some(&(index, layer)) {
                self.locations.remove(&old.hash);
                if let Some(fallback) = self.fallbacks.remove(&old.hash) {
                    self.locations.insert(old.hash, fallback);
                }
            } else {
                self.fallbacks.remove(&old.hash);
            }
        }
        self.locations.insert(hash, (index, layer));
        self.clock += 1;
        array.slots[layer] = Some(Slot {
            hash,
            side: size,
            original: image.original,
            tile: image.tile.is_some(),
            dimensions: [first.width, first.height],
            used: self.clock,
            mips: (image.mips.len() - first_mip) as u32,
            detail_lod: 0.,
        });
        for (level, mip) in image.mips.iter().skip(first_mip).enumerate() {
            // BC7 blocks are 4x4; the write extent and stride are block-aligned.
            let (width, height, bytes_per_row, rows) = match self.image_format {
                ImageFormat::Rgba8 => (mip.width, mip.height, mip.width * 4, mip.height),
                ImageFormat::Bc7 => (
                    mip.width.next_multiple_of(4),
                    mip.height.next_multiple_of(4),
                    mip.width.next_multiple_of(4) * 4,
                    mip.height.div_ceil(4),
                ),
            };
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
                &mip.bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(rows),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    fn page(
        &mut self,
        device: &wgpu::Device,
        size: u32,
        layers: u32,
        evict: bool,
    ) -> Option<usize> {
        // Newest pages are the ones still filling; searching from the end keeps a
        // board-sized upload linear instead of rescanning every full page.
        let free = self.arrays.iter().rposition(|a| {
            a.as_ref()
                .is_some_and(|a| a.size == size && a.slots.iter().any(Option::is_none))
        });
        let index = if let Some(i) = free {
            i
        } else {
            let bytes = array_bytes(size, layers, self.image_format);
            self.budget = budget_for(self.allocated.saturating_add(bytes), self.ceiling);
            if !evict && self.allocated + bytes > self.budget {
                return None;
            }
            if evict
                && self.allocated + bytes > self.budget
                && let Some((index, _)) = self
                    .arrays
                    .iter()
                    .enumerate()
                    .filter_map(|(index, array)| {
                        array.as_ref().filter(|a| a.size == size).map(|a| {
                            (
                                index,
                                a.slots.iter().flatten().map(|s| s.used).min().unwrap_or(0),
                            )
                        })
                    })
                    .min_by_key(|(_, used)| *used)
            {
                return Some(index);
            }
            while self.allocated + bytes > self.budget {
                if !self.evict_oldest() {
                    return None;
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
            let bytes = array_bytes(size, layers, self.image_format);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("image size class"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: layers,
                },
                mip_level_count: mip_levels(size, self.image_format),
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu_format(self.image_format),
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
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
        Some(index)
    }

    fn shrink(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, hash: BlobId, side: u32) {
        let Some(&(source, source_layer)) = self.locations.get(&hash) else {
            return;
        };
        let slot = self.arrays[source]
            .as_ref()
            .and_then(|a| a.slots[source_layer]);
        let Some(slot) = slot else { return };
        // Keep zoom headroom so small wheel reversals do not trigger another decode.
        if slot.side <= side.saturating_mul(4) {
            return;
        }
        let size = side.saturating_mul(2).clamp(16, 2048).next_power_of_two();
        let base = slot.side.ilog2() - size.ilog2();
        if base >= slot.mips {
            return;
        }
        let Some(&(_, layers)) = CLASSES.iter().find(|c| c.0 == size) else {
            return;
        };
        // Never evict the source (or another visible image) to make room for a copy.
        let Some(dest) = self.page(device, size, layers, false) else {
            return;
        };
        let layer = self.arrays[dest]
            .as_ref()
            .and_then(|a| a.slots.iter().position(Option::is_none));
        let Some(layer) = layer else { return };
        let mut encoder = device.create_command_encoder(&Default::default());
        let source_array = self.arrays[source]
            .as_ref()
            .expect("source page cannot be evicted during a shrink");
        let dest_array = self.arrays[dest]
            .as_ref()
            .expect("page allocation succeeded");
        let mips = (slot.mips - base).min(mip_levels(size, self.image_format));
        for level in 0..mips {
            let extent = size >> level;
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &source_array.texture,
                    mip_level: base + level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: source_layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &dest_array.texture,
                    mip_level: level,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: extent,
                    height: extent,
                    depth_or_array_layers: 1,
                },
            );
        }
        queue.submit([encoder.finish()]);
        self.arrays[dest]
            .as_mut()
            .expect("page allocation succeeded")
            .slots[layer] = Some(Slot {
            side: size,
            dimensions: slot.dimensions.map(|v| (v >> base).max(1)),
            mips,
            detail_lod: (slot.detail_lod - base as f32).max(0.),
            ..slot
        });
        self.locations.insert(hash, (dest, layer));
        self.arrays[source]
            .as_mut()
            .expect("source page cannot be evicted during a shrink")
            .slots[source_layer] = None;
        if size <= FALLBACK_SIDE
            && let Some(fallback) = self.fallbacks.remove(&hash)
        {
            self.arrays[fallback.0]
                .as_mut()
                .expect("fallback mapping owns a live page")
                .slots[fallback.1] = None;
        }
        self.release_empty_pages();
    }

    fn evict_oldest(&mut self) -> bool {
        let oldest = self
            .arrays
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                a.as_ref().map(|a| {
                    (
                        i,
                        (
                            a.size <= FALLBACK_SIDE,
                            a.slots.iter().flatten().map(|s| s.used).max().unwrap_or(0),
                        ),
                    )
                })
            })
            .min_by_key(|a| a.1)
            .map(|a| a.0);
        let Some(i) = oldest else {
            return false;
        };
        if let Some(a) = self.arrays[i].take() {
            for (layer, slot) in a.slots.iter().enumerate() {
                if let Some(slot) = slot {
                    self.forget(slot.hash, (i, layer));
                }
            }
            self.allocated -= array_bytes(a.size, a.slots.len() as u32, self.image_format);
        }
        while self.arrays.last().is_some_and(Option::is_none) {
            self.arrays.pop();
        }
        true
    }

    fn forget(&mut self, hash: BlobId, location: (usize, usize)) {
        if self.locations.get(&hash) == Some(&location) {
            self.locations.remove(&hash);
            if let Some(fallback) = self.fallbacks.remove(&hash) {
                self.locations.insert(hash, fallback);
            }
        } else if self.fallbacks.get(&hash) == Some(&location) {
            self.fallbacks.remove(&hash);
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
    /// Draws on top of what is already in the target (the minimap) with resident
    /// textures only: nothing is requested, grown or shrunk for these items.
    pub fn draw_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: Target<'_>,
        items: &[&Item],
        camera: Camera,
    ) {
        if items.is_empty() {
            return;
        }
        let scale = target.view.texture().width() as f32 / target.screen[0];
        let mut instances = Vec::with_capacity(items.len().min(MAX_INSTANCES));
        let mut classes = Vec::with_capacity(items.len().min(MAX_INSTANCES));
        let target = Target {
            grid: false,
            ..target
        };
        let mut first = false;
        for item in items {
            self.item_instances(item, camera, target, scale, &mut instances, &mut classes);
            if instances.len() >= MAX_INSTANCES {
                self.submit_instances(device, queue, target, &instances, &classes, &mut first);
                instances.clear();
                classes.clear();
            }
        }
        if !instances.is_empty() {
            self.submit_instances(device, queue, target, &instances, &classes, &mut first);
        }
    }
    fn draw_images(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: Target<'_>,
        items: &[&Item],
        camera: Camera,
    ) {
        let _span = tracing::info_span!("canvas_draw", visible = items.len()).entered();
        self.clock += 1;
        let scale = target.view.texture().width() as f32 / target.screen[0];
        let mut demand = std::collections::HashMap::<BlobId, u32>::new();
        for item in items {
            if let Some(hash) = item.blob() {
                let side =
                    preview_side(item.rect.width.max(item.rect.height) * camera.zoom * scale);
                demand
                    .entry(hash)
                    .and_modify(|s| *s = (*s).max(side))
                    .or_insert(side);
            }
        }
        if !self.keep_all {
            for (hash, side) in demand {
                self.shrink(device, queue, hash, side);
            }
        }
        let mut instances = Vec::with_capacity(MAX_INSTANCES);
        let mut classes = Vec::with_capacity(MAX_INSTANCES);
        let mut first = true;
        for item in items {
            self.item_instances(item, camera, target, scale, &mut instances, &mut classes);
            if instances.len() >= MAX_INSTANCES {
                self.submit_instances(device, queue, target, &instances, &classes, &mut first);
                instances.clear();
                classes.clear();
            }
        }
        if !instances.is_empty() || first {
            self.submit_instances(device, queue, target, &instances, &classes, &mut first);
        }
    }
    fn item_instances(
        &mut self,
        item: &Item,
        camera: Camera,
        target: Target<'_>,
        scale: f32,
        instances: &mut Vec<Instance>,
        classes: &mut Vec<usize>,
    ) {
        if let Some(group) = item.group_style() {
            let mut style = self
                .backdrops
                .get(group.color as usize)
                .copied()
                .unwrap_or(self.panel);
            // Real groups keep the identity transform; app overlays (the minimap) fade
            // their panel through the item's opacity.
            style.fill[3] *= item.transform.opacity;
            style.border[3] *= item.transform.opacity;
            instances.push(backdrop_instance(
                item.rect,
                style,
                camera,
                target.screen,
                scale,
            ));
            classes.push(BACKDROP);
            return;
        }
        let Some(blob) = item.blob() else { return };
        let Some(&(class, layer)) = self.locations.get(&blob) else {
            return;
        };
        let Some(slot) = self.arrays[class].as_ref().and_then(|a| a.slots[layer]) else {
            return;
        };
        self.arrays[class]
            .as_mut()
            .expect("resident page was just checked")
            .slots[layer]
            .as_mut()
            .expect("resident slot was just checked")
            .used = self.clock;
        let wanted = (item.rect.width.max(item.rect.height) * camera.zoom * scale).max(1.);
        let base_lod = ((slot.dimensions[0].max(slot.dimensions[1]) as f32 / wanted).log2()
            - item.transform.content_scale.log2())
        .max(slot.detail_lod)
        .min(slot.mips.saturating_sub(1) as f32);
        let pixel_scale = (item.rect.width / slot.original[0] as f32)
            .max(item.rect.height / slot.original[1] as f32)
            * item.transform.content_scale
            * camera.zoom
            * scale;
        let nearest = item.transform.sampling.nearest(self.sampling, pixel_scale);
        let base_lod = if nearest { base_lod.round() } else { base_lod };
        let appearance = [
            item.transform.opacity,
            if item.transform.grayscale || self.grayscale {
                1.
            } else {
                0.
            },
            0.,
            if nearest { 1. } else { 0. },
        ];
        let clip = target.clip.map_or(
            [0., 0., target.screen[0], target.screen[1]],
            |[x, y, w, h]| {
                [
                    x as f32 / scale,
                    y as f32 / scale,
                    (x + w) as f32 / scale,
                    (y + h) as f32 / scale,
                ]
            },
        );
        let tiles = if self.tile_detail {
            tiles::visible(
                item,
                slot.original,
                camera,
                target.screen,
                clip,
                scale,
                self.detail(self.tile_lod_bias),
            )
        } else {
            Vec::new()
        };
        if tiles.is_empty() {
            let mut shape = content(item, slot.original, camera, target.screen);
            shape.region = source_region(slot.dimensions.map(|v| v as f32), item.transform.flip);
            instances.push(Instance {
                corners: shape.corners,
                frame: shape.frame,
                region: shape.region,
                layer: layer as u32,
                lod: base_lod,
                appearance,
            });
            classes.push(class);
            return;
        }
        let shape = content(item, slot.original, camera, target.screen);
        for tile in tiles {
            let range = tiles::range(tile, slot.original);
            let shape = tiles::piece(&shape, range, item.transform.flip);
            let (class, layer, region, continuous) = if let Some(&(page, layer)) =
                self.locations.get(&tile.key(blob))
            {
                if let Some(slot) = self.arrays[page]
                    .as_mut()
                    .and_then(|a| a.slots[layer].as_mut())
                {
                    slot.used = self.clock;
                }
                let bounds = tile
                    .bounds(slot.original)
                    .expect("tile coverage was clipped");
                (
                    page,
                    layer,
                    [0.5, 0.5, bounds[2] as f32 + 1., bounds[3] as f32 + 1.],
                    0.,
                )
            } else if let Some((page, layer, region)) = self.ancestor(blob, tile, slot.original) {
                (page, layer, region, 1.)
            } else {
                (
                    class,
                    layer,
                    [
                        range[0] * slot.dimensions[0].saturating_sub(1) as f32,
                        range[1] * slot.dimensions[1].saturating_sub(1) as f32,
                        (range[2] - range[0]) * slot.dimensions[0].saturating_sub(1) as f32,
                        (range[3] - range[1]) * slot.dimensions[1].saturating_sub(1) as f32,
                    ],
                    1.,
                )
            };
            instances.push(Instance {
                corners: shape.corners,
                frame: shape.frame,
                region,
                layer: layer as u32,
                lod: 0.,
                appearance: [appearance[0], appearance[1], continuous, appearance[3]],
            });
            classes.push(class);
        }
    }
    /// The closest resident coarser tile and the texel region covering `tile`.
    ///
    /// The region uses the continuous sampling path: `0.5 + xy` is the left/top edge of
    /// the child's content inside the parent layer, whose texel 0 is the filter gutter.
    fn ancestor(
        &mut self,
        blob: BlobId,
        tile: moth_io::tiles::Tile,
        original: [u32; 2],
    ) -> Option<(usize, usize, [f32; 4])> {
        let bounds = tile.bounds(original)?;
        for levels in 1..=PARENT_LEVELS {
            let parent = tile.parent(levels)?;
            let Some(&(page, layer)) = self.locations.get(&parent.key(blob)) else {
                continue;
            };
            let Some(slot) = self.arrays[page]
                .as_mut()
                .and_then(|a| a.slots[layer].as_mut())
            else {
                continue;
            };
            slot.used = self.clock;
            let origin = parent.bounds(original)?;
            let scale = (1u32 << levels) as f32;
            return Some((
                page,
                layer,
                [
                    0.5 + bounds[0] as f32 / scale - origin[0] as f32,
                    0.5 + bounds[1] as f32 / scale - origin[1] as f32,
                    bounds[2] as f32 / scale,
                    bounds[3] as f32 / scale,
                ],
            ));
        }
        None
    }
    fn submit_instances(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: Target<'_>,
        instances: &[Instance],
        classes: &[usize],
        first: &mut bool,
    ) {
        for batch in 0..instances.len().div_ceil(MAX_INSTANCES).max(1) {
            let start = batch * MAX_INSTANCES;
            let end = (start + MAX_INSTANCES).min(instances.len());
            queue.write_buffer(
                &self.instances,
                0,
                bytemuck::cast_slice(&instances[start..end]),
            );
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("canvas batch"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: if *first {
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
                if *first && target.grid {
                    pass.set_pipeline(&self.grid);
                    pass.draw(0..3, 0..1);
                }
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, self.instances.slice(..));
                let mut run = start;
                while run < end {
                    let class = classes[run];
                    let mut next = run + 1;
                    while next < end && classes[next] == class {
                        next += 1;
                    }
                    if class == BACKDROP {
                        pass.set_pipeline(&self.backdrop);
                        pass.draw(0..6, (run - start) as u32..(next - start) as u32);
                        pass.set_pipeline(&self.pipeline);
                    } else if let Some(array) = self.arrays.get(class).and_then(Option::as_ref) {
                        pass.set_bind_group(0, &array.bind, &[]);
                        pass.draw(0..6, (run - start) as u32..(next - start) as u32);
                    }
                    run = next;
                }
            }
            queue.submit([encoder.finish()]);
            *first = false;
        }
    }
    /// Board teardown explicitly releases arrays; accounting must return to zero.
    pub fn clear(&mut self) {
        self.opacity.clear();
        for array in &mut self.arrays {
            if let Some(a) = array.take() {
                self.allocated -= array_bytes(a.size, a.slots.len() as u32, self.image_format);
            }
        }
        self.locations.clear();
        self.fallbacks.clear();
        self.locations.shrink_to_fit();
        self.fallbacks.shrink_to_fit();
        self.arrays.clear();
        self.arrays.shrink_to_fit();
        debug_assert_eq!(self.allocated, 0);
    }
}
impl Drop for Registry {
    fn drop(&mut self) {
        self.clear();
    }
}
/// `ceiling` is already capped by the registry's hard limit.
fn budget_for(bytes: u64, ceiling: u64) -> u64 {
    bytes.max(MIN_TEXTURE_BUDGET).min(ceiling)
}

fn array_bytes(size: u32, layers: u32, format: ImageFormat) -> u64 {
    (0..mip_levels(size, format))
        .map(|m| {
            u64::from((size >> m).max(1)).pow(2)
                * match format {
                    ImageFormat::Rgba8 => 4,
                    ImageFormat::Bc7 => 1,
                }
                * u64::from(layers)
        })
        .sum()
}
/// Texture format for image mips, from the runtime [`ImageFormat`] choice.
fn wgpu_format(format: ImageFormat) -> wgpu::TextureFormat {
    match format {
        ImageFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        ImageFormat::Bc7 => wgpu::TextureFormat::Bc7RgbaUnormSrgb,
    }
}
/// Number of mip levels a texture of `size` gets: BC7 stops at 4x4 blocks.
fn mip_levels(size: u32, format: ImageFormat) -> u32 {
    match format {
        ImageFormat::Rgba8 => size.ilog2() + 1,
        ImageFormat::Bc7 => size.ilog2() - 1,
    }
}
/// Preview requests use size classes to avoid decoding on every zoom tick.
pub fn preview_side(pixels: f32) -> u32 {
    (pixels.ceil().clamp(1., 2048.) as u32)
        .next_power_of_two()
        .clamp(16, 2048)
}

/// Chooses the runtime image format from adapter capabilities: BC7 when the
/// adapter can bind and copy it and this build can encode it, otherwise RGBA8.
///
/// The BC7 encoder only exists on x86/x86_64. Apple silicon and Windows on Arm
/// adapters can report BC support; requesting BC7 there would make every decoded
/// RGBA8 preview mismatch the registry format and never upload.
pub fn pick_image_format(adapter: &wgpu::Adapter) -> ImageFormat {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64"))
        // wgpu's GL copies attach the source to an FBO; compressed textures cannot attach.
        && adapter.get_info().backend != wgpu::Backend::Gl
        && adapter
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    {
        ImageFormat::Bc7
    } else {
        ImageFormat::Rgba8
    }
}

/// Device descriptor that enables BC7 where the adapter supports it, so the
/// compressed image path can actually create `Bc7RgbaUnormSrgb` textures.
fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    let mut required = wgpu::Features::empty();
    if adapter
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    {
        required |= wgpu::Features::TEXTURE_COMPRESSION_BC;
    }
    wgpu::DeviceDescriptor {
        required_features: required,
        ..Default::default()
    }
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
    match adapter.request_device(&device_descriptor(&adapter)).await {
        Ok((device, queue)) => Ok((adapter, device, queue)),
        Err(_) if !software => {
            let a = instance
                .request_adapter(&options(true))
                .await
                .map_err(|e| e.to_string())?;
            let (d, q) = a
                .request_device(&device_descriptor(&a))
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
    fn adaptive_budget_grows_shrinks_and_has_a_hard_ceiling() {
        assert_eq!(budget_for(0, 0), 0);
        assert_eq!(budget_for(1, MAX_TEXTURE_BUDGET), MIN_TEXTURE_BUDGET);
        assert_eq!(
            budget_for(100 * 1024 * 1024, MAX_TEXTURE_BUDGET),
            100 * 1024 * 1024
        );
        assert_eq!(
            budget_for(1024 * 1024 * 1024, TEXTURE_BUDGET),
            TEXTURE_BUDGET
        );
        assert_eq!(budget_for(100 * 1024 * 1024, 1024), 1024);
        assert_eq!(budget_for(u64::MAX, MAX_TEXTURE_BUDGET), MAX_TEXTURE_BUDGET);
    }
    #[test]
    fn texture_budget_covers_all_classes() {
        for format in [ImageFormat::Rgba8, ImageFormat::Bc7] {
            let bytes: u64 = CLASSES
                .iter()
                .map(|&(s, l)| array_bytes(s, l, format))
                .sum();
            assert!(bytes <= TEXTURE_BUDGET, "{format:?} exceeds budget");
        }
        assert_eq!(preview_side(257.), 512);
    }
}

pub mod headless;
