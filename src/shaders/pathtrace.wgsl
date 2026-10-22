// Path tracer with single-scattering atmosphere and cached local sky lighting.
// Scene visibility uses analytic bodies, rings, and metre-scale landscape primitives.
// Direct light: one solid-angle sample per stellar disc, with power-heuristic MIS.
// Indirect light: cosine-weighted Lambertian bounces and Russian roulette.
struct Globals {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>, // w = tan(vertical FOV / 2)
    viewport: vec4<f32>, // trace width, height, exposure, display aspect
    // Host-atmosphere shell, in telescope space.
    atmo_center: vec4<f32>, // xyz = host centre, w = host radius
    atmo_rayleigh: vec4<f32>, // rgb = Rayleigh coefficients (1/AU), w = Mie
    atmo_params: vec4<f32>, // x = Mie g, y = scale height (AU), z = top altitude, w = host index + 1 (0 off)
    ground_east: vec4<f32>,
    ground_up: vec4<f32>,
    ground_north: vec4<f32>,
    ground_counts: vec4<u32>,
};
struct Settings {
    g: Globals,
    counts: vec4<u32>, // bodies, accumulated samples, samples this dispatch, max surface vertices
    background: vec4<f32>, // Milky Way brightness, map width, map height, point-source gain
    lights: vec4<u32>, // first three emissive indices, total count; fallback for >3
};
struct Body {
    center: vec4<f32>, // xyz = high part of telescope-space centre, w = radius
    material: vec4<f32>, // rgb = reflectance OR emitted radiance, w = emissive
    low: vec4<f32>, // xyz = residual centre, w = procedural albedo strength
    ring_plane: vec4<f32>, // xyz = pole, w = inner radius in planet radii
    ring_params: vec4<f32>, // outer radius in planet radii (0 = absent), optical depth, banded, unused
    ring_color: vec4<f32>, // rgb = single-scattering albedo
    axis_x: vec4<f32>, // xyz = body-fixed X axis, w = surface style (0 mottled, 1 gas giant)
    axis_y: vec4<f32>, // xyz = body-fixed Y axis, w = pattern seed
    surface_color: vec4<f32>, // rgb = belt albedo, w = turbulence
    surface_params: vec4<f32>, // x = band pairs, y = storm count
};
struct CatalogueStar {
    direction: vec4<f32>, // xyz = unit direction, w = sin(angular radius)^2
    radiance: vec4<f32>,
};
@group(0) @binding(0) var<uniform> settings: Settings;
@group(0) @binding(1) var<storage, read> bodies: array<Body>;
@group(0) @binding(2) var<storage, read> catalogue: array<CatalogueStar>;
// First 256*128+1 entries are offsets into the remaining star-index lists.
@group(0) @binding(3) var<storage, read> sky_cells: array<u32>;
@group(0) @binding(4) var<storage, read_write> accumulation: array<vec4<f32>>;
// One linear RGB10 texel per word.
@group(0) @binding(6) var<storage, read> milky_way_map: array<u32>;
@group(0) @binding(7) var<storage, read_write> backdrop_accumulation: array<vec4<f32>>;

const PI: f32 = 3.141592653589793;
const TAU: f32 = 6.283185307179586;
const MISS: u32 = 0xffffffffu;
// A *background* catalogue star never covers more than this many pixels in
// radius, so a deep zoom resolves points of light rather than filling the view
// with discs. Simulated bodies (including the system's real stars) are not
// capped: they grow to their true physical size when zoomed in.
const STAR_DOT_PIXELS: f32 = 3.0;
// View-ray and light-ray samples used by the single-scattering atmosphere.
const ATMO_VIEW_STEPS: u32 = 8u;
const ATMO_SUN_STEPS: u32 = 4u;

struct Ray {
    // Origin = bodies[anchor].center + offset; MISS means camera origin.
    // Rebase at each hit rather than forming a low-precision world-space hit.
    offset: vec3<f32>,
    direction: vec3<f32>,
    anchor: u32,
};
struct Hit {
    distance: f32,
    index: u32,
    normal: vec3<f32>,
};
struct LightSample {
    direction: vec3<f32>,
    pdf: f32,
};

fn hash(value: u32) -> u32 {
    var x = value;
    x = (x ^ (x >> 16u)) * 0x7feb352du;
    x = (x ^ (x >> 15u)) * 0x846ca68bu;
    return x ^ (x >> 16u);
}
fn random(state: ptr<function, u32>) -> f32 {
    *state = *state * 747796405u + 2891336453u;
    let word = ((*state >> ((*state >> 28u) + 4u)) ^ *state) * 277803737u;
    // 24 bits: never round up to 1.0.
    return f32(((word >> 22u) ^ word) >> 8u) * (1.0 / 16777216.0);
}

fn relative_center(ray: Ray, index: u32) -> vec3<f32> {
    if (ray.anchor == index) {
        // Never involve astronomical centres in the self-intersection test.
        // Even (centre-centre)+(low-low-offset) can be reassociated by GPU
        // floating-point optimization, quantizing the tiny surface offset
        // against the centre and producing concentric self-shadowing rings.
        return -ray.offset;
    }
    if (ray.anchor == MISS) {
        return bodies[index].center.xyz + (bodies[index].low.xyz - ray.offset);
    }
    return (bodies[index].center.xyz - bodies[ray.anchor].center.xyz)
        + ((bodies[index].low.xyz - bodies[ray.anchor].low.xyz) - ray.offset);
}

// Cap primary-ray stars in the actual tangent projection. An angular cap
// grows towards the edges of a wide view; a world-space ray comparison also
// loses subpixel jitter at deep zoom. Projecting the star once avoids both.
fn inside_star_dot(view_direction: vec3<f32>, star_direction: vec3<f32>) -> bool {
    let star_view = vec3<f32>(dot(star_direction, settings.g.cam_right.xyz),
        dot(star_direction, settings.g.cam_up.xyz), -dot(star_direction, settings.g.cam_forward.xyz));
    if (star_view.z >= 0.0) {
        return false;
    }
    let delta = view_direction.xy / -view_direction.z - star_view.xy / -star_view.z;
    let pixel_scale = settings.g.viewport.y / (2.0 * settings.g.cam_forward.w);
    let pixels = delta * pixel_scale;
    return dot(pixels, pixels) <= STAR_DOT_PIXELS * STAR_DOT_PIXELS;
}

fn sphere_hit(ray: Ray, index: u32) -> Hit {
    return sphere_hit_radius(ray, index, bodies[index].center.w);
}

fn sphere_hit_radius(ray: Ray, index: u32, radius: f32) -> Hit {
    let oc = relative_center(ray, index);
    // A normalized f32 direction is not exactly unit length. Retain the
    // quadratic coefficient, especially for a camera metres above a planet.
    let a = dot(ray.direction, ray.direction);
    let b = dot(oc, ray.direction);
    let perpendicular_cross = cross(oc, ray.direction);
    // Cross products preserve the discriminant for tiny, distant bodies.
    let h = a * radius * radius - dot(perpendicular_cross, perpendicular_cross);
    if (h < 0.0) {
        return Hit(1.0e30, MISS, vec3<f32>(0.0));
    }
    let root = sqrt(h);
    // Compute one root without subtraction, then use the product of roots
    // for the other. b + sqrt(h) catastrophically cancels for outward rays
    // near a surface: a negative root can turn positive, painting false
    // ground hits as concentric rings across the sky.
    let q = b + select(-root, root, b >= 0.0);
    if (q == 0.0) {
        return Hit(1.0e30, MISS, vec3<f32>(0.0));
    }
    let c = dot(oc, oc) - radius * radius;
    let near = select(c / q, q / a, b < 0.0);
    let far = select(q / a, c / q, b < 0.0);
    var distance = near;
    var delta = -root / a;
    if (distance <= 0.0) {
        distance = far;
        delta = root / a;
    }
    if (distance <= 0.0) {
        return Hit(1.0e30, MISS, vec3<f32>(0.0));
    }
    // Keep the normal body-local: distance*direction-oc loses small, distant
    // surfaces to cancellation even though the intersection distance is sound.
    let perpendicular = cross(ray.direction, perpendicular_cross) / a;
    return Hit(distance, index, normalize(-perpendicular + ray.direction * delta));
}

fn closest_hit(ray: Ray) -> Hit {
    var closest = Hit(1.0e30, MISS, vec3<f32>(0.0));
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        if (settings.g.ground_north.w == f32(i + 1u)) { continue; }
        let hit = sphere_hit(ray, i);
        if (hit.distance < closest.distance) {
            closest = hit;
        }
    }
    let local = landscape_hit(ray);
    if (local.distance < closest.distance) { closest = local; }
    return closest;
}

fn basis_direction(axis: vec3<f32>, x: f32, y: f32, z: f32) -> vec3<f32> {
    var helper = vec3<f32>(0.0, 0.0, 1.0);
    if (abs(axis.z) > 0.9) {
        helper = vec3<f32>(0.0, 1.0, 0.0);
    }
    let tangent = normalize(cross(helper, axis));
    return normalize(tangent * x + cross(axis, tangent) * y + axis * z);
}

