//! Native Blueprint typography, geometry, observation HUD and body labels.
//!
//! All geometry is generated in physical pixels (origin top-left) and converted
//! to NDC when pushed, so the caller only deals with screen coordinates.

use crate::typeface::{self, Face};
use bytemuck::{Pod, Zeroable};

pub const ATLAS_W: usize = typeface::WIDTH;
pub const ATLAS_H: usize = typeface::HEIGHT;
const TEXT_SCALE: f32 = 1.15;
pub const BG: [f32; 4] = [0.0075, 0.0395, 0.3712, 1.0];
pub const DEEP: [f32; 4] = [0.004, 0.021, 0.26, 1.0];
pub const INK: [f32; 4] = [0.871, 0.904, 1.0, 1.0];
pub const MUTED: [f32; 4] = [0.527, 0.618, 0.913, 1.0];
pub const ACCENT: [f32; 4] = [0.871, 0.799, 0.144, 1.0];
pub const OK: [f32; 4] = [0.267, 0.831, 0.644, 1.0];
pub const WARN: [f32; 4] = [1.0, 0.333, 0.195, 1.0];
pub const LINE: [f32; 4] = [0.42, 0.54, 1.0, 0.34];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub edge: [f32; 4],
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
    pub(crate) fn center_y(&self) -> f32 {
        self.y + self.h * 0.5
    }
    pub(crate) fn right(&self) -> f32 {
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
    pub day_step_failed: bool,
    pub auto_exposure: bool,
    pub ev_bias: f32,
    pub steady_stars: bool,
    pub minutes_per_second: f64,
    pub targets_open: bool,
}

pub struct Layout {
    pub bar: Rect,
    pub home: Rect,
    pub toggle: Rect,
    pub scale: f32,
    pub exposure: Rect,
    pub auto: Rect,
    pub lock: Rect,
    pub horizon: Rect,
    pub stars: Rect,
    pub targets: Rect,
    pub settings: Rect,
    pub hide: Rect,
    pub draw: Rect,
    pub stop: Rect,
    pub previous_day: Rect,
    pub next_day: Rect,
    pub slower: Rect,
    pub faster: Rect,
    pub speed: Rect,
    pub time_label: Rect,
    pub clock: Rect,
    requested_scale: f32,
    puzzle: bool,
    style: ToolbarStyle,
    dividers: [f32; 5],
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

    pub fn with_hidden(mut self, hidden: bool) -> Self {
        if hidden {
            let s = self.scale;
            let y = self.bar.y + 8.0 * s;
            self.home = Rect {
                x: 10.0 * s,
                y,
                w: 84.0 * s,
                h: 38.0 * s,
            };
            self.lock = Rect {
                x: 102.0 * s,
                y,
                w: 174.0 * s,
                h: 38.0 * s,
            };
            self.hide = Rect {
                x: self.bar.w - 184.0 * s,
                y,
                w: 174.0 * s,
                h: 38.0 * s,
            };
        }
        self
    }

    pub fn exposure_bias_at(&self, x: f64) -> f32 {
        let fraction = ((x as f32 - self.exposure.x) / self.exposure.w).clamp(0.0, 1.0);
        ((MIN_EV + fraction * (MAX_EV - MIN_EV)) * 4.0).round() / 4.0
    }
}

pub fn layout(w: f32, h: f32, scale: f32) -> Layout {
    layout_mode(w, h, scale, false)
}

/// Both observatory and theory use the same responsive bar and fixed navigation.
/// Decide key visibility from the requested logical width, before fitting the bar.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ToolbarStyle {
    pub scale: f32,
    pub keys: bool,
}

impl ToolbarStyle {
    pub fn new(w: f32, h: f32, requested: f32) -> Self {
        let keys = w / requested > 1300.0;
        let minimum = 217.0
            + time_group_width(keys)
            + lock_group_width(keys)
            + exposure_group_width(keys)
            + toolbar_width("Settings [F11]", keys)
            + 13.0
            + toolbar_width("Hide [H]", keys)
            + 12.0;
        Self {
            scale: requested.min(w / minimum).min(h / 100.0),
            keys,
        }
    }

    pub fn bar(self, w: f32, h: f32) -> Rect {
        Rect {
            x: 0.0,
            y: h - 58.0 * self.scale,
            w,
            h: 58.0 * self.scale,
        }
    }

    pub fn control(self, bar: Rect, x: f32, width: f32) -> Rect {
        Rect {
            x,
            y: bar.y + 12.0 * self.scale,
            w: width * self.scale,
            h: 34.0 * self.scale,
        }
    }

    pub fn nav(self, bar: Rect) -> [Rect; 2] {
        [
            self.control(bar, 8.0 * self.scale, 84.0),
            self.control(bar, 96.0 * self.scale, 112.0),
        ]
    }
}

fn binding(label: &str) -> (&str, Option<&str>) {
    label
        .rsplit_once(" [")
        .map_or((label, None), |(name, key)| {
            (name, Some(key.trim_end_matches(']')))
        })
}

fn key_width(key: &str) -> f32 {
    (typeface::width(Face::Mono, key, 8.5) + 8.0).max(14.0)
}

pub(crate) fn toolbar_width(label: &str, keys: bool) -> f32 {
    let (name, key) = binding(label);
    let content = if matches!(name, "Targets" | "Hide" | "Settings") {
        15.0
    } else {
        typeface::width(Face::Mono, name, 10.5)
    };
    (18.0
        + content
        + key
            .filter(|_| keys || name == "Check theory")
            .map_or(0.0, |key| 8.0 + key_width(key)))
    .max(34.0)
}

fn play_width(keys: bool) -> f32 {
    if keys {
        18.0 + 15.0 + 6.0 + key_width("Spc")
    } else {
        34.0
    }
}

fn time_group_width(keys: bool) -> f32 {
    17.0 + 32.0
        + 30.0
        + 84.0
        + 30.0
        + play_width(keys)
        + toolbar_width("−1d [PgDn]", keys)
        + 100.0
        + toolbar_width("+1d [PgUp]", keys)
        + 7.0 * 4.0
}

fn lock_group_width(keys: bool) -> f32 {
    17.0 + 148.0 + 4.0 + if keys { 95.0 + 78.0 } else { 44.0 + 44.0 } + 6.0 + key_width("R")
}

fn auto_width() -> f32 {
    16.0 + 22.0 + 8.0 + typeface::width(Face::Mono, "Auto", 10.5) + 8.0 + key_width("A")
}

fn exposure_group_width(keys: bool) -> f32 {
    17.0 + 112.0 + 4.0 + auto_width() + 4.0 + toolbar_width("Targets [G]", keys)
}

