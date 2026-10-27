//! Cached, continuous terrain: warped multifractal noise, thermal erosion,
//! and a min/max quadtree over a triangulated heightfield. See docs/terrain.md.
use std::sync::OnceLock;

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
pub const MAX_CELLS: usize = 1024;

pub const fn node_count(cells: usize) -> usize {
    let leaves = cells / LEAF;
    (4 * leaves * leaves - 1) / 3
}
pub const fn bounds_offset(cells: usize) -> usize {
    (cells + 1) * (cells + 1) * 3
}

/// Grid cells per side for a ground. The airless crater field has no
/// atmosphere to pay for, so it spends the time on a grid twice as fine.
pub fn cells(theme: crate::ground::Theme) -> usize {
    // The volcano field spends it on gullies, craters and small cones, the
    // dune sea on knife-edge crests and the small dunes riding on the large,
    // the ice field on crevasses and granite spires.
    if matches!(
        theme,
        crate::ground::Theme::Craters
            | crate::ground::Theme::Basalt
            | crate::ground::Theme::Dunes
            | crate::ground::Theme::Alpine
    ) {
        MAX_CELLS
    } else {
        CELLS
    }
}
/// Source heights/derivatives/bounds, plus cached host-local bounds and vertices/normals.
pub const GPU_FLOATS: usize =
    bounds_offset(MAX_CELLS) + node_count(MAX_CELLS) * 8 + (MAX_CELLS + 1) * (MAX_CELLS + 1) * 6;
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
/// Zero for grounds with a levelled lookout (the forest and desert); the
/// dune crest, crater plain, volcano summit and nunatak lift the eye onto
/// the surface instead.
pub fn lookout_height(theme: crate::ground::Theme, radius: f64) -> f64 {
    f64::from(for_theme(theme).height(0.0, 0.0) * scale(radius))
}

/// Quadratic spacing gives sub-metre geometry at the observatory, with
/// gradually larger cells towards the edge. Every neighbour shares vertices.
#[cfg(test)]
pub fn coordinate(i: usize) -> f32 {
    coordinate_in(i, CELLS)
}

/// `coordinate` for a grid of `cells` per side.
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

const CRATER_CELLS: usize = MAX_CELLS;

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

const ALPINE_CELLS: usize = MAX_CELLS;

/// Direction the ice flows towards (radians east of north): west-south-west,
/// off the ice-field plateau. Crevasses open across it (drawn by the
/// shader); snow drifts and sastrugi run along it. Mirrored by ICE_FLOW in ground.wgsl.
pub const ICE_FLOW: f32 = 4.4;

/// Height and base radius of the nunatak under the lookout: a rocky peak
/// rising out of the ice. Mirrored by lookout_hill in ground.wgsl, which
/// measures the ice from its slopes and treats it as mountain.
pub const LOOKOUT_HILL: (f32, f32) = (180.0, 700.0);

/// Concave all round, steepest just below the small summit, so from the
/// top the eye sees straight down onto the ice field instead of over a
/// rounded shoulder.
fn lookout_hill(r: f32) -> f32 {
    let u = (r.hypot(30.0) / LOOKOUT_HILL.1).min(1.0);
    LOOKOUT_HILL.0 * (1.0 - u) * (1.0 - u)
}

