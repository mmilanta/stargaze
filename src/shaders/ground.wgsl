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

// Tumbled-rock angles occupy 24 integer bits, exactly representable in f32.
// The order matches Primitive::rotation: Rz * Ry * Rx.
fn rock_angles(packed: f32) -> vec3<f32> {
    let bits = u32(packed);
    return vec3<f32>(f32(bits & 255u), f32((bits >> 8u) & 255u), f32((bits >> 16u) & 255u))
        * (TAU / 256.0);
}
fn rotate_east(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(p.x, c*p.y-s*p.z, s*p.y+c*p.z);
}
fn rotate_up(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(c*p.x+s*p.z, p.y, -s*p.x+c*p.z);
}
fn rock_to_local(p: vec3<f32>, angles: vec3<f32>) -> vec3<f32> {
    return rotate_east(rotate_up(rotate_north(p,-angles.z),-angles.y),-angles.x);
}
fn rock_from_local(p: vec3<f32>, angles: vec3<f32>) -> vec3<f32> {
    return rotate_north(rotate_up(rotate_east(p,angles.x),angles.y),angles.z);
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
    // The host sphere lies below the observer's tangent plane (y <= 0), and
    // every prop and terrain vertex below `landscape_top`. A ray leaving
    // upwards from above all of them cannot hit anything here.
    if (d.y >= 0.0 && o.y > landscape_top()) { return closest; }
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
        var p = o - object.center.xyz;
        var v = d;
        var angles = vec3<f32>(0.0);
        if (object.center.w == 2.25) {
            angles = rock_angles(object.extent.w);
            p = rock_to_local(p, angles);
            v = rock_to_local(v, angles);
        } else {
            p = rotate_north(p, -object.extent.w);
            v = rotate_north(v, -object.extent.w);
        }
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
            if (object.center.w == 2.25) { n = rock_from_local(n, angles); }
            else { n = rotate_north(n, object.extent.w); }
            closest = Hit(distance / METRES_PER_AU, LOCAL_BASE + 1u + i, from_ground(n));
        }
    }
    return closest;
}

