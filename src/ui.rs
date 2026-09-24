//! Minimal immediate-mode UI: an 8x8 bitmap font atlas, a bottom bar with a
//! labels toggle and signed playback controls, and screen-space body labels.
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
// Two Unicode arrows occupy unused atlas cells after the printable ASCII set.
const ARROW_LEFT_CELL: usize = 95;
const ARROW_RIGHT_CELL: usize = 96;

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
    /// Scene index of the labelled body, for lock highlighting.
    pub body: usize,
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
            && px <= self.right() as f64
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

#[derive(Clone, Copy, Debug, Default)]
pub struct LabelOptions {
    pub show: bool,
    /// Scene index of the body the view is locked onto, if any.
    pub locked: Option<usize>,
}

pub const MIN_EV: f32 = -8.0;
pub const MAX_EV: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct HudState<'a> {
    pub labels: LabelOptions,
    pub lock_label: Option<&'a str>,
    pub sim_time: f64,
    pub auto_exposure: bool,
    pub ev_bias: f32,
    pub steady_stars: bool,
    pub minutes_per_second: f64,
}

pub struct Layout {
    pub bar: Rect,
    pub home: Rect,
    pub toggle: Rect,
    pub scale: f32,
    pub exposure: Rect,
    pub auto: Rect,
    pub lock: Rect,
    pub orientation: Rect,
    pub draw: Rect,
    pub stop: Rect,
    pub slower: Rect,
    pub faster: Rect,
    pub speed: Rect,
    pub time_label: Rect,
    pub clock: Rect,
    requested_scale: f32,
    puzzle: bool,
}

impl Layout {
    pub fn with_puzzle(self, puzzle: bool) -> Self {
        layout_mode(
            self.bar.w,
            self.bar.y + self.bar.h,
            self.requested_scale,
            puzzle,
        )
    }

    pub fn exposure_bias_at(&self, x: f64) -> f32 {
        let fraction = ((x as f32 - self.exposure.x) / self.exposure.w).clamp(0.0, 1.0);
        ((MIN_EV + fraction * (MAX_EV - MIN_EV)) * 4.0).round() / 4.0
    }
}

pub fn layout(w: f32, h: f32, scale: f32) -> Layout {
    layout_mode(w, h, scale, false)
}

fn layout_mode(w: f32, h: f32, scale: f32, puzzle: bool) -> Layout {
    let s = scale.min(w / 1316.0).min(h / 100.0);
    let bar = Rect {
        x: 0.0,
        y: h - 52.0 * s,
        w,
        h: 52.0 * s,
    };
    let mut x = 10.0 * s;
    let mut control = |width: f32, gap: f32| {
        x += gap * s;
        let r = Rect {
            x,
            y: bar.y + 9.0 * s,
            w: width * s,
            h: 34.0 * s,
        };
        x += (width + 6.0) * s;
        r
    };
    let home = control(84.0, 0.0);
    let toggle = control(120.0, 0.0);
    let draw = toggle;
    let time_label = control(44.0, 18.0);
    let slower = control(34.0, 0.0);
    let speed = control(112.0, 0.0);
    let faster = control(34.0, 0.0);
    let stop = control(98.0, 0.0);
    let clock = control(164.0, 0.0);
    let lock = control(122.0, 18.0);
    let orientation = control(84.0, 0.0);
    let exposure = control(168.0, 18.0);
    let auto = control(110.0, 0.0);
    Layout {
        bar,
        home,
        lock,
        orientation,
        toggle,
        draw,
        stop,
        slower,
        speed,
        time_label,
        faster,
        clock,
        exposure,
        auto,
        scale: s,
        puzzle,
        requested_scale: scale,
    }
}

