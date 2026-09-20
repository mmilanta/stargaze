//! Minimal immediate-mode UI: an 8x8 bitmap font atlas, a bottom bar with a
//! labels toggle and delicate time-step buttons, and screen-space body labels.
//!
//! All geometry is generated in physical pixels (origin top-left) and converted
//! to NDC when pushed, so the caller only deals with screen coordinates.

use bytemuck::{Pod, Zeroable};
use font8x8::{BASIC_FONTS, UnicodeFonts};

pub const ATLAS_COLS: usize = 16;
pub const ATLAS_ROWS: usize = 8;
pub const GLYPH: usize = 8;
pub const ATLAS_W: usize = ATLAS_COLS * GLYPH; // 128
pub const ATLAS_H: usize = ATLAS_ROWS * GLYPH; // 64
/// Rendering text taller than 8px makes it readable at ordinary DPI.
const TEXT_SCALE: f32 = 2.0;
const SOLID_CELL: usize = 127;

/// Time steps, in simulated days, applied by the bar buttons.
pub const TIME_BUTTONS: [(&str, f64); 6] = [
    ("-1d", -1.0),
    ("-1h", -1.0 / 24.0),
    ("-1m", -1.0 / 1440.0),
    ("+1m", 1.0 / 1440.0),
    ("+1h", 1.0 / 24.0),
    ("+1d", 1.0),
];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct Label {
    /// Screen position of the body centre (physical pixels).
    pub x: f32,
    pub y: f32,
    /// On-screen radius of the body, in pixels.
    pub radius: f32,
    pub text: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x as f64
            && px <= (self.x + self.w) as f64
            && py >= self.y as f64
            && py <= (self.y + self.h) as f64
    }
    fn center_y(&self) -> f32 {
        self.y + self.h * 0.5
    }
    fn right(&self) -> f32 {
        self.x + self.w
    }
}

pub struct Layout {
    pub bar: Rect,
    pub toggle: Rect,
    /// One rectangle per entry of [`TIME_BUTTONS`].
    pub buttons: [Rect; TIME_BUTTONS.len()],
    pub scale: f32,
}

pub fn layout(w: f32, h: f32, scale: f32) -> Layout {
    // Shrink the whole HUD if the window is too narrow for it, so the toggle,
    // the six buttons and the clock always fit.
    let natural = 12.0 + 116.0 + 18.0 + 6.0 * (52.0 + 6.0) + 14.0 + 220.0;
    let s = scale.min((w / natural).max(scale * 0.45));

    let bar_h = 46.0 * s;
    let bar = Rect { x: 0.0, y: h - bar_h, w, h: bar_h };
    let pad = 12.0 * s;
    let toggle = Rect {
        x: pad,
        y: bar.y + (bar_h - 28.0 * s) * 0.5,
        w: 116.0 * s,
        h: 28.0 * s,
    };
    let bw = 52.0 * s;
    let bh = 28.0 * s;
    let gap = 6.0 * s;
    let mut bx = toggle.right() + 18.0 * s;
    let mut buttons = [Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }; TIME_BUTTONS.len()];
    for b in buttons.iter_mut() {
        *b = Rect { x: bx, y: bar.y + (bar_h - bh) * 0.5, w: bw, h: bh };
        bx += bw + gap;
    }
    Layout { bar, toggle, buttons, scale: s }
}

/// Build the atlas bitmap (coverage in R) and return it as a tight byte buffer.
pub fn build_atlas() -> Vec<u8> {
    let mut data = vec![0u8; ATLAS_W * ATLAS_H];
    for code in 32u32..127 {
        let Some(ch) = char::from_u32(code) else { continue };
        let Some(glyph) = BASIC_FONTS.get(ch) else { continue };
        let idx = (code - 32) as usize;
        let (cx, cy) = (idx % ATLAS_COLS, idx / ATLAS_COLS);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..GLYPH {
                if bits & (1 << col) != 0 {
                    data[(cy * GLYPH + row) * ATLAS_W + cx * GLYPH + col] = 255;
                }
            }
        }
    }
    // A fully opaque cell used to draw solid rectangles through the same shader.
    let (cx, cy) = (SOLID_CELL % ATLAS_COLS, SOLID_CELL / ATLAS_COLS);
    for y in 0..GLYPH {
        for x in 0..GLYPH {
            data[(cy * GLYPH + y) * ATLAS_W + cx * GLYPH + x] = 255;
        }
    }
    data
}

