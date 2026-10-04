// Cached heightfield; no noise generation or erosion in the rendering loop.
// Layout matches terrain.rs: heights, derivatives, min/max quadtree,
// then host-local cached bounds and interleaved vertices/normals.
const TERRAIN_CELLS: u32 = 512u;
const TERRAIN_WIDTH: u32 = 513u;
const TERRAIN_VERTICES: u32 = 263169u;
const TERRAIN_BOUNDS: u32 = 789507u;
const TERRAIN_NODES: u32 = 87381u;
const TERRAIN_CACHE: u32 = TERRAIN_BOUNDS + 2u * TERRAIN_NODES;
const TERRAIN_VERTEX_CACHE: u32 = TERRAIN_CACHE + 6u * TERRAIN_NODES;
const TERRAIN_ID: u32 = 0x80100000u;

fn terrain_scale() -> f32 {
    return clamp(settings.g.ground_up.w / 200000.0, 0.0001, 1.0);
}
fn terrain_coordinate(i: u32) -> f32 {
    let u = 2.0 * f32(i) / f32(TERRAIN_CELLS) - 1.0;
    return u * abs(u) * 16000.0 * terrain_scale();
}
fn terrain_curvature(p: vec2<f32>) -> f32 {
    let r = settings.g.ground_up.w;
    let d2 = dot(p, p);
    return -d2 / (r + sqrt(max(r * r - d2, 0.0)));
}
fn terrain_vertex_uncached(x: u32, z: u32) -> vec3<f32> {
    let p = vec2<f32>(terrain_coordinate(x), terrain_coordinate(z));
    return vec3<f32>(p.x, ground_data.terrain[z * TERRAIN_WIDTH + x] * terrain_scale()
        + terrain_curvature(p), p.y);
}
fn terrain_normal_uncached(x: u32, z: u32) -> vec3<f32> {
    let index = z * TERRAIN_WIDTH + x;
    let p = vec2<f32>(terrain_coordinate(x), terrain_coordinate(z));
    let r = settings.g.ground_up.w;
    let curve_gradient = p / sqrt(max(r * r - dot(p, p), 1.0));
    return normalize(vec3<f32>(
        -ground_data.terrain[TERRAIN_VERTICES + index] + curve_gradient.x, 1.0,
        -ground_data.terrain[2u * TERRAIN_VERTICES + index] + curve_gradient.y));
}

// Returns barycentric weights (b,c) and distance. All leaves share vertices,
// so the triangle tests meet exactly across cell and quadtree boundaries.
fn terrain_triangle(o: vec3<f32>, d: vec3<f32>, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    let ab = b - a;
    let ac = c - a;
    let p = cross(d, ac);
    let determinant = dot(ab, p);
    if (abs(determinant) < 1.0e-12) { return vec3<f32>(0.0, 0.0, 1.0e30); }
    let inverse = 1.0 / determinant;
    let relative = o - a;
    let u = dot(relative, p) * inverse;
    let q = cross(relative, ab);
    let v = dot(d, q) * inverse;
    let distance = dot(ac, q) * inverse;
    if (u < -1.0e-6 || v < -1.0e-6 || u + v > 1.000001 || distance <= 0.001) {
        return vec3<f32>(0.0, 0.0, 1.0e30);
    }
    return vec3<f32>(u, v, distance);
}

struct TerrainBounds { low: vec3<f32>, high: vec3<f32> };

fn terrain_bounds_uncached(node: vec4<u32>) -> TerrainBounds {
    let lo = vec2<f32>(terrain_coordinate(node.y), terrain_coordinate(node.z));
    let hi = vec2<f32>(terrain_coordinate(node.y + node.w), terrain_coordinate(node.z + node.w));
    let near = clamp(vec2<f32>(0.0), lo, hi);
    let far = max(abs(lo), abs(hi));
    let lower = ground_data.terrain[TERRAIN_BOUNDS + node.x * 2u] * terrain_scale()
        + terrain_curvature(far) - 0.02;
    let upper = ground_data.terrain[TERRAIN_BOUNDS + node.x * 2u + 1u] * terrain_scale()
        + terrain_curvature(near) + 0.02;
    return TerrainBounds(vec3<f32>(lo.x, lower, lo.y), vec3<f32>(hi.x, upper, hi.y));
}

