//! Cached, continuous terrain: warped multifractal noise, thermal erosion,
//! and a min/max quadtree over a triangulated heightfield. See docs/terrain.md.
use std::sync::OnceLock;

#[path = "volcano.rs"]
mod volcano;
pub use volcano::detail as basalt_detail;

/// Default grid: cells per side. Grounds may use finer grids (`cells`).
pub const CELLS: usize = 512;
#[cfg(test)]
pub const WIDTH: usize = CELLS + 1;
pub const EXTENT: f32 = 16_000.0;
const LEAF: usize = 2;
#[cfg(test)]
pub const NODE_COUNT: usize = node_count(CELLS);
#[cfg(test)]
pub const BOUNDS_OFFSET: usize = bounds_offset(CELLS);
/// The finest grid any ground uses; the GPU buffer is sized for it.
pub const MAX_CELLS: usize = 2048;

pub const fn node_count(cells: usize) -> usize {
    let leaves = cells / LEAF;
    (4 * leaves * leaves - 1) / 3
}
pub const fn bounds_offset(cells: usize) -> usize {
    (cells + 1) * (cells + 1) * if cells > 1024 { 1 } else { 3 }
}

/// Grid cells per side. Crater rims and measured volcanic cliffs need the
/// finer grid to preserve their silhouettes and geometric shadowing.
pub fn cells(theme: crate::ground::Theme) -> usize {
    match theme {
        crate::ground::Theme::Basalt => MAX_CELLS,
        crate::ground::Theme::Craters => CRATER_CELLS,
        _ => CELLS,
    }
}
/// Source heights/derivatives/bounds, plus cached host-local bounds and vertices/normals.
pub const DETAIL_OFFSET: usize =
    bounds_offset(MAX_CELLS) + node_count(MAX_CELLS) * 8 + (MAX_CELLS + 1) * (MAX_CELLS + 1) * 2;
pub const GPU_FLOATS: usize = DETAIL_OFFSET + volcano::DETAIL_FLOATS;
const SEED: u32 = 0x7433_37a9;

pub struct Terrain {
    /// Heights, x/z derivatives, then min/max bounds in breadth-first order.
    pub data: Vec<f32>,
    /// Grid cells per side.
    pub cells: usize,
}

pub fn cached() -> &'static Terrain {
    for_theme(crate::ground::Theme::Forest)
}

pub fn for_theme(theme: crate::ground::Theme) -> &'static Terrain {
    // One lazily generated surface per theme, indexed by its GPU theme id.
    static TERRAIN: [OnceLock<Terrain>; 6] = [const { OnceLock::new() }; 6];
    TERRAIN[theme as usize].get_or_init(|| Terrain::generate(SEED, theme))
}

/// Terrain height under the observer, in metres for a host of `radius`.
/// Zero for grounds with a levelled lookout; the dune crest and crater
/// plain lift the eye onto the surface instead.
pub fn lookout_height(theme: crate::ground::Theme, radius: f64) -> f64 {
    f64::from(for_theme(theme).height(0.0, 0.0) * scale(radius))
}

/// Quadratic spacing gives sub-metre geometry at the observatory, with
/// gradually larger cells towards the edge. Every neighbour shares vertices.
pub fn coordinate_in(i: usize, cells: usize) -> f32 {
    let u = 2.0 * i as f32 / cells as f32 - 1.0;
    u * u.abs() * EXTENT
}

pub fn scale(radius: f64) -> f32 {
    (radius / 200_000.0).clamp(0.0001, 1.0) as f32
}

fn hash(mut v: u32) -> u32 {
    v = (v ^ (v >> 16)).wrapping_mul(0x7feb_352d);
    v = (v ^ (v >> 15)).wrapping_mul(0x846c_a68b);
    v ^ (v >> 16)
}

fn noise(x: f32, z: f32, seed: u32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let x = x - x.floor();
    let z = z - z.floor();
    let fade = |v: f32| v * v * v * (v * (v * 6.0 - 15.0) + 10.0);
    let grad = |dx: i32, dz: i32| {
        let h = hash(
            ((ix + dx) as u32).wrapping_mul(0x8da6_b343)
                ^ ((iz + dz) as u32).wrapping_mul(0xd816_3841)
                ^ seed,
        );
        let u = x - dx as f32;
        let v = z - dz as f32;
        match h & 7 {
            0 => u,
            1 => -u,
            2 => v,
            3 => -v,
            4 => (u + v) * std::f32::consts::FRAC_1_SQRT_2,
            5 => (u - v) * std::f32::consts::FRAC_1_SQRT_2,
            6 => (-u + v) * std::f32::consts::FRAC_1_SQRT_2,
            _ => (-u - v) * std::f32::consts::FRAC_1_SQRT_2,
        }
    };
    let a = grad(0, 0);
    let b = grad(1, 0);
    let c = grad(0, 1);
    let d = grad(1, 1);
    let low = a + (b - a) * fade(x);
    let high = c + (d - c) * fade(x);
    (low + (high - low) * fade(z)) * 1.5
}