fn solid_uv() -> [f32; 2] {
    let (cx, cy) = (SOLID_CELL % ATLAS_COLS, SOLID_CELL / ATLAS_COLS);
    [
        (cx as f32 + 0.5) * GLYPH as f32 / ATLAS_W as f32,
        (cy as f32 + 0.5) * GLYPH as f32 / ATLAS_H as f32,
    ]
}

fn glyph_uv(code: u32) -> [[f32; 2]; 4] {
    let idx = if (32..127).contains(&code) {
        (code - 32) as usize
    } else {
        // Unknown glyphs fall back to the solid cell (a filled box).
        SOLID_CELL
    };
    let (cx, cy) = (idx % ATLAS_COLS, idx / ATLAS_COLS);
    let u0 = (cx * GLYPH) as f32 / ATLAS_W as f32;
    let v0 = (cy * GLYPH) as f32 / ATLAS_H as f32;
    let u1 = ((cx + 1) * GLYPH) as f32 / ATLAS_W as f32;
    let v1 = ((cy + 1) * GLYPH) as f32 / ATLAS_H as f32;
    [[u0, v0], [u1, v0], [u0, v1], [u1, v1]]
}

fn to_ndc(x: f32, y: f32, w: f32, h: f32) -> [f32; 2] {
    [2.0 * x / w - 1.0, 1.0 - 2.0 * y / h]
}

#[allow(clippy::too_many_arguments)]
fn push_quad(
    verts: &mut Vec<UiVertex>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    uv: [[f32; 2]; 4],
    color: [f32; 4],
    w: f32,
    h: f32,
) {
    let p00 = to_ndc(x0, y0, w, h);
    let p10 = to_ndc(x1, y0, w, h);
    let p01 = to_ndc(x0, y1, w, h);
    let p11 = to_ndc(x1, y1, w, h);
    for (pos, u) in [
        (p00, uv[0]),
        (p10, uv[1]),
        (p01, uv[2]),
        (p01, uv[2]),
        (p10, uv[1]),
        (p11, uv[3]),
    ] {
        verts.push(UiVertex { pos, uv: u, color });
    }
}

fn push_rect(
    verts: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    rw: f32,
    rh: f32,
    color: [f32; 4],
    w: f32,
    h: f32,
) {
    let uv = solid_uv();
    push_quad(verts, x, y, x + rw, y + rh, [uv; 4], color, w, h);
}

pub fn text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * 8.0 * scale
}

fn push_text(
    verts: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    scale: f32,
    text: &str,
    color: [f32; 4],
    w: f32,
    h: f32,
) {
    let step = 8.0 * scale;
    let mut cx = x;
    for ch in text.chars() {
        if ch != ' ' {
            push_quad(
                verts,
                cx,
                y,
                cx + step,
                y + step,
                glyph_uv(ch as u32),
                color,
                w,
                h,
            );
        }
        cx += step;
    }
}

/// Emit the whole HUD: labels first (in the sky), then the bottom bar.
#[allow(clippy::too_many_arguments)]
pub fn build(
    verts: &mut Vec<UiVertex>,
    layout: &Layout,
    w: f32,
    h: f32,
    show_labels: bool,
    sim_time: f64,
    labels: &[Label],
) {
    let s = layout.scale;
    if show_labels {
        build_labels(verts, labels, s, w, h);
    }
    build_bar(verts, layout, w, h, show_labels, sim_time);
}