fn landscape_top() -> f32 {
    var top = bitcast<f32>(settings.g.ground_counts.w);
    if (settings.g.ground_counts.y != 0u) {
        // Root quadtree bound: highest terrain vertex plus margin.
        top = max(top, ground_data.terrain[terrain_cache() + 4u]);
    }
    return top;
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
fn desert_floor(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let broad = vnoise(p * 0.007 + vec3<f32>(19.0, 2.0, 4.0));
    let gravel = vnoise(p * 2.8);
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let detail = 1.0 - smoothstep(0.025, 0.2, footprint);
    let sand = mix(vec3<f32>(0.31, 0.21, 0.125), vec3<f32>(0.53, 0.40, 0.26), broad);
    let rock = mix(vec3<f32>(0.18, 0.10, 0.055), vec3<f32>(0.38, 0.245, 0.145), broad);
    var color = mix(sand, rock, smoothstep(0.08, 0.4, 1.0 - max(normal.y, 0.0)));
    color *= 1.0 + (gravel - 0.5) * 0.32 * detail;
    // A small, meandering off-road trail: two worn tyre tracks with desert
    // between them. Soft irregular edges blend directly into the gravel.
    let center = 2.8 + 0.55 * sin(p.z / 38.0) + 1.4 * sin(p.z / 210.0);
    let edge = abs(p.x - center);
    let aa = max(0.015, footprint * 0.5);
    let fade = (1.0 - smoothstep(0.25, 0.9, footprint))
        * (1.0 - smoothstep(1000.0, 2200.0, abs(p.z)))
        * (1.0 - smoothstep(0.12, 0.35, 1.0 - max(normal.y, 0.0)));
    let worn = 1.0 - smoothstep(0.8, 1.2, edge);
    let rut_edge = abs(edge - 0.68) + (gravel - 0.5) * 0.1 * detail;
    let tracks = 1.0 - smoothstep(0.12 - aa, 0.23 + aa, rut_edge);
    let packed = sand * (0.72 + 0.10 * gravel * detail);
    color = mix(color, sand * 0.93, worn * fade * 0.35);
    return mix(color, packed, tracks * fade * 0.8);
}
// Grey lunar regolith: broad mottling, brighter fresh ejecta, fine grain.
fn crater_floor(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let broad = vnoise(p * 0.004 + vec3<f32>(3.0, 0.0, 8.0));
    let middle = vnoise(p * 0.09);
    let grain = vnoise(p * 3.1);
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let detail = 1.0 - smoothstep(0.025, 0.2, footprint);
    var tone = mix(0.11, 0.21, broad) * (0.86 + 0.28 * middle);
    // Steep crater walls expose brighter, less weathered material.
    tone *= 1.0 + 0.35 * smoothstep(0.15, 0.5, 1.0 - max(normal.y, 0.0));
    tone *= 1.0 + (grain - 0.5) * 0.3 * detail;
    return vec3<f32>(tone, tone * 0.97, tone * 0.93);
}
// Mirrors ICE_FLOW and LOOKOUT_HILL in terrain.rs: the ice flows
// west-south-west; the lookout stands on a nunatak (height, base radius).
const ICE_FLOW: f32 = 4.4;
const LOOKOUT_HILL: vec2<f32> = vec2<f32>(180.0, 700.0);
fn lookout_hill(r: f32) -> f32 {
    let u = min(length(vec2<f32>(r, 30.0)) / LOOKOUT_HILL.y, 1.0);
    return LOOKOUT_HILL.x * (1.0 - u) * (1.0 - u);
}

// Crevasses: lens-shaped cracks across the flow in rows about 60 m apart,
// where the ice is under tension (in patches, and round the nunatak).
// Returns (coverage of this pixel, 0 at a crack's lip to 1 at its centre).
// Drawn here rather than in the heightfield: metre-wide cracks are far
// narrower than the distant grid cells, and the pixel footprint
// antialiases them at any distance.
fn ice_crevasse(q: vec2<f32>, footprint: f32) -> vec2<f32> {
    let r = length(q);
    // Ice flowing round the nunatak stretches and cracks below its foot.
    let tension = smoothstep(650.0, 900.0, r) * (1.0 - smoothstep(1800.0, 2600.0, r));
    let strain = max(smoothstep(0.47, 0.58, fbm(vec3<f32>(q.x / 2600.0 + 4.0, 2.0, q.y / 2600.0), 2)),
        tension) * smoothstep(600.0, 750.0, r);
    if (strain <= 0.0) { return vec2<f32>(0.0); }
    let flow = vec2<f32>(sin(ICE_FLOW), cos(ICE_FLOW));
    let along = dot(q, flow) + 280.0 * (vnoise(vec3<f32>(q / 1500.0, 1.0)) - 0.5);
    let across = dot(q, vec2<f32>(flow.y, -flow.x)) + 120.0 * (vnoise(vec3<f32>(q / 900.0, 5.0)) - 0.5);
    let spacing = 60.0;
    let row = floor(along / spacing);
    let key = hash(bitcast<u32>(i32(row)) * 0x8da6b343u ^ 0xc3e5u);
    let length_m = 120.0 + 300.0 * f32(key & 255u) / 255.0;
    let s = across / length_m + f32((key >> 8u) & 255u) / 255.0;
    let segment = floor(s);
    let lens = pow(max(sin(PI * (s - segment)), 0.0), 0.7);
    let open = hash(key ^ bitcast<u32>(i32(segment)));
    if ((open & 255u) < 70u) { return vec2<f32>(0.0); }
    let centre = 0.5 + 0.3 * (f32((open >> 8u) & 255u) / 255.0 - 0.5);
    // Crack edges waver; widths of 2 to 16 m.
    let half_width = 0.5 * (2.0 + 14.0 * f32((open >> 16u) & 255u) / 255.0) * lens * strain;
    let d = abs(along / spacing - row - centre) * spacing + 0.8 * (vnoise(vec3<f32>(q * 0.15, 9.0)) - 0.5);
    let aa = max(footprint, 1.0e-3);
    let coverage = clamp((half_width - d) / aa + 0.5, 0.0, 1.0) * min(1.0, 2.0 * half_width / aa);
    return vec2<f32>(coverage, 1.0 - clamp(d / max(half_width, 1.0e-3), 0.0, 1.0));
}

// Patagonian ice field: wind-packed and fresh snow, bare blue glacier ice
// with white fractures, deep blue crevasse walls and dirty, rock-strewn ice
// under the mountains; granite spires with joints and
// lichen, snow on every ledge that can hold it.
fn alpine_floor(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let scale = terrain_scale();
    // Heights above the ice, not counting the nunatak under the lookout.
    var q = vec3<f32>(p.x, p.y - terrain_curvature(p.xz), p.z) / scale;
    let hill = lookout_hill(length(q.xz));
    q.y -= hill;
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0) / scale;
    let detail = 1.0 - smoothstep(0.025, 0.2, footprint);
    let slope = 1.0 - max(normal.y, 0.0);
    let broad = vnoise(q * 0.004 + vec3<f32>(11.0, 4.0, 1.0));
    let middle = vnoise(q * 0.2);
    let grain = vnoise(q * 4.0);
    let flow = vec2<f32>(sin(ICE_FLOW), cos(ICE_FLOW));
    let along = dot(q.xz, flow);
    let across = dot(q.xz, vec2<f32>(flow.y, -flow.x));
    // Snow: wind-packed crust in long patches along the wind, fresh powder.
    let crust = smoothstep(0.45, 0.6, fbm(vec3<f32>(along * 0.004, 2.0, across * 0.012), 3));
    var snow = mix(vec3<f32>(0.86, 0.88, 0.91), vec3<f32>(0.76, 0.8, 0.86), crust);
    snow *= 0.97 + 0.05 * middle;
    // Bare glacier ice where wind scours the snow away: blue-grey, with
    // white fracture lines and trapped bubbles.
    let bare = smoothstep(0.47, 0.53, fbm(q * 0.0016 + vec3<f32>(3.0, 0.0, 8.0), 4) + 0.1 * (broad - 0.5));
    var ice = mix(vec3<f32>(0.36, 0.55, 0.7), vec3<f32>(0.52, 0.67, 0.78), middle);
    let fracture_width = 0.03 + footprint / 4.0;
    let fractures = (1.0 - smoothstep(0.0, fracture_width, dune_cracks(q.xz / 4.0 + 9.0)))
        * 0.03 / fracture_width;
    ice = mix(ice, vec3<f32>(0.82, 0.86, 0.9), min(fractures, 1.0) * 0.7);
    ice *= 1.0 + (grain - 0.5) * 0.12 * detail;
    var glacier = mix(snow, ice, bare);
    let debris = mix(vec3<f32>(0.12, 0.115, 0.11), vec3<f32>(0.26, 0.24, 0.22), grain);
    // Dirty, rock-strewn ice at the foot of the mountains.
    let dirty = smoothstep(6.0, 30.0, q.y) * (1.0 - smoothstep(30.0, 70.0, q.y));
    glacier = mix(glacier, mix(debris, glacier, 0.45), dirty * smoothstep(0.4, 0.7, broad) * 0.8);
    // Crevasse walls and seracs: deep blue ice, darkest far down.
    let wall = smoothstep(0.35, 0.6, slope) * (1.0 - smoothstep(20.0, 45.0, q.y));
    let depth = smoothstep(4.0, -4.0, q.y);
    let blue = mix(vec3<f32>(0.32, 0.55, 0.72), vec3<f32>(0.1, 0.25, 0.42), depth);
    glacier = mix(glacier, blue * (0.9 + 0.2 * middle), wall);
    // Crevasses: blue ice walls fading to darkness, with a bright lip of
    // overhanging snow.
    let crack = ice_crevasse(q.xz, footprint) * (1.0 - smoothstep(10.0, 25.0, q.y));
    let lip = (1.0 - smoothstep(0.0, 0.12, crack.y)) * crack.x;
    let inside = mix(vec3<f32>(0.3, 0.55, 0.74), vec3<f32>(0.02, 0.06, 0.12), smoothstep(0.1, 0.8, crack.y));
    glacier = mix(glacier, inside, crack.x);
    glacier = mix(glacier, vec3<f32>(0.9, 0.92, 0.95), lip * 0.5);
    // Granite above the ice, snow on ledges and summits up to ~45 degrees.
    let mountain = max(smoothstep(25.0, 70.0, q.y + 30.0 * (broad - 0.5)), smoothstep(4.0, 15.0, hill));
    let joints = 1.0 - smoothstep(0.0, 0.06 + footprint / 3.0, dune_cracks(vec2<f32>(q.x + q.y * 0.5, q.z) / 3.0));
    var granite = mix(vec3<f32>(0.09, 0.085, 0.08), vec3<f32>(0.24, 0.215, 0.19), broad);
    granite *= 0.85 + 0.3 * middle - 0.35 * joints * (1.0 - smoothstep(0.1, 1.0, footprint));
    // Orange and black lichen on the less steep granite.
    let lichen = smoothstep(0.68, 0.78, vnoise(q * 0.9)) * (1.0 - smoothstep(0.6, 0.9, slope));
    granite = mix(granite, vec3<f32>(0.3, 0.17, 0.06), lichen * 0.5 * detail);
    let cover = 1.0 - smoothstep(0.17, 0.33, slope + (broad - 0.5) * 0.18 + (middle - 0.5) * 0.08);
    // Wind scours outcrops of bare rock on the nunatak's flanks.
    let scoured = smoothstep(0.52, 0.62, fbm(q * 0.012 + vec3<f32>(2.0, 0.0, 6.0), 4))
        * smoothstep(25.0, 90.0, hill) * (1.0 - smoothstep(150.0, 172.0, hill));
    let cover_hill = cover * (1.0 - scoured);
    let peak = mix(granite, snow, cover_hill);
    var color = mix(glacier, peak, mountain);
    color *= 1.0 + (grain - 0.5) * 0.08 * detail;
    return color;
}
// Sastrugi and drift ridges: wind-carved snow, elongated along the wind,
// sharp-edged on the lee. Each band fades once a pixel covers it.
fn alpine_shading_normal(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let n = to_ground(normal);
    let snowy = 1.0 - smoothstep(0.3, 0.5, 1.0 - n.y);
    let flow = vec2<f32>(sin(ICE_FLOW), cos(ICE_FLOW));
    let side = vec2<f32>(flow.y, -flow.x);
    var gradient = vec2<f32>(0.0);
    var scale = 1.2;
    var strength = 0.2;
    for (var octave = 0u; octave < 3u; octave += 1u) {
        let fade = 1.0 - smoothstep(0.08 * scale, 0.25 * scale, footprint);
        if (fade > 0.0) {
            // Stretched 6:1 along the wind.
            let a = dot(p.xz, flow) / (6.0 * scale);
            let c = dot(p.xz, side) / scale;
            let e = 0.08;
            let o = f32(octave) * 13.0;
            let da = vnoise(vec3<f32>(a + e, o, c)) - vnoise(vec3<f32>(a - e, o, c));
            let dc = vnoise(vec3<f32>(a, o, c + e)) - vnoise(vec3<f32>(a, o, c - e));
            // Sharpen into ridges: steeper where the noise crosses its middle.
            let ridge = 1.0 + 1.5 * (1.0 - abs(vnoise(vec3<f32>(a, o, c)) * 2.0 - 1.0));
            gradient += (flow * da / (6.0 * scale) + side * dc / scale) / (2.0 * e)
                * (strength * fade * ridge * scale);
        }
        scale *= 6.0;
        strength *= 0.6;
    }
    gradient *= snowy;
    return normalize(normal + from_ground(vec3<f32>(-gradient.x, 0.0, -gradient.y)));
}

