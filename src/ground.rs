//! A small, deterministic landscape in metres: east, height, north.
//! It rotates with the observer's surface frame rather than the telescope.
use bytemuck::{Pod, Zeroable};

pub const CAPACITY: usize = 256;
pub const SKY_TEXELS: usize = 128;
/// Faceted rock with three packed 8-bit Euler angles in extent.w.
pub const TUMBLED_ROCK: f32 = 2.25;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum Theme {
    #[default]
    Forest = 0,
    Desert = 1,
}

pub fn landscape_for_theme(radius_m: f64, density: u32, theme: Theme) -> Vec<Primitive> {
    if theme == Theme::Forest {
        return landscape_with_density(radius_m, density);
    }
    let mut rocks = Vec::new();
    let scale = crate::terrain::scale(radius_m);
    let mut seed = 0xa7ac_a4a1_u32;
    let mut random = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / 16_777_216.0
    };
    for i in 0..28 {
        let n = i as f32;
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let z = -9.0 + n * 6.5 + (random() - 0.5) * 5.0;
        let x = side * (9.0 + (n * 2.4).sin().abs() * 24.0);
        let h = 0.09 + 0.70 * random().powi(2);
        // Slabs, elongated fragments, squat stones and chunky boulders.
        let proportions = match i % 4 {
            0 => [1.8, 0.45, 1.1],
            1 => [0.7, 0.75, 1.9],
            2 => [1.05, 0.8, 0.9],
            _ => [0.9, 1.25, 1.15],
        };
        let extent = proportions.map(|v| v * h * (0.75 + random() * 0.5));
        let angles = [random(), random(), random()].map(|v| (v * 256.0) as u32);
        let orientation = (angles[0] | (angles[1] << 8) | (angles[2] << 16)) as f32;
        let tint = match i % 4 {
            0 => [0.36, 0.25, 0.15],
            1 => [0.19, 0.17, 0.14],
            2 => [0.43, 0.36, 0.26],
            _ => [0.27, 0.13, 0.065],
        };
        let shade = 0.75 + random() * 0.45;
        let d2 = f64::from(x * x + z * z);
        let curve = -d2 / (radius_m + (radius_m * radius_m - d2).max(0.0).sqrt());
        let y = crate::terrain::for_theme(theme).height(x / scale, z / scale) * scale;
        let mut rock = material(
            shape(
                [x, y + curve as f32, z],
                extent,
                tint.map(|v| v * shade),
                TUMBLED_ROCK,
                orientation,
            ),
            2.0,
        );
        // Embed the bottom in the terrain using the rotated vertical extent.
        rock.center[1] += rock.world_extent().y * (0.12 + random() * 0.24);
        rocks.push(rock);
    }
    let mut out = Vec::new();
    cluster(&mut out, rocks);
    out
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Primitive {
    pub center: [f32; 4], // xyz in metres, w: 0 box, 1 ellipsoid, 2 rock, 2.25 tumbled rock, 3 conifer, 4 grass, 5 log; negative = group child count
    pub extent: [f32; 4], // half extents; w = north rotation, or packed xyz Euler angles for tumbled rocks
    pub albedo: [f32; 4], // rgb; w: 0 solid, 1 terrain, 2 stone, 3 foliage, 4 bark, 5 grass
}

impl Primitive {
    fn rotation(&self) -> glam::Mat3 {
        if self.center[3] != TUMBLED_ROCK {
            return glam::Mat3::from_rotation_z(self.extent[3]);
        }
        let bits = self.extent[3] as u32;
        let radians = std::f32::consts::TAU / 256.0;
        let x = (bits & 255) as f32 * radians;
        let y = ((bits >> 8) & 255) as f32 * radians;
        let z = ((bits >> 16) & 255) as f32 * radians;
        glam::Mat3::from_rotation_z(z)
            * glam::Mat3::from_rotation_y(y)
            * glam::Mat3::from_rotation_x(x)
    }

    fn world_extent(&self) -> glam::Vec3 {
        let r = self.rotation();
        r.x_axis.abs() * self.extent[0]
            + r.y_axis.abs() * self.extent[1]
            + r.z_axis.abs() * self.extent[2]
    }
}

fn shape(center: [f32; 3], extent: [f32; 3], albedo: [f32; 3], kind: f32, angle: f32) -> Primitive {
    Primitive {
        center: [center[0], center[1], center[2], kind],
        extent: [extent[0], extent[1], extent[2], angle],
        albedo: [albedo[0], albedo[1], albedo[2], 0.0],
    }
}