// 1-cos(theta_max), evaluated without subtracting nearly equal numbers.
fn cone_width(ray: Ray, index: u32) -> f32 {
    let oc = relative_center(ray, index);
    let r = bodies[index].center.w;
    let sin2 = clamp(r * r / dot(oc, oc), 1.0e-20, 1.0);
    return sin2 / (1.0 + sqrt(1.0 - sin2));
}
// Most systems have one or two suns. Avoid scanning every non-emissive body
// inside each atmospheric integration step; larger custom systems remain supported.
fn light_count() -> u32 {
    return select(settings.counts.x, settings.lights.w, settings.lights.w <= 3u);
}
fn light_index(slot: u32) -> u32 {
    if (settings.lights.w <= 3u) { return settings.lights[slot]; }
    return slot;
}
fn sample_light(ray: Ray, index: u32, rng: ptr<function, u32>) -> LightSample {
    let axis = normalize(relative_center(ray, index));
    let width = cone_width(ray, index);
    let one_minus_cos = random(rng) * width;
    // Also avoid cancellation in 1-cos(theta)^2 for small stellar discs.
    let sin_theta = sqrt(one_minus_cos * (2.0 - one_minus_cos));
    let phi = TAU * random(rng);
    return LightSample(
        basis_direction(axis, sin_theta * cos(phi), sin_theta * sin(phi), 1.0 - one_minus_cos),
        1.0 / (TAU * width),
    );
}
fn power_weight(a: f32, b: f32) -> f32 {
    let ratio = b / max(a, 1.0e-30);
    return 1.0 / (1.0 + ratio * ratio);
}

fn world_direction(direction: vec3<f32>) -> vec3<f32> {
    return normalize(settings.g.cam_right.xyz * direction.x
        + settings.g.cam_up.xyz * direction.y - settings.g.cam_forward.xyz * direction.z);
}

fn environment(view_direction: vec3<f32>, primary: bool) -> vec3<f32> {
    // The procedural catalogue is a visual backdrop, not calibrated stellar
    // illumination. Random diffuse hits on its tiny bright discs produce
    // fireflies on otherwise unlit ground, amplified by automatic exposure.
    // Finite scene stars and reflected light still use full path transport.
    if (!primary) {
        return vec3<f32>(0.0);
    }
    let direction = world_direction(view_direction);
    let phi = atan2(direction.y, direction.x) + PI;
    let theta = acos(clamp(direction.z, -1.0, 1.0));
    let x = min(u32(phi * (256.0 / TAU)), 255u);
    let y = min(u32(theta * (128.0 / PI)), 127u);
    let cell = y * 256u + x;
    var light = vec3<f32>(0.0);
    for (var j = sky_cells[cell]; j < sky_cells[cell + 1u]; j += 1u) {
        let star = catalogue[sky_cells[32769u + j]];
        let perpendicular = cross(direction, star.direction.xyz);
        if (dot(direction, star.direction.xyz) > 0.0
            && dot(perpendicular, perpendicular) <= star.direction.w) {
            if (inside_star_dot(view_direction, star.direction.xyz)) {
                light += star.radiance.rgb;
            }
        }
    }
    return light;
}

fn milky_way_texel(pixel: vec2<i32>) -> vec3<f32> {
    let width = i32(settings.background.y);
    let height = i32(settings.background.z);
    let x = (pixel.x + width) % width;
    let y = clamp(pixel.y, 0, height - 1);
    let index = u32(y * width + x);
    let packed = milky_way_map[index];
    return vec3<f32>(f32(packed & 1023u), f32((packed >> 10u) & 1023u),
        f32((packed >> 20u) & 1023u)) / 1023.0;
}

fn milky_way(view_direction: vec3<f32>) -> vec3<f32> {
    if (settings.background.x == 0.0) { return vec3<f32>(0.0); }
    let direction = world_direction(view_direction);
    // Match the procedural catalogue's galactic plane and its Duff basis.
    // This is an illustrative orientation in our fictional systems, not an
    // Earth equatorial-coordinate model.
    let pole = normalize(vec3<f32>(0.35, 0.82, 0.45));
    let a = -1.0 / (1.0 + pole.z);
    let right = vec3<f32>(pole.x * pole.y * a, 1.0 + pole.y * pole.y * a, -pole.y);
    let forward = cross(pole, right);
    let longitude = atan2(dot(direction, forward), dot(direction, right));
    let uv = vec2<f32>(0.5 - longitude / TAU,
        acos(clamp(dot(direction, pole), -1.0, 1.0)) / PI);
    let pixel = uv * settings.background.yz - vec2<f32>(0.5);
    let lo = vec2<i32>(floor(pixel));
    let weight = fract(pixel);
    let luminance = mix(
        mix(milky_way_texel(lo), milky_way_texel(lo + vec2<i32>(1, 0)), weight.x),
        mix(milky_way_texel(lo + vec2<i32>(0, 1)), milky_way_texel(lo + vec2<i32>(1, 1)), weight.x),
        weight.y);
    return luminance * settings.background.x;
}

// Retain the original procedural surface material. This modulates reflectance,
// not illumination; it cannot brighten an unlit surface by itself.
fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}
fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(hash31(i), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let b = mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let c = mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let d = mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(a, b, u.y), mix(c, d, u.y), u.z);
}
// Fractal value noise. Each octave is rotated (and doubled in frequency) so
// the lattice axes of the value noise do not line up between octaves.
fn fbm(p_in: vec3<f32>, octaves: u32) -> f32 {
    let rotate = mat3x3<f32>(
        vec3<f32>(0.00, 1.60, 1.20),
        vec3<f32>(-1.60, 0.72, -0.96),
        vec3<f32>(-1.20, -0.96, 1.28));
    var p = p_in;
    var amplitude = 0.5;
    var sum = 0.0;
    var norm = 0.0;
    for (var octave = 0u; octave < octaves; octave += 1u) {
        sum += amplitude * vnoise(p);
        norm += amplitude;
        p = rotate * p + vec3<f32>(17.3, 5.1, 11.7);
        amplitude *= 0.5;
    }
    return sum / norm;
}

// Surface normal in the body's own rotating frame (Z = spin pole), so
// markings turn with the body and differ in orientation between bodies.
fn body_local(body: Body, normal: vec3<f32>) -> vec3<f32> {
    let x = body.axis_x.xyz;
    let y = body.axis_y.xyz;
    return vec3<f32>(dot(normal, x), dot(normal, y), dot(normal, cross(x, y)));
}

// A distinct, stable noise domain for each seed.
fn seed_offset(seed: f32) -> vec3<f32> {
    return vec3<f32>(
        hash31(vec3<f32>(seed, 1.0, 7.0)),
        hash31(vec3<f32>(3.0, seed, 5.0)),
        hash31(vec3<f32>(9.0, 2.0, seed))) * 512.0;
}