fn fbm(mut x: f32, mut z: f32, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut weight = 0.55;
    for octave in 0..octaves {
        sum += weight * noise(x, z, seed.wrapping_add(octave * 71));
        (x, z) = (
            (0.8 * x + 0.6 * z) * 2.03 + 17.2,
            (-0.6 * x + 0.8 * z) * 2.03 + 9.1,
        );
        weight *= 0.48;
    }
    sum
}

fn ridges(mut x: f32, mut z: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 0.55;
    let mut feedback = 1.0;
    for octave in 0..7 {
        let ridge = (1.0 - noise(x, z, seed + octave * 109).abs()).max(0.0);
        let signal = ridge * ridge * feedback;
        sum += amplitude * signal;
        feedback = (signal * 1.9).clamp(0.0, 1.0);
        (x, z) = (
            (0.8 * x + 0.6 * z) * 2.05 + 5.7,
            (-0.6 * x + 0.8 * z) * 2.05 + 13.4,
        );
        amplitude *= 0.49;
    }
    ((sum - 0.28) * 1.45).clamp(0.0, 1.0)
}

fn smooth(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn elevation(x: f32, z: f32, seed: u32) -> f32 {
    let wx = 1_150.0 * fbm(x / 4_700.0 + 5.0, z / 4_700.0, seed, 3);
    let wz = 1_150.0 * fbm(x / 4_700.0, z / 4_700.0 - 11.0, seed ^ 0x59ab, 3);
    let continental =
        (0.65 + 0.5 * fbm(x / 8_000.0 + 7.3, z / 8_000.0 - 3.9, seed, 3)).clamp(0.2, 1.0);
    let ridge = ridges((x + wx) / 2_200.0 + 4.2, (z + wz) / 2_200.0 + 1.7, seed);
    // A meandering valley gives the lookout an open observing direction.
    // Its sides and tributary ridges inherit the warped multifractal surface.
    let valley_center = 850.0 * (z / 2_800.0).sin() + 230.0 * (z / 950.0).sin();
    let valley = smooth(80.0, 2_000.0, (x - valley_center).abs());
    let mountain = 900.0 * continental * ridge.powf(1.35) * valley;
    let floor = 9.0
        + 20.0 * fbm(x / 430.0, z / 430.0, seed ^ 0x3847, 5)
        + 2.0 * fbm(x / 34.0, z / 34.0, seed, 3);
    let clearing = smooth(12.0, 85.0, x.hypot(z));
    let edge = 1.0 - smooth(EXTENT - 2_000.0, EXTENT, x.abs().max(z.abs()));
    (mountain + floor).max(0.0) * clearing * edge
}

fn desert_elevation(x: f32, z: f32, seed: u32) -> f32 {
    // Broad barren alluvial basin and distant broken ridges, not sand dunes.
    let basin = smooth(450.0, 2400.0, (x + 150.0 * (z / 3000.0).sin()).abs());
    let hills = 700.0 * basin * ridges(x / 2800.0 + 2.1, z / 3300.0 + 7.0, seed);
    let plain = 3.0
        + 5.0 * fbm(x / 650.0, z / 650.0, seed, 4)
        + 0.35 * fbm(x / 25.0, z / 25.0, seed ^ 0x1234, 3);
    let edge = 1.0 - smooth(EXTENT - 2000.0, EXTENT, x.abs().max(z.abs()));
    (hills + plain).max(0.0) * smooth(15.0, 90.0, x.hypot(z)) * edge
}

/// Fades every surface to zero at the tile edge, where it meets the sphere.
fn edge(x: f32, z: f32) -> f32 {
    1.0 - smooth(EXTENT - 2000.0, EXTENT, x.abs().max(z.abs()))
}

/// Grid-jittered craters at one size class: a parabolic bowl, a raised rim
/// and ejecta falling off with distance. Returns a height change in metres.
fn craters(x: f32, z: f32, cell: f32, seed: u32) -> f32 {
    let (cx, cz) = ((x / cell).floor() as i32, (z / cell).floor() as i32);
    let mut sum = 0.0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (ix, iz) = (cx + dx, cz + dz);
            let h = hash(
                (ix as u32).wrapping_mul(0x8da6_b343)
                    ^ (iz as u32).wrapping_mul(0xd816_3841)
                    ^ seed,
            );
            let unit = |shift: u32| ((h >> shift) & 1023) as f32 / 1023.0;
            if unit(30) > 0.75 {
                continue; // a quarter of the cells hold no crater
            }
            let centre_x = (ix as f32 + 0.15 + 0.7 * unit(0)) * cell;
            let centre_z = (iz as f32 + 0.15 + 0.7 * unit(10)) * cell;
            let radius = cell * (0.12 + 0.3 * unit(20));
            let distance = (x - centre_x).hypot(z - centre_z);
            let u = distance / radius;
            let depth = 0.2 * radius;
            let rim = 0.1 * radius;
            // Ejecta must reach zero before the crater leaves the 3x3 cells
            // summed here (at least 1.15 cells away), or each cell boundary
            // becomes a straight step in the ground.
            let reach = 1.0 - smooth(0.6 * cell, 1.1 * cell, distance);
            // Inner wall and outer slope meet at the rim crest; round it
            // over the local grid cell size; low sunlight magnifies any
            // residual faceting of a sharper rim into stepped shadows.
            let inner = depth * (u * u - 1.0) + rim * u.powi(4);
            let outer = rim * u.max(1e-3).powi(-3) * reach;
            sum += smooth_min(inner, outer, 1.2 * rounding_width(x, z, CRATER_CELLS));
        }
    }
    sum
}

