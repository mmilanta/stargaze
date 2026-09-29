// Metre-scale geometry shares the planetary path integrator and visibility.
// The surface frame is independent of where the telescope points.
struct Primitive {
    center: vec4<f32>,
    extent: vec4<f32>,
    albedo: vec4<f32>,
};
@group(0) @binding(5) var<storage, read> ground_objects: array<Primitive>;
const LOCAL_BASE: u32 = 0x80000000u;
const METRES_PER_AU: f32 = 149597870700.0;

fn camera_origin(ray: Ray) -> vec3<f32> {
    if (ray.anchor == MISS) { return ray.offset; }
    return bodies[ray.anchor].center.xyz + (bodies[ray.anchor].low.xyz + ray.offset);
}
fn to_ground(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(v, settings.g.ground_east.xyz),
        dot(v, settings.g.ground_up.xyz), dot(v, settings.g.ground_north.xyz));
}
fn from_ground(v: vec3<f32>) -> vec3<f32> {
    return settings.g.ground_east.xyz * v.x + settings.g.ground_up.xyz * v.y
        + settings.g.ground_north.xyz * v.z;
}
fn ground_origin(ray: Ray) -> vec3<f32> {
    return to_ground(camera_origin(ray) * METRES_PER_AU)
        + vec3<f32>(0.0, settings.g.ground_east.w, 0.0);
}
fn rotate_north(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
}

// Slabs with explicit parallel-ray handling: never form 0 * infinity.
fn box_interval(p: vec3<f32>, d: vec3<f32>, extent: vec3<f32>) -> vec2<f32> {
    var near = -1.0e30;
    var far = 1.0e30;
    for (var axis = 0u; axis < 3u; axis += 1u) {
        if (abs(d[axis]) < 1.0e-12) {
            if (abs(p[axis]) > extent[axis]) { return vec2<f32>(1.0e30, -1.0e30); }
        } else {
            let a = (-extent[axis] - p[axis]) / d[axis];
            let b = (extent[axis] - p[axis]) / d[axis];
            near = max(near, min(a, b));
            far = min(far, max(a, b));
        }
    }
    return vec2<f32>(near, far);
}

fn landscape_hit(ray: Ray) -> Hit {
    var closest = Hit(1.0e30, MISS, vec3<f32>(0.0));
    if (settings.g.ground_north.w == 0.0) { return closest; }
    let o = ground_origin(ray);
    let d = to_ground(ray.direction);
    let radius = settings.g.ground_up.w;
    // Distant paths retain the cross-product celestial intersection: the
    // local quadratic is designed to preserve height close to the host.
    if (dot(o, o) > 4.0 * radius * radius) {
        return sphere_hit(ray, u32(settings.g.ground_north.w) - 1u);
    }
    // Host sphere in a surface frame. Expanding c avoids subtracting two
    // squared planetary radii to recover a height of a few centimetres.
    let a = dot(d, d);
    let b = -radius * d.y - dot(o, d);
    let c = dot(o.xz, o.xz) + o.y * (2.0 * radius + o.y);
    let h = b * b - a * c;
    if (h >= 0.0) {
        let root = sqrt(h);
        let q = b + select(-root, root, b >= 0.0);
        if (q != 0.0) {
            let t0 = min(c / q, q / a);
            let t1 = max(c / q, q / a);
            let distance = select(t1, t0, t0 > 0.001);
            if (distance > 0.001) {
                let p = o + d * distance;
                let n = normalize(vec3<f32>(p.x / radius, 1.0 + p.y / radius, p.z / radius));
                closest = Hit(distance / METRES_PER_AU, LOCAL_BASE, from_ground(n));
            }
        }
    }
    // Broad phase keeps atmospheric/astronomical rays out of the prop loop.
    let bounds = box_interval(o - vec3<f32>(0.0, 7.5, 0.0), d, vec3<f32>(190.0, 12.5, 190.0));
    if (bounds.x > bounds.y || bounds.y <= 0.001
        || bounds.x / METRES_PER_AU > closest.distance) { return closest; }
    for (var i = 0u; i < settings.g.ground_counts.x; i += 1u) {
        let object = ground_objects[i];
        let p = rotate_north(o - object.center.xyz, -object.extent.w);
        let v = rotate_north(d, -object.extent.w);
        let e = object.extent.xyz;
        var distance = 1.0e30;
        var n = vec3<f32>(0.0);
        if (object.center.w > 0.5) {
            let sp = p / e;
            let sd = v / e;
            let aa = dot(sd, sd);
            let bb = dot(sp, sd);
            let cc = dot(sp, sp) - 1.0;
            let hh = bb * bb - aa * cc;
            if (hh >= 0.0) {
                let root = sqrt(hh);
                distance = (-bb - root) / aa;
                if (distance <= 0.001) { distance = (-bb + root) / aa; }
                n = normalize((p + v * distance) / (e * e));
            }
        } else {
            let interval = box_interval(p, v, e);
            if (interval.x <= interval.y && interval.y > 0.001) {
                distance = select(interval.y, interval.x, interval.x > 0.001);
                let point = (p + v * distance) / e;
                let ap = abs(point);
                if (ap.x >= ap.y && ap.x >= ap.z) { n.x = sign(point.x); }
                else if (ap.y >= ap.z) { n.y = sign(point.y); }
                else { n.z = sign(point.z); }
            }
        }
        if (distance > 0.001 && distance < 1.0e29 && distance / METRES_PER_AU < closest.distance) {
            closest = Hit(distance / METRES_PER_AU, LOCAL_BASE + 1u + i,
                from_ground(rotate_north(n, object.extent.w)));
        }
    }
    return closest;
}

fn landscape_point(ray: Ray, hit: Hit) -> vec3<f32> {
    return ground_origin(ray) + to_ground(ray.direction) * (hit.distance * METRES_PER_AU);
}
fn landscape_albedo(ray: Ray, hit: Hit) -> vec3<f32> {
    if (hit.index != LOCAL_BASE) { return ground_objects[hit.index - LOCAL_BASE - 1u].albedo.rgb; }
    let p = landscape_point(ray, hit);
    // Broad patches of grass, with fine detail faded over distance to limit shimmer.
    let grass_patch = vnoise(vec3<f32>(p.x * 0.055, 0.3, p.z * 0.055));
    let fine = vnoise(vec3<f32>(p.x * 1.7, 0.6, p.z * 1.7));
    let detail = mix(fine, 0.5, smoothstep(20.0, 150.0, hit.distance * METRES_PER_AU));
    return mix(vec3<f32>(0.12, 0.20, 0.035), vec3<f32>(0.27, 0.36, 0.075), grass_patch)
        * (0.85 + 0.3 * detail);
}
fn landscape_outgoing(ray: Ray, hit: Hit) -> Ray {
    var p = landscape_point(ray, hit);
    if (hit.index == LOCAL_BASE) {
        // Reproject onto the sphere without subtracting nearly equal radii.
        let radius = settings.g.ground_up.w;
        let d2 = dot(p.xz, p.xz);
        if (p.y > -radius && d2 < radius * radius) {
            p.y = -d2 / (radius + sqrt(radius * radius - d2));
        }
    }
    p += to_ground(hit.normal) * 0.005;
    p.y -= settings.g.ground_east.w;
    return Ray(from_ground(p) / METRES_PER_AU, hit.normal, MISS);
}