// A volcano for the basalt materials: summit, height, base radius, crater
// radius (unscaled metres), kind (0 large, 1 scoria, 2 spatter) and hash.
struct BasaltCone {
    centre: vec2<f32>,
    height: f32,
    radius: f32,
    crater: f32,
    kind: u32,
    key: u32,
};

// Mirrors terrain.rs volcano_cone: same hash, jitter and rejection, so the
// colouring follows the generated cones. Radius 0 means an empty cell.
fn basalt_cone(kind: u32, ix: i32, iz: i32) -> BasaltCone {
    var cells = array<f32, 3>(5200.0, 2000.0, 720.0);
    var chance = array<f32, 3>(0.62, 0.55, 0.42);
    var low = array<f32, 3>(360.0, 80.0, 12.0);
    var high = array<f32, 3>(1050.0, 300.0, 60.0);
    var spread_low = array<f32, 3>(2.4, 2.6, 2.6);
    var spread_high = array<f32, 3>(3.4, 3.6, 3.8);
    var crater_low = array<f32, 3>(0.05, 0.12, 0.16);
    var crater_high = array<f32, 3>(0.11, 0.24, 0.3);
    var clear = array<f32, 3>(2600.0, 1100.0, 420.0);
    var none = BasaltCone(vec2<f32>(0.0), 0.0, 0.0, 0.0, kind, 0u);
    let key = hash((bitcast<u32>(ix) * 0x8da6b343u) ^ (bitcast<u32>(iz) * 0xd8163841u)
        ^ (0x5eed0000u + kind * 0x9e37u));
    let more = hash(key ^ 0x68e31da4u);
    if (f32((key >> 16u) & 255u) / 255.0 > chance[kind]) { return none; }
    let cell = cells[kind];
    let x = (f32(ix) + 0.2 + 0.6 * f32(key & 255u) / 255.0) * cell;
    let z = (f32(iz) + 0.2 + 0.6 * f32((key >> 8u) & 255u) / 255.0) * cell;
    let height = low[kind] + (high[kind] - low[kind]) * pow(f32(key >> 24u) / 255.0, 1.6);
    let radius = height * (spread_low[kind]
        + (spread_high[kind] - spread_low[kind]) * f32(more & 255u) / 255.0);
    if (length(vec2<f32>(x, z)) < clear[kind] + radius
        || max(abs(x), abs(z)) + 1.3 * radius > 15000.0) { return none; }
    let crater = radius * (crater_low[kind]
        + (crater_high[kind] - crater_low[kind]) * f32((more >> 8u) & 255u) / 255.0);
    return BasaltCone(vec2<f32>(x, z), height, radius, crater, kind, key);
}