const CRATER_CELLS: usize = 1024;

fn crater_elevation(x: f32, z: f32, seed: u32) -> f32 {
    // A raised regolith plain lets bowls dig below its level; the observer
    // stands on the plain and is lifted onto it.
    let plain = 60.0
        + 10.0 * fbm(x / 900.0, z / 900.0, seed, 4)
        + 0.6 * fbm(x / 30.0, z / 30.0, seed ^ 0x77, 3) * detail(x, z, 30.0, CRATER_CELLS);
    let pits = craters(x, z, 2600.0, seed ^ 0x1001)
        + craters(x, z, 700.0, seed ^ 0x2002)
        + 0.6 * craters(x, z, 160.0, seed ^ 0x3003);
    // The observer stands on the crest of the south rim of a large, fresh
    // crater: its bowl drops away to the north, the ejecta slope and plain to
    // the south, with nothing blocking the view in any direction.
    let u = x.hypot(z - 950.0) / 950.0;
    let landmark = smooth_min(
        170.0 * (u * u - 1.0) + 75.0 * u.powi(6),
        75.0 * u.max(1e-3).powi(-3),
        1.2 * rounding_width(x, z, CRATER_CELLS),
    );
    // Smaller craters leave the rim itself intact.
    let lookout = smooth(15.0, 90.0, x.hypot(z));
    (plain + landmark + pits * lookout).max(0.0) * edge(x, z)
}

/// Smooth (Catmull-Rom) interpolation of a square grid of little-endian u16
/// decimetre heights at fractional sample coordinates. Bilinear sampling
/// leaves a crease along every grid line, which low sunlight turns into
/// stepped shadows.
fn dem_sample(data: &[u8], base: usize, samples: usize, fx: f32, fz: f32) -> f32 {
    let height = |i: isize, j: isize| {
        let i = i.clamp(0, samples as isize - 1) as usize;
        let j = j.clamp(0, samples as isize - 1) as usize;
        let k = 2 * (base + j * samples + i);
        f32::from(u16::from_le_bytes([data[k], data[k + 1]])) * 0.1
    };
    let weights = |t: f32| {
        let (t2, t3) = (t * t, t * t * t);
        [
            -0.5 * t3 + t2 - 0.5 * t,
            1.5 * t3 - 2.5 * t2 + 1.0,
            -1.5 * t3 + 2.0 * t2 + 0.5 * t,
            0.5 * t3 - 0.5 * t2,
        ]
    };
    let (i, j) = (fx.floor() as isize, fz.floor() as isize);
    let (wx, wz) = (weights(fx - i as f32), weights(fz - j as f32));
    let mut sum = 0.0;
    for (b, wz) in wz.iter().enumerate() {
        for (a, wx) in wx.iter().enumerate() {
            sum += wx * wz * height(i + a as isize - 1, j + b as isize - 1);
        }
    }
    sum.max(0.0)
}

/// Real elevation of the Southern Patagonian Ice Field around the lookout:
/// 641 x 641 decimetre samples, 50 m apart, x east and z north. See
/// assets/terrain/README.md and scripts/patagonia_dem.py.
static PATAGONIA: &[u8] = include_bytes!("../assets/terrain/patagonia-ice-field.u16");
const PATAGONIA_SAMPLES: usize = 641;
const PATAGONIA_SPACING: f32 = 50.0;

fn patagonia(x: f32, z: f32) -> f32 {
    let half = (PATAGONIA_SAMPLES - 1) as f32 * 0.5;
    let fx = (x / PATAGONIA_SPACING + half).clamp(0.0, 2.0 * half);
    let fz = (z / PATAGONIA_SPACING + half).clamp(0.0, 2.0 * half);
    dem_sample(PATAGONIA, 0, PATAGONIA_SAMPLES, fx, fz)
}