// Gas giant: alternating bright zones (the body's albedo) and darker belts
// along latitude, their edges warped by elongated turbulence, fine streaks,
// hazy poles and up to three oval storms that the bands swirl around.
fn gas_giant_albedo(body: Body, local: vec3<f32>) -> vec3<f32> {
    let off = seed_offset(body.axis_y.w);
    let zone = body.material.rgb;
    let belt = body.surface_color.rgb;
    let turbulence = body.surface_color.w;
    let bands = body.surface_params.x;
    let storms = u32(body.surface_params.y + 0.5);
    var q = local;
    var masks = vec3<f32>(0.0);
    var rims = vec3<f32>(0.0);
    for (var k = 0u; k < storms && k < 3u; k += 1u) {
        let fk = f32(k);
        let lat = (hash31(off + vec3<f32>(7.1 * fk, 1.3, 2.9)) - 0.5) * 1.1;
        let lon = hash31(off + vec3<f32>(3.7, 5.3 * fk, 8.1)) * TAU;
        let size = select(0.05, 0.11, k == 0u) * (0.8 + 0.4 * hash31(off + vec3<f32>(fk, 9.0, 4.0)));
        let centre = vec3<f32>(cos(lat) * cos(lon), cos(lat) * sin(lon), sin(lat));
        if (dot(q, centre) <= 0.0) {
            continue;
        }
        let east = vec3<f32>(-sin(lon), cos(lon), 0.0);
        let north = cross(centre, east);
        // Ovals are twice as wide (east-west) as they are tall.
        let dx = dot(q - centre, east) / (2.0 * size);
        let dy = dot(q - centre, north) / size;
        let e = length(vec2<f32>(dx, dy));
        // Swirl the band lookup around the storm; anticyclones turn opposite
        // ways in the two hemispheres.
        let influence = 1.0 - smoothstep(0.0, 2.4, e);
        let angle = select(-3.2, 3.2, lat < 0.0) * influence * influence;
        let c = cos(angle);
        let s = sin(angle);
        let rx = c * dx - s * dy;
        let ry = s * dx + c * dy;
        q = normalize(q + east * ((rx - dx) * 2.0 * size) + north * ((ry - dy) * size));
        let ragged = e + 0.25 * (fbm(q * 18.0 + off, 3) - 0.5);
        masks[k] = 1.0 - smoothstep(0.7, 1.0, ragged);
        rims[k] = smoothstep(0.75, 1.0, ragged) * (1.0 - smoothstep(1.0, 1.35, ragged));
    }
    // Noise stretched east-west (z scaled up) gives zonal streaks. Domain
    // warping is strongest at band edges, where the zonal winds shear.
    let p = vec3<f32>(q.x * 2.2, q.y * 2.2, q.z * 9.0) + off;
    let warp1 = fbm(p, 5) - 0.5;
    let warp2 = fbm(p * 2.1 + vec3<f32>(5.2, 1.3, 7.7) + 4.0 * warp1, 5) - 0.5;
    // Unequal band widths: a smooth remapping of latitude per seed.
    let lat = q.z + 0.07 * sin(3.0 * PI * q.z + off.z) + 0.04 * sin(5.0 * PI * q.z + off.y);
    let shear = 0.35 + 0.65 * abs(cos(PI * bands * lat + off.x));
    // Waves travelling along the band edges, plus the turbulent warp.
    let lon = atan2(q.y, q.x);
    let ripple = sin(lon * (9.0 + floor(bands)) + 6.0 * warp1 + off.y) * 0.5
        + sin(lon * 23.0 + 9.0 * warp2 + off.z) * 0.25;
    let mu = lat + turbulence * shear * (0.16 * warp1 + 0.10 * warp2 + 0.025 * ripple);
    // Irregular band widths: a regular wave plus 1D noise along latitude.
    let wave = 0.5 + 0.5 * sin(PI * bands * mu + off.x);
    let irregular = fbm(vec3<f32>(mu * bands * 1.7, off.y, off.z), 3);
    let profile = 0.5 * wave + 0.5 * irregular;
    // Each band also gets its own tone, from creamy to deep orange-brown.
    let tone = fbm(vec3<f32>(mu * bands * 0.9, off.z, off.x), 2);
    let fine = fbm(vec3<f32>(q.x * 7.0, q.y * 7.0, q.z * 70.0) + off + 3.0 * warp2, 4);
    let eddies = fbm(p * 5.0 + 6.0 * vec3<f32>(warp1, warp2, warp1), 4);
    var color = mix(belt, zone, smoothstep(0.3, 0.75, profile));
    // Deeper tones keep the body's hue but saturate it (dark channels drop most).
    color = mix(color, color * (0.55 + 0.5 * color), smoothstep(0.35, 0.75, tone) * 0.8);
    color *= (0.78 + 0.44 * fine) * (0.9 + 0.2 * eddies);
    // Polar regions lose band contrast under a duller haze.
    let polar = smoothstep(0.55, 0.92, abs(local.z));
    color = mix(color, mix(zone, belt, 0.55) * (0.8 + 0.2 * fine), polar * 0.8);
    for (var k = 0u; k < storms && k < 3u; k += 1u) {
        // The first storm is the large, deeply coloured one (red on a tan giant,
        // dark blue on a blue one); the others are white ovals.
        let tint = select(zone * 1.15, belt * belt * 1.5, k == 0u);
        // Inside, the storm's own spiral texture and a darker core.
        let core = 1.0 - 0.25 * (1.0 - smoothstep(0.0, 0.6, masks[k] - rims[k]));
        color = mix(color, tint * (0.7 + 0.6 * fine) * core, masks[k] * 0.92);
        color = mix(color, zone * 1.1, rims[k] * 0.45);
    }
    return color;
}

// --- Solid-body surfaces ------------------------------------------------

// Signed distances occur on both sides of a rim or latitude band. WGSL pow
// has no defined result for a negative base, even with exponent 2.0; square
// directly so these profiles stay finite on every GPU/compiler path.
fn surface_gaussian(distance: f32) -> f32 {
    return exp(-(distance * distance));
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(hash31(p), hash31(p + vec3<f32>(19.1, 7.3, 3.7)),
        hash31(p + vec3<f32>(5.9, 31.7, 11.3)));
}

// Ridged noise: thin bright lines where the noise crosses 0.5.
fn ridge(n: f32, width: f32) -> f32 {
    return 1.0 - smoothstep(0.0, width, abs(n - 0.5));
}

// Lunar maria: smooth, dark lava plains (0-1).
fn maria(q: vec3<f32>, off: vec3<f32>) -> f32 {
    let warp = fbm(q * 2.0 + off, 4) - 0.5;
    return smoothstep(0.5, 0.56, fbm(q * 1.2 + off + 1.5 * warp, 5));
}

// Centre of the crater (or vent) owned by `cell`, on the unit sphere. A
// jittered point is projected onto the sphere; the projection must stay in
// the cell, otherwise a neighbour search around a point could miss it and
// cut it off. Three jitters are tried. w = 0 when the cell has none.
// `key` seeds the jitter (it differs between craters sharing a cell).
fn cell_centre(cell: vec3<f32>, key: vec3<f32>, scale: f32, off: vec3<f32>) -> vec4<f32> {
    for (var t = 0u; t < 3u; t += 1u) {
        let centre = normalize(cell + hash33(key + vec3<f32>(f32(t) * 17.0)) - off);
        if (all(floor(centre * scale + off) == cell)) {
            return vec4<f32>(centre, 1.0);
        }
    }
    return vec4<f32>(0.0);
}

// One octave of craters on the unit sphere. A 3D cell grid scattered over
// the sphere holds at most two craters per cell, centred on the sphere and
// inside their cell. A crater's influence (ejecta, rays) stays within half a
// cell of its centre, so only the 2x2x2 cells nearest the point can reach it.
//
// The state carried through the layers is x = floor darkness (multiplier)
// and y = relief brightness (rims, ejecta, rays; additive). Craters are
// painted in a fixed age order, oldest first, like real impacts: inside its
// rim a younger crater replaces the rims and rays beneath it, and its ejecta
// blanket partly buries them around it, so overlapping rims never cross.
// Floors persist: a crater inside a basin stays dark, and overlapping floors
// of one layer darken only once. Every choice about a crater (existence,
// size, age, flooding) depends on the crater alone, never on the shaded
// point, so no crater is ever cut off.
// `flood` > 0: old craters centred in lunar maria were buried by the lava.
fn crater_layer(q: vec3<f32>, scale: f32, density: f32, off: vec3<f32>,
    body_off: vec3<f32>, flood: f32, contrast: f32, rays: f32,
    state: vec2<f32>) -> vec2<f32> {
    let p = q * scale + off;
    let base = floor(p - 0.5);
    // Gather the craters that reach this point, then sort them by age. Each
    // cell holds up to two craters (slots); `key` is a per-crater value,
    // offset by the slot, from which all its properties are hashed. Only
    // the slot index and age are stored, to keep the arrays small.
    // At most six craters reach a point in practice; if more do, the
    // oldest (painted first, then mostly covered) are dropped.
    var slots: array<u32, 6>;
    var ages: array<f32, 6>;
    var centres: array<vec3<f32>, 6>;
    var count = 0u;
    for (var i = 0u; i < 16u; i += 1u) {
        let grid = base + vec3<f32>(f32(i & 1u), f32((i >> 1u) & 1u), f32((i >> 2u) & 1u));
        // Skip cells too far from the point for any of their craters to
        // reach it (largest reach: 2.2 x 0.22 cells).
        let gap = max(max(grid - p, p - grid - 1.0), vec3<f32>(0.0));
        if (dot(gap, gap) > 0.2343) {
            continue;
        }
        let key = grid + vec3<f32>(f32(i >> 3u) * 1000.0);
        if (hash31(key + vec3<f32>(71.3, 3.1, 9.7)) > density) {
            continue;
        }
        let found = cell_centre(grid, key, scale, off);
        if (found.w == 0.0) {
            continue;
        }
        let size = hash31(key + vec3<f32>(3.1, 8.3, 1.7));
        let radius = (0.06 + 0.16 * size * size) / scale;
        if (length(q - found.xyz) > 2.2 * radius) {
            continue;
        }
        let age = hash31(key + vec3<f32>(29.0, 61.0, 7.0));
        if (flood > 0.0 && age < 0.7 && maria(found.xyz, body_off) > 0.5) {
            continue;
        }
        // Insertion sort: ages ascending (larger value = younger crater).
        if (count == 6u) {
            if (age <= ages[0]) {
                continue;
            }
            // Full: drop the oldest to make room.
            for (var m = 0u; m < 5u; m += 1u) {
                ages[m] = ages[m + 1u];
                slots[m] = slots[m + 1u];
                centres[m] = centres[m + 1u];
            }
            count = 5u;
        }
        var j = count;
        while (j > 0u && ages[j - 1u] > age) {
            ages[j] = ages[j - 1u];
            slots[j] = slots[j - 1u];
            centres[j] = centres[j - 1u];
            j -= 1u;
        }
        ages[j] = age;
        slots[j] = i;
        centres[j] = found.xyz;
        count += 1u;
    }
    var floor_shade = 1.0;
    var relief = state.y;
    for (var k = 0u; k < count; k += 1u) {
        let i = slots[k];
        let grid = base + vec3<f32>(f32(i & 1u), f32((i >> 1u) & 1u), f32((i >> 2u) & 1u));
        let cell = grid + vec3<f32>(f32(i >> 3u) * 1000.0);
        let centre = centres[k];
        // Size distribution: many small craters, few large ones.
        let size = hash31(cell + vec3<f32>(3.1, 8.3, 1.7));
        let radius = (0.06 + 0.16 * size * size) / scale;
        let offset = q - centre;
        let d = length(offset) / radius;
        // Local frame around the crater, for its ragged rim and its rays.
        var east = cross(vec3<f32>(0.0, 0.0, 1.0), centre);
        east = east / max(length(east), 1.0e-4);
        let north = cross(centre, east);
        let angle = atan2(dot(offset, north), dot(offset, east));
        // Ragged outline: a few low harmonics with per-crater phases.
        let phase = hash33(cell + vec3<f32>(41.0, 5.0, 23.0)) * TAU;
        let wobble = 0.025 * (sin(3.0 * angle + phase.x) + sin(5.0 * angle + phase.y)
            + 0.6 * sin(8.0 * angle + phase.z));
        let r = d + wobble;
        floor_shade = min(floor_shade, 1.0 - 0.18 * contrast * (1.0 - smoothstep(0.55, 0.9, r)));
        // Inside the footprint the crater's own rim replaces older relief.
        let rim = 0.22 * contrast * surface_gaussian((r - 1.0) / 0.13);
        let footprint = 1.0 - smoothstep(1.1, 1.3, r);
        relief = mix(relief, rim, footprint);
        // Outside: a bright ejecta blanket that thins outwards and partly
        // buries older relief.
        let blanket = (1.0 - smoothstep(1.0, 1.9, r)) * (1.0 - footprint);
        relief = mix(relief, 0.0, blanket * 0.3) + 0.1 * contrast * blanket;
        // A few fresh craters throw long bright rays and have bright floors.
        let fresh = hash31(cell + vec3<f32>(13.0, 2.0, 5.0));
        if (rays > 0.0 && fresh > 0.82) {
            let streaks = pow(vnoise(vec3<f32>(cos(angle) * 9.0, sin(angle) * 9.0, fresh * 40.0)), 3.0);
            relief += contrast * (rays * 0.9 * streaks * (1.0 - smoothstep(1.0, 2.2, d)) * step(0.9, d)
                + rays * 0.25 * (1.0 - smoothstep(0.0, 1.0, d)));
        }
    }
    return vec2<f32>(state.x * floor_shade, relief);
}