fn terrain_vertex(x: u32, z: u32) -> vec3<f32> {
    let i = TERRAIN_VERTEX_CACHE + (z * TERRAIN_WIDTH + x) * 6u;
    return vec3<f32>(ground_data.terrain[i], ground_data.terrain[i + 1u], ground_data.terrain[i + 2u]);
}
fn terrain_normal(x: u32, z: u32) -> vec3<f32> {
    let i = TERRAIN_VERTEX_CACHE + (z * TERRAIN_WIDTH + x) * 6u + 3u;
    return vec3<f32>(ground_data.terrain[i], ground_data.terrain[i + 1u], ground_data.terrain[i + 2u]);
}

// Node is (heap index, first x, first z, number of cells along one edge).
fn terrain_interval(o: vec3<f32>, inverse: vec3<f32>, node: vec4<u32>) -> vec2<f32> {
    let i = TERRAIN_CACHE + node.x * 6u;
    let low = vec3<f32>(ground_data.terrain[i], ground_data.terrain[i + 1u], ground_data.terrain[i + 2u]);
    let high = vec3<f32>(ground_data.terrain[i + 3u], ground_data.terrain[i + 4u], ground_data.terrain[i + 5u]);
    let parallel = abs(inverse) >= vec3<f32>(1.0e29);
    if (any(parallel & ((o < low) | (o > high)))) { return vec2<f32>(1.0e30, -1.0e30); }
    let a = (low - o) * inverse;
    let b = (high - o) * inverse;
    // A ray on a tile boundary and parallel to it remains inside that slab.
    let entry = select(min(a, b), vec3<f32>(-1.0e30), parallel);
    let exit = select(max(a, b), vec3<f32>(1.0e30), parallel);
    return vec2<f32>(max(entry.x, max(entry.y, entry.z)), min(exit.x, min(exit.y, exit.z)));
}

fn terrain_compact_bits(value: u32) -> u32 {
    var x = value & 0x5555u;
    x = (x | (x >> 1u)) & 0x3333u;
    x = (x | (x >> 2u)) & 0x0f0fu;
    return (x | (x >> 4u)) & 0x00ffu;
}

