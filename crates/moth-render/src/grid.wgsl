// SPDX-License-Identifier: GPL-3.0-or-later
// Board grid: dots or lines at a world spacing, drawn behind images. All lengths are
// physical pixels; `offset` is where a world grid line crosses the screen.
override OUTPUT_SRGB: bool = false;
struct Grid {
    offset: vec2<f32>,
    spacing: f32,
    lines: f32,
    color: vec4<f32>,
    // Every `major`-th line or dot is drawn at full strength, the others fainter.
    major: f32,
    // Dot radius or half the line width.
    size: f32,
    minor: f32,
    pad: f32,
};
@group(0) @binding(0) var<uniform> grid: Grid;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
    let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
fn wrap(v: vec2<f32>, m: f32) -> vec2<f32> {
    return v - m * floor(v / m);
}
@fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
    let local = p.xy - grid.offset;
    let q = wrap(local, grid.spacing);
    let edge = min(q, vec2(grid.spacing) - q);
    // Which line or dot is nearest, to tell major ones apart.
    let index = floor(local / grid.spacing + vec2(0.5));
    let major = abs(wrap(index, grid.major)) < vec2(0.5);
    var alpha: f32;
    if grid.lines > 0.5 {
        let ax = 1. - smoothstep(grid.size - 0.5, grid.size + 0.5, edge.x);
        let ay = 1. - smoothstep(grid.size - 0.5, grid.size + 0.5, edge.y);
        let wx = select(grid.minor, 1., major.x);
        let wy = select(grid.minor, 1., major.y);
        alpha = max(ax * wx, ay * wy);
    } else {
        let d = length(edge);
        alpha = (1. - smoothstep(grid.size - 0.5, grid.size + 0.5, d))
            * select(grid.minor, 1., major.x && major.y);
    }
    var color = grid.color.rgb;
    if OUTPUT_SRGB {
        color = select(color / 12.92, pow((color + vec3(0.055)) / 1.055, vec3(2.4)), color > vec3(0.04045));
    }
    return vec4(color, alpha * grid.color.a);
}