// Five octaves of craters, painted from large to small: big basins are the
// oldest, so smaller craters sit on their rims and floors, never under them.
fn craters(q: vec3<f32>, density: f32, off: vec3<f32>, flood: f32, contrast: f32, rays: f32) -> f32 {
    if (density <= 0.0) {
        return 1.0;
    }
    var state = crater_layer(q, 1.5, density, off, off, flood, contrast, rays, vec2<f32>(1.0, 0.0));
    state = crater_layer(q, 3.5, density, off + 101.0, off, flood, contrast, rays * 0.8, state);
    state = crater_layer(q, 8.0, density, off + 203.0, off, flood, contrast * 0.85, rays * 0.5, state);
    // Small craters are younger than the lava plains: no flooding (cheaper too).
    state = crater_layer(q, 18.0, density, off + 307.0, off, 0.0, contrast * 0.7, 0.0, state);
    state = crater_layer(q, 40.0, density, off + 401.0, off, 0.0, contrast * 0.55, 0.0, state);
    return state.x * (1.0 + state.y);
}

// Polar ice: caps reach down to |sin(latitude)| = 1 - extent, ragged edges.
fn polar_caps(q: vec3<f32>, extent: f32, off: vec3<f32>) -> f32 {
    if (extent <= 0.0) {
        return 0.0;
    }
    let edge = 1.0 - extent + 0.12 * (fbm(q * 5.0 + off, 5) - 0.5) + 0.05 * (fbm(q * 18.0 + off, 3) - 0.5);
    return smoothstep(edge - 0.015, edge + 0.015, abs(q.z));
}

// Moon/Mercury: highland albedo, smooth dark maria with fewer craters,
// layered craters and fine regolith grain.
fn cratered_albedo(body: Body, q: vec3<f32>) -> vec3<f32> {
    let off = seed_offset(body.axis_y.w);
    let mare = maria(q, off);
    let grain = fbm(q * 45.0 + off, 3);
    var color = mix(body.material.rgb, body.surface_color.rgb, mare);
    color *= 0.9 + 0.2 * grain;
    // Old craters centred in the maria were flooded; young ones remain.
    color *= craters(q, body.surface_color.w, off, 1.0, 1.0, 1.0);
    return color;
}

// Mars: ochre plains, large dark albedo regions, softened craters, dust
// streaks and polar caps.
fn rocky_albedo(body: Body, q: vec3<f32>) -> vec3<f32> {
    let off = seed_offset(body.axis_y.w);
    let warp = vec3<f32>(fbm(q * 1.8 + off, 4), fbm(q * 1.8 + off + 7.0, 4), 0.0) - 0.5;
    let dark = smoothstep(0.5, 0.64, fbm(q * 2.2 + off + vec3<f32>(2.0 * warp.x, 2.0 * warp.y, warp.x), 6));
    let tone = fbm(q * 5.0 + off + 3.0, 4);
    var color = mix(body.material.rgb, body.surface_color.rgb, dark * 0.85);
    color *= 0.85 + 0.3 * tone;
    // Bright dust streaks trailing downwind of craters and highlands.
    let streak = fbm(vec3<f32>(q.x * 4.0, q.y * 4.0, q.z * 18.0) + off + 9.0, 3);
    color *= 0.95 + 0.1 * streak;
    color *= craters(q, body.surface_color.w, off, 0.0, 0.45, 0.0);
    let ice = polar_caps(q, body.surface_params.x, off);
    return mix(color, vec3<f32>(0.92, 0.92, 0.94), ice);
}

// Europa: bright water ice with mottled, darker chaos regions and a web of
// reddish-brown cracks (lineae) at two scales; few craters.
fn icy_albedo(body: Body, q: vec3<f32>) -> vec3<f32> {
    let off = seed_offset(body.axis_y.w);
    let ice = body.material.rgb;
    let rust = body.surface_color.rgb;
    let mottle = fbm(q * 3.0 + off, 5);
    let chaos = smoothstep(0.55, 0.7, fbm(q * 2.5 + off + 5.0, 5));
    var color = mix(ice, mix(ice, rust, 0.55), chaos) * (0.92 + 0.16 * mottle);
    // Lineae: long arcs of great (and some small) circles, gently warped,
    // each fading in and out along its length so families of cracks cross.
    let bend = vec3<f32>(fbm(q * 2.0 + off + 11.0, 3), fbm(q * 2.0 + off + 23.0, 3), fbm(q * 2.0 + off + 37.0, 3)) - 0.5;
    var lines = 0.0;
    var margin = 0.0;
    for (var i = 0u; i < 16u; i += 1u) {
        let h = hash33(off + vec3<f32>(f32(i) * 3.7, 1.0, 2.0));
        let n = normalize(h * 2.0 - 1.0);
        // Small circles too: offset the plane from the centre.
        let shift = (hash31(off + vec3<f32>(f32(i), 9.0, 4.0)) - 0.5) * 0.8;
        let dist = abs(dot(q + 0.05 * bend, n) - shift);
        let width = 0.003 + 0.006 * hash31(off + vec3<f32>(5.0, f32(i), 7.0));
        let along = smoothstep(0.35, 0.55, vnoise(q * 2.5 + h * 50.0));
        lines = max(lines, (1.0 - smoothstep(width * 0.4, width, dist)) * along);
        margin = max(margin, (1.0 - smoothstep(width, width * 4.0, dist)) * along * 0.45);
    }
    // Fine background ridges (the ridged plains between the major bands).
    let fine = ridge(fbm(q * 14.0 + off + 83.0, 2), 0.02) * 0.3;
    color = mix(color, rust, max(max(lines, margin), fine) * 0.9);
    color *= craters(q, body.surface_color.w, off, 0.0, 0.5, 0.6);
    return color;
}

// Io: yellow sulphur plains, orange and white frost patches, reddish poles
// and dark volcanic vents ringed by red plume deposits.
fn volcanic_albedo(body: Body, q: vec3<f32>) -> vec3<f32> {
    let off = seed_offset(body.axis_y.w);
    let sulphur = body.material.rgb;
    let deposit = body.surface_color.rgb;
    let warp = fbm(q * 2.0 + off, 4) - 0.5;
    let patches = fbm(q * 3.0 + off + 2.0 * warp, 5);
    var color = mix(sulphur, deposit, smoothstep(0.45, 0.7, patches));
    let frost = smoothstep(0.62, 0.72, fbm(q * 4.0 + off + 19.0, 4));
    color = mix(color, vec3<f32>(0.92, 0.9, 0.82), frost * 0.7);
    color *= 0.88 + 0.24 * fbm(q * 20.0 + off, 3);
    color = mix(color, deposit * 0.75, smoothstep(0.6, 0.9, abs(q.z)) * 0.6);
    // Vents: a sparse cell field with a black core and a red plume ring.
    let density = body.surface_color.w;
    let scale = 5.0;
    let p = q * scale + off + 300.0;
    let base = floor(p);
    for (var i = 0; i < 27; i += 1) {
        let cell = base + vec3<f32>(f32(i % 3 - 1), f32((i / 3) % 3 - 1), f32(i / 9 - 1));
        if (hash31(cell + vec3<f32>(4.0, 41.0, 9.0)) > 0.35 * density) {
            continue;
        }
        let found = cell_centre(cell, cell, scale, off + 300.0);
        if (found.w == 0.0) {
            continue;
        }
        let centre = found.xyz;
        let size = hash31(cell + vec3<f32>(8.0, 1.0, 3.0));
        let radius = (0.03 + 0.08 * size * size) / scale;
        let offset = q - centre;
        let d = length(offset) / radius + 0.35 * (vnoise(offset / radius * 2.0 + cell) - 0.5);
        // Only some vents have a large plume ring; their radius varies.
        let plume = hash31(cell + vec3<f32>(2.0, 6.0, 17.0));
        let ring_radius = 2.0 + 4.0 * plume;
        let ring = surface_gaussian((d - ring_radius) / (0.25 * ring_radius)) * step(0.7, plume);
        color = mix(color, vec3<f32>(0.62, 0.22, 0.1), ring * 0.65);
        // Smaller vents leave a diffuse dark-orange stain instead.
        color = mix(color, deposit * 0.6, (1.0 - smoothstep(0.5, 2.0, d)) * 0.5);
        color = mix(color, vec3<f32>(0.05, 0.04, 0.035), 1.0 - smoothstep(0.35, 0.6, d));
        // Lava flows: dark lobes running out from the vent.
        let flow = smoothstep(0.55, 0.75, vnoise(offset / radius * 1.2 + cell * 3.0))
            * (1.0 - smoothstep(0.6, 2.5, d));
        color = mix(color, vec3<f32>(0.12, 0.08, 0.06), flow * 0.8);
    }
    return color;
}

