// Path tracer with a camera-ray single-scattering atmosphere.
// All scene visibility uses analytic spheres and equatorial ring annuli.
// Direct light: one solid-angle sample per stellar disc, with power-heuristic MIS.
// Indirect light: cosine-weighted Lambertian bounces and Russian roulette.
struct Globals {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>, // w = tan(vertical FOV / 2)
    viewport: vec4<f32>, // width, height, exposure, unused
    // Host-atmosphere shell, in telescope space.
    atmo_center: vec4<f32>, // xyz = host centre, w = host radius
    atmo_rayleigh: vec4<f32>, // rgb = Rayleigh coefficients (1/AU), w = Mie
    atmo_params: vec4<f32>, // x = Mie g, y = scale height (AU), z = top altitude, w = enabled
};
struct Settings {
    g: Globals,
    counts: vec4<u32>, // bodies, accumulated samples, samples this dispatch, max surface vertices
};
struct Body {
    center: vec4<f32>, // xyz = high part of telescope-space centre, w = radius
    material: vec4<f32>, // rgb = reflectance OR emitted radiance, w = emissive
    low: vec4<f32>, // xyz = residual centre, w = procedural albedo strength
    ring_plane: vec4<f32>, // xyz = pole, w = inner radius in planet radii
    ring_params: vec4<f32>, // outer radius in planet radii (0 = absent), optical depth, banded, unused
    ring_color: vec4<f32>, // rgb = single-scattering albedo
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
// A *background* catalogue star never covers more than this many pixels in
// radius, so a deep zoom resolves points of light rather than filling the view
// with discs. Simulated bodies (including the system's real stars) are not
// capped: they grow to their true physical size when zoomed in.
const STAR_DOT_PIXELS: f32 = 3.0;
// View-ray and light-ray samples used by the single-scattering atmosphere.
const ATMO_VIEW_STEPS: u32 = 24u;
const ATMO_SUN_STEPS: u32 = 12u;

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
    let oc = relative_center(ray, index);
    let radius = bodies[index].center.w;
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

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta);
}
fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5));
}

// Single scattering along a camera ray, up to `limit` (a surface hit) or the
// top of the shell. Returns the view transmittance and the in-scattered light
// accumulated in front of whatever the ray reaches.
fn atmosphere(origin: vec3<f32>, dir: vec3<f32>, limit: f32, rng: ptr<function, u32>) -> Scatter {
    var result = Scatter(vec3<f32>(1.0), vec3<f32>(0.0));
    if (settings.g.atmo_params.w < 0.5) {
        return result;
    }
    let t_max = min(limit, shell_exit(origin, dir));
    if (t_max <= 0.0) {
        return result;
    }

    let beta_r = settings.g.atmo_rayleigh.rgb;
    let beta_m = settings.g.atmo_rayleigh.w;
    let beta_ext = beta_r + vec3<f32>(beta_m);
    let g = settings.g.atmo_params.x;
    let r = host_radius();
    let dt = t_max / f32(ATMO_VIEW_STEPS);

    var t_view = vec3<f32>(1.0);
    var inscatter = vec3<f32>(0.0);
    for (var i = 0u; i < ATMO_VIEW_STEPS; i += 1u) {
        let p = origin + dir * ((f32(i) + 0.5) * dt);
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
        for (var b = 0u; b < settings.counts.x; b += 1u) {
            if (bodies[b].material.w < 0.5) {
                continue;
            }
            // Use the same finite-disc visibility test as surface lighting.
            // A tangent-plane horizon test wrongly kills twilight at altitude
            // and cannot see eclipses caused by moons or other planets.
            var light_ray = Ray(p, dir, MISS);
            let light = sample_light(light_ray, b, rng);
            light_ray.direction = light.direction;
            let visibility = light_visibility(light_ray, b);
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
        t_view *= step_t;
    }
    result.transmittance = t_view;
    result.inscatter = inscatter;
    return result;
}

fn trace(initial: Ray, rng: ptr<function, u32>) -> vec3<f32> {
    var ray = initial;
    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    var previous_pdf = 0.0;
    // Null ring crossings move the ray origin without changing the vertex
    // whose light-sampling PDF competes with this continuation.
    var previous_vertex = initial;
    var depth = 0u;
    var camera_segment = true;
    var null_crossings = 0u;
    loop {
        let hit = closest_hit(ray);
        let ring = closest_ring(ray, hit.distance);
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
        }
        if (ring.index != MISS) {
            let n = bodies[ring.index].ring_plane.xyz;
            let optics = ring_optics(ring.index, ring.point);
            let mu_out = dot(-ray.direction, n);
            let transmission = ring_transmission(ray, ring);
            let scatter_probability = one_minus_exp(optics.w / max(abs(mu_out), 1.0e-7));
            // Uniform-sphere continuation matches the isotropic particle phase.
            let bsdf_pdf = scatter_probability / (4.0 * PI);
            let last_vertex = depth + 1u == settings.counts.w;
            for (var i = 0u; i < settings.counts.x; i += 1u) {
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
                radiance += throughput * ring_bsdf(optics, mu_out, mu_in)
                    * bodies[i].material.rgb * (abs(mu_in) * visibility * weight / light.pdf);
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
            let z = 1.0 - 2.0 * random(rng);
            let phi = TAU * random(rng);
            let radial = sqrt(max(0.0, 1.0 - z * z));
            let direction = basis_direction(n, radial * cos(phi), radial * sin(phi), z);
            throughput *= ring_bsdf(optics, mu_out, z) * (abs(z) / bsdf_pdf);
            previous_pdf = bsdf_pdf;
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
            break;
        }
        let body = bodies[hit.index];
        if (body.material.w > 0.5) {
            var weight = 1.0;
            if (depth > 0u) {
                let light_pdf = 1.0 / (TAU * cone_width(previous_vertex, hit.index));
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
            let visibility = light_visibility(outgoing, i);
            if (visibility > 0.0) {
                let bsdf_pdf = cosine / PI;
                var weight = power_weight(sample.pdf, bsdf_pdf);
                if (last_vertex) {
                    // No BSDF continuation at the depth limit: NEE alone.
                    weight = 1.0;
                }
                radiance += throughput * albedo * bodies[i].material.rgb
                    * (bsdf_pdf * weight * visibility / sample.pdf);
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
        previous_vertex = outgoing;
        depth += 1u;
        null_crossings = 0u;
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