// Of a size kind, the cone whose base reaches furthest over q (smallest
// fraction of its radius); radius 0 if none covers it.
fn basalt_nearest(q: vec2<f32>, kind: u32) -> BasaltCone {
    var cells = array<f32, 3>(5200.0, 2000.0, 720.0);
    let cell = cells[kind];
    let g = vec2<i32>(floor(q / cell));
    var best = BasaltCone(vec2<f32>(0.0), 0.0, 0.0, 0.0, kind, 0u);
    var best_t = 1.25;
    for (var dz = -1; dz <= 1; dz += 1) {
        for (var dx = -1; dx <= 1; dx += 1) {
            let cone = basalt_cone(kind, g.x + dx, g.y + dz);
            if (cone.radius > 0.0) {
                let t = length(q - cone.centre) / cone.radius;
                if (t < best_t) { best_t = t; best = cone; }
            }
        }
    }
    return best;
}

// The large volcano nearest q: Stromboli under the lookout, the eight real
// Aeolian islands on the horizon (summits as placed by basalt_elevation),
// or a procedural stratovolcano.
fn basalt_large(q: vec2<f32>) -> BasaltCone {
    var best = basalt_nearest(q, 0u);
    var best_t = select(1.25, length(q - best.centre) / best.radius, best.radius > 0.0);
    let stromboli = BasaltCone(vec2<f32>(0.0, 120.0), 900.0, 2600.0, 140.0, 0u, 0x51a7u);
    let ts = length(q - stromboli.centre) / stromboli.radius;
    if (ts < best_t) { best_t = ts; best = stromboli; }
    var bearing = array<f32, 8>(8.0, 52.0, 95.0, 138.0, 182.0, 228.0, 270.0, 318.0);
    var distance = array<f32, 8>(12300.0, 12800.0, 11800.0, 13200.0, 12000.0, 13000.0, 11600.0, 12600.0);
    var turn = array<f32, 8>(0.0, -40.0, 160.0, 90.0, 30.0, 300.0, 200.0, 120.0);
    var size = array<f32, 8>(1.2, 1.0, 0.85, 0.8, 1.15, 0.9, 1.05, 0.95);
    var island = array<u32, 8>(1u, 2u, 0u, 1u, 2u, 0u, 1u, 2u);
    var summit = array<vec4<f32>, 3>(
        vec4<f32>(-12.0, -612.0, 894.0, 2080.0),
        vec4<f32>(-138.0, 262.0, 651.0, 1360.0),
        vec4<f32>(-362.0, 462.0, 745.0, 1820.0));
    for (var i = 0u; i < 8u; i += 1u) {
        let b = radians(bearing[i]);
        let a = radians(turn[i]);
        let s = summit[island[i]];
        let c = distance[i] * vec2<f32>(sin(b), cos(b))
            + size[i] * vec2<f32>(cos(a) * s.x + sin(a) * s.y, -sin(a) * s.x + cos(a) * s.y);
        let cone = BasaltCone(c, size[i] * s.z, size[i] * s.w * 1.15, size[i] * 110.0, 0u,
            hash(i + 0x7aa1u));
        let t = length(q - c) / cone.radius;
        if (t < best_t) { best_t = t; best = cone; }
    }
    return best;
}