// Earth-like: oceans with lighter shallows, continents whose colour varies
// with moisture and altitude (green, desert, rock, snow), polar ice and
// swirling clouds.
fn terran_albedo(body: Body, q: vec3<f32>) -> vec3<f32> {
    let off = seed_offset(body.axis_y.w);
    let land_base = body.material.rgb;
    let ocean_color = body.surface_color.rgb;
    let warp = vec3<f32>(fbm(q * 1.5 + off, 4), fbm(q * 1.5 + off + 13.0, 4), fbm(q * 1.5 + off + 29.0, 4)) - 0.5;
    let height = fbm(q * 1.1 + off + 0.9 * warp, 7);
    // fbm is roughly centred on 0.5 with a spread of about 0.1.
    let sea = 0.5 + (body.surface_params.y - 0.5) * 0.3;
    let coast = smoothstep(sea, sea + 0.004, height);
    let shallow = 1.0 - smoothstep(sea - 0.04, sea, height);
    let water = mix(ocean_color * 1.8 + vec3<f32>(0.0, 0.04, 0.05), ocean_color, shallow);
    // Land: moist (vegetated, darker green) vs dry (the albedo, desert).
    let moisture = fbm(q * 3.0 + off + 41.0, 5) + 0.25 * (abs(q.z) - 0.35);
    let green = land_base * vec3<f32>(0.45, 0.62, 0.38);
    var land = mix(land_base, green, smoothstep(0.45, 0.6, moisture));
    let altitude = smoothstep(sea + 0.08, sea + 0.2, height);
    land = mix(land, vec3<f32>(0.42, 0.38, 0.34), altitude * 0.6);
    land = mix(land, vec3<f32>(0.9, 0.9, 0.92), smoothstep(sea + 0.19, sea + 0.24, height));
    land *= 0.88 + 0.24 * fbm(q * 25.0 + off, 3);
    var color = mix(water, land, coast);
    let ice = polar_caps(q, body.surface_params.x, off);
    color = mix(color, vec3<f32>(0.9, 0.92, 0.95), ice);
    // Clouds: domain-warped, streaky along latitude, with spiral cyclones.
    let cover = body.surface_params.z;
    if (cover > 0.0) {
        // Two levels of domain warping, stretched east-west, turn the noise
        // into streaks and curls; a soft remap keeps thin veils.
        let w1 = vec3<f32>(fbm(q * 1.6 + off + 77.0, 4), fbm(q * 1.6 + off + 91.0, 4), 0.5) - 0.5;
        let w2 = vec3<f32>(fbm(q * 4.0 + off + 3.0 * w1, 4), fbm(q * 4.0 + off + 17.0 + 3.0 * w1, 4), 0.5) - 0.5;
        let cq = vec3<f32>(q.x * 2.5, q.y * 2.5, q.z * 7.0) + off + 2.5 * w1 + 1.2 * w2;
        let cloud = fbm(cq, 7);
        // More cloud in the equatorial and mid-latitude storm belts, less
        // over the subtropics.
        let lat = abs(q.z);
        let belts = 0.06 * (surface_gaussian(lat / 0.12) + surface_gaussian((lat - 0.72) / 0.15))
            - 0.04 * surface_gaussian((lat - 0.42) / 0.1);
        let threshold = 0.62 - 0.22 * cover - belts;
        let mask = smoothstep(threshold - 0.04, threshold + 0.14, cloud);
        color = mix(color, vec3<f32>(0.93, 0.94, 0.96), mask * 0.95);
    }
    return color;
}

fn surface_albedo(body: Body, normal: vec3<f32>) -> vec3<f32> {
    let local = body_local(body, normal);
    let style = u32(body.axis_x.w + 0.5);
    if (style > 0u) {
        var color: vec3<f32>;
        switch style {
            case 1u: { color = gas_giant_albedo(body, local); }
            case 2u: { color = cratered_albedo(body, local); }
            case 3u: { color = rocky_albedo(body, local); }
            case 4u: { color = icy_albedo(body, local); }
            case 5u: { color = volcanic_albedo(body, local); }
            default: { color = terran_albedo(body, local); }
        }
        return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    var mottle = 1.0;
    if (body.low.w > 0.0) {
        var p = local * 2.4 + seed_offset(body.axis_y.w);
        var amplitude = 0.5;
        var noise = 0.0;
        for (var octave = 0u; octave < 5u; octave += 1u) {
            noise += amplitude * vnoise(p);
            p *= 2.07;
            amplitude *= 0.5;
        }
        mottle = mix(1.0, 0.68 + 0.64 * noise, body.low.w);
    }
    return clamp(body.material.rgb * mottle, vec3<f32>(0.0), vec3<f32>(1.0));
}

// --- Host atmosphere: single-scattering Rayleigh + Mie --------------------

struct Scatter {
    transmittance: vec3<f32>,
    inscatter: vec3<f32>,
};

fn host_radius() -> f32 {
    return settings.g.atmo_center.w;
}
fn atmosphere_top() -> f32 {
    return settings.g.atmo_center.w + settings.g.atmo_params.z;
}
fn atmo_scale_height() -> f32 {
    return settings.g.atmo_params.y;
}

// Distance from a point inside the shell to the top of the atmosphere along
// `dir`, or 0 if the ray never reaches the shell.
fn shell_exit(origin: vec3<f32>, dir: vec3<f32>) -> f32 {
    let oc = origin - settings.g.atmo_center.xyz;
    let b = dot(oc, dir);
    let c = dot(oc, oc) - atmosphere_top() * atmosphere_top();
    let disc = b * b - c;
    if (disc < 0.0) {
        return 0.0;
    }
    return -b + sqrt(disc);
}

// Column density integral of exp(-h/H) along `dir`, in AU.
fn density_integral(origin: vec3<f32>, dir: vec3<f32>, steps: u32) -> f32 {
    let distance = shell_exit(origin, dir);
    if (distance <= 0.0) {
        return 0.0;
    }
    let dt = distance / f32(steps);
    let inv_h = 1.0 / atmo_scale_height();
    let r = host_radius();
    var sum = 0.0;
    for (var i = 0u; i < steps; i += 1u) {
        let p = origin + dir * ((f32(i) + 0.5) * dt);
        let h = max(length(p - settings.g.atmo_center.xyz) - r, 0.0);
        sum += exp(-h * inv_h);
    }
    return sum * dt;
}

// A ray origin relative to the host centre. Anchored origins subtract the
// nearly equal high parts first, as the body is usually the host itself.
fn host_offset(origin: Ray) -> vec3<f32> {
    if (origin.anchor == MISS) {
        return origin.offset - settings.g.atmo_center.xyz;
    }
    let body = bodies[origin.anchor];
    return (body.center.xyz - settings.g.atmo_center.xyz) + body.low.xyz + origin.offset;
}

fn inside_air(origin: Ray) -> bool {
    if (settings.g.atmo_params.w < 0.5) {
        return false;
    }
    let oc = host_offset(origin);
    return dot(oc, oc) < atmosphere_top() * atmosphere_top();
}

// Extinction from a point to the top of the host atmosphere along `dir`, for
// light that reaches surfaces inside the shell. Points outside see no host air.
fn air_transmittance(origin: Ray, dir: vec3<f32>) -> vec3<f32> {
    if (!inside_air(origin)) {
        return vec3<f32>(1.0);
    }
    let beta_ext = settings.g.atmo_rayleigh.rgb + vec3<f32>(settings.g.atmo_rayleigh.w);
    let p = settings.g.atmo_center.xyz + host_offset(origin);
    return exp(-beta_ext * density_integral(p, dir, ATMO_SUN_STEPS));
}

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta);
}
fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5));
}

struct ReflectorSample {
    index: u32,
    inverse_probability: f32,
};