/// Build the atlas bitmap (coverage in R) and return it as a tight byte buffer.
pub fn build_atlas() -> Vec<u8> {
    let mut data = vec![0u8; ATLAS_W * ATLAS_H];
    for code in 32u32..127 {
        let Some(ch) = char::from_u32(code) else {
            continue;
        };
        let Some(glyph) = BASIC_FONTS.get(ch) else {
            continue;
        };
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
    for (cell, glyph) in [
        (ARROW_LEFT_CELL, [0, 0x08, 0x04, 0x7e, 0x04, 0x08, 0, 0]),
        (ARROW_RIGHT_CELL, [0, 0x10, 0x20, 0x7e, 0x20, 0x10, 0, 0]),
    ] {
        let (cx, cy) = (cell % ATLAS_COLS, cell / ATLAS_COLS);
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
        match code {
            0x2190 => ARROW_LEFT_CELL,
            0x2192 => ARROW_RIGHT_CELL,
            // Unknown glyphs fall back to the solid cell (a filled box).
            _ => SOLID_CELL,
        }
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

fn push_quad(
    verts: &mut Vec<UiVertex>,
    bounds: [[f32; 2]; 2],
    uv: [[f32; 2]; 4],
    color: [f32; 4],
    viewport: [f32; 2],
) {
    let [[x0, y0], [x1, y1]] = bounds;
    let [w, h] = viewport;
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

pub(crate) fn push_rect(
    verts: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    rw: f32,
    rh: f32,
    color: [f32; 4],
    viewport: [f32; 2],
) {
    let uv = solid_uv();
    push_quad(verts, [[x, y], [x + rw, y + rh]], [uv; 4], color, viewport);
}

pub fn text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * 8.0 * scale
}

pub(crate) fn push_text(
    verts: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    scale: f32,
    text: &str,
    color: [f32; 4],
    viewport: [f32; 2],
) {
    let step = 8.0 * scale;
    let mut cx = x;
    for ch in text.chars() {
        if ch != ' ' {
            push_quad(
                verts,
                [[cx, y], [cx + step, y + step]],
                glyph_uv(ch as u32),
                color,
                viewport,
            );
        }
        cx += step;
    }
}

/// Emit the whole HUD: labels first (in the sky), then the bottom bar.
pub fn build(
    verts: &mut Vec<UiVertex>,
    layout: &Layout,
    w: f32,
    h: f32,
    state: HudState<'_>,
    body_labels: &[Label],
) {
    let s = layout.scale;
    if state.labels.show {
        build_labels(verts, body_labels, state.labels.locked, s, w, h);
    }
    build_bar(verts, layout, w, h, state);
}

fn build_labels(
    verts: &mut Vec<UiVertex>,
    labels: &[Label],
    locked: Option<usize>,
    s: f32,
    w: f32,
    h: f32,
) {
    let text_scale = TEXT_SCALE * s * 0.85;
    for label in labels {
        let is_locked = locked == Some(label.body);
        if is_locked {
            // Corner brackets around the tracked body.
            let r = (label.radius + 5.0 * s).max(8.0 * s);
            let len = (r * 0.45).max(4.0 * s);
            let t = (1.5 * s).max(1.0);
            let c = [0.55, 0.85, 1.0, 0.95];
            for (x, y, dx, dy) in [
                (label.x - r, label.y - r, 1.0, 1.0),
                (label.x + r, label.y - r, -1.0, 1.0),
                (label.x - r, label.y + r, 1.0, -1.0),
                (label.x + r, label.y + r, -1.0, -1.0),
            ] {
                // Horizontal arm, then vertical arm, each tucked inside the
                // corner so the two overlap into a clean bracket.
                push_rect(
                    verts,
                    if dx > 0.0 { x } else { x - len },
                    if dy > 0.0 { y } else { y - t },
                    len,
                    t,
                    c,
                    [w, h],
                );
                push_rect(
                    verts,
                    if dx > 0.0 { x } else { x - t },
                    if dy > 0.0 { y } else { y - len },
                    t,
                    len,
                    c,
                    [w, h],
                );
            }
        }
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
            [w, h],
        );
        // Marker tick next to the body.
        push_rect(
            verts,
            label.x + label.radius + 1.0 * s,
            label.y - 0.5 * s,
            4.0 * s,
            1.0 * s,
            if is_locked {
                [0.55, 0.85, 1.0, 0.95]
            } else {
                [0.75, 0.85, 1.0, 0.9]
            },
            [w, h],
        );
        push_text(
            verts,
            x,
            y,
            text_scale,
            &label.text,
            if is_locked {
                [0.72, 0.92, 1.0, 1.0]
            } else {
                [0.92, 0.95, 1.0, 1.0]
            },
            [w, h],
        );
    }
}

/// Also drawn with the rest of the HUD hidden, so a lock can always be released.
pub fn build_lock(
    verts: &mut Vec<UiVertex>,
    layout: &Layout,
    viewport: [f32; 2],
    target: Option<&str>,
) {
    let s = layout.scale;
    let r = layout.lock;
    let color = if target.is_some() {
        [0.45, 0.88, 0.77, 1.0]
    } else {
        [0.58, 0.65, 0.74, 1.0]
    };
    push_rect(
        verts,
        r.x,
        r.y,
        r.w,
        r.h,
        if target.is_some() {
            [0.045, 0.17, 0.16, 0.98]
        } else {
            [0.05, 0.07, 0.10, 0.95]
        },
        viewport,
    );
    // Padlock body and shackle; unlocked shackle has an open right side.
    let x = r.x + 9.0 * s;
    let y = r.y + 12.0 * s;
    push_rect(verts, x, y, 12.0 * s, 10.0 * s, color, viewport);
    push_rect(
        verts,
        x + 2.0 * s,
        y - 6.0 * s,
        2.0 * s,
        7.0 * s,
        color,
        viewport,
    );
    push_rect(
        verts,
        x + 2.0 * s,
        y - 7.0 * s,
        8.0 * s,
        2.0 * s,
        color,
        viewport,
    );
    if target.is_some() {
        push_rect(
            verts,
            x + 8.0 * s,
            y - 6.0 * s,
            2.0 * s,
            7.0 * s,
            color,
            viewport,
        );
    }
    let label = if target.is_some() {
        "Locked [U]"
    } else {
        "Unlocked"
    };
    push_text(
        verts,
        r.x + 30.0 * s,
        r.y + 6.0 * s,
        1.0 * s,
        label,
        color,
        viewport,
    );
    let detail = target.unwrap_or("[U] / click sky");
    let columns = ((r.w - 12.0 * s) / (7.0 * s)).floor() as usize;
    let mut detail: String = detail
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    if detail.len() > columns {
        detail.truncate(columns.saturating_sub(3));
        detail.push_str("...");
    }
    push_text(
        verts,
        r.x + 6.0 * s,
        r.y + 23.0 * s,
        0.875 * s,
        &detail,
        color,
        viewport,
    );
}

pub(crate) fn toolbar_panel(
    verts: &mut Vec<UiVertex>,
    bar: Rect,
    scale: f32,
    groups: &[Rect],
    viewport: [f32; 2],
) {
    push_rect(
        verts,
        bar.x,
        bar.y,
        bar.w,
        bar.h,
        [0.015, 0.02, 0.03, 0.86],
        viewport,
    );
    push_rect(
        verts,
        bar.x,
        bar.y,
        bar.w,
        scale,
        [1.0, 1.0, 1.0, 0.10],
        viewport,
    );
    for next_group in groups {
        push_rect(
            verts,
            next_group.x - 12.0 * scale,
            bar.y + 13.0 * scale,
            scale,
            26.0 * scale,
            [0.32, 0.38, 0.46, 0.8],
            viewport,
        );
    }
}

pub(crate) fn toolbar_button(
    verts: &mut Vec<UiVertex>,
    rect: Rect,
    label: &str,
    scale: f32,
    active: bool,
    hovered: bool,
    viewport: [f32; 2],
) {
    let color = if active {
        [0.08, 0.25, 0.23, 0.98]
    } else if hovered {
        [0.12, 0.20, 0.28, 0.98]
    } else {
        [0.10, 0.13, 0.17, 0.95]
    };
    push_rect(verts, rect.x, rect.y, rect.w, rect.h, color, viewport);
    let size = if matches!(label, "←" | "→") {
        2.0
    } else {
        1.1
    } * scale;
    let size = size.min((rect.w - 8.0 * scale) / text_width(label, 1.0).max(1.0));
    push_text(
        verts,
        rect.x + (rect.w - text_width(label, size)) * 0.5,
        rect.center_y() - 4.0 * size,
        size,
        label,
        [0.88, 0.92, 0.98, 1.0],
        viewport,
    );
}

fn build_bar(verts: &mut Vec<UiVertex>, layout: &Layout, w: f32, h: f32, state: HudState<'_>) {
    let show_labels = state.labels.show;
    let sim_time = state.sim_time;
    let s = layout.scale;
    let bar = layout.bar;
    toolbar_panel(
        verts,
        bar,
        s,
        &[layout.time_label, layout.lock, layout.exposure],
        [w, h],
    );
    push_text(
        verts,
        layout.time_label.x,
        layout.time_label.center_y() - 4.4 * s,
        1.1 * s,
        "Time",
        [0.68, 0.75, 0.84, 1.0],
        [w, h],
    );
    build_lock(verts, layout, [w, h], state.lock_label);
    let ts = 1.1 * s;
    let button = |verts: &mut Vec<UiVertex>, rect: Rect, label: &str, active: bool| {
        toolbar_button(verts, rect, label, s, active, false, [w, h]);
    };
    button(verts, layout.home, "[M]enu", false);
    button(verts, layout.orientation, "[S]tars", state.steady_stars);
    button(
        verts,
        layout.toggle,
        if layout.puzzle {
            "Draw [Tab]"
        } else {
            "[L]abels"
        },
        !layout.puzzle && show_labels,
    );
    button(
        verts,
        layout.stop,
        "Stop [Spc]",
        state.minutes_per_second == 0.0,
    );
    button(verts, layout.slower, "←", false);
    button(verts, layout.faster, "→", false);
    let speed_text = if state.minutes_per_second == 0.0 {
        "0 min/s".into()
    } else {
        format!("{:+} min/s", state.minutes_per_second)
    };
    let speed_scale = ts.min((layout.speed.w - 8.0 * s) / text_width(&speed_text, 1.0));
    push_text(
        verts,
        layout.speed.x + (layout.speed.w - text_width(&speed_text, speed_scale)) * 0.5,
        layout.speed.center_y() - 4.0 * speed_scale,
        speed_scale,
        &speed_text,
        [0.85, 0.88, 0.92, 1.0],
        [w, h],
    );
    let clock = layout.clock;
    let time_text = format!("{:.1} min", sim_time * 1440.0);
    let clock_scale = ts.min((clock.w - 8.0 * s) / (time_text.len() as f32 * 8.0));
    push_text(
        verts,
        clock.x + 4.0 * s,
        clock.center_y() - 4.0 * clock_scale,
        clock_scale,
        &time_text,
        [0.85, 0.88, 0.92, 1.0],
        [w, h],
    );

    // Exposure compensation is applied to presentation in both modes.
    let slider = layout.exposure;
    let exposure_color = |manual: [f32; 4]| {
        if state.auto_exposure {
            [0.46, 0.46, 0.46, 1.0]
        } else {
            manual
        }
    };
    let caption = format!("EV {:+.2} [,][.]", state.ev_bias);
    push_text(
        verts,
        slider.x,
        slider.y,
        s * 1.1,
        &caption,
        exposure_color([0.88, 0.92, 0.98, 1.0]),
        [w, h],
    );
    let track_y = slider.y + 23.0 * s;
    push_rect(
        verts,
        slider.x,
        track_y,
        slider.w,
        2.0 * s,
        exposure_color([0.3, 0.35, 0.43, 1.0]),
        [w, h],
    );
    let fraction = ((state.ev_bias - MIN_EV) / (MAX_EV - MIN_EV)).clamp(0.0, 1.0);
    push_rect(
        verts,
        slider.x,
        track_y,
        slider.w * fraction,
        2.0 * s,
        exposure_color([0.45, 0.73, 0.95, 1.0]),
        [w, h],
    );
    push_rect(
        verts,
        slider.x + slider.w * 0.5,
        track_y - 3.0 * s,
        s,
        8.0 * s,
        exposure_color([0.65, 0.68, 0.73, 1.0]),
        [w, h],
    );
    push_rect(
        verts,
        slider.x + slider.w * fraction - 3.0 * s,
        track_y - 4.0 * s,
        6.0 * s,
        10.0 * s,
        exposure_color([0.78, 0.91, 1.0, 1.0]),
        [w, h],
    );

    let a = layout.auto;
    push_rect(verts, a.x, a.y, a.w, a.h, [0.10, 0.11, 0.13, 0.95], [w, h]);
    let check_y = a.center_y() - 6.0 * s;
    push_rect(
        verts,
        a.x + 8.0 * s,
        check_y,
        12.0 * s,
        12.0 * s,
        [0.35, 0.40, 0.48, 1.0],
        [w, h],
    );
    push_rect(
        verts,
        a.x + 9.0 * s,
        check_y + s,
        10.0 * s,
        10.0 * s,
        [0.05, 0.06, 0.07, 1.0],
        [w, h],
    );
    if state.auto_exposure {
        push_rect(
            verts,
            a.x + 11.0 * s,
            check_y + 3.0 * s,
            6.0 * s,
            6.0 * s,
            [0.55, 0.85, 1.0, 1.0],
            [w, h],
        );
    }
    push_text(
        verts,
        a.x + 27.0 * s,
        a.center_y() - 4.0 * ts,
        ts,
        "Auto [A]",
        [0.88, 0.92, 0.98, 1.0],
        [w, h],
    );
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
        for b in [l.stop, l.slower, l.faster] {
            assert!(!b.contains(tc.0, tc.1), "toggle must not overlap a button");
        }

        for b in [l.stop, l.slower, l.faster] {
            let c = ((b.x + b.w * 0.5) as f64, (b.y + b.h * 0.5) as f64);
            assert!(b.contains(c.0, c.1));
            assert!(!l.toggle.contains(c.0, c.1));
            assert!(l.bar.contains(c.0, c.1));
        }

        // Buttons must not run into each other.
        for pair in [l.slower, l.faster, l.stop].windows(2) {
            assert!(pair[0].right() <= pair[1].x);
        }

        // A point in the sky must not be swallowed by the bar.
        assert!(!l.bar.contains(700.0, 100.0));
    }

    #[test]
    fn exposure_slider_maps_clamps_and_fits_after_time_buttons() {
        for (w, scale) in [(1400.0, 1.0), (1190.0, 1.6), (581.0, 1.6), (320.0, 2.0)] {
            let l = layout(w, 800.0, scale);
            assert!(l.exposure.x > l.faster.right());
            assert!(l.auto.x > l.exposure.right());
            assert!(l.auto.right() < w);
            assert_eq!(l.exposure_bias_at(l.exposure.x as f64 - 100.0), MIN_EV);
            assert_eq!(
                l.exposure_bias_at(l.exposure.right() as f64 + 100.0),
                MAX_EV
            );
            assert_eq!(
                l.exposure_bias_at((l.exposure.x + l.exposure.w * 0.5) as f64),
                0.0
            );
            assert_eq!(
                l.exposure_bias_at((l.exposure.x + l.exposure.w * 0.75) as f64),
                4.0
            );
        }
    }

    #[test]
    fn single_row_controls_and_minute_clock_fit_at_small_and_hidpi_sizes() {
        for (w, h, scale) in [
            (1280.0, 720.0, 1.0),
            (1190.0, 800.0, 1.6),
            (581.0, 700.0, 1.6),
            (320.0, 240.0, 2.0),
        ] {
            for puzzle in [false, true] {
                let l = layout(w, h, scale).with_puzzle(puzzle);
                let controls = [
                    l.home,
                    l.toggle,
                    l.time_label,
                    l.slower,
                    l.speed,
                    l.faster,
                    l.stop,
                    l.clock,
                    l.lock,
                    l.orientation,
                    l.exposure,
                    l.auto,
                ];
                assert!(l.time_label.x - l.toggle.right() > l.toggle.x - l.home.right());
                assert!(l.exposure.x - l.orientation.right() > l.auto.x - l.exposure.right());
                assert!(controls.windows(2).all(|pair| pair[0].right() < pair[1].x));
                for r in controls {
                    assert_eq!(r.y, l.lock.y);
                    assert!(r.x >= 0.0 && r.right() <= w && r.y >= l.bar.y && r.y + r.h <= h);
                }
                // Long target names and large/negative minute counts must stay in the viewport.
                for time in [-1e6, 0.0, 1e9] {
                    let mut vertices = Vec::new();
                    build(
                        &mut vertices,
                        &l,
                        w,
                        h,
                        HudState {
                            labels: LabelOptions::default(),
                            lock_label: Some(&"Long name ".repeat(20)),
                            sim_time: time,
                            auto_exposure: true,
                            ev_bias: MAX_EV,
                            steady_stars: true,
                            minutes_per_second: if time < 0.0 {
                                -57600.0
                            } else if time == 0.0 {
                                0.0
                            } else {
                                57600.0
                            },
                        },
                        &[],
                    );
                    assert!(vertices.iter().all(|v| {
                        v.pos
                            .iter()
                            .all(|p| p.is_finite() && (-1.001..=1.001).contains(p))
                    }));
                }
            }
        }
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
        for (ch, cell) in [('←', ARROW_LEFT_CELL), ('→', ARROW_RIGHT_CELL)] {
            let uv = glyph_uv(ch as u32);
            assert_eq!(
                uv[0],
                [
                    (cell % ATLAS_COLS) as f32 / ATLAS_COLS as f32,
                    (cell / ATLAS_COLS) as f32 / ATLAS_ROWS as f32
                ]
            );
            let mut lit = 0;
            for y in 0..GLYPH {
                for x in 0..GLYPH {
                    if a[(cell / ATLAS_COLS * GLYPH + y) * ATLAS_W + cell % ATLAS_COLS * GLYPH + x]
                        != 0
                    {
                        lit += 1;
                    }
                }
            }
            assert!(
                lit > 0 && lit < GLYPH * GLYPH,
                "arrows must not be missing-glyph blocks"
            );
        }
    }
}