fn layout_mode(w: f32, h: f32, scale: f32, puzzle: bool) -> Layout {
    let style = ToolbarStyle::new(w, h, scale);
    let s = style.scale;
    let bar = style.bar(w, h);
    let [home, toggle] = style.nav(bar);
    let nav_end = 217.0 * s;
    let mut x = nav_end + 8.0 * s;
    let mut control = |width: f32| {
        let r = style.control(bar, x, width);
        x += (width + 4.0) * s;
        r
    };
    let time_label = control(32.0);
    let slower = control(30.0);
    let speed = control(84.0);
    let faster = control(30.0);
    let stop = control(play_width(style.keys));
    let previous_day = control(toolbar_width("−1d [PgDn]", style.keys));
    let clock = control(100.0);
    let next_day = control(toolbar_width("+1d [PgUp]", style.keys));
    let time_end = nav_end + time_group_width(style.keys) * s;
    let lock = style.control(bar, time_end + 8.0 * s, 148.0);
    let mut horizon = style.control(
        bar,
        lock.right() + 4.0 * s,
        if style.keys { 95.0 } else { 44.0 },
    );
    let mut stars = style.control(bar, horizon.right(), if style.keys { 78.0 } else { 44.0 });
    for r in [&mut horizon, &mut stars] {
        r.y = bar.y + 13.0 * s;
        r.h = 32.0 * s;
    }
    let lock_end = time_end + lock_group_width(style.keys) * s;
    let hide = style.control(
        bar,
        w - (6.0 + toolbar_width("Hide [H]", style.keys)) * s,
        toolbar_width("Hide [H]", style.keys),
    );
    let hide_start = hide.x - 6.0 * s;
    let settings = style.control(
        bar,
        hide_start - (7.0 + toolbar_width("Settings [F11]", style.keys)) * s,
        toolbar_width("Settings [F11]", style.keys),
    );
    let settings_start = settings.x - 6.0 * s;
    let targets = style.control(
        bar,
        settings_start - (8.0 + toolbar_width("Targets [G]", style.keys)) * s,
        toolbar_width("Targets [G]", style.keys),
    );
    let auto_right = if puzzle {
        settings_start - 8.0 * s
    } else {
        targets.x - 4.0 * s
    };
    let auto = style.control(bar, auto_right - auto_width() * s, auto_width());
    let exposure_right = auto.x - 4.0 * s;
    let exposure_width = ((exposure_right - lock_end) / s - 8.0).clamp(112.0, 170.0);
    let exposure = style.control(bar, exposure_right - exposure_width * s, exposure_width);
    Layout {
        bar,
        home,
        toggle,
        draw: toggle,
        time_label,
        slower,
        speed,
        faster,
        stop,
        previous_day,
        clock,
        next_day,
        lock,
        horizon,
        stars,
        exposure,
        auto,
        targets,
        settings,
        hide,
        scale: s,
        style,
        dividers: [nav_end, time_end, lock_end, settings_start, hide_start],
        puzzle,
        requested_scale: scale,
    }
}

#[cfg(test)]
pub fn build_atlas() -> Vec<u8> {
    typeface::snapshot(0).unwrap().bitmap
}

fn solid_uv() -> [f32; 2] {
    [1.5 / ATLAS_W as f32, 1.5 / ATLAS_H as f32]
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
    let solid = uv.iter().all(|u| *u == uv[0]);
    let cx = (x0 + x1) * 0.5;
    let cy = (y0 + y1) * 0.5;
    let hx = (x1 - x0) * 0.5;
    let hy = (y1 - y0) * 0.5;
    let pad = if solid { 0.5 } else { 0.0 };
    let xa = (x0 - pad).max(0.0);
    let xb = (x1 + pad).min(w);
    let ya = (y0 - pad).max(0.0);
    let yb = (y1 + pad).min(h);
    if xb <= xa || yb <= ya {
        return;
    }
    let points = [[xa, ya], [xb, ya], [xa, yb], [xb, yb]];
    let vertices = std::array::from_fn::<_, 4, _>(|i| UiVertex {
        pos: to_ndc(points[i][0], points[i][1], w, h),
        uv: uv[i],
        color,
        edge: if solid {
            [points[i][0] - cx, points[i][1] - cy, hx, hy]
        } else {
            [0.0, 0.0, -1.0, 0.0]
        },
    });
    verts.extend([
        vertices[0],
        vertices[1],
        vertices[2],
        vertices[2],
        vertices[1],
        vertices[3],
    ]);
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
    typeface::width(Face::Mono, text, scale * 13.333333)
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
    font_text(
        verts,
        [x, y],
        scale * 13.333333,
        Face::Mono,
        text,
        color,
        viewport,
    );
}

pub(crate) fn eyebrow(
    v: &mut Vec<UiVertex>,
    pos: [f32; 2],
    px: f32,
    text: &str,
    color: [f32; 4],
    vp: [f32; 2],
) {
    let mut x = pos[0];
    for ch in text.chars() {
        font_text(v, [x, pos[1]], px, Face::Mono, &ch.to_string(), color, vp);
        x += typeface::width(Face::Mono, &ch.to_string(), px) + px * 0.14;
    }
}

pub(crate) fn heading(
    verts: &mut Vec<UiVertex>,
    x: f32,
    y: f32,
    px: f32,
    text: &str,
    color: [f32; 4],
    viewport: [f32; 2],
) {
    font_text(
        verts,
        [x, y],
        px,
        if px > 48.0 { Face::Display } else { Face::Sans },
        text,
        color,
        viewport,
    );
}

pub(crate) fn font_text(
    verts: &mut Vec<UiVertex>,
    [x, y]: [f32; 2],
    px: f32,
    face: Face,
    text: &str,
    color: [f32; 4],
    viewport: [f32; 2],
) {
    let mut cx = x;
    for ch in text.chars() {
        let g = typeface::glyph(face, ch, px);
        let s = px / g.em;
        let m = g.metrics;
        let x0 = cx + m.xmin as f32 * s;
        let y0 = y + px * 0.8 - (m.ymin as f32 + m.height as f32) * s;
        if m.width > 0 && m.height > 0 {
            push_quad(
                verts,
                [
                    [x0, y0],
                    [x0 + m.width as f32 * s, y0 + m.height as f32 * s],
                ],
                g.uv,
                color,
                viewport,
            );
        }
        cx += m.advance_width * s;
    }
}

pub(crate) fn triangle(
    v: &mut Vec<UiVertex>,
    points: [[f32; 2]; 3],
    color: [f32; 4],
    vp: [f32; 2],
) {
    v.extend(points.map(|p| UiVertex {
        pos: to_ndc(p[0], p[1], vp[0], vp[1]),
        uv: solid_uv(),
        color,
        edge: [0.0, 0.0, -1.0, 0.0],
    }));
}

