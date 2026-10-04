//! Cached, continuous terrain: warped multifractal noise, thermal erosion,
//! and a min/max quadtree over a triangulated heightfield. See docs/terrain.md.
use std::sync::OnceLock;

pub const CELLS: usize = 512;
pub const WIDTH: usize = CELLS + 1;
pub const EXTENT: f32 = 16_000.0;
const LEAF: usize = 2;
const LEAF_AXIS: usize = CELLS / LEAF;
pub const NODE_COUNT: usize = (4 * LEAF_AXIS * LEAF_AXIS - 1) / 3;
pub const BOUNDS_OFFSET: usize = WIDTH * WIDTH * 3;
const SEED: u32 = 0x7433_37a9;

pub struct Terrain {
    /// Heights, x/z derivatives, then min/max bounds in breadth-first order.
    pub data: Vec<f32>,
}

pub fn cached() -> &'static Terrain {
    static TERRAIN: OnceLock<Terrain> = OnceLock::new();
    TERRAIN.get_or_init(|| Terrain::generate(SEED))
}

/// Quadratic spacing gives sub-metre geometry at the observatory, with
/// gradually larger cells towards the edge. Every neighbour shares vertices.
pub fn coordinate(i: usize) -> f32 {
    let u = 2.0 * i as f32 / CELLS as f32 - 1.0;
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
    fn generate(seed: u32) -> Self {
        let started = std::time::Instant::now();
        let coordinates: Vec<_> = (0..WIDTH).map(coordinate).collect();
        let mut heights: Vec<_> = (0..WIDTH * WIDTH)
            .map(|i| elevation(coordinates[i % WIDTH], coordinates[i / WIDTH], seed))
            .collect();
        erode(&mut heights, &coordinates, 18);
        let mut data = vec![0.0; BOUNDS_OFFSET + 2 * NODE_COUNT];
        data[..WIDTH * WIDTH].copy_from_slice(&heights);
        for z in 0..WIDTH {
            for x in 0..WIDTH {
                let i = z * WIDTH + x;
                let (x0, x1) = (x.saturating_sub(1), (x + 1).min(CELLS));
                let (z0, z1) = (z.saturating_sub(1), (z + 1).min(CELLS));
                data[WIDTH * WIDTH + i] = (heights[z * WIDTH + x1] - heights[z * WIDTH + x0])
                    / (coordinates[x1] - coordinates[x0]);
                data[2 * WIDTH * WIDTH + i] = (heights[z1 * WIDTH + x] - heights[z0 * WIDTH + x])
                    / (coordinates[z1] - coordinates[z0]);
            }
        }
        fn bounds(data: &mut [f32], node: usize, x: usize, z: usize, size: usize) -> (f32, f32) {
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            if size == LEAF {
                for row in z..=z + size {
                    for column in x..=x + size {
                        let h = data[row * WIDTH + column];
                        lo = lo.min(h);
                        hi = hi.max(h);
                    }
                }
            } else {
                for corner in 0..4 {
                    let (a, b) = bounds(
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
            data[BOUNDS_OFFSET + 2 * node] = lo;
            data[BOUNDS_OFFSET + 2 * node + 1] = hi;
            (lo, hi)
        }
        bounds(&mut data, 0, 0, 0, CELLS);
        log::info!(
            "terrain generated: {} triangles, {:.2} MiB, {:?}",
            CELLS * CELLS * 2,
            data.len() as f64 * 4.0 / (1024.0 * 1024.0),
            started.elapsed()
        );
        Self { data }
    }

    /// Match the two triangles in each GPU cell, for grounded props.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let index = |v: f32| {
            let u = v.signum() * (v.abs() / EXTENT).sqrt();
            (((u + 1.0) * 0.5 * CELLS as f32).floor() as usize).min(CELLS - 1)
        };
        let (ix, iz) = (index(x), index(z));
        let u = ((x - coordinate(ix)) / (coordinate(ix + 1) - coordinate(ix))).clamp(0.0, 1.0);
        let v = ((z - coordinate(iz)) / (coordinate(iz + 1) - coordinate(iz))).clamp(0.0, 1.0);
        let a = self.data[iz * WIDTH + ix];
        let b = self.data[iz * WIDTH + ix + 1];
        let c = self.data[(iz + 1) * WIDTH + ix];
        let d = self.data[(iz + 1) * WIDTH + ix + 1];
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
        let terrain = cached();
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