fn alpine_elevation(x: f32, z: f32, _seed: u32) -> f32 {
    // Real terrain, unexaggerated, measured from the ice at the lookout. The
    // host sphere stands in for the ice field: ground below the ice level
    // (valleys beyond the peaks) is levelled to it, and the tile edge fades
    // only over its last kilometre so Fitz Roy (~15 km east) stays.
    let relative = (patagonia(x, z) - patagonia(0.0, 0.0)).max(0.0);
    let fade = 1.0 - smooth(EXTENT - 1_000.0, EXTENT, x.abs().max(z.abs()));
    relative * fade * smooth(12.0, 85.0, x.hypot(z))
}

/// Real elevation of six Aeolian volcanoes, above sea level only: 320 x 320
/// decimetre samples each, 25 m apart, x east and z north, in this order:
/// Stromboli, Alicudi, Filicudi, Salina, Panarea, Vulcano. See
/// assets/terrain/README.md and scripts/aeolian_dem.py.
static AEOLIAN: &[u8] = include_bytes!("../assets/terrain/aeolian-islands.u16");
const AEOLIAN_SAMPLES: usize = 320;
const AEOLIAN_SPACING: f32 = 25.0;

/// Height of island `index` at island-local metres, fading to sea level over
/// the last 300 m of its grid so nothing ends in a cut.
fn aeolian(index: usize, x: f32, z: f32) -> f32 {
    let half = (AEOLIAN_SAMPLES - 1) as f32 * 0.5;
    let reach = half * AEOLIAN_SPACING;
    if x.abs() >= reach || z.abs() >= reach {
        return 0.0;
    }
    let base = index * AEOLIAN_SAMPLES * AEOLIAN_SAMPLES;
    let fx = x / AEOLIAN_SPACING + half;
    let fz = z / AEOLIAN_SPACING + half;
    dem_sample(AEOLIAN, base, AEOLIAN_SAMPLES, fx, fz)
        * (1.0 - smooth(reach - 300.0, reach, x.abs().max(z.abs())))
}

/// INGV's 2023 airborne LiDAR survey, downsampled from 50 cm to a 5 m
/// whole-island grid and a nested 1 m summit grid. Decimetres, little-endian
/// u16; the origin is the actual Pizzo summit. No synthetic cone or crater.
/// See assets/terrain/README.md and scripts/stromboli_dem.py.
static STROMBOLI: &[u8] = include_bytes!("../assets/terrain/stromboli-2023.u16");
const STROMBOLI_SAMPLES: usize = 1281;
const SUMMIT_SAMPLES: usize = 1025;

fn stromboli(x: f32, z: f32) -> f32 {
    let reach = x.abs().max(z.abs());
    if reach >= 3200.0 {
        return 0.0;
    }
    let coarse = dem_sample(
        STROMBOLI,
        0,
        STROMBOLI_SAMPLES,
        x / 5.0 + 640.0,
        z / 5.0 + 640.0,
    );
    let fine_z = z - 250.0;
    let fine_reach = x.abs().max(fine_z.abs());
    if fine_reach >= 512.0 {
        return coarse;
    }
    let fine = dem_sample(
        STROMBOLI,
        STROMBOLI_SAMPLES * STROMBOLI_SAMPLES,
        SUMMIT_SAMPLES,
        x + 512.0,
        fine_z + 512.0,
    );
    coarse + (fine - coarse) * (1.0 - smooth(440.0, 512.0, fine_reach))
}

fn basalt_elevation(x: f32, z: f32, seed: u32) -> f32 {
    // The real Pizzo summit overlooks the active vent complex to the NNW.
    // Reuse surveyed islands on the horizon, without blending their gullies,
    // asymmetric ridges and collapse scars back into mathematical cones.
    let mut volcanoes = stromboli(x, z);
    for (index, bearing, distance, turn, size) in volcano::SITES {
        let b = bearing.to_radians();
        let (dx, dz) = (
            (x - distance * b.sin()) / size,
            (z - distance * b.cos()) / size,
        );
        let (c, s) = (turn.to_radians().cos(), turn.to_radians().sin());
        let (lx, lz) = (c * dx - s * dz, s * dx + c * dz);
        let real = if index == 0 {
            stromboli(lx, lz)
        } else {
            aeolian(index, lx, lz)
        };
        volcanoes = volcanoes.max(size * real);
    }
    // The lava sea: flat at large scale, blocky and broken up close by.
    let rough = 4.0 * fbm(x / 600.0, z / 600.0, seed, 3)
        + 2.5 * fbm(x / 45.0, z / 45.0, seed ^ 0xbeef, 3).abs() * detail(x, z, 45.0, MAX_CELLS)
        + 0.8 * fbm(x / 9.0, z / 9.0, seed ^ 0x0f, 2) * detail(x, z, 9.0, MAX_CELLS);
    let sea = 20.0 + rough * (1.0 - smooth(20.0, 100.0, volcanoes));
    // Fade only over the last kilometre, so the horizon cones stay whole.
    let fade = 1.0 - smooth(EXTENT - 1_000.0, EXTENT, x.abs().max(z.abs()));
    (sea + volcanoes).max(0.0) * fade
}