pub(crate) fn line(
    v: &mut Vec<UiVertex>,
    a: [f32; 2],
    b: [f32; 2],
    thickness: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = dx.hypot(dy).max(0.001);
    let along = [dx / len, dy / len];
    let normal = [-along[1], along[0]];
    let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let half = len * 0.5;
    let radius = thickness * 0.5;
    let local = [
        [-half - 0.5, -radius - 0.5],
        [half + 0.5, -radius - 0.5],
        [-half - 0.5, radius + 0.5],
        [half + 0.5, radius + 0.5],
    ];
    let vertices = local.map(|[x, y]| UiVertex {
        pos: to_ndc(
            center[0] + x * along[0] + y * normal[0],
            center[1] + x * along[1] + y * normal[1],
            vp[0],
            vp[1],
        ),
        uv: solid_uv(),
        color,
        edge: [x, y, half, radius],
    });
    v.extend([
        vertices[0],
        vertices[1],
        vertices[2],
        vertices[2],
        vertices[1],
        vertices[3],
    ]);
}

pub(crate) fn ellipse(
    v: &mut Vec<UiVertex>,
    p: [f32; 2],
    radii: [f32; 2],
    thickness: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    arc(v, p, radii, thickness, std::f32::consts::TAU, color, vp);
}

pub(crate) fn arc(
    v: &mut Vec<UiVertex>,
    p: [f32; 2],
    radii: [f32; 2],
    thickness: f32,
    sweep: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    arc_from(
        v,
        p,
        radii,
        thickness,
        [-std::f32::consts::FRAC_PI_2, sweep],
        color,
        vp,
    );
}

pub(crate) fn arc_from(
    v: &mut Vec<UiVertex>,
    p: [f32; 2],
    radii: [f32; 2],
    thickness: f32,
    [start, sweep]: [f32; 2],
    color: [f32; 4],
    vp: [f32; 2],
) {
    if sweep <= 0.0 || radii.iter().any(|r| *r <= 0.0) {
        return;
    }
    let n = ((radii[0].max(radii[1]) * sweep / 3.0).ceil() as usize).max(16);
    let half = thickness * 0.5;
    let pad = half + 0.75;
    let radius = radii[0].min(radii[1]);
    let vertex = |a: f32, side: f32| {
        let q = [radii[0] * a.cos(), radii[1] * a.sin()];
        let normal = [a.cos() / radii[0], a.sin() / radii[1]];
        let len = normal[0].hypot(normal[1]);
        let dx = q[0] + normal[0] / len * pad * side;
        let dy = q[1] + normal[1] / len * pad * side;
        UiVertex {
            pos: to_ndc(p[0] + dx, p[1] + dy, vp[0], vp[1]),
            uv: solid_uv(),
            color,
            edge: [dx / radii[0], dy / radii[1], radius, -half - 2.0],
        }
    };
    for i in 0..n {
        let a = start + sweep * i as f32 / n as f32;
        let b = start + sweep * (i + 1) as f32 / n as f32;
        let q = [
            vertex(a, -1.0),
            vertex(a, 1.0),
            vertex(b, -1.0),
            vertex(b, 1.0),
        ];
        v.extend([q[0], q[1], q[2], q[2], q[1], q[3]]);
    }
}

pub(crate) fn dashed_arc(
    v: &mut Vec<UiVertex>,
    p: [f32; 2],
    radii: [f32; 2],
    [thickness, dash]: [f32; 2],
    [start, sweep]: [f32; 2],
    color: [f32; 4],
    vp: [f32; 2],
) {
    let n = (radii[0].max(radii[1]) * sweep / (dash * 0.2))
        .ceil()
        .clamp(32.0, 4096.0) as usize;
    let point = |i: usize| {
        let a = start + sweep * i as f32 / n as f32;
        [p[0] + radii[0] * a.cos(), p[1] + radii[1] * a.sin()]
    };
    let mut distance = 0.0;
    for i in 0..n {
        let a = point(i);
        let b = point(i + 1);
        if distance % (dash * 2.0) < dash {
            line(v, a, b, thickness, color, vp);
        }
        distance += (b[0] - a[0]).hypot(b[1] - a[1]);
    }
}

pub(crate) fn disk(v: &mut Vec<UiVertex>, p: [f32; 2], r: f32, color: [f32; 4], vp: [f32; 2]) {
    let pad = r + 1.0;
    let local = [[-pad, -pad], [pad, -pad], [-pad, pad], [pad, pad]];
    let vertices = local.map(|[x, y]| UiVertex {
        pos: to_ndc(p[0] + x, p[1] + y, vp[0], vp[1]),
        uv: solid_uv(),
        color,
        edge: [x, y, r, -1.0],
    });
    v.extend([
        vertices[0],
        vertices[1],
        vertices[2],
        vertices[2],
        vertices[1],
        vertices[3],
    ]);
}

pub(crate) fn tracked_heading(
    v: &mut Vec<UiVertex>,
    pos: [f32; 2],
    px: f32,
    text: &str,
    tracking: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    let face = if px > 48.0 { Face::Display } else { Face::Sans };
    let mut x = pos[0];
    for ch in text.chars() {
        font_text(v, [x, pos[1]], px, face, &ch.to_string(), color, vp);
        x += typeface::width(face, &ch.to_string(), px) + tracking;
    }
}

pub(crate) fn keycap(
    v: &mut Vec<UiVertex>,
    r: Rect,
    key: &str,
    s: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    outline(v, r, s.max(0.75), [color[0], color[1], color[2], 0.4], vp);
    push_text(
        v,
        r.x + (r.w - text_width(key, 0.75 * s)) * 0.5,
        r.y + 2.0 * s,
        0.75 * s,
        key,
        color,
        vp,
    );
}

pub(crate) fn switch(v: &mut Vec<UiVertex>, r: Rect, on: bool, color: [f32; 4], vp: [f32; 2]) {
    outline(v, r, (r.h / 12.0).max(0.75), color, vp);
    if on {
        push_rect(v, r.x, r.y, r.w, r.h, color, vp);
    }
    let unit = r.h / 12.0;
    push_rect(
        v,
        r.x + if on { 12.0 * unit } else { 2.0 * unit },
        r.y + 2.0 * unit,
        6.0 * unit,
        6.0 * unit,
        if on { BG } else { color },
        vp,
    );
}

