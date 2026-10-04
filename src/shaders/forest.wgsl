// The cache contains atmospheric radiance only: no star discs, Milky Way or
// constant ambient term. It is rebuilt on scene changes, then shared by pixels.
@compute @workgroup_size(8, 8)
fn cache_surface_sky(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= 16u || id.y >= 8u) { return; }
    let index = id.y * 16u + id.x;
    var light = vec3<f32>(0.0);
    if (settings.g.atmo_params.w > 0.5 && settings.g.ground_counts.y != 0u) {
        let mu = (f32(id.y) + 0.5) / 8.0;
        let phi = TAU * (f32(id.x) + 0.5) / 16.0;
        let r = sqrt(1.0 - mu * mu);
        let dir = from_ground(vec3<f32>(r * cos(phi), mu, r * sin(phi)));
        var rng = hash(index ^ 0x91ce452du);
        light = atmosphere(vec3<f32>(0.0), dir, 1.0e30, &rng).inscatter;
    }
    ground_data.sky_light[index] = vec4<f32>(light, 0.0);
}
fn surface_sky_texel(x: i32, y: i32) -> vec3<f32> {
    return ground_data.sky_light[u32(clamp(y, 0, 7)) * 16u + u32((x + 16) % 16)].rgb;
}
fn surface_sky_lookup(dir: vec3<f32>) -> vec3<f32> {
    let p = to_ground(dir);
    if (p.y <= 0.0) { return vec3<f32>(0.0); }
    let uv = vec2<f32>((atan2(p.z, p.x) / TAU + 1.0) % 1.0 * 16.0 - 0.5,
        p.y * 8.0 - 0.5);
    let cell = vec2<i32>(floor(uv));
    let f = fract(uv);
    return mix(mix(surface_sky_texel(cell.x, cell.y), surface_sky_texel(cell.x+1, cell.y), f.x),
        mix(surface_sky_texel(cell.x, cell.y+1), surface_sky_texel(cell.x+1, cell.y+1), f.x), f.y);
}
fn surface_sky(origin: Ray, normal: vec3<f32>, rng: ptr<function, u32>) -> vec3<f32> {
    let u = random(rng);
    let phi = TAU * random(rng);
    let dir = basis_direction(normal, sqrt(u) * cos(phi), sqrt(u) * sin(phi), sqrt(1.0-u));
    let light = surface_sky_lookup(dir);
    if (max(light.r, max(light.g, light.b)) <= 0.0) { return vec3<f32>(0.0); }
    let ray = Ray(origin.offset, dir, origin.anchor);
    if (closest_hit(ray).index != MISS) { return vec3<f32>(0.0); }
    var transmission = 1.0;
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        let hit = ring_hit(ray, i);
        if (hit.index != MISS) { transmission *= ring_transmission(ray, hit); }
    }
    // Cosine sampling cancels Lambert's cosine/pi and its sampling density.
    return light * transmission;
}

fn trunk_hit(p: vec3<f32>, d: vec3<f32>, e: vec3<f32>) -> vec4<f32> {
    let o = p / e;
    let v = d / e;
    var result = vec4<f32>(0.0, 0.0, 0.0, 1.0e30);
    let a = dot(v.xz, v.xz);
    let b = dot(o.xz, v.xz);
    let c = dot(o.xz, o.xz) - 1.0;
    let disc = b*b-a*c;
    if (a > 1.0e-15 && disc >= 0.0) {
        for (var side = 0u; side < 2u; side += 1u) {
            let t = (-b + select(-sqrt(disc), sqrt(disc), side == 1u)) / a;
            let q = o + v*t;
            if (t > 0.001 && t < result.w && abs(q.y) <= 1.0) {
                result = vec4<f32>(normalize(vec3<f32>(q.x, 0.0, q.z) / e), t);
            }
        }
    }
    if (abs(v.y) > 1.0e-15) {
        for (var side = 0u; side < 2u; side += 1u) {
            let sign_y = select(-1.0, 1.0, side == 1u);
            let t = (sign_y - o.y) / v.y;
            let q = o + v*t;
            if (t > 0.001 && t < result.w && dot(q.xz,q.xz) <= 1.0) {
                result = vec4<f32>(0.0, sign_y, 0.0, t);
            }
        }
    }
    return result;
}
fn conifer_hit(p: vec3<f32>, d: vec3<f32>, e: vec3<f32>) -> vec4<f32> {
    var result = trunk_hit(p + vec3<f32>(0.0, e.y*0.08, 0.0), d,
        vec3<f32>(e.x*0.065, e.y*0.92, e.z*0.065));
    let o = p/e;
    let v = d/e;
    for (var tier = 0u; tier < 6u; tier += 1u) {
        let t = f32(tier);
        let base = -0.6 + t*0.27;
        let height = min(0.66 - t*0.045, 1.0-base);
        let radius = 0.94 - t*0.148;
        let offset = vec2<f32>(sin(t*4.1), cos(t*3.7))*0.022;
        let q = o - vec3<f32>(offset.x, base + height, offset.y);
        let k = radius / height;
        let a = dot(v.xz,v.xz) - k*k*v.y*v.y;
        let b = dot(q.xz,v.xz) - k*k*q.y*v.y;
        let c = dot(q.xz,q.xz) - k*k*q.y*q.y;
        let disc = b*b-a*c;
        if (abs(a) < 1.0e-12 || disc < 0.0) { continue; }
        for (var side = 0u; side < 2u; side += 1u) {
            let distance = (-b + select(-sqrt(disc), sqrt(disc), side == 1u)) / a;
            let h = q + v*distance;
            if (distance <= 0.001 || distance >= result.w || h.y > 0.0 || h.y < -height) { continue; }
            // Gaps in the skirts and small irregular branch tips, continuous
            // in world space so camera rotation does not change the silhouette.
            let angle = atan2(h.z,h.x);
            let skirt = -h.y / height;
            let branches = 0.5+0.5*sin(angle*(9.0+t)+t*3.7);
            let needle = vnoise((p+d*distance)*vec3<f32>(9.0,17.0,9.0)+vec3<f32>(t*7.0));
            if (skirt > 0.76 + 0.19*branches || (skirt > 0.4 && needle < 0.26)) { continue; }
            result = vec4<f32>(normalize(vec3<f32>(h.x,-k*k*h.y,h.z)/e), distance);
        }
    }
    return result;
}
fn grass_hit(p: vec3<f32>, d: vec3<f32>, e: vec3<f32>) -> vec4<f32> {
    var result = vec4<f32>(0.0,0.0,0.0,1.0e30);
    for (var blade = 0u; blade < 9u; blade += 1u) {
        let i = f32(blade);
        let angle = i*2.39996;
        let side = vec3<f32>(cos(angle),0.0,sin(angle));
        let base = side*e.x*0.15 - vec3<f32>(0.0,e.y,0.0);
        let tip = side*e.x*(0.6+0.3*sin(i*3.0)) + vec3<f32>(0.0,e.y*(0.5+0.5*cos(i*1.3)),0.0);
        let width = vec3<f32>(-side.z,0.0,side.x)*0.035;
        let hit = terrain_triangle(p,d,base-width,base+width,tip);
        if (hit.z < result.w) {
            var normal = normalize(cross(width,tip-base));
            if (dot(normal,d)>0.0) { normal = -normal; }
            result = vec4<f32>(normal,hit.z);
        }
    }
    return result;
}
