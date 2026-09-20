// Fullscreen sky: dark background, ground below the horizon, a glow around each
// star, and ACES tone mapping. Rendered first with no depth writes.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>, // w = tan(fov_y / 2)
    cam_zenith: vec4<f32>,
    stars: array<vec4<f32>, 4>,
    star_color: array<vec4<f32>, 4>,
    occluders: array<vec4<f32>, 8>,
    star_meta: vec4<f32>,
    occluder_meta: vec4<f32>,
    params: vec4<f32>,      // x = aspect, y = ambient, z = night, w = sun light
    viewport: vec4<f32>,    // x = width, y = height, z = exposure
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let xy = corners[vi];
    var out: VsOut;
    out.pos = vec4<f32>(xy, 0.0, 1.0);
    out.ndc = xy;
    return out;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = g.params.x;
    let tan_half = g.cam_forward.w;
    let dir = normalize(
        g.cam_forward.xyz
        + in.ndc.x * aspect * tan_half * g.cam_right.xyz
        + in.ndc.y * tan_half * g.cam_up.xyz
    );

    let night = g.params.z;
    let zen = dot(dir, g.cam_zenith.xyz);

    let night_sky = vec3<f32>(0.0014, 0.0018, 0.0036);
    let day_sky = vec3<f32>(0.06, 0.13, 0.30);
    let night_gnd = vec3<f32>(0.0011, 0.0011, 0.0013);
    let day_gnd = vec3<f32>(0.012, 0.011, 0.010);

    var col = mix(day_sky, night_sky, night);
    let gnd = mix(day_gnd, night_gnd, night);

    if (zen < 0.0) {
        col = gnd;
    } else {
        // Slight haze / darkening towards the horizon.
        let horizon = clamp(1.0 - zen * 5.0, 0.0, 1.0);
        col = mix(col, mix(col, gnd, 0.35), horizon * 0.6);

        // A coloured glow around every star that is above the horizon.
        let count = i32(g.star_meta.x);
        for (var i = 0; i < count; i = i + 1) {
            let sdir = normalize(g.stars[i].xyz);
            let up = smoothstep(-0.10, 0.05, dot(sdir, g.cam_zenith.xyz));
            let sd = max(dot(dir, sdir), 0.0);
            let halo = pow(sd, 4000.0) * 1.5 + pow(sd, 150.0) * 0.10 + pow(sd, 30.0) * 0.02;
            col += g.star_color[i].rgb * halo * up * g.star_color[i].a;
        }
    }

    col = aces(col * g.viewport.z);
    return vec4<f32>(col, 1.0);
}
