// Metre-scale geometry shares the planetary path integrator and visibility.
// The surface frame is independent of where the telescope points.
struct Primitive {
    center: vec4<f32>,
    extent: vec4<f32>,
    albedo: vec4<f32>,
};
struct Landscape {
    objects: array<Primitive, 256>,
    sky_light: array<vec4<f32>, 128>,
    terrain: array<f32>,
};
@group(0) @binding(5) var<storage, read_write> ground_data: Landscape;
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

// A box with clipped corners gives rocks broad, irregularly sized facets.
// Intersect convex planes directly; no distance-field marching or mesh scan.
fn rock_hit(p: vec3<f32>, d: vec3<f32>, e: vec3<f32>) -> vec4<f32> {
    let origin = p / e;
    let direction = d / e;
    var near = -1.0e30;
    var far = 1.0e30;
    var near_normal = vec3<f32>(0.0);
    var far_normal = vec3<f32>(0.0);
    for (var face = 0u; face < 14u; face += 1u) {
        var n = vec3<f32>(0.0);
        var limit = 1.0;
        if (face < 6u) {
            n[face / 2u] = select(-1.0, 1.0, (face & 1u) == 1u);
        } else {
            let bits = face - 6u;
            n = vec3<f32>(select(-1.0, 1.0, (bits & 1u) != 0u),
                select(-1.0, 1.0, (bits & 2u) != 0u),
                select(-1.0, 1.0, (bits & 4u) != 0u));
            limit = 1.65 + 0.08 * f32(bits % 3u);
        }
        let denominator = dot(n, direction);
        let gap = limit - dot(n, origin);
        if (abs(denominator) < 1.0e-12) {
            if (gap < 0.0) { return vec4<f32>(0.0, 0.0, 0.0, 1.0e30); }
        } else {
            let distance = gap / denominator;
            if (denominator < 0.0 && distance > near) {
                near = distance;
                near_normal = n / e;
            }
            if (denominator > 0.0 && distance < far) {
                far = distance;
                far_normal = n / e;
            }
        }
        if (near > far) { return vec4<f32>(0.0, 0.0, 0.0, 1.0e30); }
    }
    if (far <= 0.001) { return vec4<f32>(0.0, 0.0, 0.0, 1.0e30); }
    return select(vec4<f32>(normalize(far_normal), far),
        vec4<f32>(normalize(near_normal), near), near > 0.001);
}

fn landscape_query(ray: Ray, limit: f32, occlusion: bool) -> Hit {
    var closest = Hit(limit, MISS, vec3<f32>(0.0));
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
            if (distance > 0.001 && distance / METRES_PER_AU < closest.distance) {
                if (occlusion) { return Hit(distance / METRES_PER_AU, LOCAL_BASE, vec3<f32>(0.0)); }
                let p = o + d * distance;
                let n = normalize(vec3<f32>(p.x / radius, 1.0 + p.y / radius, p.z / radius));
                closest = Hit(distance / METRES_PER_AU, LOCAL_BASE, from_ground(n));
            }
        }
    }
    if (settings.g.ground_counts.y != 0u) {
        let limit = min(closest.distance, 1.0e18) * METRES_PER_AU;
        let terrain = terrain_query(o, d, limit, occlusion);
        if (terrain.w < limit && terrain.w < 1.0e29) {
            closest = Hit(terrain.w / METRES_PER_AU, TERRAIN_ID, from_ground(terrain.xyz));
            if (occlusion) { return closest; }
        }
    }
    for (var i = 0u; i < settings.g.ground_counts.x; i += 1u) {
        let object = ground_data.objects[i];
        if (object.center.w < 0.0) {
            // Group markers are bounds, never visible surfaces. Their skip
            // count includes nested bounds and all following descendants.
            let bounds = box_interval(o - object.center.xyz, d, object.extent.xyz);
            if (bounds.x > bounds.y || bounds.y <= 0.001
                || bounds.x / METRES_PER_AU > closest.distance) {
                i += u32(-object.center.w);
            }
            continue;
        }
        let p = rotate_north(o - object.center.xyz, -object.extent.w);
        let v = rotate_north(d, -object.extent.w);
        let e = object.extent.xyz;
        var distance = 1.0e30;
        var n = vec3<f32>(0.0);
        if (object.center.w > 2.5) {
            let bounds = box_interval(p, v, e);
            if (bounds.x > bounds.y || bounds.y <= 0.001
                || bounds.x / METRES_PER_AU > closest.distance) { continue; }
            var vegetation = vec4<f32>(0.0, 0.0, 0.0, 1.0e30);
            if (object.center.w < 3.5) { vegetation = conifer_hit(p, v, e); }
            else if (object.center.w < 4.5) { vegetation = grass_hit(p, v, e); }
            else { vegetation = trunk_hit(p, v, e); }
            distance = vegetation.w;
            n = vegetation.xyz;
        } else if (object.center.w > 1.5) {
            let bounds = box_interval(p, v, e);
            if (bounds.x > bounds.y || bounds.y <= 0.001
                || bounds.x / METRES_PER_AU > closest.distance) { continue; }
            let rock = rock_hit(p, v, e);
            distance = rock.w;
            n = rock.xyz;
        } else if (object.center.w > 0.5) {
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
            if (occlusion) { return Hit(distance / METRES_PER_AU, LOCAL_BASE + 1u + i, vec3<f32>(0.0)); }
            closest = Hit(distance / METRES_PER_AU, LOCAL_BASE + 1u + i,
                from_ground(rotate_north(n, object.extent.w)));
        }
    }
    return closest;
}

