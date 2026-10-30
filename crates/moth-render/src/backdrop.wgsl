// SPDX-License-Identifier: GPL-3.0-or-later
// Group backdrops: a rounded, tinted rectangle with a thin border, drawn in z-order
// between image quads. Shares the image instance layout; see `backdrop_instance`.
override OUTPUT_SRGB: bool = false;
struct Out {
    @builtin(position) pos: vec4<f32>,
    // Position inside the backdrop in physical pixels.
    @location(0) pixel: vec2<f32>,
    @location(1) @interpolate(flat) size: vec2<f32>,
    @location(2) @interpolate(flat) fill: vec4<f32>,
    @location(3) @interpolate(flat) border: vec4<f32>,
    @location(4) @interpolate(flat) radius: f32,
}
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
    let corners = array<vec2<f32>, 4>(c0, c1, c2, c3);
    let frames = array<vec2<f32>, 4>(f0, f1, f2, f3);
    let order = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let i = order[vertex];
    var out: Out;
    out.pos = vec4(corners[i], 0., 1.);
    out.pixel = frames[i];
    out.size = f3;
    out.fill = region;
    out.border = appearance;
    out.radius = lod;
    return out;
}
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
    // Signed distance to the rounded rectangle, negative inside.
    let half = in.size * 0.5;
    let radius = min(in.radius, min(half.x, half.y));
    let q = abs(in.pixel - half) - (half - vec2(radius));
    let distance = length(max(q, vec2(0.))) + min(max(q.x, q.y), 0.) - radius;
    let inside = clamp(0.5 - distance, 0., 1.);
    let edge = clamp(distance + 1.5, 0., 1.) * inside;
    var color = mix(in.fill, in.border, edge);
    color.a *= inside;
    if OUTPUT_SRGB { return color; }
    let gamma = select(color.rgb * 12.92, 1.055 * pow(color.rgb, vec3(1. / 2.4)) - 0.055, color.rgb > vec3(0.0031308));
    return vec4(gamma, color.a);
}
