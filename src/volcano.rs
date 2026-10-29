//! Filtered measured elevation for volcanic shading, separate from the
//! intersection mesh. Metadata and source transforms share the CPU layout.
use std::sync::OnceLock;

pub(super) const SITES: [(usize, f32, f32, f32, f32); 8] = [
    (1, 8.0, 12_300.0, 0.0, 1.2),
    (2, 52.0, 12_800.0, -40.0, 1.0),
    (0, 95.0, 11_800.0, 160.0, 0.85),
    (1, 138.0, 13_200.0, 90.0, 0.8),
    (2, 182.0, 12_000.0, 30.0, 1.15),
    (0, 228.0, 13_000.0, 300.0, 0.9),
    (1, 270.0, 11_600.0, 200.0, 1.05),
    (2, 318.0, 12_600.0, 120.0, 0.95),
];
// Four vec4 map descriptors, then nine two-vec4 site records. Each map is
// a complete area-averaged mip pyramid; odd edges repeat their last sample.
const HEADER: usize = 4 * 4 + 9 * 8;
const MAPS: [(usize, f32); 4] = [
    (super::STROMBOLI_SAMPLES, 5.0),
    (super::SUMMIT_SAMPLES, 1.0),
    (super::AEOLIAN_SAMPLES, super::AEOLIAN_SPACING),
    (super::AEOLIAN_SAMPLES, super::AEOLIAN_SPACING),
];

const fn pyramid_len(mut width: usize) -> usize {
    let mut count = width * width;
    while width > 1 {
        width = width.div_ceil(2);
        count += width * width;
    }
    count
}
pub(super) const DETAIL_FLOATS: usize = HEADER
    + pyramid_len(MAPS[0].0)
    + pyramid_len(MAPS[1].0)
    + pyramid_len(MAPS[2].0)
    + pyramid_len(MAPS[3].0);

/// Built once, uploaded once when Basalt is selected. The maps carry real
/// elevations, not pre-lit colours, and remain fixed in the ground frame.
pub fn detail() -> &'static [f32] {
    static DETAIL: OnceLock<Vec<f32>> = OnceLock::new();
    DETAIL.get_or_init(|| {
        let mut data = vec![0.0; HEADER];
        data[16..24].copy_from_slice(&[0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
        for (i, &(island, bearing, distance, turn, scale)) in SITES.iter().enumerate() {
            let (b, t) = (bearing.to_radians(), turn.to_radians());
            data[24 + i * 8..32 + i * 8].copy_from_slice(&[
                distance * b.sin(),
                distance * b.cos(),
                t.cos(),
                t.sin(),
                scale,
                if island == 0 {
                    0.0
                } else {
                    (island + 1) as f32
                },
                0.0,
                0.0,
            ]);
        }
        for (map, &(width, spacing)) in MAPS.iter().enumerate() {
            let offset = data.len();
            data[map * 4..map * 4 + 4].copy_from_slice(&[
                offset as f32,
                width as f32,
                spacing,
                if map == 1 { 250.0 } else { 0.0 },
            ]);
            let (source, start) = match map {
                0 => (super::STROMBOLI, 0),
                1 => (super::STROMBOLI, super::STROMBOLI_SAMPLES.pow(2)),
                _ => (super::AEOLIAN, (map - 1) * super::AEOLIAN_SAMPLES.pow(2)),
            };
            let mut level: Vec<f32> = source[2 * start..2 * (start + width * width)]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| f32::from(u16::from_le_bytes([p[0], p[1]])) * 0.1)
                .collect();
            let mut width = width;
            loop {
                data.extend_from_slice(&level);
                if width == 1 {
                    break;
                }
                let next = width.div_ceil(2);
                level = (0..next * next)
                    .map(|i| {
                        let (x, z) = ((i % next) * 2, (i / next) * 2);
                        let (x1, z1) = ((x + 1).min(width - 1), (z + 1).min(width - 1));
                        0.25 * (level[z * width + x]
                            + level[z * width + x1]
                            + level[z1 * width + x]
                            + level[z1 * width + x1])
                    })
                    .collect();
                width = next;
            }
        }
        assert_eq!(data.len(), DETAIL_FLOATS);
        data
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survey_preserves_summit_vent_floor_and_collapse_flank() {
        // Surveyed elevations in metres, at coordinates relative to Pizzo.
        // These asymmetric depressions must survive without a cone blend
        // or an invented summit bowl filling them back in.
        for (x, z, height) in [
            (0.0, 0.0, 924.4),
            (-300.0, 420.0, 738.9),
            (-800.0, 1000.0, 295.3),
            (300.0, -200.0, 745.4),
        ] {
            assert!((super::super::stromboli(x, z) - height).abs() < 2.0);
        }
        assert_eq!(super::super::stromboli(3200.0, 0.0), 0.0);
        // The nested survey blends into the whole island without a height step.
        for z in [-262.0, -190.0, 690.0, 762.0] {
            assert!(
                (super::super::stromboli(100.0, z - 0.001)
                    - super::super::stromboli(100.0, z + 0.001))
                .abs()
                    < 0.05
            );
        }
    }

    #[test]
    fn elevation_atlas_is_finite_bounded_and_fits_default_gpu_limits() {
        let atlas = detail();
        assert_eq!(atlas.len(), DETAIL_FLOATS);
        assert!(atlas.iter().all(|v| v.is_finite()));
        let mut end = HEADER;
        for map in 0..4 {
            let desc = &atlas[map * 4..map * 4 + 4];
            assert_eq!(desc[0] as usize, end);
            let width = desc[1] as usize;
            let count = pyramid_len(width);
            assert!(
                atlas[end..end + count]
                    .iter()
                    .all(|h| *h >= 0.0 && *h < 1200.0)
            );
            // Every averaged mip must stay within its source height range.
            let top = atlas[end..end + width * width]
                .iter()
                .copied()
                .fold(0.0_f32, f32::max);
            assert!(
                atlas[end + width * width..end + count]
                    .iter()
                    .all(|h| *h <= top)
            );
            end += count;
        }
        assert_eq!(end, atlas.len());
        let bytes = (super::super::GPU_FLOATS
            + crate::ground::CAPACITY * 12
            + crate::ground::SKY_TEXELS * 4)
            * 4;
        assert!(bytes <= wgpu::Limits::default().max_storage_buffer_binding_size as usize);
    }
}
