// SPDX-License-Identifier: GPL-3.0-or-later
override OUTPUT_SRGB: bool = false;
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) frame: vec2<f32>,
    @location(2) @interpolate(flat) region: vec4<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) lod: f32,
    @location(5) @interpolate(flat) appearance: vec4<f32>,
}
@group(0) @binding(0) var images: texture_2d_array<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@vertex fn vs(
    @builtin(vertex_index) vertex: u32,
    @location(0) c0: vec2<f32>,
    @location(1) c1: vec2<f32>,
    @location(2) c2: vec2<f32>,
    @location(3) c3: vec2<f32>,
    @location(4) f0: vec2<f32>,
    @location(5) f1: vec2<f32>,
    @location(6) f2: vec2<f32>,
    @location(7) f3: vec2<f32>,
    @location(8) region: vec4<f32>,
    @location(9) layer: u32,
    @location(10) lod: f32,
    @location(11) appearance: vec4<f32>,
) -> Out {
    // Two triangles over four corners; the CPU supplies rotated positions, so the shader
    // only picks one per vertex.
    let corners = array<vec2<f32>, 4>(c0, c1, c2, c3);
    let frames = array<vec2<f32>, 4>(f0, f1, f2, f3);
    let order = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let locals = array<vec2<f32>, 4>(
        vec2(0., 0.), vec2(1., 0.), vec2(0., 1.), vec2(1., 1.),
    );
    let i = order[vertex];
    var out: Out;
    out.pos = vec4(corners[i], 0., 1.);
    out.local = locals[i];
    out.frame = frames[i];
    out.region = region;
    out.layer = layer;
    out.lod = lod;
    out.appearance = appearance;
    return out;
}
fn sample_mip(in: Out, level: f32) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(images, i32(level)));
    var uv: vec2<f32>;
    if (in.appearance.z > 0.5) {
        // Missing tile cells share one continuous base-preview coordinate system.
        uv = (vec2(0.5) + in.region.xy + in.local * in.region.zw) / size;
    } else {
        let step = in.region.zw / exp2(level);
        let pixels = max(vec2(1.), floor(abs(step)));
        // Padded mips and mirrored content retain the same coordinates for both filters.
        uv = (in.region.xy / exp2(level)
            + (vec2(0.5) + in.local * (pixels - vec2(1.))) * sign(step)) / size;
    }
    if (in.appearance.w > 0.5) {
        let pixel = vec2<i32>(clamp(floor(uv * size), vec2(0.), size - vec2(1.)));
        return textureLoad(images, pixel, i32(in.layer), i32(level));
    }
    return textureSampleLevel(images, linear_sampler, uv, i32(in.layer), level);
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    // The frame clips the content, which only the fragment stage can do once it rotates.
    // The slack is under a thousandth of a frame and only absorbs edge rounding.
    if (any(in.frame < vec2(-0.0001)) || any(in.frame > vec2(1.0001))) {
        discard;
    }
    var color = mix(sample_mip(in, floor(in.lod)), sample_mip(in, ceil(in.lod)), fract(in.lod));
    color = vec4(mix(color.rgb, vec3(dot(color.rgb, vec3(0.2126, 0.7152, 0.0722))), in.appearance.y), color.a * in.appearance.x);
    if OUTPUT_SRGB { return color; }
    let gamma = select(color.rgb * 12.92, 1.055 * pow(color.rgb, vec3(1. / 2.4)) - 0.055, color.rgb > vec3(0.0031308));
    return vec4(gamma, color.a);
}
