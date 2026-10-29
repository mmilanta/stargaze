// Measured elevation mipmaps. The atlas follows the cached terrain geometry;
// four map descriptors and nine site transforms are uploaded by volcano.rs.
fn volcano_data(i: u32) -> f32 {
    return ground_data.terrain[terrain_vertex_cache() + terrain_vertices() * terrain_vertex_stride() + i];
}
fn volcano_record(i: u32) -> vec4<f32> {
    return vec4<f32>(volcano_data(i), volcano_data(i+1u), volcano_data(i+2u), volcano_data(i+3u));
}
fn volcano_bilinear(offset: u32, width: u32, uv: vec2<f32>) -> f32 {
    let pixel = uv * f32(width) - 0.5;
    let lo = vec2<i32>(floor(pixel));
    let a = vec2<u32>(clamp(lo, vec2<i32>(0), vec2<i32>(i32(width)-1)));
    let b = vec2<u32>(clamp(lo+vec2<i32>(1), vec2<i32>(0), vec2<i32>(i32(width)-1)));
    let f = fract(pixel);
    return mix(mix(volcano_data(offset+a.y*width+a.x), volcano_data(offset+a.y*width+b.x), f.x),
        mix(volcano_data(offset+b.y*width+a.x), volcano_data(offset+b.y*width+b.x), f.x), f.y);
}
fn volcano_map(map: u32, p: vec2<f32>, footprint: f32) -> f32 {
    let desc = volcano_record(map * 4u);
    let local = p - vec2<f32>(0.0, desc.w);
    let reach = 0.5 * (desc.y - 1.0) * desc.z;
    if (any(abs(local) > vec2<f32>(reach))) { return 0.0; }
    let uv = (local / desc.z + 0.5 * desc.y) / desc.y;
    let lod = max(log2(max(footprint / desc.z, 1.0)), 0.0);
    var offset = u32(desc.x);
    var width = u32(desc.y);
    for (var level = 0u; level < u32(lod) && width > 1u; level += 1u) {
        offset += width * width;
        width = (width + 1u) / 2u;
    }
    let a = volcano_bilinear(offset, width, uv);
    if (width == 1u) { return a; }
    let b = volcano_bilinear(offset + width * width, (width + 1u)/2u, uv);
    return mix(a, b, fract(lod));
}
fn volcano_fine_weight(p: vec2<f32>) -> f32 {
    let desc = volcano_record(4u);
    let local = p - vec2<f32>(0.0, desc.w);
    let reach = 0.5 * (desc.y - 1.0) * desc.z;
    return 1.0 - smoothstep(reach - 72.0, reach, max(abs(local.x), abs(local.y)));
}
fn volcano_height(map: u32, p: vec2<f32>, footprint: f32) -> f32 {
    let coarse = volcano_map(map, p, footprint);
    if (map != 0u) { return coarse; }
    let weight = volcano_fine_weight(p);
    if (weight <= 0.0) { return coarse; }
    return mix(coarse, volcano_map(1u, p, footprint), weight);
}
fn volcano_normal(p: vec3<f32>, distance: f32, fallback: vec3<f32>) -> vec3<f32> {
    let point = p.xz / terrain_scale();
    let footprint = distance * 2.0 * settings.g.cam_forward.w
        / max(settings.g.viewport.y, 1.0) / terrain_scale();
    var best = 1.0;
    var chosen = 0u;
    var local = vec2<f32>(0.0);
    var transform = vec4<f32>(0.0, 0.0, 1.0, 0.0);
    var site_scale = 1.0;
    for (var site = 0u; site < 9u; site += 1u) {
        let frame = volcano_record(16u + site * 8u);
        let info = volcano_record(20u + site * 8u);
        let relative = (point - frame.xy) / info.x;
        let q = vec2<f32>(frame.z * relative.x - frame.w * relative.y,
            frame.w * relative.x + frame.z * relative.y);
        let height = volcano_height(u32(info.y), q, footprint / info.x) * info.x;
        if (height > best) {
            best = height; chosen = u32(info.y); local = q;
            transform = frame; site_scale = info.x;
        }
    }
    if (best <= 1.0) { return fallback; }
    var spacing = volcano_record(chosen * 4u).z;
    if (chosen == 0u) { spacing = mix(spacing, 1.0, volcano_fine_weight(local)); }
    let delta = max(spacing, footprint / site_scale * 0.5);
    let du = vec2<f32>(delta, 0.0);
    let dx = (volcano_height(chosen, local+du, footprint/site_scale)
        - volcano_height(chosen, local-du, footprint/site_scale)) / (2.0*delta);
    let dz = (volcano_height(chosen, local+du.yx, footprint/site_scale)
        - volcano_height(chosen, local-du.yx, footprint/site_scale)) / (2.0*delta);
    // Height and horizontal scale cancel; rotate derivatives back to the site.
    let gradient = vec2<f32>(transform.z * dx + transform.w * dz,
        -transform.w * dx + transform.z * dz);
    let radius = settings.g.ground_up.w;
    let curvature = p.xz / sqrt(max(radius*radius - dot(p.xz,p.xz), 1.0));
    let n = normalize(vec3<f32>(-gradient.x+curvature.x, 1.0, -gradient.y+curvature.y));
    // Dense survey normals must stay in the coarse intersection's hemisphere.
    if (dot(n, fallback) < 0.2) { return normalize(mix(fallback, n, 0.7)); }
    return n;
}
