//! Procedural star catalogue: uniform sky + a faint Milky Way band.

use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug)]
pub struct CatalogueStar {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    /// Fixed angular radius in radians, not a screen-space point size.
    pub angular_radius: f32,
    pub bright: f32,
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
    fn f32(&mut self) -> f32 {
        (self.u32() as f32) / (u32::MAX as f32)
    }
}

/// Approximate blackbody colour (Tanner Helland), returned in linear space.
fn blackbody_linear(temp_k: f32) -> [f32; 3] {
    let t = (temp_k / 100.0).clamp(10.0, 400.0);
    let r = if t <= 66.0 {
        255.0
    } else {
        329.698_73 * (t - 60.0).powf(-0.133_204_76)
    };
    let g = if t <= 66.0 {
        99.470_8 * t.ln() - 161.119_57
    } else {
        288.122_17 * (t - 60.0).powf(-0.075_514_85)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_73 * (t - 10.0).ln() - 305.044_8
    };
    [
        srgb_to_linear(r / 255.0),
        srgb_to_linear(g / 255.0),
        srgb_to_linear(b / 255.0),
    ]
}

fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn random_dir(rng: &mut Rng) -> [f32; 3] {
    let z = 1.0 - 2.0 * rng.f32();
    let phi = std::f32::consts::TAU * rng.f32();
    let r = (1.0 - z * z).max(0.0).sqrt();
    [r * phi.cos(), r * phi.sin(), z]
}

fn star(rng: &mut Rng, dim: f32) -> CatalogueStar {
    let u = rng.f32();
    // Power law: many faint, few bright.
    let bright = (0.02 + u * u * u * 1.6) * dim;
    let temp = 2500.0 + 12000.0 * rng.f32().powi(2);
    let angular_radius = (0.7 + 2.6 * bright.sqrt()).clamp(0.7, 3.4) * 0.0004;
    CatalogueStar {
        dir: random_dir(rng),
        color: blackbody_linear(temp),
        angular_radius,
        bright,
    }
}

/// Build `count` uniform stars plus `band` extra stars along the Milky Way.
pub fn generate(count: usize, band: usize, seed: u64) -> Vec<CatalogueStar> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(count + band);

    for _ in 0..count {
        out.push(star(&mut rng, 1.0));
    }

    // Great circle with normal `n`, then bulge out of plane.
    let n = glam::Vec3::new(0.35, 0.82, 0.45).normalize();
    let u = n.any_orthonormal_vector();
    let v = n.cross(u).normalize();
    for _ in 0..band {
        let a = std::f32::consts::TAU * rng.f32();
        let spread = (rng.f32() + rng.f32() + rng.f32() - 1.5) * 0.28;
        let d = (u * a.cos() + v * a.sin() + n * spread).normalize();
        let mut s = star(&mut rng, 0.28);
        s.dir = d.into();
        // Cooler, dustier population.
        s.color = blackbody_linear(2500.0 + 6000.0 * rng.f32());
        out.push(s);
    }

    out
}

/// Fixed angular discs, independent of zoom/resolution. These are a stylized
/// catalogue at infinity, not finite scene lights or screen-space billboards.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RayStar {
    pub direction: [f32; 4],
    pub radiance: [f32; 4],
}

/// Conservative spherical grid: a miss ray tests only discs overlapping its
/// cell, rather than all 6000 catalogue entries. Includes wraparound and poles.
pub fn ray_catalogue(stars: &[CatalogueStar]) -> (Vec<RayStar>, Vec<u32>) {
    const W: usize = 256;
    const H: usize = 128;
    use std::f64::consts::{PI, TAU};
    let mut bins = vec![Vec::<u32>::new(); W * H];
    let mut data = Vec::new();
    for (index, star) in stars.iter().enumerate() {
        let dir = glam::DVec3::from_array(star.dir.map(f64::from)).normalize();
        let radius = (star.angular_radius as f64).clamp(1e-6, 0.1);
        data.push(RayStar {
            direction: [
                dir.x as f32,
                dir.y as f32,
                dir.z as f32,
                radius.sin().powi(2) as f32,
            ],
            radiance: [
                star.color[0] * star.bright,
                star.color[1] * star.bright,
                star.color[2] * star.bright,
                0.0,
            ],
        });
        let theta = dir.z.clamp(-1.0, 1.0).acos();
        let phi = dir.y.atan2(dir.x) + PI;
        let y0 = (((theta - radius).max(0.0) / PI * H as f64) as usize).min(H - 1);
        let y1 = (((theta + radius).min(PI) / PI * H as f64) as usize).min(H - 1);
        let longitude_radius = if theta <= radius || theta + radius >= PI {
            PI
        } else {
            (radius.sin() / theta.sin()).clamp(0.0, 1.0).asin()
        };
        for x in 0..W {
            let cell_phi = (x as f64 + 0.5) * TAU / W as f64;
            let separation = ((cell_phi - phi + PI).rem_euclid(TAU) - PI).abs();
            if separation <= longitude_radius + PI / W as f64 + 1e-6 {
                for y in y0..=y1 {
                    bins[y * W + x].push(index as u32);
                }
            }
        }
    }
    let mut offsets = Vec::with_capacity(W * H + 1);
    let mut indices = Vec::new();
    for bin in bins {
        offsets.push(indices.len() as u32);
        indices.extend(bin);
    }
    offsets.push(indices.len() as u32);
    offsets.extend(indices);
    // Bindings cannot be empty; no cell references this dummy entry.
    if data.is_empty() {
        data.push(RayStar::zeroed());
    }
    (data, offsets)
}

#[cfg(test)]
mod ray_tests {
    use super::*;

    #[test]
    fn grid_contains_disc_samples_including_seam_and_poles() {
        let mut stars = generate(100, 0, 123);
        for dir in [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
            stars.push(CatalogueStar {
                dir,
                color: [1.0; 3],
                angular_radius: 0.0012,
                bright: 1.0,
            });
        }
        let (_, cells) = ray_catalogue(&stars);
        for (i, star) in stars.iter().enumerate() {
            let axis = glam::DVec3::from_array(star.dir.map(f64::from)).normalize();
            let tangent = axis.any_orthonormal_vector();
            for j in 0..32 {
                let phi = j as f64 * std::f64::consts::TAU / 32.0;
                let radius = star.angular_radius as f64 * 0.999;
                let dir = axis * radius.cos()
                    + (tangent * phi.cos() + axis.cross(tangent) * phi.sin()) * radius.sin();
                let x = (((dir.y.atan2(dir.x) + std::f64::consts::PI) / std::f64::consts::TAU
                    * 256.0) as usize)
                    .min(255);
                let y = ((dir.z.clamp(-1.0, 1.0).acos() / std::f64::consts::PI * 128.0) as usize)
                    .min(127);
                let cell = y * 256 + x;
                let start = 32769 + cells[cell] as usize;
                let end = 32769 + cells[cell + 1] as usize;
                assert!(
                    cells[start..end].contains(&(i as u32)),
                    "disc {i} missing from cell {cell}"
                );
            }
        }
    }
}
