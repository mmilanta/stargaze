//! Embedded Blueprint fonts, rasterized at their physical screen size.
//! UI and its GPU uploader share a thread-local coverage atlas. Cached glyphs
//! remain sharp across DPI changes without changing the layout's font metrics.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use fontdue::{Font, FontSettings, Metrics};

pub const WIDTH: usize = 2048;
pub const HEIGHT: usize = 2048;
const SYMBOLS: &str = "←→↑↓↔·°−–—×±◇◆▸✕✓…↗";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Face {
    Mono,
    Sans,
    Display,
}

#[derive(Clone, Copy)]
pub struct Glyph {
    pub metrics: Metrics,
    pub uv: [[f32; 2]; 4],
    pub em: f32,
}

struct Atlas {
    bitmap: Vec<u8>,
    glyphs: HashMap<(Face, char, u32), Glyph>,
    revision: u64,
    x: usize,
    y: usize,
    row: usize,
    frame: Option<([u32; 2], u32)>,
}

impl Atlas {
    fn new() -> Self {
        let mut atlas = Self {
            bitmap: vec![0; WIDTH * HEIGHT],
            glyphs: HashMap::new(),
            revision: 0,
            x: 8,
            y: 8,
            row: 0,
            frame: None,
        };
        atlas.clear();
        atlas
    }

    fn clear(&mut self) {
        self.bitmap.fill(0);
        // Linear-filtered geometry samples a padded white texel.
        for y in 0..4 {
            self.bitmap[y * WIDTH..y * WIDTH + 4].fill(255);
        }
        self.glyphs.clear();
        self.x = 8;
        self.y = 8;
        self.row = 0;
        self.revision += 1;
    }

    fn glyph(&mut self, face: Face, ch: char, px: f32) -> Glyph {
        // Quarter-pixel sizes prevent animated/fitted text filling the cache with
        // near-identical bitmaps; resampling is at most 1/8px away from native size.
        let em = (px * 4.0).round().max(1.0) / 4.0;
        let ch = supported(ch);
        let key = (face, ch, em.to_bits());
        if let Some(glyph) = self.glyphs.get(&key) {
            return *glyph;
        }
        let (metrics, pixels) = font(face).rasterize(ch, em);
        if self.x + metrics.width + 2 >= WIDTH {
            self.x = 8;
            self.y += self.row + 2;
            self.row = 0;
        }
        assert!(
            self.y + metrics.height + 2 < HEIGHT,
            "physical font atlas capacity"
        );
        for j in 0..metrics.height {
            let start = (self.y + j) * WIDTH + self.x;
            self.bitmap[start..start + metrics.width]
                .copy_from_slice(&pixels[j * metrics.width..(j + 1) * metrics.width]);
        }
        let (u0, v0, u1, v1) = (
            self.x as f32 / WIDTH as f32,
            self.y as f32 / HEIGHT as f32,
            (self.x + metrics.width) as f32 / WIDTH as f32,
            (self.y + metrics.height) as f32 / HEIGHT as f32,
        );
        let glyph = Glyph {
            metrics,
            uv: [[u0, v0], [u1, v0], [u0, v1], [u1, v1]],
            em,
        };
        self.glyphs.insert(key, glyph);
        self.x += metrics.width + 2;
        self.row = self.row.max(metrics.height);
        self.revision += 1;
        glyph
    }
}

thread_local! { static ATLAS: RefCell<Atlas> = RefCell::new(Atlas::new()); }

fn font(face: Face) -> &'static Font {
    static FONTS: OnceLock<[Font; 3]> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        [
            Font::from_bytes(
                include_bytes!("../prototypes/menu-ui/assets/plex-mono.ttf") as &[u8],
                FontSettings::default(),
            )
            .expect("bundled Plex Mono"),
            Font::from_bytes(
                include_bytes!("../assets/fonts/dm-sans-medium.ttf") as &[u8],
                FontSettings::default(),
            )
            .expect("bundled DM Sans"),
            Font::from_bytes(
                include_bytes!("../assets/fonts/dm-sans-display.ttf") as &[u8],
                FontSettings::default(),
            )
            .expect("bundled DM Sans Display"),
        ]
    });
    &fonts[face as usize]
}

fn supported(ch: char) -> char {
    if (' '..='~').contains(&ch) || SYMBOLS.contains(ch) {
        ch
    } else {
        '?'
    }
}

/// Reset at frame boundaries, so all vertices in a frame retain valid UVs.
/// Resize/DPI changes discard obsolete sizes; a high-water mark bounds the cache.
pub fn begin_frame(viewport: [u32; 2], scale: f32) {
    ATLAS.with_borrow_mut(|atlas| {
        let frame = (viewport, scale.to_bits());
        if atlas.frame != Some(frame) || atlas.y + atlas.row > HEIGHT * 3 / 4 {
            atlas.clear();
            atlas.frame = Some(frame);
        }
    });
}

