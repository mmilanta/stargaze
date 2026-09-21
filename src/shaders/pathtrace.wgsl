// Vacuum path tracer. All scene visibility is analytic ray/sphere intersection.
// Direct light: one solid-angle sample per stellar disc, with power-heuristic MIS.
// Indirect light: cosine-weighted Lambertian bounces and Russian roulette.
struct Globals {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>, // w = tan(vertical FOV / 2)
    viewport: vec4<f32>, // width, height, exposure, unused
};
struct Settings {
    g: Globals,
    counts: vec4<u32>, // bodies, accumulated samples, samples this dispatch, max surface vertices
};
struct Body {
    center: vec4<f32>, // xyz = high part of telescope-space centre, w = radius
    material: vec4<f32>, // rgb = reflectance OR emitted radiance, w = emissive
    low: vec4<f32>, // xyz = residual centre, w = procedural albedo strength
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

const PI: f32 = 3.141592653589793;
const TAU: f32 = 6.283185307179586;
const MISS: u32 = 0xffffffffu;

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

fn sphere_hit(ray: Ray, index: u32) -> Hit {
    let oc = relative_center(ray, index);
    let radius = bodies[index].center.w;
    let along = dot(oc, ray.direction);
    // Cross products avoid catastrophic cancellation in b*b - c for tiny
    // distant bodies (notably Phobos). Compute the normal in body-local space.
    let perpendicular = cross(ray.direction, cross(oc, ray.direction));
    let h = radius * radius - dot(perpendicular, perpendicular);
    if (h < 0.0) {
        return Hit(1.0e30, MISS, vec3<f32>(0.0));
    }
    let root = sqrt(h);
    var delta = -root;
    var distance = along + delta;
    if (distance <= 0.0) {
        delta = root;
        distance = along + delta;
    }
    if (distance <= 0.0) {
        return Hit(1.0e30, MISS, vec3<f32>(0.0));
    }
    return Hit(distance, index, normalize(-perpendicular + ray.direction * delta));
}

fn closest_hit(ray: Ray) -> Hit {
    var closest = Hit(1.0e30, MISS, vec3<f32>(0.0));
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        let hit = sphere_hit(ray, i);
        if (hit.distance < closest.distance) {
            closest = hit;
        }
    }
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

fn environment(view_direction: vec3<f32>) -> vec3<f32> {
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
        if (dot(direction, star.direction.xyz) > 0.0 && dot(perpendicular, perpendicular) <= star.direction.w) {
            light += star.radiance.rgb;
        }
    }
    return light;
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
fn surface_albedo(body: Body, normal: vec3<f32>) -> vec3<f32> {
    var mottle = 1.0;
    if (body.low.w > 0.0) {
        var p = world_direction(normal) * 2.4;
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

fn trace(initial: Ray, rng: ptr<function, u32>) -> vec3<f32> {
    var ray = initial;
    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    var previous_pdf = 0.0;
    for (var depth = 0u; depth < settings.counts.w; depth += 1u) {
        let hit = closest_hit(ray);
        if (hit.index == MISS) {
            // The catalogue is an environment sampled only by path rays, so
            // there is no competing light-sampling technique and no MIS here.
            radiance += throughput * environment(ray.direction);
            break;
        }
        let body = bodies[hit.index];
        if (body.material.w > 0.5) {
            var weight = 1.0;
            if (depth > 0u) {
                let light_pdf = 1.0 / (TAU * cone_width(ray, hit.index));
                weight = power_weight(previous_pdf, light_pdf);
            }
            radiance += throughput * body.material.rgb * weight;
            break;
        }
        let normal = hit.normal;
        let albedo = surface_albedo(body, normal);
        // Radius-relative offset is evaluated locally, not added to a huge
        // world coordinate. About 12 m on Terra, much smaller on its moons.
        let origin = normal * (body.center.w * (1.0 + 2.0e-6));
        var outgoing = Ray(origin, normal, hit.index);
        let last_vertex = depth + 1u == settings.counts.w;
        for (var i = 0u; i < settings.counts.x; i += 1u) {
            if (bodies[i].material.w < 0.5) {
                continue;
            }
            let sample = sample_light(outgoing, i, rng);
            let cosine = max(dot(normal, sample.direction), 0.0);
            if (cosine <= 0.0) {
                continue;
            }
            outgoing.direction = sample.direction;
            if (closest_hit(outgoing).index == i) {
                let bsdf_pdf = cosine / PI;
                var weight = power_weight(sample.pdf, bsdf_pdf);
                if (last_vertex) {
                    // No BSDF continuation at the depth limit: NEE alone.
                    weight = 1.0;
                }
                radiance += throughput * albedo * bodies[i].material.rgb
                    * (bsdf_pdf * weight / sample.pdf);
            }
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
    }
    return radiance;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dimensions = vec2<u32>(settings.g.viewport.xy);
    if (id.x >= dimensions.x || id.y >= dimensions.y) {
        return;
    }
    let pixel = id.y * dimensions.x + id.x;
    var sum = vec3<f32>(0.0);
    for (var s = 0u; s < settings.counts.z; s += 1u) {
        var rng = hash(pixel ^ hash(settings.counts.y + s + 0x9e3779b9u));
        let jitter = vec2<f32>(random(&rng), random(&rng));
        let uv = (vec2<f32>(id.xy) + jitter) / settings.g.viewport.xy;
        let ndc = vec2<f32>(2.0 * uv.x - 1.0, 1.0 - 2.0 * uv.y);
        let tangent = settings.g.cam_forward.w;
        // Stay in telescope space: adding 1e-9-radian jitter to a world-space
        // unit vector would lose the jitter in f32 at high magnification.
        let direction = normalize(vec3<f32>(
            ndc.x * tangent * settings.g.viewport.x / settings.g.viewport.y,
            ndc.y * tangent, -1.0));
        sum += trace(Ray(vec3<f32>(0.0), direction, MISS), &rng);
    }
    let old_count = settings.counts.y;
    let new_count = old_count + settings.counts.z;
    var mean = sum / f32(settings.counts.z);
    if (old_count > 0u) {
        let old = accumulation[pixel].rgb;
        mean = old + (mean - old) * (f32(settings.counts.z) / f32(new_count));
    }
    accumulation[pixel] = vec4<f32>(mean, 1.0);
}
