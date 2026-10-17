// SPDX-License-Identifier: GPL-3.0-or-later
@group(0) @binding(0) var board: texture_2d<f32>;
@group(0) @binding(1) var<uniform> opacity:vec4<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
    let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
@fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
    return textureLoad(board,vec2<i32>(p.xy),0)*opacity.x;
}