// Select one external reflecting sphere per camera path, weighted by its
// apparent solid angle and albedo. Reusing this choice along the air column
// bounds the extra shadow-ray work independently of the number of planets.
// The local ground and ring scattering are not atmospheric light sources.
fn sample_reflector(origin: vec3<f32>, rng: ptr<function, u32>) -> ReflectorSample {
    let ray = Ray(origin, vec3<f32>(0.0, 0.0, -1.0), MISS);
    let host = u32(settings.g.atmo_params.w) - 1u;
    var selected = MISS;
    var selected_weight = 0.0;
    var total = 0.0;
    for (var b = 0u; b < settings.counts.x; b += 1u) {
        let body = bodies[b];
        if (b == host || body.material.w > 0.5) {
            continue;
        }
        let weight = cone_width(ray, b) * max(body.material.r, max(body.material.g, body.material.b));
        if (weight <= 0.0) {
            continue;
        }
        total += weight;
        if (random(rng) * total < weight) {
            selected = b;
            selected_weight = weight;
        }
    }
    return ReflectorSample(selected, total / max(selected_weight, 1.0e-30));
}

// One stellar reflection at the sampled, observer-facing surface. The same
// finite-star sampling and ring/sphere shadows as surface transport resolve
// phases, eclipses, albedo and multiple suns without an independent glow gain.
fn reflected_radiance(ray: Ray, index: u32, rng: ptr<function, u32>) -> vec3<f32> {
    let hit = sphere_hit(ray, index);
    if (hit.index == MISS) {
        return vec3<f32>(0.0);
    }
    let body = bodies[index];
    let albedo = surface_albedo(body, hit.normal);
    var outgoing = Ray(hit.normal * (body.center.w * (1.0 + 2.0e-6)), hit.normal, index);
    var radiance = vec3<f32>(0.0);
    for (var light_slot = 0u; light_slot < light_count(); light_slot += 1u) {
        let s = light_index(light_slot);
        if (bodies[s].material.w < 0.5) {
            continue;
        }
        let light = sample_light(outgoing, s, rng);
        let cosine = max(dot(hit.normal, light.direction), 0.0);
        if (cosine > 0.0) {
            outgoing.direction = light.direction;
            radiance += albedo * bodies[s].material.rgb
                * (cosine * light_visibility(outgoing, s) / (PI * light.pdf));
        }
    }
    return radiance;
}

// Largest unoccluded irradiance (max channel) from a star above the
// observer's horizon, or 0 when every star has set.
fn brightest_risen_light(origin: vec3<f32>) -> f32 {
    let ray = Ray(origin, vec3<f32>(0.0, 0.0, -1.0), MISS);
    var brightest = 0.0;
    for (var light_slot = 0u; light_slot < light_count(); light_slot += 1u) {
        let b = light_index(light_slot);
        if (bodies[b].material.w < 0.5) { continue; }
        if (dot(normalize(relative_center(ray, b)), settings.g.ground_up.xyz) <= 0.0) { continue; }
        let radiance = bodies[b].material.rgb;
        brightest = max(brightest, max(radiance.r, max(radiance.g, radiance.b)) * TAU * cone_width(ray, b));
    }
    return brightest;
}

// Single scattering along a camera ray, up to `limit` (a surface hit) or the
// top of the shell. Returns the view transmittance and the in-scattered light
// accumulated in front of whatever the ray reaches.
fn atmosphere(origin: vec3<f32>, dir: vec3<f32>, limit: f32, rng: ptr<function, u32>) -> Scatter {
    var result = Scatter(vec3<f32>(1.0), vec3<f32>(0.0));
    if (settings.g.atmo_params.w < 0.5) {
        return result;
    }
    let exit = shell_exit(origin, dir);
    let t_max = min(limit, exit);
    if (t_max <= 0.0) {
        return result;
    }

    let beta_r = settings.g.atmo_rayleigh.rgb;
    let beta_m = settings.g.atmo_rayleigh.w;
    let beta_ext = beta_r + vec3<f32>(beta_m);
    let g = settings.g.atmo_params.x;
    let r = host_radius();
    // Short segments (camera rays that end on nearby terrain or props) cross
    // nearly uniform air: cap each step at a quarter scale height instead of
    // always paying for ATMO_VIEW_STEPS shadow rays. Long sky rays keep all.
    let steps = clamp(u32(ceil(t_max / (0.25 * atmo_scale_height()))), 2u, ATMO_VIEW_STEPS);
    let dt = t_max / f32(steps);
    // Jitter per camera ray wherever the air may hold terrain shadows: on
    // shortened segments, fixed midpoints miss thin shadowed air in front of
    // a slope; on any ray ending on the ground, a midpoint crossing a
    // mountain's shadow volume from one pixel row to the next makes bands in
    // the haze. Full sky rays keep the reviewed midpoint rule.
    var jitter_rng = hash(*rng ^ 0x3c6ef372u);
    let jitter = select(0.5, random(&jitter_rng), steps < ATMO_VIEW_STEPS || limit < exit);

    // Separate random stream keeps direct starlight sampling unchanged.
    var reflected_rng = hash(*rng ^ 0xa511e9b3u);
    let reflector = sample_reflector(origin, &reflected_rng);
    // Planetshine: sample one point on the reflector per camera ray and reuse
    // its outgoing radiance along the column. The reflector is far beyond the
    // column, so the direction barely changes; visibility (eclipses, horizon)
    // and extinction are still evaluated at every step. An unlit sample skips
    // the per-step shadow rays entirely.
    var reflected = vec3<f32>(0.0);
    var reflected_direction = dir;
    var reflected_phase = vec3<f32>(0.0);
    if (reflector.index != MISS) {
        var reflected_ray = Ray(origin, dir, MISS);
        let light = sample_light(reflected_ray, reflector.index, &reflected_rng);
        reflected_ray.direction = light.direction;
        reflected = reflected_radiance(reflected_ray, reflector.index, &reflected_rng)
            * (reflector.inverse_probability / light.pdf);
        reflected_direction = light.direction;
        let cos_theta = dot(dir, light.direction);
        reflected_phase = beta_r * rayleigh_phase(cos_theta)
            + vec3<f32>(beta_m) * mie_phase(cos_theta, g);
    }
    // In daylight, planetshine is a vanishing fraction of the sky (moonlight
    // is ~1e-6 of sunlight). Russian roulette keeps it unbiased while rarely
    // paying for its per-step shadow rays. At night or twilight (no sun above
    // the observer's horizon) it is always evaluated.
    if (any(reflected > vec3<f32>(0.0))) {
        let sun = brightest_risen_light(origin);
        if (sun > 0.0) {
            let ratio = max(reflected.r, max(reflected.g, reflected.b)) / sun;
            let keep = clamp(ratio * 1.0e3, 1.0 / 64.0, 1.0);
            reflected = select(vec3<f32>(0.0), reflected / keep, random(&reflected_rng) < keep);
        }
    }
    let planetshine = any(reflected > vec3<f32>(0.0));

    var t_view = vec3<f32>(1.0);
    var inscatter = vec3<f32>(0.0);
    for (var i = 0u; i < steps; i += 1u) {
        let p = origin + dir * ((f32(i) + jitter) * dt);
        let to_center = p - settings.g.atmo_center.xyz;
        let altitude = max(length(to_center) - r, 0.0);
        let density = exp(-altitude / atmo_scale_height());
        let column = density * dt;
        let tau = beta_ext * column;
        let step_t = exp(-tau);
        // Integrate view extinction within the segment, rather than using
        // its unattenuated near edge (which over-brightens thick haze).
        // The thin limit also avoids cancellation in 1-exp(-tau).
        let segment = select((vec3<f32>(1.0) - step_t) / max(beta_ext, vec3<f32>(1.0e-20)),
            vec3<f32>(column) * (vec3<f32>(1.0) - 0.5 * tau), tau < vec3<f32>(1.0e-3));
        // Every luminous body contributes; typically one or two stars.
        for (var light_slot = 0u; light_slot < light_count(); light_slot += 1u) {
            let b = light_index(light_slot);
            if (bodies[b].material.w < 0.5) {
                continue;
            }
            // Use the same finite-disc visibility test as surface lighting.
            // A tangent-plane horizon test wrongly kills twilight at altitude
            // and cannot see eclipses caused by moons or other planets.
            var light_ray = Ray(p, dir, MISS);
            let light = sample_light(light_ray, b, rng);
            light_ray.direction = light.direction;
            let visibility = air_light_visibility(light_ray, b);
            if (visibility == 0.0) {
                continue;
            }
            let e_sun = bodies[b].material.rgb * (visibility / light.pdf);
            let sun_t = exp(-beta_ext * density_integral(p, light.direction, ATMO_SUN_STEPS));
            let cos_theta = dot(dir, light.direction);
            let beta_phase = beta_r * rayleigh_phase(cos_theta)
                + vec3<f32>(beta_m) * mie_phase(cos_theta, g);
            inscatter += t_view * segment * beta_phase * e_sun * sun_t;
        }
        if (planetshine) {
            let visibility = air_light_visibility(Ray(p, reflected_direction, MISS), reflector.index);
            if (visibility > 0.0) {
                let light_t = exp(-beta_ext * density_integral(p, reflected_direction, ATMO_SUN_STEPS));
                inscatter += t_view * segment * reflected_phase * reflected * (visibility * light_t);
            }
        }
        t_view *= step_t;
    }
    result.transmittance = t_view;
    result.inscatter = inscatter;
    return result;
}