/// Trim triangle geometry at a scroll viewport; interpolate coverage UVs at each edge.
pub(crate) fn clip(v: &mut Vec<UiVertex>, start: usize, r: Rect, vp: [f32; 2]) {
    let min = to_ndc(r.x, r.y + r.h, vp[0], vp[1]);
    let max = to_ndc(r.x + r.w, r.y, vp[0], vp[1]);
    let source = v.split_off(start);
    for tri in source.as_chunks::<3>().0 {
        let mut poly = tri.to_vec();
        for (axis, bound, greater) in [
            (0, min[0], true),
            (0, max[0], false),
            (1, min[1], true),
            (1, max[1], false),
        ] {
            let old = std::mem::take(&mut poly);
            if old.is_empty() {
                break;
            }
            for i in 0..old.len() {
                let a = old[i];
                let b = old[(i + 1) % old.len()];
                let inside = |p: UiVertex| {
                    if greater {
                        p.pos[axis] >= bound
                    } else {
                        p.pos[axis] <= bound
                    }
                };
                if inside(a) {
                    poly.push(a);
                }
                if inside(a) != inside(b) {
                    let t = (bound - a.pos[axis]) / (b.pos[axis] - a.pos[axis]);
                    let mut p = a;
                    for j in 0..2 {
                        p.pos[j] = a.pos[j] + t * (b.pos[j] - a.pos[j]);
                        p.uv[j] = a.uv[j] + t * (b.uv[j] - a.uv[j]);
                    }
                    for j in 0..4 {
                        p.color[j] = a.color[j] + t * (b.color[j] - a.color[j]);
                        p.edge[j] = a.edge[j] + t * (b.edge[j] - a.edge[j]);
                    }
                    poly.push(p);
                }
            }
        }
        for i in 1..poly.len().saturating_sub(1) {
            v.extend([poly[0], poly[i], poly[i + 1]]);
        }
    }
}

pub(crate) fn outline(
    v: &mut Vec<UiVertex>,
    r: Rect,
    thickness: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    for edge in [
        Rect { h: thickness, ..r },
        Rect {
            y: r.y + r.h - thickness,
            h: thickness,
            ..r
        },
        Rect { w: thickness, ..r },
        Rect {
            x: r.x + r.w - thickness,
            w: thickness,
            ..r
        },
    ] {
        push_rect(v, edge.x, edge.y, edge.w, edge.h, color, vp);
    }
}

pub(crate) fn button(
    v: &mut Vec<UiVertex>,
    r: Rect,
    label: &str,
    s: f32,
    primary: bool,
    hovered: bool,
    vp: [f32; 2],
) {
    let mut r = r;
    if crate::motion::pressed(label, r) {
        r.y += 3.0 * s;
    }
    let color = if primary {
        ACCENT
    } else if hovered {
        [0.6, 0.7, 1.0, 0.13]
    } else {
        [0.0; 4]
    };
    push_rect(
        v,
        r.x,
        r.y + r.h,
        r.w,
        3.0 * s,
        [0.0006, 0.003, 0.023, 0.8],
        vp,
    );
    push_rect(v, r.x, r.y, r.w, r.h, color, vp);
    outline(v, r, s.max(0.75), if primary { ACCENT } else { LINE }, vp);
    let (label, key) = label
        .rsplit_once(" [")
        .map_or((label, None), |(a, b)| (a, Some(b.trim_end_matches(']'))));
    let size = (0.90 * s).min((r.w - 18.0 * s) / text_width(label, 1.0).max(1.0));
    let ink = if primary { BG } else { INK };
    let key_w = key.map_or(0.0, |k| text_width(k, 0.75 * s) + 10.0 * s);
    let total = text_width(label, size) + if key.is_some() { 14.0 * s + key_w } else { 0.0 };
    let x = r.x + (r.w - total) * 0.5;
    push_text(v, x, r.center_y() - 6.0 * size, size, label, ink, vp);
    if let Some(key) = key {
        keycap(
            v,
            Rect {
                x: x + text_width(label, size) + 14.0 * s,
                y: r.center_y() - 8.0 * s,
                w: key_w,
                h: 16.0 * s,
            },
            key,
            s,
            ink,
            vp,
        );
    }
}

pub(crate) fn menu_background(v: &mut Vec<UiVertex>, vp: [f32; 2], s: f32) {
    let nx = 32;
    let ny = 18;
    for iy in 0..ny {
        for ix in 0..nx {
            let corners = [[ix, iy], [ix + 1, iy], [ix, iy + 1], [ix + 1, iy + 1]];
            let vertices = corners.map(|[x, y]| {
                let px = x as f32 / nx as f32;
                let py = y as f32 / ny as f32;
                let d = (((px - 0.5) / 0.7).powi(2) + ((py - 0.42) / 0.6).powi(2)).sqrt();
                let f = (d / 0.75).clamp(0.0, 1.0);
                let c = [
                    BG[0] * (1.0 - f) + 0.00335 * f,
                    BG[1] * (1.0 - f) + 0.016 * f,
                    BG[2] * (1.0 - f) + 0.162 * f,
                    0.66 + 0.20 * f,
                ];
                UiVertex {
                    pos: to_ndc(px * vp[0], py * vp[1], vp[0], vp[1]),
                    uv: solid_uv(),
                    color: c,
                    edge: [0.0, 0.0, -1.0, 0.0],
                }
            });
            v.extend([
                vertices[0],
                vertices[1],
                vertices[2],
                vertices[2],
                vertices[1],
                vertices[3],
            ]);
        }
    }
    let step = (32.0 * s).max(8.0);
    let mut x = 0.0;
    while x < vp[0] {
        push_rect(v, x.round(), 0.0, 1.0, vp[1], [0.69, 0.75, 1.0, 0.015], vp);
        x += step;
    }
    let mut y = 0.0;
    while y < vp[1] {
        push_rect(v, 0.0, y.round(), vp[0], 1.0, [0.69, 0.75, 1.0, 0.015], vp);
        y += step;
    }
}