/// Flat bounding hierarchy: a missed bound skips all its following children.
/// This keeps distant groves out of most foreground and atmospheric rays.
fn cluster(out: &mut Vec<Primitive>, children: Vec<Primitive>) {
    if children.is_empty() {
        return;
    }
    let mut lo = glam::Vec3::splat(f32::INFINITY);
    let mut hi = glam::Vec3::splat(f32::NEG_INFINITY);
    for object in &children {
        let e = object.world_extent();
        let center = glam::Vec3::from_array(object.center[..3].try_into().unwrap());
        lo = lo.min(center - e);
        hi = hi.max(center + e);
    }
    out.push(shape(
        ((lo + hi) * 0.5).into(),
        ((hi - lo) * 0.5 + glam::Vec3::splat(0.02)).into(),
        [0.0; 3],
        -(children.len() as f32),
        0.0,
    ));
    out.extend(children);
}

fn material(mut object: Primitive, kind: f32) -> Primitive {
    object.albedo[3] = kind;
    object
}

#[cfg(test)]
pub fn landscape(radius_m: f64) -> Vec<Primitive> {
    landscape_with_density(radius_m, 100)
}

pub fn landscape_with_density(radius_m: f64, density: u32) -> Vec<Primitive> {
    let density = density.min(100);
    let mut landscape = Vec::new();
    let surface = |x: f32, z: f32| {
        let r = radius_m;
        let d2 = (x as f64).powi(2) + (z as f64).powi(2);
        (-d2 / (r + (r * r - d2).max(0.0).sqrt())) as f32
            + crate::terrain::cached()
                .height(x / crate::terrain::scale(r), z / crate::terrain::scale(r))
                * crate::terrain::scale(r)
    };
    // Weathered, partly buried outcrops frame the lookout without covering it.
    for (cx, cz, size) in [
        (-8.0_f32, 14.0, 0.75),
        (11.0, 26.0, 1.0),
        (-35.0, 68.0, 2.6),
        (53.0, 95.0, 3.2),
    ] {
        let mut rocks = Vec::new();
        for j in 0..4 {
            let n = j as f32;
            let x = cx + (n * 2.4).sin() * size * 1.6;
            let z = cz + (n * 3.7).cos() * size;
            let h = size * (0.5 + 0.5 * (n * 1.8 + 0.4).sin().abs());
            rocks.push(material(
                shape(
                    [x, surface(x, z) + h * 0.38, z],
                    [h * (1.1 + 0.3 * (n * 1.3).cos()), h, h * 0.85],
                    [0.13, 0.125, 0.115],
                    2.0,
                    0.17 * (n - 1.5),
                ),
                2.0,
            ));
        }
        cluster(&mut landscape, rocks);
    }
    // An open clearing with irregular groves, never a ring of identical trees.
    let wood = [0.095, 0.055, 0.026];
    for (cx, cz, count) in [
        (-33.0_f32, 72.0, 9),
        (50.0, 110.0, 10),
        (-110.0, 210.0, 13),
        (145.0, 270.0, 13),
        (-240.0, 530.0, 15),
        (285.0, 590.0, 15),
        (-65.0, -100.0, 10),
        (80.0, -160.0, 10),
    ] {
        let mut trees = Vec::new();
        for i in 0..count {
            if (i * 37 + 17) % 100 >= density {
                continue;
            }
            let n = i as f32;
            let phase = n * 2.39996 + cx * 0.07;
            let spread = 7.5 * (n + 0.5).sqrt();
            let x = cx + spread * phase.cos();
            let z = cz + spread * phase.sin();
            if z > 0.0 && x.abs() < z * 0.24 {
                continue;
            }
            let h = 9.0 + 6.0 * (n * 7.13 + cx).sin().abs();
            let width = h * (0.18 + 0.035 * (n * 3.4).cos());
            trees.push(material(
                shape(
                    [x, surface(x, z) + h * 0.5, z],
                    [width, h * 0.5, width * 0.9],
                    [0.045 + 0.012 * (n * 2.1).sin().abs(), 0.095, 0.042],
                    3.0,
                    0.018 * (n * 1.3).sin(),
                ),
                3.0,
            ));
        }
        cluster(&mut landscape, trees);
    }
    // Sparse undergrowth follows the clearing edge and rock patches.
    for (cx, cz) in [(-5.5_f32, 8.0), (8.0, 18.0), (-14.0, 29.0), (21.0, 42.0)] {
        let mut plants = Vec::new();
        for i in 0..12 {
            if (i * 37 + 17) % 100 >= density {
                continue;
            }
            let n = i as f32;
            let angle = n * 2.39996;
            let spread = 0.9 * (n + 0.5).sqrt();
            let x = cx + spread * angle.cos();
            let z = cz + spread * angle.sin();
            let h = 0.22 + 0.24 * (n * 3.1).sin().abs();
            plants.push(material(
                shape(
                    [x, surface(x, z) + h * 0.5, z],
                    [0.30, h * 0.5, 0.30],
                    [0.12, 0.18, 0.043],
                    4.0,
                    0.0,
                ),
                5.0,
            ));
        }
        cluster(&mut landscape, plants);
    }
    for (x, z, length) in [(-9.0_f32, 20.0, 2.6), (17.0, 42.0, 1.8)] {
        landscape.push(material(
            shape(
                [x, surface(x, z) + 0.22, z],
                [0.25, length, 0.25],
                wood,
                5.0,
                1.43,
            ),
            4.0,
        ));
    }
    let mut objects = Vec::new();
    // Cottage off to the east, leaving the central observing view open.
    let (x, z) = (22.0, 30.0);
    let base = surface(x, z);
    let plaster = [0.42, 0.34, 0.24];
    let roof = [0.16, 0.085, 0.055];
    objects.push(shape(
        [x, base + 1.7, z],
        [3.2, 1.7, 3.8],
        plaster,
        0.0,
        0.0,
    ));
    // Two sloping roof slabs meet at a ridge, with generous eaves.
    let pitch = 30_f32.to_radians();
    for side in [-1.0, 1.0] {
        objects.push(shape(
            [x + side * 1.85, base + 4.39, z],
            [2.16, 0.13, 4.15],
            roof,
            0.0,
            -side * pitch,
        ));
    }
    // Timber boards fill the gable without requiring mesh assets.
    for row in 0..8 {
        let y = 3.4 + (row as f32 + 0.5) * 0.23;
        let half_width = (5.46 - y) / pitch.tan();
        objects.push(shape(
            [x, base + y, z],
            [half_width.min(3.2), 0.12, 3.8],
            plaster,
            0.0,
            0.0,
        ));
    }
    objects.push(shape(
        [x - 0.5, base + 1.05, z - 3.82],
        [0.55, 1.05, 0.035],
        wood,
        0.0,
        0.0,
    ));
    for dx in [-2.0, 1.6] {
        // Matte blue panes for now; no artificial emission or fake glow.
        objects.push(shape(
            [x + dx, base + 2.0, z - 3.83],
            [0.55, 0.55, 0.04],
            wood,
            0.0,
            0.0,
        ));
        objects.push(shape(
            [x + dx, base + 2.0, z - 3.88],
            [0.45, 0.45, 0.015],
            [0.10, 0.19, 0.23],
            0.0,
            0.0,
        ));
        objects.push(shape(
            [x + dx, base + 2.0, z - 3.90],
            [0.035, 0.46, 0.015],
            wood,
            0.0,
            0.0,
        ));
    }
    objects.push(shape(
        [x + 1.5, base + 5.0, z + 1.8],
        [0.35, 1.2, 0.45],
        [0.32, 0.20, 0.14],
        0.0,
        0.0,
    ));
    cluster(&mut landscape, objects);
    let mut bounded = Vec::new();
    cluster(&mut bounded, landscape);
    debug_assert!(bounded.len() <= CAPACITY);
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn density_keeps_tree_positions_and_the_observing_gap() {
        let full = landscape_with_density(6_371_000.0, 100);
        for density in [0, 25, 50, 75, 100] {
            let sparse = landscape_with_density(6_371_000.0, density);
            assert!(sparse.len() <= CAPACITY);
            for p in sparse
                .iter()
                .filter(|p| p.center[3] == 3.0 || p.center[3] == 4.0)
            {
                assert!(density > 0);
                assert!(
                    full.iter()
                        .any(|q| bytemuck::bytes_of(p) == bytemuck::bytes_of(q))
                );
                if p.center[3] == 3.0 && p.center[2] > 0.0 {
                    assert!(p.center[0].abs() >= p.center[2] * 0.24);
                }
            }
        }
    }
    #[test]
    fn bounds_enclose_descendants_and_landscape_stays_within_buffer_capacity() {
        for radius in [5_000.0, 1_500_000.0, 6_371_000.0, 60_000_000.0] {
            for theme in [Theme::Forest, Theme::Desert] {
                let objects = landscape_for_theme(radius, 100, theme);
                assert!(objects.len() <= CAPACITY);
                assert_eq!(
                    bytemuck::cast_slice::<_, u8>(&objects),
                    bytemuck::cast_slice::<_, u8>(&landscape_for_theme(radius, 100, theme))
                );
                for (i, object) in objects.iter().enumerate() {
                    assert!(
                        object
                            .center
                            .iter()
                            .chain(&object.extent)
                            .chain(&object.albedo)
                            .all(|x| x.is_finite())
                    );
                    assert!(object.extent[..3].iter().all(|x| *x > 0.0));
                    if object.center[3] >= 0.0 {
                        continue;
                    }
                    let end = i + 1 + (-object.center[3]) as usize;
                    assert!(end <= objects.len());
                    for child in &objects[i + 1..end] {
                        // Check all transformed corners, independently of the
                        // abs-matrix shortcut used to build the hierarchy.
                        for bits in 0..8 {
                            let corner = glam::Vec3::from_array(std::array::from_fn(|axis| {
                                child.extent[axis]
                                    * if bits & (1 << axis) == 0 { -1.0 } else { 1.0 }
                            }));
                            let rotated = child.rotation() * corner;
                            for axis in 0..3 {
                                assert!(
                                    (child.center[axis] + rotated[axis] - object.center[axis])
                                        .abs()
                                        <= object.extent[axis] + 0.01
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