fn landscape_point(ray: Ray, hit: Hit) -> vec3<f32> {
    return ground_origin(ray) + to_ground(ray.direction) * (hit.distance * METRES_PER_AU);
}
// World-space procedural materials have no UV seams or repeating image tiles.
fn forest_floor(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let broad = vnoise(p * 0.045 + vec3<f32>(7.0, 0.0, 13.0));
    let middle = vnoise(p * 0.7);
    let grain = vnoise(p * 18.0);
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let detail = 1.0 - smoothstep(0.006, 0.055, footprint);
    let moss = mix(vec3<f32>(0.028,0.054,0.014), vec3<f32>(0.105,0.14,0.039), middle);
    let earth = mix(vec3<f32>(0.055,0.029,0.013), vec3<f32>(0.17,0.115,0.057), middle);
    var color = mix(earth, moss, smoothstep(0.35,0.63,broad));
    // Fine, broken strokes suggest fallen needles; their contrast fades at
    // subpixel sizes instead of sparkling on distant slopes.
    let needles = pow(max(0.0,sin(p.x*92.0+p.z*137.0+middle*7.0)),18.0)
        * smoothstep(0.50,0.68,grain);
    color = mix(color,vec3<f32>(0.22,0.145,0.067),needles*detail*0.5);
    color *= 1.0 + (grain-0.5)*0.38*detail;
    let slope = 1.0 - max(normal.y,0.0);
    let stone = mix(vec3<f32>(0.095,0.105,0.10),vec3<f32>(0.24,0.23,0.20),middle);
    color = mix(color,stone,smoothstep(0.10,0.43,slope+(broad-0.5)*0.17));
    let center = 2.0 + 0.30*p.z + 2.0*sin(p.z*0.055);
    let edge = abs(p.x-center) + (middle-0.5)*0.55;
    let path = (1.0-smoothstep(0.8,2.1,edge))
        * (1.0-smoothstep(45.0,95.0,p.z))*smoothstep(-10.0,0.0,p.z);
    let gravel = mix(vec3<f32>(0.12,0.095,0.062),vec3<f32>(0.26,0.23,0.17),grain*detail+0.5*(1.0-detail));
    return mix(color,gravel,path*0.85);
}
fn landscape_albedo(ray: Ray, hit: Hit) -> vec3<f32> {
    let p = landscape_point(ray, hit);
    let distance = hit.distance * METRES_PER_AU;
    let n = to_ground(hit.normal);
    if (hit.index == LOCAL_BASE || hit.index == TERRAIN_ID) {
        return forest_floor(p,distance,n);
    }
    let object = ground_data.objects[hit.index - LOCAL_BASE - 1u];
    let tint = object.albedo.rgb;
    let kind = object.albedo.w;
    if (kind == 0.0) { return tint; }
    let middle = vnoise(p * 1.7);
    let fine = mix(vnoise(p * 12.0),0.5,smoothstep(18.0,110.0,distance));
    if (kind > 4.5) { return tint*(0.65+0.6*middle); }
    if (kind > 2.5) {
        let q = rotate_north(p-object.center.xyz,-object.extent.w)/object.extent.xyz;
        if (kind > 3.5 || (q.y < -0.35 && length(q.xz) < 0.09)) {
            let bark = vnoise(p*vec3<f32>(25.0,1.5,25.0));
            return vec3<f32>(0.13,0.073,0.032)*(0.5+0.85*bark);
        }
        return tint*(0.7+0.55*middle+0.2*fine);
    }
    let strata = sin(p.y*8.0+2.5*middle);
    let lichen = smoothstep(0.52,0.76,middle)*max(n.y,0.0);
    let stone = tint*(0.7+0.45*fine+0.07*strata);
    return mix(stone,vec3<f32>(0.11,0.145,0.037),lichen*0.65);
}
fn landscape_shading_normal(ray: Ray, hit: Hit) -> vec3<f32> {
    if (hit.index != LOCAL_BASE && hit.index != TERRAIN_ID) { return hit.normal; }
    let p = landscape_point(ray,hit);
    let distance = hit.distance*METRES_PER_AU;
    let strength = 0.11*(1.0-smoothstep(8.0,65.0,distance));
    let dx = vnoise((p+vec3<f32>(0.025,0.0,0.0))*6.0)-vnoise((p-vec3<f32>(0.025,0.0,0.0))*6.0);
    let dz = vnoise((p+vec3<f32>(0.0,0.0,0.025))*6.0)-vnoise((p-vec3<f32>(0.0,0.0,0.025))*6.0);
    return normalize(hit.normal+from_ground(vec3<f32>(-dx,0.0,-dz))*strength);
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

fn landscape_hit(ray: Ray) -> Hit {
    return landscape_query(ray, 1.0e30, false);
}
