// SPDX-License-Identifier: GPL-3.0-or-later
override OUTPUT_SRGB: bool = false;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
    let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
@fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
    let d=length((p.xy%40.)-vec2(20.));
    var color=mix(vec3(17.,19.,23.)/255.,vec3(43.,46.,52.)/255.,1.-smoothstep(0.5,1.,d));
    if OUTPUT_SRGB {
        color=select(color/12.92,pow((color+vec3(0.055))/1.055,vec3(2.4)),color>vec3(0.04045));
    }
    return vec4(color,1.);
}