// Ring area as seen by ringshine sampling (uniform over the annulus).
fn ring_area(index: u32) -> f32 {
    let body = bodies[index];
    let r = body.center.w;
    return PI * r * r * (body.ring_params.x * body.ring_params.x - body.ring_plane.w * body.ring_plane.w);
}

// Ringshine: next-event estimation of a planet's own sunlit rings onto its
// surface. Continuations rarely hit the rings, so a planet's night side and
// eclipsed face, lit mostly by them, otherwise fill in one speck at a time.
// Returns the contribution before throughput * albedo, MIS-weighted against
// the cosine continuation (see ring_mis in trace()).
fn ringshine(index: u32, origin: vec3<f32>, normal: vec3<f32>, last_vertex: bool,
    rng: ptr<function, u32>) -> vec3<f32> {
    let body = bodies[index];
    if (body.ring_params.x <= body.ring_plane.w || body.ring_params.y <= 0.0) {
        return vec3<f32>(0.0);
    }
    let n = body.ring_plane.xyz;
    let inner = body.ring_plane.w * body.ring_plane.w;
    let outer = body.ring_params.x * body.ring_params.x;
    let radius = sqrt(mix(inner, outer, random(rng))) * body.center.w;
    let phi = TAU * random(rng);
    let point = basis_direction(n, cos(phi), sin(phi), 0.0) * radius;
    let offset = point - origin;
    let distance2 = dot(offset, offset);
    let distance = sqrt(distance2);
    let direction = offset / distance;
    let cos_surface = dot(normal, direction);
    let cos_ring = abs(dot(direction, n));
    if (cos_surface <= 0.0 || cos_ring < 1.0e-6) {
        return vec3<f32>(0.0);
    }
    let blocker = sphere_hit(Ray(origin, direction, index), index);
    if (blocker.index != MISS && blocker.distance < distance) {
        return vec3<f32>(0.0);
    }
    let optics = ring_optics(index, point);
    if (optics.w == 0.0) {
        return vec3<f32>(0.0);
    }
    let mu_out = dot(-direction, n);
    var shine = vec3<f32>(0.0);
    for (var light_slot = 0u; light_slot < light_count(); light_slot += 1u) {
        let i = light_index(light_slot);
        if (bodies[i].material.w < 0.5) {
            continue;
        }
        let light = sample_light(Ray(point, n, index), i, rng);
        let mu_in = dot(light.direction, n);
        let visibility = light_visibility(ring_origin(RingHit(0.0, index, point), light.direction), i);
        shine += ring_bsdf(optics, mu_out, mu_in) * bodies[i].material.rgb
            * (abs(mu_in) * visibility / light.pdf);
    }
    let pdf = distance2 / (ring_area(index) * cos_ring);
    var weight = power_weight(pdf, cos_surface / PI);
    if (last_vertex) {
        weight = 1.0;
    }
    return shine * (cos_surface * weight / (PI * pdf));
}

// Smallest angular radius of a body on camera rays: that of the brightest
// catalogue star (stars.rs), kept between 0.7 and STAR_DOT_PIXELS pixels.
const POINT_DISC_RADIUS: f32 = 0.00136;

// Camera rays see an unresolved body as a disc as large as a bright star's,
// its light spread over it (true / drawn disc area), so it shows at once
// rather than as one speck in thousands of samples. Overlapping discs (a
// distant planet and its moons) must not hide each other: one is picked in
// proportion to its area ratio, and `scale` (the sum of the ratios) keeps
// every body's light.
fn point_disc_hit(ray: Ray, closest: Hit, scale: ptr<function, f32>,
    rng: ptr<function, u32>) -> Hit {
    var result = closest;
    var total = 0.0;
    let pixel = 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let angle = clamp(POINT_DISC_RADIUS, 0.7 * pixel, STAR_DOT_PIXELS * pixel);
    let host = u32(settings.g.atmo_params.w) - 1u;
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        if (i == host || settings.g.ground_north.w == f32(i + 1u)) { continue; }
        let radius = bodies[i].center.w;
        let drawn = angle * length(relative_center(ray, i));
        if (radius <= 0.0 || drawn <= radius) { continue; }
        let hit = sphere_hit_radius(ray, i, drawn);
        if (hit.distance < closest.distance) {
            let ratio = (radius / drawn) * (radius / drawn);
            total += ratio;
            if (random(rng) * total < ratio) {
                result = hit;
            }
        }
    }
    if (total > 0.0) {
        *scale = total;
    }
    return result;
}

// See ring_cap in trace().
const RING_INDIRECT_CAP: f32 = 0.003;

// Camera-ray gain for a body (or ring system) of angular `extent` radius at
// `distance`: the shared point-source gain while it is under a pixel,
// fading to physical brightness once it spans three (see stars.rs).
fn point_gain(extent: f32, distance: f32) -> f32 {
    let pixel = 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let pixels = 2.0 * extent / max(distance, 1.0e-30) / pixel;
    return mix(settings.background.w, 1.0, smoothstep(1.0, 3.0, pixels));
}

