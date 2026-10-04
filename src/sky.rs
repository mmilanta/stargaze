//! A seamless, procedural galactic haze map. This is a visual backdrop only.
//! Cache linear RGB clouds for cheap, bilinear GPU lookup rather than
//! evaluating several octaves of noise for every path-tracing sample.

use glam::Vec3;
use std::sync::OnceLock;

pub const WIDTH: usize = 1024;
pub const HEIGHT: usize = 512;

fn lattice(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f)
        ^ 0x5eed_1234;
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb_352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846c_a68b);
    ((h ^ (h >> 16)) >> 8) as f32 / 16_777_216.0
}

fn noise(p: Vec3) -> f32 {
    let cell = p.floor().as_ivec3();
    let t = p - p.floor();
    let t = t * t * t * (t * (t * 6.0 - Vec3::splat(15.0)) + Vec3::splat(10.0));
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |z| {
        mix(
            mix(
                lattice(cell.x, cell.y, z),
                lattice(cell.x + 1, cell.y, z),
                t.x,
            ),
            mix(
                lattice(cell.x, cell.y + 1, z),
                lattice(cell.x + 1, cell.y + 1, z),
                t.x,
            ),
            t.y,
        )
    };
    mix(plane(cell.z), plane(cell.z + 1), t.z)
}

fn clouds(mut p: Vec3) -> f32 {
    let mut sum = 0.0;
    let mut weight = 0.533_333_36;
    for _ in 0..4 {
        sum += weight * noise(p);
        p = p * 2.07 + Vec3::new(13.1, 7.7, 3.3);
        weight *= 0.5;
    }
    sum
}

/// Coordinates within the galactic plane. Spherical noise keeps the longitude
/// seam continuous; the taper vanishes before either pole, avoiding pole pinches.
fn color(longitude: f32, latitude: f32) -> Vec3 {
    if latitude.abs() > 1.35 {
        return Vec3::ZERO;
    }
    let longitude =
        (longitude + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let (sin_l, cos_l) = longitude.sin_cos();
    let (sin_b, cos_b) = latitude.sin_cos();
    let direction = Vec3::new(cos_l * cos_b, sin_l * cos_b, sin_b);
    let equator = Vec3::new(cos_l, sin_l, 0.0);
    let coarse = noise(equator * 3.7 + Vec3::splat(17.0));
    let center = (-0.5 * (longitude / 0.72).powi(2)).exp();
    // The centre is a broad asymmetric bulge, not just a brighter stripe.
    let midline = 0.075 * (2.0 * longitude).sin() + 0.10 * (coarse - 0.5);
    let width = 0.12 + 0.12 * coarse + 0.16 * center;
    let warp = Vec3::new(
        noise(direction * 5.0 + Vec3::new(11.0, 0.0, 0.0)),
        noise(direction * 5.0 + Vec3::new(0.0, 19.0, 0.0)),
        noise(direction * 5.0 + Vec3::new(0.0, 0.0, 23.0)),
    ) - Vec3::splat(0.5);
    let clumps = clouds(direction * 10.0 + warp * 4.0);
    let wisps = clouds(direction * 29.0 + warp * 7.0);
    let irregular_latitude = latitude - midline + 0.15 * (clumps - 0.5);
    let band = (-0.5 * (irregular_latitude / width).powi(2)).exp();
    let halo = 0.07 * (-0.5 * (latitude / (width * 1.7)).powi(2)).exp();
    let bulge = 0.46 * center * (-0.5 * ((latitude + 0.04) / 0.28).powi(2)).exp();
    let starlight = band * (0.05 + 1.35 * clumps.powi(2)) * (0.60 + 0.70 * wisps) + bulge;
    // Ragged, branching rifts vary in width and opacity instead of forming
    // one straight dark line through the entire sky.
    let spine = midline + 0.08 * (noise(direction * 8.0) - 0.5);
    let rift_width = 0.014 + 0.042 * noise(direction * 12.0);
    let rift = (-0.5 * ((latitude - spine) / rift_width).powi(2)).exp();
    let branch_spine = spine + 0.13 * (3.0 * longitude + 0.7).sin();
    let branch = (-0.5 * ((latitude - branch_spine) / 0.025).powi(2)).exp();
    let dust_clouds = clouds(direction * 19.0 + warp * 8.0 + Vec3::splat(11.0));
    let patches = ((0.62 - dust_clouds) * 3.0).clamp(0.0, 1.0);
    let dust = (0.82 * rift + 0.35 * branch * center + 0.65 * patches * band).clamp(0.0, 0.96);
    let warmth = (center * 0.85 + (coarse - 0.5) * 0.35).clamp(0.0, 1.0);
    let tint = Vec3::new(0.38, 0.55, 1.0).lerp(Vec3::new(1.0, 0.67, 0.36), warmth);
    let violet = Vec3::new(0.64, 0.32, 0.85) * halo * (0.5 + clumps);
    let nebula =
        Vec3::new(0.85, 0.22, 0.38) * band * ((wisps - 0.58) * 2.0).max(0.0) * (0.3 + center);
    ((tint * starlight + nebula) * (1.0 - dust) + violet).clamp(Vec3::ZERO, Vec3::ONE)
}

/// One linear RGB10 texel per u32, matching the GPU storage-buffer sampler.
pub fn panorama() -> &'static [u32] {
    static MAP: OnceLock<Vec<u32>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut packed = vec![0; WIDTH * HEIGHT];
        for y in 0..HEIGHT {
            let latitude = std::f32::consts::PI * (0.5 - (y as f32 + 0.5) / HEIGHT as f32);
            for x in 0..WIDTH {
                let longitude = std::f32::consts::TAU * (0.5 - (x as f32 + 0.5) / WIDTH as f32);
                let rgb = (color(longitude, latitude) * 1023.0).round().as_uvec3();
                packed[y * WIDTH + x] = rgb.x | (rgb.y << 10) | (rgb.z << 20);
            }
        }
        packed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galactic_haze_wraps_and_has_dust_contrast_without_pole_glow() {
        for latitude in [-0.2, -0.05, 0.0, 0.05, 0.2] {
            let left = color(-std::f32::consts::PI, latitude);
            let right = color(std::f32::consts::PI, latitude);
            assert!((left - right).length() < 1e-6);
        }
        assert_eq!(color(0.0, 1.5), Vec3::ZERO);
        assert_eq!(color(0.0, -1.5), Vec3::ZERO);
        assert!(
            color(0.0, 0.35).length() > color(std::f32::consts::PI, 0.35).length() * 2.0,
            "the central bulge must be broader than the outer band"
        );
        let warm = color(0.0, 0.25);
        let cool = color(std::f32::consts::PI, 0.1);
        assert!(
            warm.x > warm.z && cool.z > cool.x,
            "warm core and cool outer clouds"
        );
        let map = panorama();
        assert_eq!(map.len(), WIDTH * HEIGHT);
        assert!(map.iter().any(|v| *v != 0));
        assert!(map.iter().any(|v| *v == 0));
        assert!(std::ptr::eq(map, panorama()), "cache the generated map");
    }
}