/// Sossusvlei pans (centre east, centre north, radius), in metres from the
/// summit of Big Daddy: flat ground at DUNE_PAN_LEVEL between the dunes.
pub const DUNE_PAN_LEVEL: f32 = 2.0;
pub const DUNE_PANS: [[f32; 3]; 2] = [
    // Deadvlei at the foot of Big Daddy's slip face, north-east.
    [842.6, 707.1, 560.0],
    // Sossusvlei further east.
    [4_346.7, 1_164.7, 900.0],
];

/// About 1.5 terrain cells at (x, z): the width over which sharp crests and
/// rims are rounded. A knife edge aliases on the coarse distant grid (40-70 m
/// cells at a few km) into a zigzag shadow line; the fine grid near the
/// lookout keeps edges there sharp.
fn rounding_width(x: f32, z: f32, cells: usize) -> f32 {
    (1.5 * cell_size(x, z, cells)).max(1.0)
}

/// Cell size of the quadratic grid at (x, z): d(coordinate)/di per axis.
fn cell_size(x: f32, z: f32, cells: usize) -> f32 {
    4.0 * EXTENT / cells as f32 * (x.abs().max(z.abs()) / EXTENT).sqrt()
}

/// Weight for surface detail of the given feature size: 1 where the grid
/// resolves it, fading to 0 where cells exceed half the feature. Unresolved
/// bumps alias into random per-row heights, which grazing light turns into
/// stripes along the grid rows.
fn detail(x: f32, z: f32, feature: f32, cells: usize) -> f32 {
    1.0 - smooth(0.25 * feature, 0.5 * feature, cell_size(x, z, cells))
}

/// Distance from a crest, rounded over `rounding_width`.
fn rounded(distance: f32, x: f32, z: f32) -> f32 {
    let width = rounding_width(x, z, CELLS);
    distance.hypot(width) - width
}

/// Smooth minimum: rounds the sharp ridge where two height profiles meet
/// (e.g. a crater's inner wall and outer slope) over a height band `k`.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

/// A sharp sand ridge from `start` along `bearing` (radians from north
/// towards east): height falls along its length, flanks fall linearly at
/// the given slopes (a steep slip face on the left, a windward ramp on the
/// right) and the crest snakes sideways.
#[allow(clippy::too_many_arguments)]
fn dune_arm(
    x: f32,
    z: f32,
    start: (f32, f32),
    bearing: f32,
    length: f32,
    height: f32,
    slopes: (f32, f32),
    phase: f32,
) -> f32 {
    let (dir_x, dir_z) = (bearing.sin(), bearing.cos());
    let (dx, dz) = (x - start.0, z - start.1);
    let along = dx * dir_x + dz * dir_z;
    if along < -400.0 || along > length + 400.0 {
        return 0.0;
    }
    let t = (along / length).clamp(0.0, 1.0);
    let wander = 140.0 * (t * 5.0 + phase).sin() * t;
    let across = dx * dir_z - dz * dir_x - wander;
    let crest =
        height * (1.0 - t).powf(0.8) - 0.55 * (along - length).max(0.0) - 0.55 * (-along).max(0.0);
    let slope = if across < 0.0 { slopes.0 } else { slopes.1 };
    (crest - slope * rounded(across, x, z)).max(0.0)
}

/// A star dune: three to five sinuous arms radiating from a high centre.
fn star_dune(x: f32, z: f32, cx: f32, cz: f32, height: f32, seed: u32) -> f32 {
    let unit = |shift: u32| ((hash(seed.wrapping_add(shift)) >> 8) as f32) / 16_777_216.0;
    let arms = 3 + (unit(1) * 2.99) as u32;
    let spin = unit(2) * std::f32::consts::TAU;
    let mut h = (height - 0.5 * rounded((x - cx).hypot(z - cz), x, z)).max(0.0);
    for k in 0..arms {
        let bearing =
            spin + k as f32 * std::f32::consts::TAU / arms as f32 + (unit(10 + k) - 0.5) * 0.9;
        let length = 700.0 + 1_100.0 * unit(20 + k);
        h = h.max(dune_arm(
            x,
            z,
            (cx, cz),
            bearing,
            length,
            height,
            (0.62, 0.3),
            unit(30 + k) * 6.0,
        ));
    }
    h
}

