//! Embedded Blueprint typography. One coverage atlas serves both native fonts.
use std::collections::HashMap;
use std::sync::OnceLock;

use fontdue::{Font, FontSettings, Metrics};

pub const WIDTH: usize = 2048;
pub const HEIGHT: usize = 2048;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Face {
    Mono,
    Sans,
    Display,
}

pub struct Glyph {
    pub metrics: Metrics,
    pub uv: [[f32; 2]; 4],
    pub em: f32,
}

pub struct Atlas {
    pub bitmap: Vec<u8>,
    glyphs: HashMap<(Face, char), Glyph>,
}

pub fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(|| {
        let mono = Font::from_bytes(
            include_bytes!("../prototypes/menu-ui/assets/plex-mono.ttf") as &[u8],
            FontSettings::default(),
        )
        .expect("bundled Plex Mono");
        let sans = Font::from_bytes(
            include_bytes!("../prototypes/menu-ui/assets/dm-sans.ttf") as &[u8],
            FontSettings::default(),
        )
        .expect("bundled DM Sans");
        let chars: Vec<char> = (32u8..127)
            .map(char::from)
            .chain("←→↑↓·°−×◇◆▸✕✓…↗".chars())
            .collect();
        let mut bitmap = vec![0; WIDTH * HEIGHT];
        // A padded white texel for geometric primitives, with linear filtering.
        for y in 0..4 {
            for x in 0..4 {
                bitmap[y * WIDTH + x] = 255;
            }
        }
        let mut glyphs = HashMap::new();
        let (mut x, mut y, mut row) = (8, 8, 0);
        for (face, font, em) in [
            (Face::Mono, &mono, 32.0),
            (Face::Sans, &sans, 64.0),
            (Face::Display, &sans, 128.0),
        ] {
            for &ch in &chars {
                let (metrics, pixels) = font.rasterize(ch, em);
                if x + metrics.width + 2 >= WIDTH {
                    x = 8;
                    y += row + 2;
                    row = 0;
                }
                assert!(y + metrics.height + 2 < HEIGHT, "font atlas capacity");
                for j in 0..metrics.height {
                    bitmap[(y + j) * WIDTH + x..(y + j) * WIDTH + x + metrics.width]
                        .copy_from_slice(&pixels[j * metrics.width..(j + 1) * metrics.width]);
                }
                let (u0, v0, u1, v1) = (
                    x as f32 / WIDTH as f32,
                    y as f32 / HEIGHT as f32,
                    (x + metrics.width) as f32 / WIDTH as f32,
                    (y + metrics.height) as f32 / HEIGHT as f32,
                );
                glyphs.insert(
                    (face, ch),
                    Glyph {
                        metrics,
                        uv: [[u0, v0], [u1, v0], [u0, v1], [u1, v1]],
                        em,
                    },
                );
                x += metrics.width + 2;
                row = row.max(metrics.height);
            }
        }
        Atlas { bitmap, glyphs }
    })
}

pub fn glyph(face: Face, ch: char) -> &'static Glyph {
    let atlas = atlas();
    atlas
        .glyphs
        .get(&(face, ch))
        .unwrap_or(&atlas.glyphs[&(face, '?')])
}

pub fn width(face: Face, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|ch| {
            let g = glyph(face, ch);
            g.metrics.advance_width * px / g.em
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fonts_and_unicode_have_coverage_and_padded_uvs() {
        let a = atlas();
        assert_eq!(a.bitmap.len(), WIDTH * HEIGHT);
        assert_eq!(a.bitmap[WIDTH + 1], 255);
        for face in [Face::Mono, Face::Sans, Face::Display] {
            for ch in ['A', 'g', '←', '→', '°'] {
                let g = glyph(face, ch);
                assert!(g.metrics.width > 0 && g.metrics.height > 0);
                assert!(g.uv.iter().flatten().all(|n| (0.0..1.0).contains(n)));
            }
        }
    }
}
