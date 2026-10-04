// Coverage fonts and analytic antialiasing for thin native Blueprint strokes.
@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
override display_composition: bool = true;
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) edge: vec4<f32>,
};
@vertex
fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>, @location(3) edge: vec4<f32>) -> VsOut {
    var out: VsOut;out.pos=vec4<f32>(pos,0.0,1.0);out.uv=uv;out.color=color;out.edge=edge;return out;
}
fn display_color(linear: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055,
        linear * 12.92, linear <= vec3<f32>(0.0031308));
}
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // Coverage is rasterized at physical pixel size; extra filtering blurs it.
    var coverage=textureSample(atlas,atlas_sampler,in.uv).r;
    var shape=1.0;
    if in.edge.z>=0.0 {
        if in.edge.w < -1.5 {shape=clamp(-in.edge.w-2.0-abs((length(in.edge.xy)-1.0)*in.edge.z)+0.5,0.0,1.0);}
        else if in.edge.w<0.0 {shape=clamp(in.edge.z-length(in.edge.xy)+0.5,0.0,1.0);}
        else {shape=clamp(in.edge.z-abs(in.edge.x)+0.5,0.0,1.0)*clamp(in.edge.w-abs(in.edge.y)+0.5,0.0,1.0);}
    }
    // Compose on the non-sRGB surface view, matching CSS alpha and glyph coverage.
    if display_composition {
        return vec4<f32>(display_color(in.color.rgb),in.color.a*coverage*shape);
    }
    // Older GL devices cannot alias the surface's sRGB and display-space views.
    if in.edge.z < 0.0 { coverage=pow(coverage,1.8); }
    return vec4<f32>(in.color.rgb,in.color.a*coverage*shape);
}