fn trace(initial: Ray, rng: ptr<function, u32>, backdrop: ptr<function, vec3<f32>>) -> vec4<f32> {
    var ray = initial;
    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    *backdrop = vec3<f32>(0.0);
    var previous_pdf = 0.0;
    // Null ring crossings move the ray origin without changing the vertex
    // whose light-sampling PDF competes with this continuation.
    var previous_vertex = initial;
    var depth = 0u;
    var camera_segment = true;
    var null_crossings = 0u;
    // The previous vertex lies inside the host air. Its in-scatter comes from
    // the sky cache; extinction is applied once the continuation leaves the
    // local scene (metre-scale hops between props see negligible air).
    var in_air = false;
    // The previous vertex was a local surface whose sky light is carried by
    // its continuation ray: add the sky cache if that ray escapes.
    var sky_on_escape = false;
    // The camera ray's first celestial surface (or ring) takes the
    // unresolved point-source gain, once.
    var point_gained = false;
    // Light reaching the camera after a ring-particle scatter is capped at
    // that point's direct starlight, or a small fraction of its unshadowed
    // starlight. A ring in its planet's shadow is lit only by rare
    // continuations onto the dayside or sunlit ring, which the night exposure
    // would otherwise turn into star-like specks; sunlit rings keep their
    // multiple scattering.
    var ring_cap = 1.0e30;
    var ring_base = vec3<f32>(0.0);
    // The previous vertex was a ringed planet's surface that sampled its
    // rings explicitly (ringshine): a continuation onto them is MIS-weighted.
    var shine_body = MISS;
    loop {
        var hit = closest_hit(ray);
        var disc_scale = 1.0;
        if (depth == 0u && null_crossings == 0u && settings.background.w > 0.0) {
            hit = point_disc_hit(ray, hit, &disc_scale, rng);
        }
        let ring = closest_ring(ray, hit.distance);
        if (in_air) {
            in_air = false;
            let host = u32(settings.g.atmo_params.w) - 1u;
            if (ring.index != MISS || (hit.index < LOCAL_BASE && hit.index != host)) {
                throughput *= air_transmittance(ray, ray.direction);
            }
        }
        if (camera_segment) {
            camera_segment = false;
            // Blanket the camera ray in the host planet's atmosphere. The sky
            // is attenuated behind it and the in-scatter sits in front.
            var limit = 1.0e30;
            if (hit.index != MISS) {
                limit = hit.distance;
            }
            if (ring.index != MISS) {
                limit = min(limit, ring.distance);
            }
            let atmo = atmosphere(ray.offset, ray.direction, limit, rng);
            radiance += throughput * atmo.inscatter;
            throughput *= atmo.transmittance;
            // No visibility rule for the catalogue or Milky Way: both carry
            // physical brightness, so illuminated air (day, twilight, thick
            // air, moonlight) out-shines them under the adapted exposure.
        }
        if (ring.index != MISS) {
            if (depth == 0u && !point_gained && settings.background.w > 0.0) {
                point_gained = true;
                let planet = bodies[ring.index];
                throughput *= point_gain(planet.center.w * planet.ring_params.x, ring.distance);
            }
            let n = bodies[ring.index].ring_plane.xyz;
            let optics = ring_optics(ring.index, ring.point);
            let mu_out = dot(-ray.direction, n);
            let transmission = ring_transmission(ray, ring);
            let scatter_probability = one_minus_exp(optics.w / max(abs(mu_out), 1.0e-7));
            // Uniform-sphere continuation matches the isotropic particle phase.
            let bsdf_pdf = scatter_probability / (4.0 * PI);
            let last_vertex = depth + 1u == settings.counts.w;
            var ring_mis = 1.0;
            if (shine_body == ring.index && null_crossings == 0u) {
                let cos_ring = max(abs(dot(ray.direction, n)), 1.0e-6);
                let shine_pdf = ring.distance * ring.distance / (ring_area(ring.index) * cos_ring);
                ring_mis = power_weight(previous_pdf, shine_pdf);
            }
            shine_body = MISS;
            var unshadowed = 0.0;
            var direct = 0.0;
            for (var light_slot = 0u; light_slot < light_count(); light_slot += 1u) {
                let i = light_index(light_slot);
                if (bodies[i].material.w < 0.5 || optics.w == 0.0) {
                    continue;
                }
                let origin = Ray(ring.point, n, ring.index);
                let light = sample_light(origin, i, rng);
                let mu_in = dot(light.direction, n);
                let visibility = light_visibility(ring_origin(ring, light.direction), i);
                var weight = power_weight(light.pdf, bsdf_pdf);
                if (last_vertex) {
                    weight = 1.0;
                }
                let lit = throughput * ring_bsdf(optics, mu_out, mu_in)
                    * bodies[i].material.rgb * (abs(mu_in) / light.pdf);
                radiance += lit * (visibility * weight * ring_mis);
                unshadowed += dot(lit, vec3<f32>(0.2126, 0.7152, 0.0722));
                direct += dot(lit, vec3<f32>(0.2126, 0.7152, 0.0722)) * visibility;
            }
            if (random(rng) < transmission) {
                // Null events neither consume bounce budget nor replace the
                // previous scattering PDF: MIS still belongs to that vertex.
                ray = ring_origin(ring, ray.direction);
                null_crossings += 1u;
                if (null_crossings > settings.counts.x) {
                    break; // defensive guard; a straight ray crosses each disc at most once
                }
                continue;
            }
            if (last_vertex || scatter_probability == 0.0) {
                break;
            }
            if (ring_cap > 1.0e29) {
                ring_cap = max(direct, RING_INDIRECT_CAP * unshadowed);
                ring_base = radiance;
            }
            let z = 1.0 - 2.0 * random(rng);
            let phi = TAU * random(rng);
            let radial = sqrt(max(0.0, 1.0 - z * z));
            let direction = basis_direction(n, radial * cos(phi), radial * sin(phi), z);
            throughput *= ring_bsdf(optics, mu_out, z) * (abs(z) / bsdf_pdf);
            previous_pdf = bsdf_pdf;
            sky_on_escape = false;
            ray = ring_origin(ring, direction);
            previous_vertex = ray;
            depth += 1u;
            null_crossings = 0u;
            if (depth >= 3u) {
                let survival = clamp(max(throughput.x, max(throughput.y, throughput.z)), 0.05, 0.95);
                if (random(rng) >= survival) {
                    break;
                }
                throughput /= survival;
            }
            continue;
        }
        if (hit.index == MISS) {
            // Background stars are visible along unscattered camera paths,
            // including straight-through ring crossings.
            radiance += throughput * environment(ray.direction, depth == 0u);
            if (sky_on_escape) {
                radiance += throughput * surface_sky_lookup(ray.direction);
            }
            if (depth == 0u) {
                // A colored visual layer, attenuated by the atmosphere
                // and unscattered ring crossings. Never feed it into bounces.
                *backdrop = milky_way(ray.direction) * throughput;
                radiance += *backdrop;
            }
            break;
        }
        let local = hit.index >= LOCAL_BASE;
        // Local IDs never index the astronomical body buffer.
        var body: Body;
        if (!local) { body = bodies[hit.index]; }
        throughput *= disc_scale;
        if (!local && body.material.w > 0.5) {
            var weight = 1.0;
            if (depth > 0u) {
                let light_pdf = 1.0 / (TAU * cone_width(previous_vertex, hit.index));
                weight = power_weight(previous_pdf, light_pdf);
            }
            radiance += throughput * body.material.rgb * weight;
            break;
        }
        if (!local && depth == 0u && !point_gained && settings.background.w > 0.0) {
            point_gained = true;
            let extent = body.center.w * max(1.0, body.ring_params.x);
            throughput *= point_gain(extent, hit.distance);
        }
        var normal = hit.normal;
        if (local) { normal = landscape_shading_normal(ray, hit); }
        var outgoing: Ray;
        if (local) {
            outgoing = landscape_outgoing(ray, hit);
        } else {
            // Celestial surfaces retain their body-relative origin; local
            // ground and props instead use a five-millimetre offset.
            let origin = normal * (body.center.w * (1.0 + 2.0e-6));
            outgoing = Ray(origin, normal, hit.index);
        }
        // Factor Lambertian color out of the light loop. Keeping the complex
        // procedural albedo live across sampling corrupted solid-body textures
        // on RADV (stripes and black fragments). Evaluate it after incoming
        // light, then apply it once; visibility and MIS remain independent.
        var direct_light = vec3<f32>(0.0);
        let last_vertex = depth + 1u == settings.counts.w;
        for (var light_slot = 0u; light_slot < light_count(); light_slot += 1u) {
            let i = light_index(light_slot);
            if (bodies[i].material.w < 0.5) {
                continue;
            }
            let sample = sample_light(outgoing, i, rng);
            let cosine = max(dot(normal, sample.direction), 0.0);
            if (cosine <= 0.0) {
                continue;
            }
            outgoing.direction = sample.direction;
            let visibility = light_visibility(outgoing, i);
            if (visibility > 0.0) {
                let bsdf_pdf = cosine / PI;
                var weight = power_weight(sample.pdf, bsdf_pdf);
                if (last_vertex) {
                    // No BSDF continuation at the depth limit: NEE alone.
                    weight = 1.0;
                }
                direct_light += bodies[i].material.rgb
                    * air_transmittance(outgoing, sample.direction)
                    * (bsdf_pdf * weight * visibility / sample.pdf);
            }
        }
        var albedo: vec3<f32>;
        if (local) { albedo = landscape_albedo(ray, hit); }
        else { albedo = surface_albedo(body, normal); }
        radiance += throughput * albedo * direct_light;
        shine_body = MISS;
        if (!local) {
            radiance += throughput * albedo * ringshine(hit.index, outgoing.offset, normal, last_vertex, rng);
            shine_body = hit.index;
        }
        let sky_lit = local && settings.g.ground_counts.y != 0u && settings.g.atmo_params.w > 0.5;
        if (sky_lit && last_vertex) {
            // Atmosphere lighting. With no continuation at the depth limit,
            // trace an explicit sky sample; otherwise the cosine-distributed
            // continuation doubles as the sky sample (added on escape).
            radiance += throughput * albedo * surface_sky(outgoing, normal, rng);
        }
        if (last_vertex) {
            break;
        }
        let u = random(rng);
        let phi = TAU * random(rng);
        let cosine = sqrt(1.0 - u);
        outgoing.direction = basis_direction(normal, sqrt(u) * cos(phi), sqrt(u) * sin(phi), cosine);
        previous_pdf = cosine / PI;
        throughput *= albedo;
        if (depth >= 2u) {
            let survival = clamp(max(throughput.x, max(throughput.y, throughput.z)), 0.05, 0.95);
            if (random(rng) >= survival) {
                break;
            }
            throughput /= survival;
        }
        ray = outgoing;
        previous_vertex = outgoing;
        in_air = inside_air(outgoing);
        sky_on_escape = sky_lit;
        depth += 1u;
        null_crossings = 0u;
    }
    // W holds backdrop luminance for metering, not presentation alpha.
    if (ring_cap < 1.0e29) {
        let indirect = radiance - ring_base;
        let luminance = dot(indirect, vec3<f32>(0.2126, 0.7152, 0.0722));
        if (luminance > ring_cap) {
            radiance = ring_base + indirect * (ring_cap / luminance);
        }
    }
    return vec4<f32>(radiance, dot(*backdrop, vec3<f32>(0.2126, 0.7152, 0.0722)));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dimensions = vec2<u32>(settings.g.viewport.xy);
    if (id.x >= dimensions.x || id.y >= dimensions.y) {
        return;
    }
    let pixel = id.y * dimensions.x + id.x;
    var sum = vec4<f32>(0.0);
    var backdrop_sum = vec3<f32>(0.0);
    for (var s = 0u; s < settings.counts.z; s += 1u) {
        var rng = hash(pixel ^ hash(settings.counts.y + s + 0x9e3779b9u));
        let jitter = vec2<f32>(random(&rng), random(&rng));
        let uv = (vec2<f32>(id.xy) + jitter) / settings.g.viewport.xy;
        let ndc = vec2<f32>(2.0 * uv.x - 1.0, 1.0 - 2.0 * uv.y);
        let tangent = settings.g.cam_forward.w;
        // Stay in telescope space: adding 1e-9-radian jitter to a world-space
        // unit vector would lose the jitter in f32 at high magnification.
        let direction = normalize(vec3<f32>(
            ndc.x * tangent * settings.g.viewport.w,
            ndc.y * tangent, -1.0));
        var backdrop = vec3<f32>(0.0);
        sum += trace(Ray(vec3<f32>(0.0), direction, MISS), &rng, &backdrop);
        backdrop_sum += backdrop;
    }
    let old_count = settings.counts.y;
    let new_count = old_count + settings.counts.z;
    var mean = sum / f32(settings.counts.z);
    var backdrop_mean = backdrop_sum / f32(settings.counts.z);
    if (old_count > 0u) {
        let old = accumulation[pixel];
        mean = old + (mean - old) * (f32(settings.counts.z) / f32(new_count));
        let old_backdrop = backdrop_accumulation[pixel].rgb;
        backdrop_mean = old_backdrop + (backdrop_mean - old_backdrop)
            * (f32(settings.counts.z) / f32(new_count));
    }
    accumulation[pixel] = mean;
    backdrop_accumulation[pixel] = vec4<f32>(backdrop_mean, 0.0);
}
