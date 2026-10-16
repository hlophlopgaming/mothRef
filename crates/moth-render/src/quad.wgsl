// SPDX-License-Identifier: GPL-3.0-or-later
override OUTPUT_SRGB: bool = false;
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) @interpolate(flat) lod: f32,
    @location(3) @interpolate(flat) uv_size: vec2<f32>,
}
@group(0) @binding(0) var images: texture_2d_array<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@vertex fn vs(
    @builtin(vertex_index) vertex: u32,
    @location(0) rect: vec4<f32>,
    @location(1) uv_size: vec2<f32>,
    @location(2) layer: u32,
    @location(3) lod: f32,
) -> Out {
    let corners = array<vec2<f32>, 6>(
        vec2(0.,0.), vec2(1.,0.), vec2(0.,1.),
        vec2(0.,1.), vec2(1.,0.), vec2(1.,1.),
    );
    let p = corners[vertex];
    var out: Out;
    out.pos = vec4(rect.xy + p * rect.zw, 0., 1.);
    out.uv = p * uv_size;
    out.layer = layer;
    out.lod = lod;
    out.uv_size = uv_size;
    return out;
}
fn sample_mip(in: Out, level: f32) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(images, i32(level)));
    let pixels = max(vec2(1.), floor(in.uv_size * vec2<f32>(textureDimensions(images)) / exp2(level)));
    // Each padded level has its own valid pixel extent, especially for odd dimensions.
    let uv = (vec2(0.5) + in.uv / in.uv_size * (pixels - vec2(1.))) / size;
    return textureSampleLevel(images, linear_sampler, uv, i32(in.layer), level);
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    let color = mix(sample_mip(in, floor(in.lod)), sample_mip(in, ceil(in.lod)), fract(in.lod));
    if OUTPUT_SRGB { return color; }
    let gamma = select(color.rgb * 12.92, 1.055 * pow(color.rgb, vec3(1. / 2.4)) - 0.055, color.rgb > vec3(0.0031308));
    return vec4(gamma, color.a);
}
