// Lit and emissive spheres. Lighting sums every star, each with its own
// inverse-square falloff, colour and cast shadows (so a body can be shadowed
// from one star while still lit by another).

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
    params: vec4<f32>,      // x = aspect, y = ambient, z = night, w = sun light
    viewport: vec4<f32>,    // x = width, y = height, z = exposure
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) center: vec3<f32>,
    @location(3) radius: f32,
    @location(4) color: vec3<f32>,
    @location(5) emissive: f32,
    @location(6) intensity: f32,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emissive: f32,
    @location(4) intensity: f32,
    @location(5) local: vec3<f32>,
};

@vertex
fn vs(in: VsIn) -> VsOut {
    // Guarantee a minimum on-screen size so distant planets stay visible.
    let d = max(length(in.center), 1.0e-9);
    let min_ang = 1.8 * 2.0 * g.cam_forward.w / g.viewport.y;
    let eff_r = max(in.radius, min_ang * d);
    let world = in.center + in.pos * eff_r;
    var out: VsOut;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    out.world = world;
    out.nrm = in.nrm;
    out.color = in.color;
    out.emissive = in.emissive;
    out.intensity = in.intensity;
    out.local = in.pos;
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

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    return mix(mix(nx00, nx10, u.y), mix(nx01, nx11, u.y), u.z);
}

fn fbm(p0: vec3<f32>) -> f32 {
    var p = p0;
    var a = 0.5;
    var s = 0.0;
    for (var k = 0; k < 5; k = k + 1) {
        s = s + a * vnoise(p);
        p = p * 2.07;
        a = a * 0.5;
    }
    return s;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // Cut bodies off at the local horizon, like the ground in the sky shader.
    if (dot(normalize(in.world), g.cam_zenith.xyz) < 0.0) {
        discard;
    }
    let n = normalize(in.nrm);
    var col: vec3<f32>;

    if (in.emissive > 0.5) {
        // Simple photosphere limb darkening.
        let v = normalize(-in.world);
        let mu = clamp(dot(n, v), 0.0, 1.0);
        let ld = 0.45 + 0.55 * pow(mu, 0.55);
        col = in.color * in.intensity * ld;
    } else {
        let mottle = 0.68 + 0.64 * fbm(in.local * 2.4);
        var light = vec3<f32>(0.0);
        var min_shadow = 1.0;

        let star_count = i32(g.star_meta.x);
        let occ_count = i32(g.occluder_meta.x);
        for (var i = 0; i < star_count; i = i + 1) {
            let to_star = g.stars[i].xyz - in.world;
            let dist = max(length(to_star), 1.0e-9);
            let l = to_star / dist;
            let ndl = max(dot(n, l), 0.0);
            let star_ang = g.stars[i].w / dist;

            // Shadows cast by every body for this particular star.
            var sh = 1.0;
            for (var j = 0; j < occ_count; j = j + 1) {
                let o = g.occluders[j];
                let oc = o.xyz - in.world;
                let r = o.w;
                let tca = dot(oc, l);
                if (tca > r) {
                    let perp = sqrt(max(dot(oc, oc) - tca * tca, 0.0));
                    let pen = star_ang * tca;
                    sh = sh * smoothstep(r - pen, r + pen, perp);
                }
            }
            min_shadow = min(min_shadow, sh);
            light = light
                + g.star_color[i].rgb * g.star_color[i].a
                    * (g.params.w / (dist * dist)) * ndl * sh;
        }

        // Faint red glow when a body is in shadow from every star.
        let umbra = vec3<f32>(0.05, 0.012, 0.008) * (1.0 - min_shadow);
        col = in.color * mottle * (light + umbra + vec3<f32>(g.params.y));
    }

    col = aces(col * g.viewport.z);
    return vec4<f32>(col, 1.0);
}
