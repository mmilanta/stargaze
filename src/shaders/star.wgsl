// Star catalogue as instanced, screen-space sized billboards at infinity.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>,
    cam_zenith: vec4<f32>,
    stars: array<vec4<f32>, 4>,
    star_color: array<vec4<f32>, 4>,
    occluders: array<vec4<f32>, 8>,
    star_meta: vec4<f32>,
    occluder_meta: vec4<f32>,
    params: vec4<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) bright: f32,
    @location(3) above: f32,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) dir: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) size: f32,
    @location(3) bright: f32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];

    // Place at a huge distance along the direction: effectively at infinity.
    let clip = g.view_proj * vec4<f32>(dir * 1.0e5, 1.0);
    let ndc_off = vec2<f32>(
        c.x * size * 2.0 / g.viewport.x,
        c.y * size * 2.0 / g.viewport.y,
    );
    var p = clip;
    p.x = p.x + ndc_off.x * clip.w;
    p.y = p.y + ndc_off.y * clip.w;

    var out: VsOut;
    out.pos = p;
    out.uv = c;
    out.color = color;
    out.bright = bright;
    out.above = dot(normalize(dir), g.cam_zenith.xyz);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    if (in.above < 0.0) {
        discard;
    }
    let d = length(in.uv);
    let a = smoothstep(1.0, 0.0, d);
    let night = g.params.z;
    let v = in.color * in.bright * a * night;
    // Additive: sky + bodies are already in the target.
    return vec4<f32>(v, 1.0);
}