fn dune_elevation(x: f32, z: f32, seed: u32) -> f32 {
    // Big Daddy above Deadvlei, Sossusvlei (Namibia). The observer stands on
    // its knife-edge summit, ~325 m above the white clay pan at the foot of
    // the north-east slip face; red star dunes with sharp, sinuous arms and
    // flat corridors run to the horizon all round.
    let deg = |d: f32| d.to_radians();
    // Big Daddy: a long crest running north-west / south-east through the
    // summit, slip face to the north-east, plus a south-western arm.
    let summit = (0.0, 0.0);
    let mut h = (325.0 - 0.5 * rounded(x.hypot(z), x, z)).max(0.0);
    for (bearing, length, height, phase) in [
        (deg(320.0), 3_200.0, 325.0, 0.0),
        (deg(140.0), 2_600.0, 325.0, 2.0),
        (deg(225.0), 2_200.0, 290.0, 4.0),
    ] {
        // Left of a north-west arm is the south-west (windward) side; keep the
        // steep face towards Deadvlei by choosing the slopes per arm.
        let slopes = if bearing.sin() < 0.0 {
            (0.3, 0.62)
        } else {
            (0.62, 0.3)
        };
        h = h.max(dune_arm(
            x, z, summit, bearing, length, height, slopes, phase,
        ));
    }
    // The surrounding dune sea: star dunes on a jittered grid.
    let cell = 1_900.0;
    let (gx, gz) = ((x / cell).floor() as i32, (z / cell).floor() as i32);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (ix, iz) = (gx + dx, gz + dz);
            if (ix, iz) == (0, 0)
                || (ix, iz) == (-1, -1)
                || (ix, iz) == (-1, 0)
                || (ix, iz) == (0, -1)
            {
                continue; // Big Daddy owns the cells around the summit
            }
            let key = hash(
                (ix as u32).wrapping_mul(0x8da6_b343)
                    ^ (iz as u32).wrapping_mul(0xd816_3841)
                    ^ seed,
            );
            let unit = |shift: u32| ((key >> shift) & 255) as f32 / 255.0;
            let cx = (ix as f32 + 0.2 + 0.6 * unit(0)) * cell;
            let cz = (iz as f32 + 0.2 + 0.6 * unit(8)) * cell;
            h = h.max(star_dune(x, z, cx, cz, 110.0 + 160.0 * unit(16), key));
        }
    }
    // Corridors between the stars fill with lower, sinuous linear ridges.
    // Rounded crests: sharp cusps alias into sawtooth teeth on the grid.
    let warp = 450.0 * fbm(x / 1_600.0, z / 1_600.0 + 9.0, seed, 3)
        + 120.0 * fbm(x / 400.0, z / 400.0 - 3.0, seed ^ 0x15, 2);
    let wave = 0.5 + 0.5 * ((x * 0.35 + z * 0.94 + warp) / 680.0 * std::f32::consts::TAU).cos();
    let strength = (0.5 + 0.5 * fbm(x / 2_500.0, z / 2_500.0, seed ^ 0x3c, 2)).clamp(0.0, 1.0);
    let ridge = 40.0 * wave * wave * strength;
    let floor = 7.0
        + 3.0 * fbm(x / 900.0, z / 900.0, seed, 3)
        + 0.5 * fbm(x / 40.0, z / 40.0, seed ^ 0x6d, 3) * detail(x, z, 40.0, CELLS);
    let mut height = h.max(floor + ridge.max(0.0));
    // Flat pans where the Tsauchab floods end, with irregular shores; dunes
    // rise straight out of them.
    for [px, pz, radius] in DUNE_PANS {
        let shore = radius * (1.0 + 0.3 * fbm(x / 350.0 + px, z / 350.0 + pz, seed ^ 0x77, 3));
        let flat = smooth(shore, shore + 140.0, (x - px).hypot(z - pz));
        height = height * flat + DUNE_PAN_LEVEL * (1.0 - flat);
    }
    height.max(0.0) * edge(x, z)
}

/// Talus relaxation transports material between neighbouring samples. The
/// transfer conserves volume even though the grid cells have different areas.
fn erode(heights: &mut [f32], coordinates: &[f32], iterations: usize) {
    let n = coordinates.len();
    let widths: Vec<f32> = (0..n)
        .map(|i| (coordinates[(i + 1).min(n - 1)] - coordinates[i.saturating_sub(1)]) * 0.5)
        .collect();
    let area: Vec<f32> = (0..n * n).map(|i| widths[i % n] * widths[i / n]).collect();
    let mut change = vec![0.0; heights.len()];
    for _ in 0..iterations {
        change.fill(0.0);
        for z in 0..n {
            for x in 0..n {
                let i = z * n + x;
                for (j, distance) in [
                    (
                        (z * n + (x + 1).min(n - 1)),
                        coordinates[(x + 1).min(n - 1)] - coordinates[x],
                    ),
                    (
                        ((z + 1).min(n - 1) * n + x),
                        coordinates[(z + 1).min(n - 1)] - coordinates[z],
                    ),
                ] {
                    if i == j {
                        continue;
                    }
                    let delta = heights[i] - heights[j];
                    let excess = (delta.abs() - 0.65 * distance).max(0.0);
                    let volume = 0.18 * excess / (1.0 / area[i] + 1.0 / area[j]) * delta.signum();
                    change[i] -= volume / area[i];
                    change[j] += volume / area[j];
                }
            }
        }
        for (h, delta) in heights.iter_mut().zip(&change) {
            *h += delta;
        }
    }
}