fn grain_noise(q: vec3<f32>) -> f32 { return vnoise(q * 1.3); }

// Colour of a volcano's own rock at q, a fraction t of its radius from the
// summit: ash and lava streaks running downhill, red oxidised scoria,
// sulphur and altered clays round the crater, layered walls where steep.
fn basalt_cone_color(q: vec3<f32>, cone: BasaltCone, t: f32, slope: f32, detail: f32) -> vec3<f32> {
    let offset = q.xz - cone.centre;
    let dir = offset / max(length(offset), 1.0e-3);
    let unit = vec3<f32>(f32(cone.key & 255u), f32((cone.key >> 8u) & 255u),
        f32((cone.key >> 20u) & 255u)) / 255.0;
    let rim = cone.crater / cone.radius;
    // Streaks: noise round the cone, stretched downhill.
    let around = 9.0 + 14.0 * unit.z;
    let streak = fbm(vec3<f32>(dir * around, t * 3.0 + unit.x * 40.0), 4);
    let streaks = smoothstep(0.38, 0.62, streak);
    let ash = mix(vec3<f32>(0.075, 0.071, 0.068), vec3<f32>(0.125, 0.118, 0.108), unit.y);
    let dark = vec3<f32>(0.042, 0.039, 0.039);
    var color: vec3<f32>;
    if (cone.kind == 0u) {
        // Stratovolcano: grey ash upper slopes cut by dark lava and
        // scoria tongues, warmer brown ash on the lower apron.
        let apron = mix(vec3<f32>(0.07, 0.058, 0.049), ash, smoothstep(0.75, 0.35, t));
        color = mix(apron, dark, streaks * smoothstep(0.95, 0.3, t) * 0.55);
        let scoria = vec3<f32>(0.12, 0.05, 0.032);
        color = mix(color, scoria, smoothstep(0.62, 0.8, streak) * smoothstep(0.7, 0.2, t) * 0.6);
    } else {
        // Scoria cone: iron-black, brown or oxidised red cinder, varying
        // between cones and in patches over each, the rim most oxidised.
        let mottle = fbm(q * (4.0 / cone.radius) + vec3<f32>(unit * 23.0), 3);
        let oxidised = clamp(smoothstep(0.25, 1.0, unit.x) + (mottle - 0.5) * 1.1
            + 0.3 * smoothstep(0.6, 0.1, t) * unit.x, 0.0, 1.0);
        let red = mix(vec3<f32>(0.1, 0.04, 0.026), vec3<f32>(0.15, 0.06, 0.034), unit.y);
        let brown = vec3<f32>(0.055, 0.04, 0.034);
        let black = vec3<f32>(0.03, 0.027, 0.027);
        let cinder = mix(mix(black, brown, smoothstep(0.0, 0.5, oxidised)), red,
            smoothstep(0.5, 1.0, oxidised));
        color = cinder * mix(0.55, 1.35, streak);
        color = mix(color, ash * 0.8, smoothstep(0.7, 1.0, t) * 0.5);
    }
    // Layered lava and tephra beds exposed on steep walls.
    let h = q.y;
    let beds = sin(h * 0.33 + 4.0 * vnoise(q * 0.012)) * 0.5 + 0.5;
    let wall = smoothstep(0.3, 0.6, slope);
    color *= mix(1.0, 0.6 + 0.75 * smoothstep(0.25, 0.75, beds), wall);
    // Crater: sulphur crusts and white fumarole deposits on the rim and
    // upper inner wall, ochre and orange altered ground in the bowl.
    if (cone.kind < 2u) {
        let u = t / rim;
        let rim_band = smoothstep(1.15, 0.95, u) * smoothstep(0.2, 0.5, u);
        let patches = fbm(q * (16.0 / max(cone.crater, 20.0)) + vec3<f32>(unit * 31.0), 4);
        let sulphur = smoothstep(0.63, 0.66, patches) * rim_band;
        let fumarole = smoothstep(0.7, 0.76, fbm(q * 0.06 + vec3<f32>(3.0, unit.x * 17.0, 5.0), 3))
            * rim_band;
        let bowl = smoothstep(1.0, 0.5, u);
        let altered = mix(vec3<f32>(0.2, 0.09, 0.04), vec3<f32>(0.3, 0.2, 0.08), patches);
        color = mix(color, altered, bowl * 0.6);
        let crust = mix(vec3<f32>(0.42, 0.24, 0.08), vec3<f32>(0.5, 0.46, 0.36), grain_noise(q));
        color = mix(color, crust, sulphur);
        color = mix(color, vec3<f32>(0.4, 0.39, 0.35), fumarole * 0.8);
    }
    return color;
}

