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

#[derive(Clone, Copy, Debug, Default)]
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
    pub lock_icon: Option<crate::hud::LockIcon>,
    pub sim_time: f64,
    pub day_step_failed: bool,
    pub auto_exposure: bool,
    pub ev_bias: f32,
    pub minutes_per_second: f64,
    pub targets_open: bool,
    /// Current ground name, shown on the explore ground switcher.
    pub ground: &'a str,
    pub air_open: bool,
    /// A field study (level) rather than Explore.
    pub puzzle: bool,
    pub fov_deg: f64,
    /// Accumulated samples, the sample limit, and whether it is still refining.
    pub samples: Option<(u32, u32, bool)>,
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
        Self::fitting(w, h, requested, 0.0)
    }

    /// As `new`, reserving `extra` logical pixels (e.g. explore-only controls).
    fn fitting(w: f32, h: f32, requested: f32, extra: f32) -> Self {
        let keys = w / requested > 1300.0;
        let minimum = extra
            + 217.0
            + time_group_width(keys)
            + lock_group_width()
            + exposure_group_width(keys)
            + toolbar_width("Settings [P]", keys)
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

pub(crate) fn key_width(key: &str) -> f32 {
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

fn lock_group_width() -> f32 {
    17.0 + 148.0
}

fn auto_width() -> f32 {
    16.0 + 22.0 + 8.0 + typeface::width(Face::Mono, "Auto", 10.5) + 8.0 + key_width("A")
}

fn exposure_group_width(keys: bool) -> f32 {
    17.0 + 112.0 + 4.0 + auto_width() + 4.0 + toolbar_width("Targets [G]", keys)
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
    let thickness = thickness.min(r.w * 0.5).min(r.h * 0.5).max(0.0);
    // The horizontal strokes own the corners so translucent borders blend once.
    for edge in [
        Rect { h: thickness, ..r },
        Rect {
            y: r.y + r.h - thickness,
            h: thickness,
            ..r
        },
        Rect {
            y: r.y + thickness,
            w: thickness,
            h: r.h - 2.0 * thickness,
            ..r
        },
        Rect {
            x: r.x + r.w - thickness,
            y: r.y + thickness,
            w: thickness,
            h: r.h - 2.0 * thickness,
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

/// Emit the whole HUD: labels first (in the sky), then the corner readouts.
pub fn build(
    verts: &mut Vec<UiVertex>,
    scale: f32,
    w: f32,
    h: f32,
    state: HudState<'_>,
    body_labels: &[Label],
) {
    let s = crate::hud::scale(w, h, scale);
    if state.labels.show {
        build_labels(verts, body_labels, state.labels.locked, s, w, h);
    }
    crate::hud::build(verts, [w, h], scale, &state);
}

pub(crate) fn build_lock_marker(
    verts: &mut Vec<UiVertex>,
    labels: &[Label],
    locked: Option<usize>,
    s: f32,
    viewport: [f32; 2],
) {
    let [w, h] = viewport;
    if let Some(label) = labels.iter().find(|label| locked == Some(label.body)) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_contains_coverage_and_a_white_geometry_texel() {
        typeface::glyph(Face::Mono, 'A', 21.0);
        let a = build_atlas();
        assert_eq!(a.len(), ATLAS_W * ATLAS_H);
        assert_eq!(a[ATLAS_W + 1], 255);
        assert!(a.iter().any(|&p| p > 0 && p < 255));
    }
}