fn build_labels(verts: &mut Vec<UiVertex>, labels: &[Label], s: f32, w: f32, h: f32) {
    let text_scale = TEXT_SCALE * s * 0.85;
    for label in labels {
        let tw = text_width(&label.text, text_scale);
        let th = 8.0 * text_scale;
        let mut x = label.x + label.radius + 7.0 * s;
        if x + tw > w - 6.0 * s {
            x = label.x - label.radius - 7.0 * s - tw;
        }
        let y = label.y - th * 0.5;
        // Dark plate for readability.
        push_rect(
            verts,
            x - 3.0 * s,
            y - 2.0 * s,
            tw + 6.0 * s,
            th + 4.0 * s,
            [0.0, 0.0, 0.0, 0.55],
            w,
            h,
        );
        // Marker tick next to the body.
        push_rect(
            verts,
            label.x + label.radius + 1.0 * s,
            label.y - 0.5 * s,
            4.0 * s,
            1.0 * s,
            [0.75, 0.85, 1.0, 0.9],
            w,
            h,
        );
        push_text(
            verts,
            x,
            y,
            text_scale,
            &label.text,
            [0.92, 0.95, 1.0, 1.0],
            w,
            h,
        );
    }
}

fn build_bar(
    verts: &mut Vec<UiVertex>,
    layout: &Layout,
    w: f32,
    h: f32,
    show_labels: bool,
    sim_time: f64,
) {
    let s = layout.scale;
    let bar = layout.bar;
    // Panel + top hairline.
    push_rect(verts, bar.x, bar.y, bar.w, bar.h, [0.015, 0.02, 0.03, 0.86], w, h);
    push_rect(verts, bar.x, bar.y, bar.w, 1.0 * s, [1.0, 1.0, 1.0, 0.10], w, h);

    // --- labels toggle ---
    let t = layout.toggle;
    let bg = if show_labels {
        [0.16, 0.34, 0.55, 0.95]
    } else {
        [0.10, 0.11, 0.13, 0.95]
    };
    push_rect(verts, t.x, t.y, t.w, t.h, bg, w, h);
    push_rect(verts, t.x, t.y, t.w, 1.0 * s, [1.0, 1.0, 1.0, 0.12], w, h);
    let box_s = 12.0 * s;
    let bx = t.x + 8.0 * s;
    let by = t.center_y() - box_s * 0.5;
    push_rect(verts, bx, by, box_s, box_s, [0.05, 0.06, 0.07, 1.0], w, h);
    if show_labels {
        push_rect(
            verts,
            bx + 3.0 * s,
            by + 3.0 * s,
            box_s - 6.0 * s,
            box_s - 6.0 * s,
            [0.55, 0.78, 1.0, 1.0],
            w,
            h,
        );
    }
    let ts = TEXT_SCALE * s * 0.85;
    push_text(
        verts,
        bx + box_s + 8.0 * s,
        t.center_y() - 4.0 * ts,
        ts,
        "Labels",
        [0.9, 0.93, 0.97, 1.0],
        w,
        h,
    );

    // --- delicate time-step buttons ---
    for (rect, (caption, _)) in layout.buttons.iter().zip(TIME_BUTTONS.iter()) {
        push_rect(
            verts,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            [0.12, 0.13, 0.16, 0.95],
            w,
            h,
        );
        push_rect(verts, rect.x, rect.y, rect.w, 1.0 * s, [1.0, 1.0, 1.0, 0.14], w, h);
        let tw = text_width(caption, ts);
        push_text(
            verts,
            rect.x + (rect.w - tw) * 0.5,
            rect.center_y() - 4.0 * ts,
            ts,
            caption,
            [0.88, 0.92, 0.98, 1.0],
            w,
            h,
        );
    }

    // --- current time, right aligned (only if it clears the buttons) ---
    let time_text = format!("t = {:.4} d", sim_time);
    let tw = text_width(&time_text, ts);
    let pad = 12.0 * s;
    let buttons_end = layout.buttons.last().map(|b| b.right()).unwrap_or(0.0);
    let x = w - pad - tw;
    if x > buttons_end + 10.0 * s {
        push_text(
            verts,
            x,
            bar.center_y() - 4.0 * ts,
            ts,
            &time_text,
            [0.85, 0.88, 0.92, 1.0],
            w,
            h,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_are_disjoint_and_hit_testable() {
        let l = layout(1400.0, 800.0, 1.0);
        let tc = (
            (l.toggle.x + l.toggle.w * 0.5) as f64,
            (l.toggle.y + l.toggle.h * 0.5) as f64,
        );
        assert!(l.toggle.contains(tc.0, tc.1));
        for b in &l.buttons {
            assert!(!b.contains(tc.0, tc.1), "toggle must not overlap a button");
        }

        for b in &l.buttons {
            let c = ((b.x + b.w * 0.5) as f64, (b.y + b.h * 0.5) as f64);
            assert!(b.contains(c.0, c.1));
            assert!(!l.toggle.contains(c.0, c.1));
            assert!(l.bar.contains(c.0, c.1));
        }

        // Buttons must not run into each other.
        for pair in l.buttons.windows(2) {
            assert!(pair[0].right() <= pair[1].x);
        }

        // A point in the sky must not be swallowed by the bar.
        assert!(!l.bar.contains(700.0, 100.0));
    }

    #[test]
    fn time_buttons_step_both_ways() {
        let deltas: Vec<f64> = TIME_BUTTONS.iter().map(|(_, d)| *d).collect();
        assert_eq!(deltas.len(), 6);
        assert!(deltas[0] < 0.0 && deltas[1] < 0.0 && deltas[2] < 0.0);
        assert!(deltas[3] > 0.0 && deltas[4] > 0.0 && deltas[5] > 0.0);
        // Symmetric, and one minute really is a minute.
        for i in 0..3 {
            assert!((deltas[i] + deltas[5 - i]).abs() < 1e-12);
        }
        assert!((deltas[5] - 1.0).abs() < 1e-12);
        assert!((deltas[4] - 1.0 / 24.0).abs() < 1e-12);
        assert!((deltas[3] - 1.0 / 1440.0).abs() < 1e-12);
    }

    #[test]
    fn time_text_clears_the_buttons_at_hidpi() {
        // Surface width and scale observed on this machine.
        let (w, scale) = (1190.0_f32, 1.6_f32);
        let l = layout(w, 800.0, scale);
        let ts = TEXT_SCALE * scale * 0.85;
        let buttons_end = l.buttons.last().unwrap().right();
        assert!(l.buttons[0].x > l.toggle.right());
        assert!(buttons_end < w, "buttons must fit on screen");
        for text in ["t = 8.7847 d", "t = 9999.9999 d"] {
            let tw = text_width(text, ts);
            let x = w - 12.0 * scale - tw;
            assert!(x > buttons_end + 10.0 * scale, "'{text}' overlaps the buttons");
        }
    }

    #[test]
    fn hud_fits_a_narrow_window() {
        // A narrow tiled window: 581 surface px at scale 1.6.
        let (w, scale) = (581.0_f32, 1.6_f32);
        let l = layout(w, 700.0, scale);
        let buttons_end = l.buttons.last().unwrap().right();
        assert!(buttons_end < w, "buttons overflow a narrow window");
        let ts = TEXT_SCALE * l.scale * 0.85;
        let tw = text_width("t = 9999.9999 d", ts);
        let x = w - 12.0 * l.scale - tw;
        assert!(x > buttons_end + 10.0 * l.scale, "clock overlaps the buttons");
    }

    #[test]
    fn atlas_contains_the_solid_cell_and_glyphs() {
        let a = build_atlas();
        assert_eq!(a.len(), ATLAS_W * ATLAS_H);
        let (cx, cy) = (SOLID_CELL % ATLAS_COLS, SOLID_CELL / ATLAS_COLS);
        assert_eq!(a[(cy * GLYPH) * ATLAS_W + cx * GLYPH], 255);

        let idx = ('A' as u32 - 32) as usize;
        let (ax, ay) = (idx % ATLAS_COLS, idx / ATLAS_COLS);
        let mut ink = 0u32;
        for y in 0..GLYPH {
            for x in 0..GLYPH {
                ink += a[(ay * GLYPH + y) * ATLAS_W + ax * GLYPH + x] as u32;
            }
        }
        assert!(ink > 0, "'A' glyph should have ink");
    }
}
