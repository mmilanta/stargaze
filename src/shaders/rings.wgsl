// Analytic, zero-thickness equatorial rings. Included with pathtrace.wgsl.
struct RingHit {
    distance: f32,
    index: u32,
    point: vec3<f32>, // planet-local, never distance*direction - distant centre
};

fn ring_hit(ray: Ray, index: u32) -> RingHit {
    let body = bodies[index];
    let miss = RingHit(1.0e30, MISS, vec3<f32>(0.0));
    if (body.ring_params.x <= body.ring_plane.w || body.ring_params.y <= 0.0) {
        return miss;
    }
    let n = body.ring_plane.xyz;
    let denom = dot(ray.direction, n);
    // An ideal zero-thickness sheet has no silhouette when exactly edge-on.
    if (abs(denom) < 1.0e-7) {
        return miss;
    }
    let oc = relative_center(ray, index);
    let a = dot(ray.direction, ray.direction);
    let perpendicular = cross(ray.direction, cross(ray.direction, oc)) / a;
    let along = -dot(perpendicular, n) / denom;
    let distance = dot(oc, ray.direction) / a + along;
    if (distance <= 0.0) {
        return miss;
    }
    let point = perpendicular + along * ray.direction;
    let radius = length(point) / body.center.w;
    if (radius < body.ring_plane.w || radius > body.ring_params.x) {
        return miss;
    }
    return RingHit(distance, index, point);
}

fn closest_ring(ray: Ray, limit: f32) -> RingHit {
    var closest = RingHit(limit, MISS, vec3<f32>(0.0));
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        let hit = ring_hit(ray, i);
        if (hit.index != MISS && hit.distance < closest.distance) {
            closest = hit;
        }
    }
    return closest;
}

// Illustrative Saturn-like optical-depth profile, not a measured texture.
// Radius normalization puts the physical C/B/A boundaries and gaps at the
// same fractions for any sized ring system. Pixel jitter antialiases ringlets.
fn ring_optics(index: u32, point: vec3<f32>) -> vec4<f32> {
    let body = bodies[index];
    let radius = length(point) / body.center.w;
    let x = (radius - body.ring_plane.w) / (body.ring_params.x - body.ring_plane.w);
    var tau = body.ring_params.y;
    var color = body.ring_color.rgb;
    if (body.ring_params.z > 0.5) {
        let broad = 0.5 + 0.5 * sin(43.0 * x + 1.8 * sin(17.0 * x));
        let fine = 0.5 + 0.5 * sin(1900.0 * x + 2.0 * sin(311.0 * x));
        let micro = 0.5 + 0.5 * sin(6100.0 * x);
        var density = 0.12; // translucent C ring
        var tint = vec3<f32>(0.70, 0.68, 0.65);
        if (x >= 0.2792 && x < 0.6910) {
            density = 1.25 + 1.1 * broad; // dense, bright B ring
            tint = vec3<f32>(1.0);
        } else if (x >= 0.6910 && x < 0.7649) {
            density = 0.025; // Cassini division is sparse, not opaque black
            tint = vec3<f32>(0.75);
        } else if (x >= 0.7649) {
            density = 0.48 + 0.22 * broad; // A ring
            tint = vec3<f32>(0.92, 0.94, 0.98);
        }
        density *= 0.65 + 0.45 * fine + 0.12 * micro;
        // Encke and Keeler gaps (approximately 325 km and 42 km wide).
        if (abs(x - 0.9487) < 0.00262 || abs(x - 0.99565) < 0.00034) {
            density = 0.0;
        }
        tau *= density;
        color *= tint * (0.94 + 0.06 * broad);
    }
    return vec4<f32>(color, tau);
}

fn ring_transmission(ray: Ray, hit: RingHit) -> f32 {
    let mu = max(abs(dot(ray.direction, bodies[hit.index].ring_plane.xyz)), 1.0e-7);
    return exp(-ring_optics(hit.index, hit.point).w / mu);
}

// Shadow rays pass through every intervening ring, but never through spheres.
// This visibility is shared by surfaces, rings and the host atmosphere.
fn light_visibility(ray: Ray, light: u32) -> f32 {
    let surface = closest_hit(ray);
    if (surface.index != light) {
        return 0.0;
    }
    var transmission = 1.0;
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        let hit = ring_hit(ray, i);
        if (hit.index != MISS && hit.distance < surface.distance) {
            transmission *= ring_transmission(ray, hit);
        }
    }
    return transmission;
}

fn one_minus_exp(x: f32) -> f32 {
    return select(1.0 - exp(-x), x * (1.0 - 0.5 * x), x < 1.0e-3);
}

// Exact single scattering in a finite plane-parallel particulate slab.
// Isotropic particles scatter to both sides; unscattered transmission is a
// separate delta event. A semi-infinite multiple-scattering H-function must
// not multiply this finite-slab term: thin rings can then create energy.
fn ring_bsdf(optics: vec4<f32>, outgoing: f32, incoming: f32) -> vec3<f32> {
    let mo = max(abs(outgoing), 1.0e-7);
    let mi = max(abs(incoming), 1.0e-7);
    let a = optics.w / mo;
    let b = optics.w / mi;
    if (outgoing * incoming < 0.0) {
        let delta = abs(a - b);
        let integral = select(one_minus_exp(delta) / max(delta, 1.0e-20),
            1.0 - 0.5 * delta, delta < 1.0e-3);
        let factor = optics.w / (mo * mi) * exp(-min(a, b)) * integral;
        return optics.rgb * (factor / (4.0 * PI));
    }
    // Same-side reflection, integrated through the finite optical depth.
    let factor = one_minus_exp(a + b) / (mo + mi);
    return optics.rgb * (factor / (4.0 * PI));
}

fn ring_origin(hit: RingHit, direction: vec3<f32>) -> Ray {
    let body = bodies[hit.index];
    let n = body.ring_plane.xyz;
    let sign = select(-1.0, 1.0, dot(direction, n) >= 0.0);
    // Reproject before offsetting to avoid self-hits after plane cancellation.
    let point = hit.point - n * dot(hit.point, n);
    return Ray(point + n * (sign * body.center.w * 2.0e-6), direction, hit.index);
}