fn alpine_elevation(x: f32, z: f32, seed: u32) -> f32 {
    // Real terrain, unexaggerated, measured from the ice at the lookout. The
    // host sphere stands in for the ice field: ground below the ice level
    // (valleys beyond the peaks) is levelled to it, and the tile edge fades
    // only over its last kilometre so Fitz Roy (~15 km east) stays.
    let cells = ALPINE_CELLS;
    let relative = (patagonia(x, z) - patagonia(0.0, 0.0)).max(0.0);
    // The 50 m samples round granite spires into domes: ridged crags and
    // broken rock restore the jagged detail on the mountains only.
    let r = x.hypot(z);
    let hill = lookout_hill(r);
    let rock = smooth(20.0, 200.0, relative);
    let crags = 70.0 * ridges(x / 380.0 + 1.3, z / 380.0 - 2.1, seed ^ 0xa1).powf(1.5) - 18.0;
    let broken = 7.0 * fbm(x / 70.0, z / 70.0, seed ^ 0xa2, 3);
    // Ribs and gullies cut into the nunatak, never rising above the summit.
    let ribs = smooth(10.0, 80.0, hill)
        * (16.0 * ridges(x / 140.0 + 7.0, z / 140.0, seed ^ 0xa5) - 13.0)
        * detail(x, z, 60.0, cells);
    let mountains = relative
        + ribs
        + rock * (crags * detail(x, z, 180.0, cells) + broken * detail(x, z, 70.0, cells));
    // The ice: broad swells and drifts elongated along the flow. Crevasses
    // are far narrower than the distant grid cells, so the shader draws them.
    let (fx, fz) = (ICE_FLOW.sin(), ICE_FLOW.cos());
    let (along, across) = (x * fx + z * fz, x * fz - z * fx);
    let swell = 5.0
        + 4.0 * fbm(x / 1_800.0, z / 1_800.0, seed ^ 0xa3, 3)
        + 1.2 * fbm(along / 260.0, across / 70.0, seed ^ 0xa4, 2) * detail(x, z, 70.0, cells);
    // The lookout stands on a nunatak, looking down over the crevasse
    // fields and blue ice; crags break its flanks but not its summit.
    let fade = 1.0 - smooth(EXTENT - 1_000.0, EXTENT, x.abs().max(z.abs()));
    ((mountains + swell).max(0.0) * smooth(12.0, 85.0, r) + hill) * fade
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

/// Summit (island-local x, z), summit height and coastline radius of the
/// three conical islands in AEOLIAN (Stromboli, Alicudi, Filicudi), for
/// blending each with an ideal cone.
const AEOLIAN_CONES: [(f32, f32, f32, f32); 3] = [
    (-12.0, -612.0, 894.0, 2_080.0),
    (-138.0, 262.0, 651.0, 1_360.0),
    (-362.0, 462.0, 745.0, 1_820.0),
];

/// One size class of procedural volcanoes on a jittered grid.
pub struct VolcanoClass {
    /// Grid cell in metres; each cell holds at most one cone.
    pub cell: f32,
    /// Probability that a cell holds a cone.
    pub chance: f32,
    /// Summit height range in metres.
    pub height: (f32, f32),
    /// Base radius as a multiple of the height.
    pub spread: (f32, f32),
    /// Summit crater radius as a fraction of the base radius.
    pub crater: (f32, f32),
    /// Cones keep their whole base this far from the lookout.
    pub clear: f32,
}

/// Large stratovolcanoes, medium scoria cones and small spatter cones. The
/// largest base (1.3 x radius with its lobes) stays within the 1.2 cells a
/// 3x3 neighbourhood search covers, so no cone is ever cut at a cell edge.
/// Mirrored by `basalt_cone` in ground.wgsl, which colours each cone.
pub const VOLCANO_CLASSES: [VolcanoClass; 3] = [
    VolcanoClass {
        cell: 5_200.0,
        chance: 0.62,
        height: (360.0, 1_050.0),
        spread: (2.4, 3.4),
        crater: (0.05, 0.11),
        clear: 2_600.0,
    },
    VolcanoClass {
        cell: 2_000.0,
        chance: 0.55,
        height: (80.0, 300.0),
        spread: (2.6, 3.6),
        crater: (0.12, 0.24),
        clear: 1_100.0,
    },
    VolcanoClass {
        cell: 720.0,
        chance: 0.42,
        height: (12.0, 60.0),
        spread: (2.6, 3.8),
        crater: (0.16, 0.3),
        clear: 420.0,
    },
];

/// A procedural volcano: summit position, height, base radius, summit crater
/// radius (all metres), breach depth (0-1) and breach direction (radians).
#[derive(Clone, Copy, Debug)]
pub struct Cone {
    pub x: f32,
    pub z: f32,
    pub height: f32,
    pub radius: f32,
    pub crater: f32,
    pub breach: f32,
    pub breach_angle: f32,
    /// Flank exponent: near 1.3 a straight scoria cone, near 2 a concave
    /// stratovolcano.
    pub shape: f32,
    pub key: u32,
}

/// The cone owned by grid cell (ix, iz) of `class`, if any. The WGSL
/// material mirrors this exactly (same hash and arithmetic).
pub fn volcano_cone(class: usize, ix: i32, iz: i32) -> Option<Cone> {
    let c = &VOLCANO_CLASSES[class];
    let key = hash(
        (ix as u32).wrapping_mul(0x8da6_b343)
            ^ (iz as u32).wrapping_mul(0xd816_3841)
            ^ (0x5eed_0000 + class as u32 * 0x9e37),
    );
    let more = hash(key ^ 0x68e3_1da4);
    let unit = |k: u32, shift: u32| ((k >> shift) & 255) as f32 / 255.0;
    if unit(key, 16) > c.chance {
        return None;
    }
    let x = (ix as f32 + 0.2 + 0.6 * unit(key, 0)) * c.cell;
    let z = (iz as f32 + 0.2 + 0.6 * unit(key, 8)) * c.cell;
    let height = c.height.0 + (c.height.1 - c.height.0) * unit(key, 24).powf(1.6);
    let radius = height * (c.spread.0 + (c.spread.1 - c.spread.0) * unit(more, 0));
    if x.hypot(z) < c.clear + radius || x.abs().max(z.abs()) + 1.3 * radius > EXTENT - 1_000.0 {
        return None;
    }
    Some(Cone {
        x,
        z,
        height,
        radius,
        crater: radius * (c.crater.0 + (c.crater.1 - c.crater.0) * unit(more, 8)),
        // About a third of the cones open into a horseshoe on one side.
        breach: ((unit(more, 16) - 0.62) / 0.38).max(0.0),
        breach_angle: unit(more, 24) * std::f32::consts::TAU,
        shape: [1.75, 1.35, 1.25][class] + 0.45 * ((key >> 4) & 15) as f32 / 15.0,
        key,
    })
}

/// Height of one procedural volcano above its base at (x, z): a concave
/// flank with an irregular, lobed footprint, a summit crater (sometimes
/// breached) and radial erosion gullies.
fn volcano(x: f32, z: f32, v: &Cone, cells: usize) -> f32 {
    let (dx, dz) = (x - v.x, z - v.z);
    let r = dx.hypot(dz);
    if r >= 1.3 * v.radius {
        return 0.0;
    }
    let seed = v.key;
    // Lava lobes and flank collapses break up the circular footprint.
    let lobes =
        1.0 + 0.2 * noise(dx / v.radius * 1.4 + 3.1, dz / v.radius * 1.4, seed).clamp(-1.0, 1.0);
    let t = r / (v.radius * lobes);
    if t >= 1.0 {
        return 0.0;
    }
    let rim = v.crater / v.radius;
    // Concave profile: steep upper cone, a long gentle apron at the base.
    let s = ((1.0 - t) / (1.0 - rim)).clamp(0.0, 1.0);
    let mut flank = v.height * s.powf(v.shape);
    // Horseshoe breach: the rim and upper flank sag towards one side.
    let (ux, uz) = (dx / r.max(1e-3), dz / r.max(1e-3));
    let facing = (ux * v.breach_angle.sin() + uz * v.breach_angle.cos()).max(0.0);
    let sag = v.breach * facing.powi(3) * (1.0 - smooth(rim, 4.0 * rim, t));
    flank -= 0.45 * v.height * sag;
    // Summit crater: a bowl whose floor sits a third of a radius deep.
    let depth = (0.9 * v.crater).min(0.35 * v.height);
    let u = t / rim;
    let bowl = v.height - depth * (1.0 - u * u) - 0.45 * v.height * sag;
    let mut h = smooth_min(flank, bowl, (0.25 * depth).max(rounding_width(x, z, cells)));
    // Radial gullies: noise sampled on a circle varies quickly around the
    // cone and slowly downhill, carving channels between sharp spurs.
    let count = 3.0 + 0.02 * v.radius.min(400.0) + v.radius / 900.0;
    let n = 1.6
        * fbm(
            ux * count + r / (0.5 * v.radius) + 9.0,
            uz * count - 4.0,
            seed ^ 0x9a11,
            2,
        );
    let groove = 1.0 - smooth(0.0, 0.4, n.abs());
    let envelope = smooth(rim, 1.6 * rim + 0.08, t) * (1.0 - smooth(0.55, 1.0, t));
    let width = 0.6 * r.max(v.crater) / count;
    h -= 0.11 * v.height * groove * envelope * detail(x, z, width, cells);
    // Loose scoria and blocky lava roughen every flank.
    h += 0.025
        * v.height
        * fbm(
            dx / (0.18 * v.radius),
            dz / (0.18 * v.radius),
            seed ^ 0x77,
            3,
        )
        * envelope.max(smooth(1.0, 0.5, t))
        * detail(x, z, 0.2 * v.radius, cells);
    h.max(0.0) * smooth(1.0, 0.82, t)
}

/// Every procedural cone of `class` whose base may cover (x, z).
fn volcano_class(x: f32, z: f32, class: usize, cells: usize) -> f32 {
    let cell = VOLCANO_CLASSES[class].cell;
    let (gx, gz) = ((x / cell).floor() as i32, (z / cell).floor() as i32);
    let mut h: f32 = 0.0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if let Some(cone) = volcano_cone(class, gx + dx, gz + dz) {
                // Where two cones of a class meet, the taller one wins, so
                // their flanks merge into saddles instead of stacking.
                h = h.max(volcano(x, z, &cone, cells));
            }
        }
    }
    h
}