// Volcanic field: overlapping lava flows of different ages around cones of
// every size, ash blown over the plain, each cone with its own rock.
fn basalt_floor(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let scale = terrain_scale();
    let q = vec3<f32>(p.x, p.y - terrain_curvature(p.xz), p.z) / scale;
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0) / scale;
    let detail = 1.0 - smoothstep(0.025, 0.2, footprint);
    let slope = 1.0 - max(normal.y, 0.0);
    // Lava sea: warped flow lobes, from fresh glassy black to weathered,
    // ash-dusted brown, with rusty oxidised crusts.
    let warp = vec3<f32>(vnoise(q * 0.0006), 0.0, vnoise(q * 0.0006 + vec3<f32>(31.0, 0.0, 7.0)));
    let age = fbm((q + warp * 1400.0) * 0.0011 + vec3<f32>(0.0, 0.0, 3.0), 3);
    let middle = vnoise(q * 0.12);
    let grain = vnoise(q * 5.0);
    let old_flow = mix(vec3<f32>(0.058, 0.05, 0.044), vec3<f32>(0.085, 0.074, 0.062), middle);
    let aa = mix(vec3<f32>(0.04, 0.037, 0.036), vec3<f32>(0.052, 0.046, 0.043), middle);
    let fresh = vec3<f32>(0.021, 0.02, 0.022) * (0.85 + 0.3 * middle);
    var color = mix(old_flow, aa, smoothstep(0.42, 0.47, age));
    color = mix(color, fresh, smoothstep(0.58, 0.61, age));
    let rust = vec3<f32>(0.11, 0.05, 0.03);
    color = mix(color, rust, smoothstep(0.66, 0.8, vnoise(q * 0.004 + vec3<f32>(5.0, 9.0, 2.0)))
        * 0.45 * (1.0 - smoothstep(0.58, 0.61, age)));
    // Each cone with its own rock, and its tephra blanket on the plain.
    let large = basalt_large(q.xz);
    if (large.radius > 0.0) {
        let t = length(q.xz - large.centre) / large.radius;
        color = mix(color, vec3<f32>(0.078, 0.072, 0.066), smoothstep(1.25, 0.9, t) * 0.5);
        color = mix(color, basalt_cone_color(q, large, t, slope, detail), smoothstep(1.0, 0.7, t));
    }
    // Scoria then spatter cones, which sit on top of everything else.
    for (var kind = 1u; kind < 3u; kind += 1u) {
        let cone = basalt_nearest(q.xz, kind);
        if (cone.radius > 0.0) {
            let t = length(q.xz - cone.centre) / cone.radius;
            // Spatter cones too small for the distant grid stay subtle there.
            let visible = select(1.0, 1.0 - smoothstep(0.2, 0.6, footprint * 40.0 / cone.height), kind == 2u);
            color = mix(color, basalt_cone_color(q, cone, t, slope, detail),
                smoothstep(1.0, 0.75, t) * visible);
        }
    }
    // Glassy, rougher rock on steeper ground reads slightly lighter.
    color *= 1.0 + 0.25 * smoothstep(0.1, 0.4, slope);
    color *= 1.0 + (vnoise(q * 0.7) - 0.5) * 0.35 * (1.0 - smoothstep(0.2, 1.5, footprint));
    color *= 1.0 + (grain - 0.5) * 0.45 * detail;
    return color;
}
// Mirrors DUNE_WIND, DUNE_PAN_LEVEL and DUNE_PANS in terrain.rs.
const DUNE_WIND: f32 = 0.61;
const DUNE_PAN_LEVEL: f32 = 2.0;
fn dune_pan(i: u32) -> vec3<f32> {
    var pans = array<vec3<f32>, 2>(vec3<f32>(842.6, 707.1, 560.0), vec3<f32>(4346.7, 1164.7, 900.0));
    return pans[i];
}

// Width of the dark gap between the nearest two jittered cells: polygonal
// desiccation cracks in the clay of a dry pan.
fn dune_cracks(p: vec2<f32>) -> f32 {
    let g = floor(p);
    let f = p - g;
    var d1 = 8.0;
    var d2 = 8.0;
    for (var j = -1; j <= 1; j += 1) {
        for (var i = -1; i <= 1; i += 1) {
            let o = vec2<f32>(f32(i), f32(j));
            let d = length(o + 0.15 + 0.7 * hash33(vec3<f32>(g + o, 3.0)).xy - f);
            if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
        }
    }
    return d2 - d1;
}

