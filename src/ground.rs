//! A small, deterministic landscape in metres: east, height, north.
//! It rotates with the observer's surface frame rather than the telescope.
use bytemuck::{Pod, Zeroable};

pub const CAPACITY: usize = 128;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Primitive {
    pub center: [f32; 4], // xyz in metres, w: 0 box, 1 ellipsoid
    pub extent: [f32; 4], // half extents; w = rotation around local north
    pub albedo: [f32; 4],
}

fn shape(center: [f32; 3], extent: [f32; 3], albedo: [f32; 3], kind: f32, angle: f32) -> Primitive {
    Primitive {
        center: [center[0], center[1], center[2], kind],
        extent: [extent[0], extent[1], extent[2], angle],
        albedo: [albedo[0], albedo[1], albedo[2], 0.0],
    }
}

pub fn landscape(radius_m: f64) -> Vec<Primitive> {
    let mut objects = Vec::new();
    // Ground follows the sphere's curvature, even on small moons.
    let surface = |x: f32, z: f32| {
        let r = radius_m;
        let d2 = (x as f64).powi(2) + (z as f64).powi(2);
        (-d2 / (r + (r * r - d2).max(0.0).sqrt())) as f32
    };
    let wood = [0.19, 0.095, 0.035];
    // Open foreground; an irregular, sparse tree line around the lookout.
    for i in 0..28 {
        let angle = i as f32 * std::f32::consts::TAU / 28.0 + 0.06 * (i as f32 * 2.3).sin();
        let distance = 100.0 + 75.0 * (0.5 + 0.5 * (i as f32 * 1.7).sin());
        let x = distance * angle.sin();
        let z = distance * angle.cos();
        let base = surface(x, z);
        let height = 6.5 + 3.5 * (0.5 + 0.5 * (i as f32 * 3.1).sin());
        objects.push(shape(
            [x, base + height * 0.3, z],
            [0.23, height * 0.3, 0.23],
            wood,
            0.0,
            0.0,
        ));
        let green = [
            0.075 + 0.015 * (i % 3) as f32,
            0.20 + 0.02 * (i % 4) as f32,
            0.045,
        ];
        objects.push(shape(
            [x, base + height * 0.64, z],
            [2.9, height * 0.33, 2.6],
            green,
            1.0,
            0.0,
        ));
        objects.push(shape(
            [x + 1.0, base + height * 0.77, z - 0.35],
            [2.1, height * 0.26, 2.2],
            green,
            1.0,
            0.0,
        ));
    }
    // Cottage off to the east, leaving the central observing view open.
    let (x, z) = (22.0, 30.0);
    let base = surface(x, z);
    let plaster = [0.65, 0.52, 0.34];
    let roof = [0.26, 0.07, 0.035];
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
    objects
}