const BASALT_CELLS: usize = MAX_CELLS;

fn basalt_elevation(x: f32, z: f32, seed: u32) -> f32 {
    // Real Aeolian volcanoes rising from a sea of dark lava rock instead of
    // water. The observer stands on the summit of Stromboli (~900 m), on the
    // south rim of a small summit crater, with an open view all round. Far
    // out on the horizon in every direction stand eight conical islands of
    // different sizes: real Stromboli, Alicudi and Filicudi, turned and
    // scaled, each blended with an ideal cone so it reads as a volcano while
    // keeping its gullies and irregularities. Between them a whole volcanic
    // field of procedural stratovolcanoes, scoria and spatter cones of every
    // size rises from the lava, the smaller ones also as parasitic vents on
    // the flanks of the larger.
    let cells = BASALT_CELLS;
    let mut volcanoes = aeolian(0, x - 12.0, z - 612.0);
    // Gullies and loose scoria on Stromboli's real, but smooth (25 m) slopes.
    let r = x.hypot(z);
    if r > 1.0 {
        let n = fbm(x / r * 12.0 + r / 700.0, z / r * 12.0, seed ^ 0x5701, 2) * 1.6;
        let groove = 1.0 - smooth(0.0, 0.4, n.abs());
        let envelope = smooth(350.0, 850.0, r) * (1.0 - smooth(2_400.0, 3_600.0, r));
        // Gullies of irregular depth, not evenly spaced ribs.
        let depth = (0.75 + 0.6 * noise(x / r * 2.5 + 7.0, z / r * 2.5, seed ^ 0x77e)).max(0.2);
        volcanoes -= 11.0
            * depth
            * groove
            * envelope
            * detail(x, z, 0.08 * r, cells)
            * smooth(20.0, 120.0, volcanoes);
        volcanoes += 6.0
            * fbm(x / 140.0, z / 140.0, seed ^ 0x3a7, 3)
            * smooth(150.0, 400.0, r)
            * smooth(20.0, 120.0, volcanoes)
            * detail(x, z, 140.0, cells);
    }
    // The small summit crater: a bowl with a raised rim through the lookout.
    let u = x.hypot(z - 120.0) / 120.0;
    volcanoes += smooth_min(
        45.0 * (u * u - 1.0) + 6.0 * u.powi(4),
        6.0 * u.max(1e-3).powi(-3) * (1.0 - smooth(2.0, 4.0, u)),
        2.0,
    );
    for (index, bearing, distance, turn, size) in [
        (1_usize, 8.0_f32, 12_300.0_f32, 0.0_f32, 1.2_f32),
        (2, 52.0, 12_800.0, -40.0, 1.0),
        (0, 95.0, 11_800.0, 160.0, 0.85),
        (1, 138.0, 13_200.0, 90.0, 0.8),
        (2, 182.0, 12_000.0, 30.0, 1.15),
        (0, 228.0, 13_000.0, 300.0, 0.9),
        (1, 270.0, 11_600.0, 200.0, 1.05),
        (2, 318.0, 12_600.0, 120.0, 0.95),
    ] {
        let b = bearing.to_radians();
        let (dx, dz) = (
            (x - distance * b.sin()) / size,
            (z - distance * b.cos()) / size,
        );
        let (c, s) = (turn.to_radians().cos(), turn.to_radians().sin());
        let (lx, lz) = (c * dx - s * dz, s * dx + c * dz);
        let (sx, sz, height, radius) = AEOLIAN_CONES[index];
        let cone = height * (1.0 - (lx - sx).hypot(lz - sz) / radius).max(0.0).powf(1.2);
        let real = aeolian(index, lx, lz);
        volcanoes = volcanoes.max(size * (real + 0.6 * (cone - real)));
    }
    // Large procedural stratovolcanoes merge with the real ones; medium
    // and small cones sit on whatever lies beneath them.
    volcanoes = volcanoes.max(volcano_class(x, z, 0, cells));
    // Spatter cones near the lookout stay small, keeping the view open.
    let parasitic = smooth(500.0, 1_400.0, r);
    volcanoes += parasitic * (volcano_class(x, z, 1, cells) + volcano_class(x, z, 2, cells));
    // The lava sea: overlapping flows with pressure ridges and tumuli, flat
    // at large scale, blocky and broken up close by.
    let flows = ridges(x / 1_900.0 + 3.3, z / 1_900.0 - 8.1, seed ^ 0x10fa);
    let sea = 20.0
        + 4.0 * fbm(x / 600.0, z / 600.0, seed, 3)
        + 7.0 * flows * detail(x, z, 300.0, cells)
        + 2.5 * fbm(x / 45.0, z / 45.0, seed ^ 0xbeef, 3).abs() * detail(x, z, 45.0, cells)
        + 0.8 * fbm(x / 9.0, z / 9.0, seed ^ 0x0f, 2) * detail(x, z, 9.0, cells);
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
/// Dune crests round over a little more than other edges: on a crest
/// running diagonally across the grid, a sharper rounding leaves the shading
/// terminator stepping from triangle to triangle.
fn rounded(distance: f32, x: f32, z: f32) -> f32 {
    let width = (2.4 * cell_size(x, z, DUNE_CELLS)).max(1.0);
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
        + 0.5 * fbm(x / 40.0, z / 40.0, seed ^ 0x6d, 3) * detail(x, z, 40.0, DUNE_CELLS);
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

const DUNE_CELLS: usize = MAX_CELLS;

/// Direction the prevailing wind blows towards (radians east of north). It
/// drives sand up the windward slopes onto slip faces to the north-east, like
/// Big Daddy's above Deadvlei. Mirrored by DUNE_WIND in ground.wgsl.
pub const DUNE_WIND: f32 = 0.61;

/// Small transverse dunes superimposed on the large ones (compound dunes):
/// sinuous, crescent-segmented crests with a long gentle windward slope and a
/// short slip face, `wavelength` metres apart. Returns height in metres.
fn small_dunes(x: f32, z: f32, wavelength: f32, height: f32, seed: u32) -> f32 {
    let (wx, wz) = (DUNE_WIND.sin(), DUNE_WIND.cos());
    let along = x * wx + z * wz;
    let across = x * wz - z * wx;
    // Crests meander and bend into crescents whose horns point downwind.
    let warp = 0.9 * wavelength * fbm(x / (4.0 * wavelength), z / (4.0 * wavelength), seed, 3)
        + 0.25
            * wavelength
            * (across / (1.3 * wavelength) + 2.0 * noise(x / 3_000.0, z / 3_000.0, seed ^ 5))
                .sin()
                .abs();
    let phase = (along - warp) / wavelength;
    let f = phase - phase.floor();
    // Crest at f = LEE; the slip face drops over the remaining fraction.
    const LEE: f32 = 0.82;
    let distance = if f < LEE {
        (LEE - f) * wavelength
    } else {
        (f - LEE) * wavelength
    };
    let slope = if f < LEE {
        height / (LEE * wavelength)
    } else {
        height / ((1.0 - LEE) * wavelength)
    };
    let crest = (height - slope * rounded(distance, x, z)).max(0.0);
    // Crest segments of varying height: some crescents die out entirely.
    let segments = smooth(
        -0.15,
        0.45,
        fbm(
            across / (2.2 * wavelength) + 3.0 * phase.floor(),
            along / (6.0 * wavelength),
            seed ^ 0x2b,
            2,
        ) + 0.15,
    );
    crest * segments * detail(x, z, 0.5 * wavelength, DUNE_CELLS)
}

/// Adds the small dunes where the large ones leave gentle ground: windward
/// flanks and corridors, never on slip faces, the pans or Big Daddy's crest.
fn superimpose_dunes(heights: &mut [f32], coordinates: &[f32], seed: u32) {
    let n = coordinates.len();
    let base = heights.to_vec();
    for z in 0..n {
        for x in 0..n {
            let (px, pz) = (coordinates[x], coordinates[z]);
            // Steepest one-sided slope: a central difference reads a knife
            // crest as flat ground.
            let i = z * n + x;
            let step = |j: usize, a: usize, b: usize| {
                ((base[j] - base[i]) / (coordinates[a] - coordinates[b]).abs().max(1e-3)).abs()
            };
            let gx = step(z * n + x.saturating_sub(1), x, x.saturating_sub(1)).max(step(
                z * n + (x + 1).min(n - 1),
                (x + 1).min(n - 1),
                x,
            ));
            let gz = step(z.saturating_sub(1) * n + x, z, z.saturating_sub(1)).max(step(
                (z + 1).min(n - 1) * n + x,
                (z + 1).min(n - 1),
                z,
            ));
            let gentle = 1.0 - smooth(0.12, 0.3, gx.hypot(gz));
            let mut mask = gentle * smooth(120.0, 400.0, px.hypot(pz)) * edge(px, pz);
            for [cx, cz, radius] in DUNE_PANS {
                mask *= smooth(radius + 100.0, radius + 450.0, (px - cx).hypot(pz - cz));
            }
            if mask <= 0.0 {
                continue;
            }
            let field = 0.6 + 0.6 * fbm(px / 3_500.0, pz / 3_500.0, seed ^ 0x51, 2);
            let dunes = small_dunes(px, pz, 260.0, 22.0 * field.clamp(0.2, 1.0), seed ^ 0xd00e)
                + small_dunes(px, pz, 47.0, 2.2, seed ^ 0x7e11)
                    * smooth(4_000.0, 1_500.0, px.hypot(pz));
            heights[z * n + x] += mask * dunes;
        }
    }
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
        if theme == crate::ground::Theme::Dunes {
            superimpose_dunes(&mut heights, &coordinates, seed);
        }
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
        for z in 0..width {
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
        // The dune crest, crater plain, volcano summit and nunatak lift the
        // eye onto the surface.
        assert!(lookout_height(Theme::Alpine, 6.371e6) > 150.0);
        assert!(lookout_height(Theme::Basalt, 6.371e6) > 200.0);
        assert!(lookout_height(Theme::Dunes, 6.371e6) > 300.0);
        assert!(lookout_height(Theme::Craters, 6.371e6) > 20.0);
        let alpine = for_theme(Theme::Alpine);
        assert!(alpine.data[bounds_offset(alpine.cells) + 1] > 1_500.0);
        assert_eq!(for_theme(Theme::Craters).cells, MAX_CELLS);
    }

    #[test]
    fn volcano_field_has_many_cones_of_every_size_clear_of_the_lookout() {
        let mut counts = [0; 3];
        for (class, c) in VOLCANO_CLASSES.iter().enumerate() {
            let reach = (EXTENT / c.cell).ceil() as i32;
            let mut heights = Vec::new();
            for iz in -reach..reach {
                for ix in -reach..reach {
                    let Some(cone) = volcano_cone(class, ix, iz) else {
                        continue;
                    };
                    // Whole base inside the grid cells a 3x3 search covers.
                    let cell_x = cone.x / c.cell - ix as f32;
                    let cell_z = cone.z / c.cell - iz as f32;
                    let margin = 1.3 * cone.radius / c.cell;
                    assert!(cell_x - margin >= -1.0 && cell_x + margin <= 2.0);
                    assert!(cell_z - margin >= -1.0 && cell_z + margin <= 2.0);
                    assert!(cone.x.hypot(cone.z) >= c.clear + cone.radius);
                    assert!(cone.crater < 0.35 * cone.radius && cone.height > 0.0);
                    heights.push(cone.height);
                }
            }
            counts[class] = heights.len();
            let (lo, hi) = heights
                .iter()
                .fold((f32::MAX, 0.0_f32), |(a, b), h| (a.min(*h), b.max(*h)));
            assert!(hi > 2.0 * lo, "class {class} sizes {lo}..{hi}");
        }
        assert!(
            counts[0] >= 10 && counts[1] >= 80 && counts[2] >= 500,
            "{counts:?}"
        );
    }

    #[test]
    fn wgsl_volcano_materials_mirror_the_generated_classes() {
        let shader = include_str!("shaders/ground.wgsl");
        let field = |f: fn(&VolcanoClass) -> f32| {
            VOLCANO_CLASSES
                .iter()
                .map(|c| format!("{:?}", f(c)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        for (name, values) in [
            ("cells", field(|c| c.cell)),
            ("chance", field(|c| c.chance)),
            ("low", field(|c| c.height.0)),
            ("high", field(|c| c.height.1)),
            ("spread_low", field(|c| c.spread.0)),
            ("spread_high", field(|c| c.spread.1)),
            ("crater_low", field(|c| c.crater.0)),
            ("crater_high", field(|c| c.crater.1)),
            ("clear", field(|c| c.clear)),
        ] {
            let line = format!("var {name} = array<f32, 3>({values});");
            assert!(shader.contains(&line), "ground.wgsl lacks `{line}`");
        }
    }

    #[test]
    fn wgsl_dune_and_ice_materials_mirror_the_terrain_constants() {
        let shader = include_str!("shaders/ground.wgsl");
        for line in [
            format!("const DUNE_WIND: f32 = {DUNE_WIND:?};"),
            format!("const DUNE_PAN_LEVEL: f32 = {DUNE_PAN_LEVEL:?};"),
            format!(
                "var pans = array<vec3<f32>, 2>({});",
                DUNE_PANS
                    .map(|[x, z, r]| format!("vec3<f32>({x:?}, {z:?}, {r:?})"))
                    .join(", ")
            ),
            format!("const ICE_FLOW: f32 = {ICE_FLOW:?};"),
            format!(
                "const LOOKOUT_HILL: vec2<f32> = vec2<f32>({:?}, {:?});",
                LOOKOUT_HILL.0, LOOKOUT_HILL.1
            ),
        ] {
            assert!(shader.contains(&line), "ground.wgsl lacks `{line}`");
        }
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