// Namib sand: deep apricot-red on the upper dunes, darker and redder on
// their plinths, fresh yellower sand streaked by grain flows on slip faces,
// dark heavy-mineral lag on gentle ground, pale gravel in the corridors and
// cracked white clay in the pans.
fn dune_floor(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let scale = terrain_scale();
    let q = vec3<f32>(p.x, p.y - terrain_curvature(p.xz), p.z) / scale;
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0) / scale;
    let detail = 1.0 - smoothstep(0.02, 0.15, footprint);
    let slope = 1.0 - max(normal.y, 0.0);
    let broad = vnoise(q * 0.0015 + vec3<f32>(1.0, 7.0, 3.0));
    let middle = vnoise(q * 0.05);
    let rise = smoothstep(6.0, 160.0, q.y + 40.0 * (broad - 0.5));
    let plinth = mix(vec3<f32>(0.3, 0.095, 0.038), vec3<f32>(0.36, 0.12, 0.045), broad);
    let upper = mix(vec3<f32>(0.42, 0.155, 0.06), vec3<f32>(0.47, 0.19, 0.072), broad);
    var sand = mix(plinth, upper, rise) * (0.94 + 0.08 * middle);
    // Slip faces: freshly avalanched, slightly paler sand in long tongues
    // running straight downhill.
    let flat = length(normal.xz);
    if (flat > 1.0e-3) {
        let down = -normal.xz / flat;
        let across = dot(q.xz, vec2<f32>(-down.y, down.x));
        let along = dot(q.xz, down);
        let tongues = fbm(vec3<f32>(across * 0.18, along * 0.012, 5.0), 3);
        let slip = smoothstep(0.32, 0.45, slope);
        sand = mix(sand, sand * vec3<f32>(1.06, 1.12, 1.16) * (0.86 + 0.28 * tongues), slip);
    }
    // Heavy minerals concentrate where wind winnows gentle lower ground.
    let lag = smoothstep(0.6, 0.76, fbm(q * vec3<f32>(0.013, 0.0, 0.004) + vec3<f32>(9.0, 0.0, 2.0), 4))
        * (1.0 - smoothstep(0.04, 0.16, slope)) * (1.0 - smoothstep(10.0, 45.0, q.y));
    sand = mix(sand, vec3<f32>(0.13, 0.065, 0.05), lag * 0.55);
    // Corridors: pale buff gravel plains with scattered dark pebbles.
    let corridor = (1.0 - smoothstep(5.0, 16.0, q.y + 6.0 * (broad - 0.5)))
        * (1.0 - smoothstep(0.03, 0.1, slope));
    let gravel = mix(vec3<f32>(0.39, 0.21, 0.115), vec3<f32>(0.45, 0.28, 0.16), vnoise(q * 0.3));
    let pebbles = smoothstep(0.72, 0.8, vnoise(q * 7.0)) * detail;
    sand = mix(sand, gravel * (1.0 - 0.45 * pebbles), corridor * 0.6);
    // Dry pans: white clay, darker where it was last wet, with polygons of
    // desiccation cracks and curled flakes.
    var pan = 0.0;
    for (var i = 0u; i < 2u; i += 1u) {
        let c = dune_pan(i);
        pan = max(pan, 1.0 - smoothstep(c.z * 1.2, c.z * 1.35 + 140.0, length(q.xz - c.xy)));
    }
    pan *= 1.0 - smoothstep(DUNE_PAN_LEVEL + 0.25, DUNE_PAN_LEVEL + 1.5, q.y);
    if (pan > 0.0) {
        let tone = fbm(q * 0.012 + vec3<f32>(4.0, 0.0, 1.0), 4);
        var clay = mix(vec3<f32>(0.4, 0.35, 0.29), vec3<f32>(0.6, 0.56, 0.49), smoothstep(0.3, 0.7, tone));
        // Damp, darker clay along the shore where the last flood lingered.
        clay *= 0.82 + 0.18 * smoothstep(0.35, 1.0, pan);
        // Cracks antialias by widening and fading as pixels grow: the
        // network still greys the clay where single cracks vanish.
        let fine_width = 0.04 + footprint / 0.9;
        let cracks = (1.0 - smoothstep(0.0, fine_width, dune_cracks(q.xz / 0.9))) * 0.04 / fine_width;
        let wide_width = 0.03 + footprint / 6.0;
        let large = (1.0 - smoothstep(0.0, wide_width, dune_cracks(q.xz / 6.0 + 17.0))) * 0.03 / wide_width;
        clay *= 1.0 - 0.6 * min(cracks, 1.0) - 0.45 * min(large, 1.0);
        clay *= 0.92 + 0.16 * vnoise(q * 2.3) * (1.0 - smoothstep(0.04, 0.25, footprint));
        sand = mix(sand, clay, pan);
    }
    sand *= 1.0 + (vnoise(q * 9.0) - 0.5) * 0.12 * detail;
    return sand;
}
// Wind ripples: sinuous, asymmetric crests across the wind, with defects
// where crests fork or end. Coarse megaripples on the corridors, none on
// slip faces or pans. Each band fades once a pixel covers it.
fn dune_ripples(p: vec3<f32>, wavelength: f32, warp: f32, amplitude: f32) -> vec2<f32> {
    let wind = vec2<f32>(sin(DUNE_WIND), cos(DUNE_WIND));
    let side = vec2<f32>(wind.y, -wind.x);
    let bend = warp * (vnoise(vec3<f32>(p.xz / (9.0 * wavelength), 1.0))
        + 0.35 * vnoise(vec3<f32>(p.xz / (2.5 * wavelength), 7.0)));
    let phase = dot(p.xz, wind) / wavelength + bend + 0.4 * sin(dot(p.xz, side) / (3.0 * wavelength));
    let theta = TAU * phase;
    // Skewed sine: a long gentle stoss slope, a short steep lee.
    let slope = cos(theta + 0.65 * sin(theta)) * (1.0 + 0.65 * cos(theta));
    let patches = smoothstep(0.25, 0.6, vnoise(vec3<f32>(p.xz / (14.0 * wavelength), 3.0)));
    return wind * (slope * amplitude * TAU / wavelength * patches);
}
fn dune_shading_normal(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    let n = to_ground(normal);
    let gentle = 1.0 - smoothstep(0.22, 0.38, 1.0 - n.y);
    let q = vec3<f32>(p.x, p.y - terrain_curvature(p.xz), p.z) / terrain_scale();
    let dry = smoothstep(DUNE_PAN_LEVEL + 0.4, DUNE_PAN_LEVEL + 1.5, q.y);
    var gradient = vec2<f32>(0.0);
    let fine = 1.0 - smoothstep(0.03, 0.07, footprint);
    if (fine > 0.0) { gradient += dune_ripples(p, 0.12, 2.5, 0.006) * fine; }
    let coarse = (1.0 - smoothstep(0.15, 0.4, footprint)) * (1.0 - smoothstep(9.0, 16.0, q.y));
    if (coarse > 0.0) { gradient += dune_ripples(p + 31.0, 0.8, 2.0, 0.03) * coarse; }
    gradient *= gentle * dry;
    return normalize(normal + from_ground(vec3<f32>(-gradient.x, 0.0, -gradient.y)));
}
fn landscape_albedo(ray: Ray, hit: Hit) -> vec3<f32> {
    let p = landscape_point(ray, hit);
    let distance = hit.distance * METRES_PER_AU;
    let n = to_ground(hit.normal);
    if (hit.index == LOCAL_BASE || hit.index == TERRAIN_ID) {
        switch ground_theme() {
            case 1u: { return desert_floor(p, distance, n); }
            case 2u: { return crater_floor(p, distance, n); }
            case 3u: { return alpine_floor(p, distance, n); }
            case 4u: { return basalt_floor(p, distance, n); }
            case 5u: { return dune_floor(p, distance, n); }
            default: { return forest_floor(p, distance, n); }
        }
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
    // Only the forest's rocks carry moss and lichen.
    if (ground_theme() != 0u) { return stone; }
    return mix(stone,vec3<f32>(0.11,0.145,0.037),lichen*0.65);
}
fn landscape_shading_normal(ray: Ray, hit: Hit) -> vec3<f32> {
    if (hit.index != LOCAL_BASE && hit.index != TERRAIN_ID) { return hit.normal; }
    let p = landscape_point(ray,hit);
    let distance = hit.distance*METRES_PER_AU;
    if (ground_theme() == 4u) { return basalt_shading_normal(p, distance, hit.normal); }
    if (ground_theme() == 5u) { return dune_shading_normal(p, distance, hit.normal); }
    if (ground_theme() == 3u) { return alpine_shading_normal(p, distance, hit.normal); }
    let strength = 0.11*(1.0-smoothstep(8.0,65.0,distance));
    let dx = vnoise((p+vec3<f32>(0.025,0.0,0.0))*6.0)-vnoise((p-vec3<f32>(0.025,0.0,0.0))*6.0);
    let dz = vnoise((p+vec3<f32>(0.0,0.0,0.025))*6.0)-vnoise((p-vec3<f32>(0.0,0.0,0.025))*6.0);
    return normalize(hit.normal+from_ground(vec3<f32>(-dx,0.0,-dz))*strength);
}
// Rough lava at several scales, from clinker to blocky flow fronts. Each band
// fades once a pixel covers half its feature size, so distant ground stays
// stable instead of sparkling.
fn basalt_shading_normal(p: vec3<f32>, distance: f32, normal: vec3<f32>) -> vec3<f32> {
    let footprint = distance * 2.0 * settings.g.cam_forward.w / max(settings.g.viewport.y, 1.0);
    var gradient = vec3<f32>(0.0);
    var feature = 0.35;
    var strength = 0.32;
    for (var octave = 0u; octave < 4u; octave += 1u) {
        let fade = 1.0 - smoothstep(0.2 * feature, 0.5 * feature, footprint);
        if (fade > 0.0) {
            let e = 0.12 * feature;
            let k = 1.0 / feature;
            let shift = vec3<f32>(f32(octave) * 17.0, 0.0, f32(octave) * 5.0);
            let dx = vnoise((p + vec3<f32>(e, 0.0, 0.0)) * k + shift) - vnoise((p - vec3<f32>(e, 0.0, 0.0)) * k + shift);
            let dz = vnoise((p + vec3<f32>(0.0, 0.0, e)) * k + shift) - vnoise((p - vec3<f32>(0.0, 0.0, e)) * k + shift);
            gradient += vec3<f32>(-dx, 0.0, -dz) * (strength * fade * 4.0);
        }
        feature *= 6.0;
        strength *= 0.7;
    }
    return normalize(normal + from_ground(gradient));
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