// Occlusion queries stop at any triangle before the light and need no normals.
fn terrain_query(o: vec3<f32>, d: vec3<f32>, initial_limit: f32, occlusion: bool) -> vec4<f32> {
    var result = vec4<f32>(0.0, 0.0, 0.0, 1.0e30);
    var limit = initial_limit;
    let inverse = select(vec3<f32>(-1.0), vec3<f32>(1.0), d >= vec3<f32>(0.0))
        / max(abs(d), vec3<f32>(1.0e-30));
    let root = terrain_interval(o, inverse, vec4<u32>(0u, 0u, 0u, TERRAIN_CELLS));
    if (root.x > root.y || root.y <= 0.001 || root.x >= limit) { return result; }
    // The heap index determines level, tile coordinates and size. Keeping only
    // index + entry distance cuts the traversal stack from 640 to 256 bytes/ray.
    var stack: array<vec2<u32>, 32>;
    stack[0] = vec2<u32>(0u, bitcast<u32>(max(root.x, 0.0)));
    var count = 1u;
    loop {
        if (count == 0u) { break; }
        count -= 1u;
        let pending = stack[count];
        if (bitcast<f32>(pending.y) >= limit) { continue; }
        let level = firstLeadingBit(3u * pending.x + 1u) / 2u;
        let size = TERRAIN_CELLS >> level;
        let morton = pending.x - ((1u << (2u * level)) - 1u) / 3u;
        let node = vec4<u32>(pending.x, terrain_compact_bits(morton) * size,
            terrain_compact_bits(morton >> 1u) * size, size);
        if (node.w == 2u) {
            for (var z = node.z; z < node.z + 2u; z += 1u) {
                for (var x = node.y; x < node.y + 2u; x += 1u) {
                    let a = terrain_vertex(x, z);
                    let b = terrain_vertex(x + 1u, z);
                    let c = terrain_vertex(x, z + 1u);
                    let e = terrain_vertex(x + 1u, z + 1u);
                    let first = terrain_triangle(o, d, a, c, b);
                    let second = terrain_triangle(o, d, e, b, c);
                    if (first.z < limit) {
                        if (occlusion) { return vec4<f32>(0.0, 0.0, 0.0, first.z); }
                        let n = normalize(terrain_normal(x, z) * (1.0 - first.x - first.y)
                            + terrain_normal(x, z + 1u) * first.x + terrain_normal(x + 1u, z) * first.y);
                        result = vec4<f32>(n, first.z);
                        limit = first.z;
                    }
                    if (second.z < limit) {
                        if (occlusion) { return vec4<f32>(0.0, 0.0, 0.0, second.z); }
                        let n = normalize(terrain_normal(x + 1u, z + 1u) * (1.0 - second.x - second.y)
                            + terrain_normal(x + 1u, z) * second.x + terrain_normal(x, z + 1u) * second.y);
                        result = vec4<f32>(n, second.z);
                        limit = second.z;
                    }
                }
            }
        } else {
            var children: array<u32, 4>;
            var distances: array<f32, 4>;
            let half_size = node.w / 2u;
            for (var child = 0u; child < 4u; child += 1u) {
                let child_node = vec4<u32>(node.x * 4u + child + 1u,
                    node.y + (child & 1u) * half_size,
                    node.z + (child >> 1u) * half_size, half_size);
                children[child] = child_node.x;
                let bounds = terrain_interval(o, inverse, child_node);
                distances[child] = select(1.0e30, max(bounds.x, 0.0),
                    bounds.x <= bounds.y && bounds.y > 0.001 && bounds.x < limit);
            }
            // Far first on the stack, so close geometry establishes the limit
            // before traversal considers terrain hidden behind it.
            for (var i = 1u; i < 4u; i += 1u) {
                var j = i;
                loop {
                    if (j == 0u || distances[j - 1u] >= distances[j]) { break; }
                    let distance = distances[j - 1u];
                    let child = children[j - 1u];
                    distances[j - 1u] = distances[j]; children[j - 1u] = children[j];
                    distances[j] = distance; children[j] = child;
                    j -= 1u;
                }
            }
            for (var i = 0u; i < 4u; i += 1u) {
                if (distances[i] < limit) {
                    // Eight internal levels need at most 3*8+1 = 25 slots.
                    stack[count] = vec2<u32>(children[i], bitcast<u32>(distances[i])); count += 1u;
                }
            }
        }
    }
    return result;
}

fn terrain_hit(o: vec3<f32>, d: vec3<f32>, limit: f32) -> vec4<f32> {
    return terrain_query(o, d, limit, false);
}

// Host-local geometry changes only when the host radius changes. Compute exactly
// the same curvature, normals and conservative bounds once, not for every ray.
@compute @workgroup_size(64)
fn cache_terrain_geometry(@builtin(global_invocation_id) id: vec3<u32>) {
    let index = id.x;
    if (index < TERRAIN_NODES) {
        let level = firstLeadingBit(3u * index + 1u) / 2u;
        let size = TERRAIN_CELLS >> level;
        let morton = index - ((1u << (2u * level)) - 1u) / 3u;
        let node = vec4<u32>(index, terrain_compact_bits(morton) * size,
            terrain_compact_bits(morton >> 1u) * size, size);
        let bounds = terrain_bounds_uncached(node);
        let i = TERRAIN_CACHE + index * 6u;
        ground_data.terrain[i] = bounds.low.x;
        ground_data.terrain[i + 1u] = bounds.low.y;
        ground_data.terrain[i + 2u] = bounds.low.z;
        ground_data.terrain[i + 3u] = bounds.high.x;
        ground_data.terrain[i + 4u] = bounds.high.y;
        ground_data.terrain[i + 5u] = bounds.high.z;
    }
    if (index < TERRAIN_VERTICES) {
        let x = index % TERRAIN_WIDTH;
        let z = index / TERRAIN_WIDTH;
        let vertex = terrain_vertex_uncached(x, z);
        let normal = terrain_normal_uncached(x, z);
        let i = TERRAIN_VERTEX_CACHE + index * 6u;
        ground_data.terrain[i] = vertex.x;
        ground_data.terrain[i + 1u] = vertex.y;
        ground_data.terrain[i + 2u] = vertex.z;
        ground_data.terrain[i + 3u] = normal.x;
        ground_data.terrain[i + 4u] = normal.y;
        ground_data.terrain[i + 5u] = normal.z;
    }
}