impl Terrain {
    fn generate(seed: u32, theme: crate::ground::Theme) -> Self {
        let started = std::time::Instant::now();
        let cells = cells(theme);
        let width = cells + 1;
        let coordinates: Vec<_> = (0..width).map(|i| coordinate_in(i, cells)).collect();
        let mut heights: Vec<_> = (0..width * width)
            .map(|i| {
                let (x, z) = (coordinates[i % width], coordinates[i / width]);
                match theme {
                    crate::ground::Theme::Forest => elevation(x, z, seed),
                    crate::ground::Theme::Desert => desert_elevation(x, z, seed),
                    crate::ground::Theme::Craters => crater_elevation(x, z, seed),
                    crate::ground::Theme::Alpine => alpine_elevation(x, z, seed),
                    crate::ground::Theme::Basalt => basalt_elevation(x, z, seed),
                    crate::ground::Theme::Dunes => dune_elevation(x, z, seed),
                }
            })
            .collect();
        // Talus erosion suits the gently levelled forest and desert lookouts.
        // Elsewhere the centre rows and columns (0.24 m wide) would collect
        // material into thin ridges running out to the horizon.
        if matches!(
            theme,
            crate::ground::Theme::Forest | crate::ground::Theme::Desert
        ) {
            erode(&mut heights, &coordinates, 18);
        }
        let offset = bounds_offset(cells);
        let mut data = vec![0.0; offset + 2 * node_count(cells)];
        data[..width * width].copy_from_slice(&heights);
        // Fine grids reconstruct vertex derivatives in the one-time GPU
        // cache pass, keeping the volcano mesh and atlas below 128 MiB.
        for z in 0..if cells > 1024 { 0 } else { width } {
            for x in 0..width {
                let i = z * width + x;
                let (x0, x1) = (x.saturating_sub(1), (x + 1).min(cells));
                let (z0, z1) = (z.saturating_sub(1), (z + 1).min(cells));
                data[width * width + i] = (heights[z * width + x1] - heights[z * width + x0])
                    / (coordinates[x1] - coordinates[x0]);
                data[2 * width * width + i] = (heights[z1 * width + x] - heights[z0 * width + x])
                    / (coordinates[z1] - coordinates[z0]);
            }
        }
        struct Grid {
            width: usize,
            offset: usize,
        }
        fn bounds(
            grid: &Grid,
            data: &mut [f32],
            node: usize,
            x: usize,
            z: usize,
            size: usize,
        ) -> (f32, f32) {
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            if size == LEAF {
                for row in z..=z + size {
                    for column in x..=x + size {
                        let h = data[row * grid.width + column];
                        lo = lo.min(h);
                        hi = hi.max(h);
                    }
                }
            } else {
                for corner in 0..4 {
                    let (a, b) = bounds(
                        grid,
                        data,
                        node * 4 + corner + 1,
                        x + (corner & 1) * size / 2,
                        z + (corner >> 1) * size / 2,
                        size / 2,
                    );
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
            }
            data[grid.offset + 2 * node] = lo;
            data[grid.offset + 2 * node + 1] = hi;
            (lo, hi)
        }
        bounds(&Grid { width, offset }, &mut data, 0, 0, 0, cells);
        log::info!(
            "terrain generated: {} triangles, {:.2} MiB, {:?}",
            cells * cells * 2,
            data.len() as f64 * 4.0 / (1024.0 * 1024.0),
            started.elapsed()
        );
        Self { data, cells }
    }

    /// Match the two triangles in each GPU cell, for grounded props.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let (cells, width) = (self.cells, self.cells + 1);
        let index = |v: f32| {
            let u = v.signum() * (v.abs() / EXTENT).sqrt();
            (((u + 1.0) * 0.5 * cells as f32).floor() as usize).min(cells - 1)
        };
        let coordinate = |i| coordinate_in(i, cells);
        let (ix, iz) = (index(x), index(z));
        let u = ((x - coordinate(ix)) / (coordinate(ix + 1) - coordinate(ix))).clamp(0.0, 1.0);
        let v = ((z - coordinate(iz)) / (coordinate(iz + 1) - coordinate(iz))).clamp(0.0, 1.0);
        let a = self.data[iz * width + ix];
        let b = self.data[iz * width + ix + 1];
        let c = self.data[(iz + 1) * width + ix];
        let d = self.data[(iz + 1) * width + ix + 1];
        if u + v <= 1.0 {
            a + (b - a) * u + (c - a) * v
        } else {
            d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_surface_is_finite_continuous_and_bounds_enclose_every_vertex() {
        for terrain in [cached(), for_theme(crate::ground::Theme::Desert)] {
            assert_eq!(terrain.data.len(), BOUNDS_OFFSET + 2 * NODE_COUNT);
            assert!(terrain.data.iter().all(|v| v.is_finite()));
            assert_eq!(terrain.height(0.0, 0.0), 0.0);
            let heights = &terrain.data[..WIDTH * WIDTH];
            assert!(heights.iter().all(|h| *h >= -1e-5));
            assert!(heights.iter().copied().fold(0.0, f32::max) > 500.0);
            fn check(t: &Terrain, node: usize, x: usize, z: usize, size: usize) {
                let lo = t.data[BOUNDS_OFFSET + 2 * node];
                let hi = t.data[BOUNDS_OFFSET + 2 * node + 1];
                for row in z..=z + size {
                    for col in x..=x + size {
                        let h = t.data[row * WIDTH + col];
                        assert!(h >= lo && h <= hi);
                    }
                }
                if size > LEAF {
                    for corner in 0..4 {
                        check(
                            t,
                            node * 4 + corner + 1,
                            x + (corner & 1) * size / 2,
                            z + (corner >> 1) * size / 2,
                            size / 2,
                        );
                    }
                }
            }
            check(terrain, 0, 0, 0, CELLS);
            for i in 0..WIDTH {
                assert!(heights[i].abs() < 1e-6 && heights[CELLS * WIDTH + i].abs() < 1e-6);
                assert!(heights[i * WIDTH].abs() < 1e-6 && heights[i * WIDTH + CELLS].abs() < 1e-6);
            }
            for (x, z) in [(100.0, 230.0), (2_000.0, -3_000.0), (-4_000.0, 1_000.0)] {
                assert!((terrain.height(x + 0.001, z) - terrain.height(x - 0.001, z)).abs() < 0.1);
            }
        }
    }
    #[test]
    fn explore_grounds_are_finite_bounded_and_meet_the_sphere_at_the_edge() {
        use crate::ground::Theme;
        for theme in [Theme::Craters, Theme::Alpine, Theme::Basalt, Theme::Dunes] {
            let terrain = for_theme(theme);
            let (cells, width) = (terrain.cells, terrain.cells + 1);
            assert_eq!(
                terrain.data.len(),
                bounds_offset(cells) + 2 * node_count(cells)
            );
            let heights = &terrain.data[..width * width];
            assert!(terrain.data.iter().all(|v| v.is_finite()), "{theme:?}");
            assert!(heights.iter().all(|h| *h >= -1e-5), "{theme:?}");
            let root_lo = terrain.data[bounds_offset(cells)];
            let root_hi = terrain.data[bounds_offset(cells) + 1];
            assert!(heights.iter().all(|h| *h >= root_lo && *h <= root_hi));
            for i in 0..width {
                assert!(heights[i].abs() < 1e-6 && heights[i * width + cells].abs() < 1e-6);
            }
            for (x, z) in [(100.0, 230.0), (2_000.0, -3_000.0)] {
                assert!((terrain.height(x + 0.001, z) - terrain.height(x - 0.001, z)).abs() < 0.1);
            }
        }
        // Levelled lookouts keep the eye at its configured height; the dune
        // sea and crater plain lift it onto the surface.
        assert_eq!(lookout_height(Theme::Alpine, 6.371e6), 0.0);
        assert!(lookout_height(Theme::Basalt, 6.371e6) > 200.0);
        assert!(lookout_height(Theme::Dunes, 6.371e6) > 300.0);
        assert!(lookout_height(Theme::Craters, 6.371e6) > 20.0);
        assert!(for_theme(Theme::Alpine).data[BOUNDS_OFFSET + 1] > 1_500.0);
        assert_eq!(for_theme(Theme::Craters).cells, CRATER_CELLS);
        assert_eq!(for_theme(Theme::Basalt).cells, MAX_CELLS);
    }

    #[test]
    fn erosion_moves_material_downhill_and_conserves_volume() {
        let coordinates = [0.0, 1.0, 2.0, 3.0, 4.0];
        let mut heights = vec![0.0; 25];
        heights[12] = 10.0;
        erode(&mut heights, &coordinates, 12);
        assert!(heights[12] < 10.0 && heights[11] > 0.0);
        let mut volume = 0.0;
        for (i, h) in heights.iter().enumerate() {
            assert!(*h >= 0.0);
            let wx = if i % 5 == 0 || i % 5 == 4 { 0.5 } else { 1.0 };
            let wz = if i / 5 == 0 || i / 5 == 4 { 0.5 } else { 1.0 };
            volume += h * wx * wz;
        }
        assert!((volume - 10.0).abs() < 1e-4);
        assert_ne!(
            elevation(1_230.0, 4_560.0, SEED),
            elevation(1_230.0, 4_560.0, SEED + 1)
        );
    }
}