pub fn glyph(face: Face, ch: char, px: f32) -> Glyph {
    ATLAS.with_borrow_mut(|atlas| atlas.glyph(face, ch, px))
}

pub struct Snapshot {
    pub bitmap: Vec<u8>,
    pub revision: u64,
}

/// Copy/upload only when a new glyph or frame reset changed the atlas.
pub fn snapshot(uploaded_revision: u64) -> Option<Snapshot> {
    ATLAS.with_borrow(|atlas| {
        (atlas.revision != uploaded_revision).then(|| Snapshot {
            bitmap: atlas.bitmap.clone(),
            revision: atlas.revision,
        })
    })
}

pub fn width(face: Face, text: &str, px: f32) -> f32 {
    let font = font(face);
    text.chars()
        .map(|ch| font.metrics(supported(ch), px).advance_width)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_sizes_have_native_coverage_and_preserve_metrics() {
        begin_frame([3840, 2160], 2.0);
        for face in [Face::Mono, Face::Sans, Face::Display] {
            for scale in [1.0, 1.25, 1.5, 2.0, 3.0, 4.0] {
                for ch in ['A', 'g', '←', '→', '°', '–'] {
                    let px = 10.5 * scale;
                    let g = glyph(face, ch, px);
                    assert!((g.em - px).abs() <= 0.125);
                    assert!(g.metrics.width > 0 && g.metrics.height > 0);
                    assert_eq!(g.metrics, font(face).metrics(ch, g.em));
                    assert!(g.uv.iter().flatten().all(|n| (0.0..1.0).contains(n)));
                }
                let text = "Observe +1,000 min/s ←";
                assert!(
                    (width(face, text, 10.5 * scale) - width(face, text, 10.5) * scale).abs()
                        < 0.001
                );
            }
        }
        let small = glyph(Face::Mono, 'M', 10.5);
        let large = glyph(Face::Mono, 'M', 42.0);
        assert!(large.metrics.height >= small.metrics.height * 3);
        assert_ne!(large.uv, small.uv);
        let a = snapshot(0).unwrap();
        assert_eq!(a.bitmap.len(), WIDTH * HEIGHT);
        assert_eq!(a.bitmap[WIDTH + 1], 255);
        assert!(a.bitmap.iter().any(|&p| p > 0 && p < 255));
    }

    #[test]
    fn cached_glyphs_skip_uploads_and_resizing_invalidates_old_sizes() {
        begin_frame([1920, 1080], 1.0);
        glyph(Face::Mono, 'A', 10.5);
        let first = snapshot(0).unwrap().revision;
        glyph(Face::Mono, 'A', 10.5);
        assert!(snapshot(first).is_none());
        width(Face::Sans, "layout does not rasterize", 26.0);
        assert!(snapshot(first).is_none());
        glyph(Face::Mono, 'B', 10.5);
        let next = snapshot(first).unwrap().revision;
        begin_frame([3840, 2160], 2.0);
        assert!(snapshot(next).is_some());
        ATLAS.with_borrow(|atlas| assert!(atlas.glyphs.is_empty()));
        let glyph = glyph(Face::Mono, 'A', 21.0);
        assert_eq!(glyph.em, 21.0);
        begin_frame([3840, 2160], 2.0);
        let current = snapshot(0).unwrap().revision;
        assert!(snapshot(current).is_none());
    }
    #[test]
    #[ignore = "requires a GPU; checks native glyph coverage and atlas reuploads"]
    fn gpu_physical_font_coverage_matches_raster() {
        let mut gpu = crate::ui_previews::Preview::new();
        for (face, requested, ch) in [
            (Face::Mono, 10.5, 'g'),
            (Face::Mono, 13.125, 'W'),
            (Face::Mono, 21.0, 'g'),
            (Face::Sans, 52.0, 'A'),
            (Face::Display, 392.0, 'g'),
            (Face::Mono, 21.0, '←'),
            (Face::Mono, 10.5, 'g'),
        ] {
            begin_frame([3840, 2160], requested);
            let g = glyph(face, ch, requested);
            let (m, reference) = font(face).rasterize(ch, g.em);
            let (w, h) = (m.width as u32 + 8, m.height as u32 + 8);
            let mut v = Vec::new();
            crate::ui::font_text(
                &mut v,
                [
                    4.0 - m.xmin as f32,
                    4.0 + (m.ymin as f32 + m.height as f32) - g.em * 0.8,
                ],
                g.em,
                face,
                &ch.to_string(),
                [1.0; 4],
                [w as f32, h as f32],
            );
            let pixels = gpu.render(&v, w, h, None);
            for y in 0..m.height {
                for x in 0..m.width {
                    let sample = pixels[((y + 4) * w as usize + x + 4) * 4];
                    let expected = reference[y * m.width + x];
                    assert!(
                        sample.abs_diff(expected) <= 2,
                        "{face:?} {ch} at {}px: ({x},{y}) expected {expected}, got {sample}",
                        g.em
                    );
                }
            }
        }
    }
}