pub(crate) fn brackets(v: &mut Vec<UiVertex>, r: Rect, color: [f32; 4], s: f32, vp: [f32; 2]) {
    let progress = crate::motion::snap();
    let gap = (5.0 + 13.0 * (1.0 - progress).powi(3)) * s;
    let mut color = color;
    color[3] *= progress;
    let len = 12.0 * s;
    let thick = 1.5 * s;
    for (x, y, dx, dy) in [
        (r.x - gap, r.y - gap, 1.0, 1.0),
        (r.x + r.w + gap, r.y - gap, -1.0, 1.0),
        (r.x - gap, r.y + r.h + gap, 1.0, -1.0),
        (r.x + r.w + gap, r.y + r.h + gap, -1.0, -1.0),
    ] {
        push_rect(
            v,
            if dx > 0.0 { x } else { x - len },
            if dy > 0.0 { y } else { y - thick },
            len,
            thick,
            color,
            vp,
        );
        push_rect(
            v,
            if dx > 0.0 { x } else { x - thick },
            if dy > 0.0 { y } else { y - len },
            thick,
            len,
            color,
            vp,
        );
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
    let mut r = layout.lock;
    if crate::motion::pressed("Unlock [U]", r) {
        r.y += 3.0 * s;
    }
    let color = if target.is_some() { OK } else { MUTED };
    if target.is_some() {
        push_rect(
            verts,
            r.x,
            r.y,
            r.w,
            r.h,
            [OK[0], OK[1], OK[2], 0.08],
            viewport,
        );
        outline(verts, r, s, OK, viewport);
    } else {
        for (a, b) in [
            ([r.x, r.y], [r.right(), r.y]),
            ([r.x, r.y + r.h], [r.right(), r.y + r.h]),
            ([r.x, r.y], [r.x, r.y + r.h]),
            ([r.right(), r.y], [r.right(), r.y + r.h]),
        ] {
            icon_segment(verts, [a, b], s, LINE, true, viewport);
        }
    }
    let ret = Rect {
        x: r.x + 14.0 * s,
        y: r.center_y() - 5.0 * s,
        w: 10.0 * s,
        h: 10.0 * s,
    };
    brackets(verts, ret, color, s * 0.5, viewport);
    let x = r.x + 38.0 * s;
    eyebrow(
        verts,
        [x, r.center_y() - 10.0 * s],
        7.5 * s,
        if target.is_some() {
            "FOLLOWING"
        } else {
            "FREE LOOK"
        },
        color,
        viewport,
    );
    let px = if target.is_some() { 10.5 } else { 8.5 } * s;
    let mut label = target.unwrap_or("right-click to lock").to_string();
    let available = r.right() - x - if target.is_some() { 30.0 } else { 9.0 } * s;
    if typeface::width(Face::Mono, &label, px) > available {
        while !label.is_empty() && typeface::width(Face::Mono, &format!("{label}…"), px) > available
        {
            label.pop();
        }
        label.push('…');
    }
    font_text(
        verts,
        [x, r.center_y() + 2.0 * s],
        px,
        Face::Mono,
        &label,
        if target.is_some() { INK } else { MUTED },
        viewport,
    );
    if target.is_some() {
        toolbar_keycap(
            verts,
            [r.right() - (9.0 + key_width("U")) * s, r.center_y()],
            "U",
            s,
            OK,
            viewport,
        );
    }
}

pub fn build_hidden(v: &mut Vec<UiVertex>, l: &Layout, vp: [f32; 2], target: Option<&str>) {
    push_rect(
        v,
        l.home.x,
        l.home.y,
        l.home.w,
        l.home.h,
        [BG[0], BG[1], BG[2], 0.93],
        vp,
    );
    outline(v, l.home, l.scale, LINE, vp);
    toolbar_button(
        v,
        l.home,
        if l.puzzle { "Maps [M]" } else { "Menu [M]" },
        l.style,
        false,
        false,
        vp,
    );
    push_rect(
        v,
        l.lock.x,
        l.lock.y,
        l.lock.w,
        l.lock.h,
        [BG[0], BG[1], BG[2], 0.85],
        vp,
    );
    build_lock(v, l, vp, target);
    button(v, l.hide, "HUD hidden [H]", l.scale, false, false, vp);
}

pub(crate) fn toolbar_panel(
    verts: &mut Vec<UiVertex>,
    bar: Rect,
    scale: f32,
    dividers: &[f32],
    viewport: [f32; 2],
) {
    push_rect(
        verts,
        bar.x,
        bar.y,
        bar.w,
        bar.h,
        [BG[0], BG[1], BG[2], 0.96],
        viewport,
    );
    push_rect(verts, bar.x, bar.y, bar.w, scale, LINE, viewport);
    for x in dividers {
        push_rect(verts, *x - scale, bar.y, scale, bar.h, LINE, viewport);
    }
}

pub(crate) fn toolbar_keycap(
    v: &mut Vec<UiVertex>,
    pos: [f32; 2],
    key: &str,
    s: f32,
    color: [f32; 4],
    vp: [f32; 2],
) {
    let r = Rect {
        x: pos[0],
        y: pos[1] - 7.25 * s,
        w: key_width(key) * s,
        h: 14.5 * s,
    };
    outline(v, r, s, [color[0], color[1], color[2], 0.4], vp);
    font_text(
        v,
        [
            r.x + (r.w - typeface::width(Face::Mono, key, 8.5 * s)) * 0.5,
            pos[1] - 4.25 * s,
        ],
        8.5 * s,
        Face::Mono,
        key,
        color,
        vp,
    );
}

// Measured content, with the same 10.5px label and 8.5px keycap in every group.
// Navigation uses fixed slots; other buttons size to their content without shrinking fonts.
fn toolbar_content(r: Rect, label: &str, style: ToolbarStyle) -> (f32, Option<f32>, Option<f32>) {
    let s = style.scale;
    let (name, binding) = binding(label);
    let key = binding.filter(|_| style.keys || name == "Check theory");
    let icon_only = matches!(name, "Targets" | "Hide" | "Settings");
    let nav = matches!(name, "Menu" | "Maps" | "Theory" | "Observe");
    let text_w = if icon_only {
        0.0
    } else {
        typeface::width(Face::Mono, name, 10.5 * s)
    };
    if nav {
        return (
            r.x + 30.0 * s,
            Some(r.x + 9.0 * s),
            key.map(|k| r.right() - (9.0 + key_width(k)) * s),
        );
    }
    let icon_w = if icon_only { 15.0 * s } else { 0.0 };
    let key_gap = if name == "Check theory" { 24.0 } else { 8.0 };
    let key_w = key.map_or(0.0, |k| (key_gap + key_width(k)) * s);
    let x = r.x + (r.w - text_w - icon_w - key_w) * 0.5;
    (
        x,
        icon_only.then_some(x),
        key.map(|_| x + text_w + icon_w + key_gap * s),
    )
}

pub(crate) fn toolbar_button(
    verts: &mut Vec<UiVertex>,
    rect: Rect,
    label: &str,
    style: ToolbarStyle,
    active: bool,
    hovered: bool,
    viewport: [f32; 2],
) {
    let s = style.scale;
    let mut r = rect;
    let pressed = crate::motion::pressed(label, rect);
    if pressed {
        r.y += 3.0 * s;
    }
    let (name, binding) = binding(label);
    let primary = name == "Check theory";
    if primary || active || hovered {
        push_rect(
            verts,
            r.x,
            r.y,
            r.w,
            r.h,
            if primary {
                ACCENT
            } else if active {
                INK
            } else {
                [INK[0], INK[1], INK[2], 0.10]
            },
            viewport,
        );
        if !primary {
            outline(verts, r, s, LINE, viewport);
        }
        if (primary || active) && !pressed {
            push_rect(
                verts,
                r.x,
                r.y + r.h,
                r.w,
                3.0 * s,
                [0.0006, 0.003, 0.023, 0.8],
                viewport,
            );
        }
    }
    let ink = if primary || active { BG } else { INK };
    let (x, icon, key) = toolbar_content(r, label, style);
    if let Some(x) = icon {
        toolbar_icon(verts, [x + 7.5 * s, r.center_y()], name, s, ink, viewport);
    }
    if !matches!(name, "Targets" | "Hide" | "Settings") {
        font_text(
            verts,
            [x, r.center_y() - 5.25 * s],
            10.5 * s,
            Face::Mono,
            name,
            ink,
            viewport,
        );
    }
    if let (Some(x), Some(key)) = (key, binding) {
        toolbar_keycap(verts, [x, r.center_y()], key, s, ink, viewport);
    }
}

fn toolbar_icon(
    v: &mut Vec<UiVertex>,
    p: [f32; 2],
    name: &str,
    s: f32,
    ink: [f32; 4],
    vp: [f32; 2],
) {
    match name {
        "Menu" | "Maps" => {
            for row in [-3.125, 0.0, 3.125] {
                line(
                    v,
                    [p[0] - 5.0 * s, p[1] + row * s],
                    [p[0] + 5.0 * s, p[1] + row * s],
                    s,
                    ink,
                    vp,
                );
            }
        }
        "Observe" | "Hide" => {
            // The Blueprint eye contour is two quadratic arches.
            for side in [-1.0, 1.0] {
                let at = |i: usize| {
                    let t = i as f32 / 16.0;
                    [
                        p[0] + (12.0 * t - 6.0) * s,
                        p[1] + side * 16.0 * t * (1.0 - t) * s,
                    ]
                };
                for i in 0..16 {
                    line(v, at(i), at(i + 1), s, ink, vp);
                }
            }
            ellipse(v, p, [2.0 * s, 2.0 * s], s, ink, vp);
            if name == "Hide" {
                line(
                    v,
                    [p[0] - 5.0 * s, p[1] + 5.0 * s],
                    [p[0] + 5.0 * s, p[1] - 5.0 * s],
                    1.2 * s,
                    ink,
                    vp,
                );
            }
        }
        "Settings" => {
            ellipse(v, p, [2.0 * s, 2.0 * s], s, ink, vp);
            let at = |i: usize| {
                let angle = (i as f32 - 0.5) * std::f32::consts::TAU / 32.0;
                let radius = if i % 4 < 2 { 6.0 } else { 4.5 } * s;
                [p[0] + angle.cos() * radius, p[1] + angle.sin() * radius]
            };
            for i in 0..32 {
                line(v, at(i), at((i + 1) % 32), s, ink, vp);
            }
        }
        "Targets" => {
            ellipse(v, p, [5.0 * s, 5.0 * s], s, ink, vp);
            ellipse(v, p, [1.25 * s, 1.25 * s], s, ink, vp);
            for axis in 0..2 {
                for sign in [-1.0, 1.0] {
                    let mut a = p;
                    let mut b = p;
                    a[axis] += sign * 3.75 * s;
                    b[axis] += sign * 6.25 * s;
                    line(v, a, b, s, ink, vp);
                }
            }
        }
        _ => {
            let u = 15.0 / 24.0 * s;
            let q = |xy: [f32; 2]| [p[0] + (xy[0] - 12.0) * u, p[1] + (xy[1] - 12.0) * u];
            for (a, b) in [([9.4, 10.8], [16.0, 7.5]), ([9.4, 13.2], [16.0, 16.5])] {
                line(v, q(a), q(b), 1.5 * u, ink, vp);
            }
            for (point, r) in [([7.0, 12.0], 2.6), ([18.0, 6.5], 2.2), ([18.0, 17.5], 2.2)] {
                ellipse(v, q(point), [r * u, r * u], 1.5 * u, ink, vp);
            }
        }
    }
}

// Exact geometry from the Blueprint keepHorizon / keepStars SVGs.
fn orientation_icon(
    v: &mut Vec<UiVertex>,
    origin: [f32; 2],
    scale: f32,
    stars: bool,
    active: bool,
    vp: [f32; 2],
) {
    let u = scale * 30.0 / 32.0;
    let q = |p: [f32; 2]| [origin[0] + p[0] * u, origin[1] + p[1] * u];
    let ink = if active { BG } else { INK };
    let spin = if active {
        [0.462, 0.254, 0.0, 1.0]
    } else {
        ACCENT
    };
    let points = if stars {
        [[8.0, 7.0], [15.0, 4.0], [23.0, 6.0]]
    } else {
        [[8.0, 9.0], [15.0, 6.0], [23.0, 8.0]]
    };
    for pair in points.windows(2) {
        icon_segment(v, [q(pair[0]), q(pair[1])], 1.4 * u, ink, !stars, vp);
    }
    for p in points {
        disk(v, q(p), if stars { 1.5 * u } else { 1.3 * u }, ink, vp);
    }
    if stars {
        icon_segment(v, [q([2.0, 20.0]), q([24.0, 14.5])], 1.4 * u, ink, true, vp);
    } else {
        line(v, q([2.0, 16.5]), q([30.0, 16.5]), 1.4 * u, ink, vp);
        for x in [6.0, 12.0, 18.0, 24.0, 30.0] {
            line(
                v,
                q([x, 16.5]),
                q([x - 2.5, 20.0]),
                u,
                [ink[0], ink[1], ink[2], 0.6],
                vp,
            );
        }
    }
    let curve = if stars {
        [[16.0, 21.5], [23.0, 22.0], [29.5, 19.0], [30.5, 12.5]]
    } else {
        [[6.0, 4.5], [10.0, 0.5], [21.0, 0.5], [26.0, 4.0]]
    };
    let at = |i: usize| {
        let t = i as f32 / 24.0;
        let a = 1.0 - t;
        q([
            a.powi(3) * curve[0][0]
                + 3.0 * a * a * t * curve[1][0]
                + 3.0 * a * t * t * curve[2][0]
                + t.powi(3) * curve[3][0],
            a.powi(3) * curve[0][1]
                + 3.0 * a * a * t * curve[1][1]
                + 3.0 * a * t * t * curve[2][1]
                + t.powi(3) * curve[3][1],
        ])
    };
    for i in 0..24 {
        line(v, at(i), at(i + 1), 1.4 * u, spin, vp);
    }
    let (tip, ends) = if stars {
        ([30.5, 12.5], [[27.9, 14.7], [31.7, 15.7]])
    } else {
        ([26.0, 4.0], [[22.8, 4.3], [25.1, 1.0]])
    };
    for end in ends {
        line(v, q(tip), q(end), 1.4 * u, spin, vp);
    }
}
fn icon_segment(
    v: &mut Vec<UiVertex>,
    [a, b]: [[f32; 2]; 2],
    width: f32,
    color: [f32; 4],
    dashed: bool,
    vp: [f32; 2],
) {
    if !dashed {
        line(v, a, b, width, color, vp);
        return;
    }
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let unit = width / 1.4;
    let point = |t: f32| {
        [
            a[0] + (b[0] - a[0]) * t / length,
            a[1] + (b[1] - a[1]) * t / length,
        ]
    };
    let mut d = 0.0;
    while d < length {
        line(
            v,
            point(d),
            point((d + 2.0 * unit).min(length)),
            width,
            [color[0], color[1], color[2], 0.75],
            vp,
        );
        d += 3.8 * unit;
    }
}

fn build_bar(v: &mut Vec<UiVertex>, l: &Layout, w: f32, h: f32, state: HudState<'_>) {
    let vp = [w, h];
    let s = l.scale;
    toolbar_panel(v, l.bar, s, &l.dividers, vp);
    let text = |v: &mut Vec<UiVertex>, r: Rect, y: f32, size: f32, value: &str, color| {
        let size = size.min((r.w / s - 8.0) / text_width(value, 1.0).max(1.0));
        push_text(
            v,
            r.x + (r.w - text_width(value, size * s)) * 0.5,
            r.y + y * s,
            size * s,
            value,
            color,
            vp,
        )
    };
    let b = |v: &mut Vec<UiVertex>, r: Rect, value: &str, on: bool| {
        toolbar_button(v, r, value, l.style, on, false, vp)
    };
    b(
        v,
        l.home,
        if l.puzzle { "Maps [M]" } else { "Menu [M]" },
        false,
    );
    b(
        v,
        l.toggle,
        if l.puzzle {
            "Theory [Tab]"
        } else {
            "Labels [L]"
        },
        !l.puzzle && state.labels.show,
    );
    text(v, l.time_label, 14.0, 0.64, "TIME", MUTED);
    for (r, right, key) in [(l.slower, false, "←"), (l.faster, true, "→")] {
        let y = r.y
            + r.h * 0.5
            + if crate::motion::pressed(key, r) {
                3.0 * s
            } else {
                0.0
            };
        let x = r.x + r.w * 0.5;
        let sign = if right { 1.0 } else { -1.0 };
        let tip = [x + sign * 3.0 * s, y];
        line(v, [x - sign * 3.0 * s, y - 6.0 * s], tip, 1.2 * s, INK, vp);
        line(v, tip, [x - sign * 3.0 * s, y + 6.0 * s], 1.2 * s, INK, vp);
    }
    let speed = if state.minutes_per_second == 0.0 {
        "0 min/s".into()
    } else {
        let magnitude = state.minutes_per_second.abs();
        let value = if magnitude.round() == 1000.0 {
            "1,000".into()
        } else {
            format!("{magnitude:.0}")
        };
        format!(
            "{}{value} min/s",
            if state.minutes_per_second < 0.0 {
                "−"
            } else {
                "+"
            }
        )
    };
    text(v, l.speed, 3.0, 0.825, &speed, INK);
    let step = crate::WARPS
        .iter()
        .position(|n| (*n * 1440.0 - state.minutes_per_second).abs() < 0.001)
        .unwrap_or(4);
    for i in 0usize..9 {
        let height = (4.0 + i.abs_diff(4) as f32 * 2.5) * s;
        push_rect(
            v,
            l.speed.x + 4.0 * s + i as f32 * 5.0 * s,
            l.speed.y + 31.0 * s - height,
            3.0 * s,
            height,
            if i == step {
                ACCENT
            } else {
                [INK[0], INK[1], INK[2], 0.25]
            },
            vp,
        );
    }
    let compact = !l.style.keys;
    let mut r = l.stop;
    let pressed = crate::motion::pressed("Play [Spc]", r);
    if pressed {
        r.y += 3.0 * s;
    }
    push_rect(
        v,
        r.x,
        r.y + r.h,
        r.w,
        if pressed { 0.0 } else { 3.0 * s },
        [0.0006, 0.003, 0.023, 0.8],
        vp,
    );
    push_rect(v, r.x, r.y, r.w, r.h, ACCENT, vp);
    let p = [
        r.x + if compact { r.w * 0.5 } else { 15.0 * s },
        r.y + r.h * 0.5,
    ];
    if state.minutes_per_second == 0.0 {
        triangle(
            v,
            [
                [p[0] - 3.0 * s, p[1] - 5.0 * s],
                [p[0] + 5.0 * s, p[1]],
                [p[0] - 3.0 * s, p[1] + 5.0 * s],
            ],
            BG,
            vp,
        );
    } else {
        for x in [-4.0, 2.0] {
            push_rect(v, p[0] + x * s, p[1] - 5.0 * s, 3.0 * s, 10.0 * s, BG, vp);
        }
    }
    if !compact {
        toolbar_keycap(
            v,
            [r.right() - (9.0 + key_width("Spc")) * s, r.center_y()],
            "Spc",
            s,
            BG,
            vp,
        );
    }
    b(v, l.previous_day, "−1d [PgDn]", false);
    b(v, l.next_day, "+1d [PgUp]", false);
    let elapsed = state.sim_time * 1440.0;
    let minutes = elapsed.floor() as i64;
    text(
        v,
        l.clock,
        5.0,
        0.825,
        &if state.day_step_failed {
            "No day found".into()
        } else {
            format!("T+ {elapsed:.1} min")
        },
        if state.day_step_failed { WARN } else { INK },
    );
    text(
        v,
        l.clock,
        23.0,
        0.60,
        &if state.day_step_failed {
            "host is locked to its star".into()
        } else {
            format!(
                "day {} · {:02}:{:02}",
                minutes.div_euclid(1440),
                minutes.rem_euclid(1440) / 60,
                minutes.rem_euclid(60)
            )
        },
        MUTED,
    );
    build_lock(v, l, vp, state.lock_label);
    let orientation = Rect {
        x: l.horizon.x,
        y: l.horizon.y,
        w: l.stars.right() - l.horizon.x,
        h: l.horizon.h,
    };
    outline(v, orientation, s, [LINE[0], LINE[1], LINE[2], 0.18], vp);
    for (r, stars, label, desc) in [
        (l.horizon, false, "Horizon", "level"),
        (l.stars, true, "Stars", "fixed"),
    ] {
        let active = stars == state.steady_stars;
        let mut r = r;
        if crate::motion::pressed("Orientation [R]", r) && active {
            r.y += 3.0 * s;
        }
        if active {
            push_rect(
                v,
                r.x,
                r.y + r.h,
                r.w,
                3.0 * s,
                [0.0006, 0.003, 0.023, 0.8],
                vp,
            );
            push_rect(v, r.x, r.y, r.w, r.h, INK, vp);
        }
        let color = if active { BG } else { INK };
        orientation_icon(
            v,
            [r.x + 7.0 * s, r.y + r.h * 0.5 - 10.5 * s],
            s,
            stars,
            active,
            vp,
        );
        if l.style.keys {
            push_text(
                v,
                r.x + 44.0 * s,
                r.y + 8.0 * s,
                0.7125 * s,
                label,
                color,
                vp,
            );
            push_text(
                v,
                r.x + 44.0 * s,
                r.y + 22.0 * s,
                0.5625 * s,
                desc,
                if active {
                    [BG[0], BG[1], BG[2], 0.7]
                } else {
                    MUTED
                },
                vp,
            );
        }
    }
    toolbar_keycap(
        v,
        [l.stars.right() + 6.0 * s, l.bar.center_y()],
        "R",
        s,
        MUTED,
        vp,
    );
    let e = l.exposure;
    let f = ((state.ev_bias - MIN_EV) / (MAX_EV - MIN_EV)).clamp(0.0, 1.0);
    let cap_y = e.center_y() - 10.0 * s;
    let ink = if state.auto_exposure { MUTED } else { INK };
    let sun = [e.x + 6.0 * s, cap_y];
    ellipse(v, sun, [2.5 * s, 2.5 * s], s, ink, vp);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        line(
            v,
            [sun[0] + a.cos() * 4.0 * s, sun[1] + a.sin() * 4.0 * s],
            [sun[0] + a.cos() * 6.0 * s, sun[1] + a.sin() * 6.0 * s],
            s,
            ink,
            vp,
        );
    }
    toolbar_keycap(v, [e.x + 15.0 * s, cap_y], ",", s, ink, vp);
    toolbar_keycap(v, [e.x + 32.0 * s, cap_y], ".", s, ink, vp);
    let value = format!("{:+.2} EV", state.ev_bias);
    font_text(
        v,
        [
            e.right() - typeface::width(Face::Mono, &value, 9.5 * s),
            cap_y - 4.75 * s,
        ],
        9.5 * s,
        Face::Mono,
        &value,
        ink,
        vp,
    );
    let y = e.center_y() + 9.0 * s;
    push_rect(v, e.x, y - 2.0 * s, e.w, 4.0 * s, LINE, vp);
    push_rect(v, e.x, y - 2.0 * s, e.w * f, 4.0 * s, ACCENT, vp);
    disk(v, [e.x + e.w * f, y], 4.0 * s, ACCENT, vp);
    let mut a = l.auto;
    if crate::motion::pressed("Auto [A]", a) {
        a.y += 3.0 * s;
    }
    let toggle = Rect {
        x: a.x + 8.0 * s,
        y: a.center_y() - 6.0 * s,
        w: 22.0 * s,
        h: 12.0 * s,
    };
    outline(
        v,
        toggle,
        s,
        if state.auto_exposure { ACCENT } else { LINE },
        vp,
    );
    if state.auto_exposure {
        push_rect(v, toggle.x, toggle.y, toggle.w, toggle.h, ACCENT, vp);
    }
    push_rect(
        v,
        toggle.x + if state.auto_exposure { 12.0 } else { 2.0 } * s,
        toggle.y + 2.0 * s,
        8.0 * s,
        8.0 * s,
        if state.auto_exposure { BG } else { MUTED },
        vp,
    );
    font_text(
        v,
        [a.x + 38.0 * s, a.center_y() - 5.25 * s],
        10.5 * s,
        Face::Mono,
        "Auto",
        INK,
        vp,
    );
    toolbar_keycap(
        v,
        [a.right() - (8.0 + key_width("A")) * s, a.center_y()],
        "A",
        s,
        INK,
        vp,
    );
    if !l.puzzle {
        b(v, l.targets, "Targets [G]", state.targets_open);
    }
    b(v, l.settings, "Settings [F11]", false);
    b(v, l.hide, "Hide [H]", false);
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
        for b in [l.stop, l.slower, l.faster, l.previous_day, l.next_day] {
            assert!(!b.contains(tc.0, tc.1), "toggle must not overlap a button");
        }

        for b in [l.stop, l.slower, l.faster, l.previous_day, l.next_day] {
            let c = ((b.x + b.w * 0.5) as f64, (b.y + b.h * 0.5) as f64);
            assert!(b.contains(c.0, c.1));
            assert!(!l.toggle.contains(c.0, c.1));
            assert!(l.bar.contains(c.0, c.1));
        }

        // Buttons must not run into each other.
        for pair in [l.slower, l.faster, l.stop, l.previous_day, l.next_day].windows(2) {
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
                    l.previous_day,
                    l.clock,
                    l.next_day,
                    l.lock,
                    l.horizon,
                    l.stars,
                    l.exposure,
                    l.auto,
                ];
                assert!(l.time_label.x - l.toggle.right() > l.toggle.x - l.home.right());
                assert!(l.exposure.x - l.stars.right() > l.auto.x - l.exposure.right());
                assert!(controls.windows(2).all(|pair| pair[0].right() <= pair[1].x));
                for r in controls {
                    assert!((r.center_y() - l.lock.center_y()).abs() < 0.001);
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
                            targets_open: false,
                            labels: LabelOptions::default(),
                            lock_label: Some(&"Long name ".repeat(20)),
                            sim_time: time,
                            day_step_failed: time < 0.0,
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
    fn right_edge_hide_and_binding_geometry_stay_inside_their_buttons() {
        for (w, h, requested) in [
            (1920.0, 1080.0, 1.0),
            (1600.0, 900.0, 1.0),
            (1440.0, 900.0, 1.0),
            (1301.0, 720.0, 1.0),
            (1300.0, 720.0, 1.0),
            (1280.0, 480.0, 1.0),
            (640.0, 480.0, 2.0),
        ] {
            let l = layout(w, h, requested);
            assert!((w - l.hide.right() - 6.0 * l.scale).abs() < 0.001);
            let controls = [
                l.home,
                l.toggle,
                l.slower,
                l.speed,
                l.faster,
                l.stop,
                l.previous_day,
                l.clock,
                l.next_day,
                l.lock,
                l.horizon,
                l.stars,
                l.exposure,
                l.auto,
                l.targets,
                l.settings,
                l.hide,
            ];
            assert!(controls.windows(2).all(|p| p[0].right() <= p[1].x + 0.001));
            for (r, label) in [
                (l.home, "Maps [M]"),
                (l.toggle, "Observe [Tab]"),
                (l.previous_day, "−1d [PgDn]"),
                (l.next_day, "+1d [PgUp]"),
                (l.hide, "Hide [H]"),
                (l.targets, "Targets [G]"),
                (l.settings, "Settings [F11]"),
            ] {
                let (text_x, _, key_x) = toolbar_content(r, label, l.style);
                if let Some(key_x) = key_x {
                    let (name, key) = binding(label);
                    if !matches!(name, "Hide" | "Targets" | "Settings") {
                        assert!(text_x + typeface::width(Face::Mono, name, 10.5 * l.scale) < key_x);
                    }
                    let mut v = Vec::new();
                    toolbar_keycap(
                        &mut v,
                        [key_x, r.center_y()],
                        key.unwrap(),
                        l.scale,
                        INK,
                        [w, h],
                    );
                    assert!(
                        v.iter().all(|v| {
                            let x = (v.pos[0] + 1.0) * w * 0.5;
                            let y = (1.0 - v.pos[1]) * h * 0.5;
                            x >= r.x - 0.001 && x <= r.right() + 0.001 && y >= r.y && y <= r.y + r.h
                        }),
                        "{label} keycap must fit at {w}×{h}"
                    );
                }
            }
        }
    }

    #[test]
    fn atlas_contains_coverage_and_a_white_geometry_texel() {
        typeface::glyph(Face::Mono, 'A', 21.0);
        let a = build_atlas();
        assert_eq!(a.len(), ATLAS_W * ATLAS_H);
        assert_eq!(a[ATLAS_W + 1], 255);
        assert!(a.iter().any(|&p| p > 0 && p < 255));
    }
}
