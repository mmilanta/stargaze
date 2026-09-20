//! Procedural star catalogue: uniform sky + a faint Milky Way band.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StarInstance {
    pub dir: [f32; 3],
    pub color: [f32; 3],
    pub size: f32,
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

fn star(rng: &mut Rng, dim: f32) -> StarInstance {
    let u = rng.f32();
    // Power law: many faint, few bright.
    let bright = (0.02 + u * u * u * 1.6) * dim;
    let temp = 2500.0 + 12000.0 * rng.f32().powi(2);
    let size = (0.7 + 2.6 * bright.sqrt()).clamp(0.7, 3.4);
    StarInstance {
        dir: random_dir(rng),
        color: blackbody_linear(temp),
        size,
        bright,
    }
}

/// Build `count` uniform stars plus `band` extra stars along the Milky Way.
pub fn generate(count: usize, band: usize, seed: u64) -> Vec<StarInstance> {
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
