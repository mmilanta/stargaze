//! Keyboard-first observation HUD: live readouts in the screen corners, each
//! action shown with its key. There are no buttons; the mouse only looks
//! around, locks (right-click) and releases (left-click).
use crate::typeface::{self, Face};
use crate::ui::{
    ACCENT, BG, DEEP, HudState, INK, LINE, MAX_EV, MIN_EV, MUTED, OK, Rect, UiVertex, WARN,
    brackets, disk, eyebrow, font_text, outline, push_rect, switch,
};
use winit::keyboard::KeyCode;

const TEXT_PX: f32 = 12.5;
const KEY_PX: f32 = 10.0;
const KEY_H: f32 = 18.0;
const MARGIN: f32 = 16.0;
const PANEL_PAD: f32 = 8.0;
const ROW: f32 = 28.0;
const PLATE: [f32; 4] = [DEEP[0], DEEP[1], DEEP[2], 0.58];

/// HUD scale: the requested UI scale, shrunk to fit small windows.
pub fn scale(w: f32, h: f32, requested: f32) -> f32 {
    requested.min(w / 1100.0).min(h / 560.0).max(0.25)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mouse {
    Left,
    Right,
    Wheel,
}

#[derive(Clone, Copy, Debug)]
pub enum LockIcon {
    Star,
    Planet,
    Background,
}

pub enum Seg {
    Key(&'static str),
    Playback(bool),
    Text(String, [f32; 4]),
    Mouse(Mouse),
    /// The nine playback speeds, highlighting the current one.
    Meter(usize),
    /// Exposure compensation position, 0..1, over this many logical pixels.
    Slider(f32, f32),
    Switch(bool),
    Gap(f32),
}

pub(crate) fn text(t: impl Into<String>, color: [f32; 4]) -> Seg {
    Seg::Text(t.into(), color)
}

fn width(seg: &Seg, s: f32) -> f32 {
    match seg {
        Seg::Key(k) => key_w(k) * s,
        Seg::Playback(_) => (key_w("Space") + 12.0) * s,
        Seg::Text(t, _) => typeface::width(Face::Mono, t, TEXT_PX * s),
        Seg::Mouse(_) => 12.0 * s,
        Seg::Meter(_) => 43.0 * s,
        Seg::Slider(_, w) => w * s,
        Seg::Switch(_) => 22.0 * s,
        Seg::Gap(g) => g * s,
    }
}

fn key_w(key: &str) -> f32 {
    (typeface::width(Face::Mono, key, KEY_PX) + 10.0).max(KEY_H)
}

fn keycap(
    v: &mut Vec<UiVertex>,
    x: f32,
    cy: f32,
    label: (&str, Option<bool>),
    down: bool,
    s: f32,
    vp: [f32; 2],
) {
    let (key, playing) = label;
    let icon_width = if playing.is_some() { 12.0 * s } else { 0.0 };
    let r = Rect {
        x,
        y: cy - KEY_H * 0.5 * s,
        w: key_w(key) * s + icon_width,
        h: KEY_H * s,
    };
    if down {
        push_rect(v, r.x, r.y, r.w, r.h, ACCENT, vp);
    } else {
        push_rect(v, r.x, r.y, r.w, r.h, [INK[0], INK[1], INK[2], 0.06], vp);
    }
    outline(
        v,
        r,
        s,
        [INK[0], INK[1], INK[2], if down { 1.0 } else { 0.5 }],
        vp,
    );
    let px = KEY_PX * s;
    let color = if down { BG } else { INK };
    if let Some(playing) = playing {
        let ix = x + 5.0 * s;
        if playing {
            for offset in [0.0, 4.0] {
                push_rect(
                    v,
                    ix + offset * s,
                    cy - 4.0 * s,
                    2.0 * s,
                    8.0 * s,
                    color,
                    vp,
                );
            }
        } else {
            crate::ui::triangle(
                v,
                [[ix, cy - 4.0 * s], [ix + 7.0 * s, cy], [ix, cy + 4.0 * s]],
                color,
                vp,
            );
        }
    }
    font_text(
        v,
        [
            r.x + icon_width + (r.w - icon_width - typeface::width(Face::Mono, key, px)) * 0.5,
            cy - px * 0.5,
        ],
        px,
        Face::Mono,
        key,
        if down { BG } else { INK },
        vp,
    );
}

fn row_width(segs: &[Seg], s: f32) -> f32 {
    let gaps = segs.len().saturating_sub(1) as f32 * 5.0 * s;
    segs.iter().map(|seg| width(seg, s)).sum::<f32>() + gaps
}

/// The physical key behind a keycap label, for press feedback.
fn code(key: &str) -> Option<KeyCode> {
    Some(match key {
        "M" => KeyCode::KeyM,
        "P" => KeyCode::KeyP,
        "H" => KeyCode::KeyH,
        "L" => KeyCode::KeyL,
        "G" => KeyCode::KeyG,
        "E" => KeyCode::KeyE,
        "T" => KeyCode::KeyT,
        "B" => KeyCode::KeyB,
        "J" => KeyCode::KeyJ,
        "A" => KeyCode::KeyA,
        "Tab" => KeyCode::Tab,
        "Space" => KeyCode::Space,
        "PgDn" => KeyCode::PageDown,
        "PgUp" => KeyCode::PageUp,
        "←" => KeyCode::ArrowLeft,
        "→" => KeyCode::ArrowRight,
        "," => KeyCode::Comma,
        "." => KeyCode::Period,
        "=" => KeyCode::Equal,
        "-" => KeyCode::Minus,
        "Home" => KeyCode::Home,
        "[" => KeyCode::BracketLeft,
        "]" => KeyCode::BracketRight,
        ";" => KeyCode::Semicolon,
        "'" => KeyCode::Quote,
        "Bksp" => KeyCode::Backspace,
        "1" => KeyCode::Digit1,
        "2" => KeyCode::Digit2,
        "3" => KeyCode::Digit3,
        "4" => KeyCode::Digit4,
        "5" => KeyCode::Digit5,
        "6" => KeyCode::Digit6,
        "7" => KeyCode::Digit7,
        "8" => KeyCode::Digit8,
        "9" => KeyCode::Digit9,
        "0" => KeyCode::Digit0,
        _ => return None,
    })
}

pub const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];

fn mouse_glyph(v: &mut Vec<UiVertex>, x: f32, cy: f32, button: Mouse, s: f32, vp: [f32; 2]) {
    let body = Rect {
        x: x + 0.5 * s,
        y: cy - 8.0 * s,
        w: 11.0 * s,
        h: 16.0 * s,
    };
    let half = body.w * 0.5;
    let top = 6.5 * s;
    match button {
        Mouse::Left => push_rect(v, body.x, body.y, half, top, ACCENT, vp),
        Mouse::Right => push_rect(v, body.x + half, body.y, half, top, ACCENT, vp),
        Mouse::Wheel => push_rect(
            v,
            body.x + half - 1.0 * s,
            body.y + 2.0 * s,
            2.0 * s,
            4.0 * s,
            ACCENT,
            vp,
        ),
    }
    outline(v, body, s, INK, vp);
    push_rect(v, body.x, body.y + top, body.w, s, INK, vp);
    if button != Mouse::Wheel {
        push_rect(v, body.x + half - 0.5 * s, body.y, s, top, INK, vp);
    }
}

/// Draw one row with its left edge at `x` (or its right edge, when `right`).
pub fn row(
    v: &mut Vec<UiVertex>,
    segs: &[Seg],
    x: f32,
    cy: f32,
    right: bool,
    s: f32,
    vp: [f32; 2],
) -> Rect {
    let total = row_width(segs, s);
    let start = if right { x - total } else { x };
    let mut x = start;
    let key = crate::motion::key();
    for seg in segs {
        let w = width(seg, s);
        match seg {
            Seg::Key(k) => {
                keycap(v, x, cy, (k, None), key.is_some() && code(k) == key, s, vp);
            }
            Seg::Playback(playing) => {
                keycap(
                    v,
                    x,
                    cy,
                    ("Space", Some(*playing)),
                    key == Some(KeyCode::Space),
                    s,
                    vp,
                );
            }
            Seg::Text(t, color) => font_text(
                v,
                [x, cy - TEXT_PX * 0.5 * s],
                TEXT_PX * s,
                Face::Mono,
                t,
                *color,
                vp,
            ),
            Seg::Mouse(button) => mouse_glyph(v, x, cy, *button, s, vp),
            Seg::Meter(step) => {
                for i in 0usize..9 {
                    let height = (3.0 + i.abs_diff(4) as f32 * 2.0) * s;
                    push_rect(
                        v,
                        x + i as f32 * 5.0 * s,
                        cy + 6.0 * s - height,
                        3.0 * s,
                        height,
                        if i == *step {
                            ACCENT
                        } else {
                            [INK[0], INK[1], INK[2], 0.25]
                        },
                        vp,
                    );
                }
            }
            Seg::Slider(f, _) => {
                push_rect(v, x, cy - 1.5 * s, w, 3.0 * s, LINE, vp);
                push_rect(v, x, cy - 1.5 * s, w * f, 3.0 * s, ACCENT, vp);
                disk(v, [x + w * f, cy], 3.5 * s, ACCENT, vp);
            }
            Seg::Switch(on) => {
                let r = Rect {
                    x,
                    y: cy - 6.0 * s,
                    w,
                    h: 12.0 * s,
                };
                switch(v, r, *on, if *on { ACCENT } else { LINE }, vp);
            }
            Seg::Gap(_) => {}
        }
        x += w + 5.0 * s;
    }
    Rect {
        x: start,
        y: cy - 9.0 * s,
        w: total,
        h: 18.0 * s,
    }
}

/// Draw `f` onto a soft backing plate sized to what it drew.
fn plated(
    v: &mut Vec<UiVertex>,
    s: f32,
    vp: [f32; 2],
    f: impl FnOnce(&mut Vec<UiVertex>) -> Rect,
) -> Rect {
    let mut content = Vec::new();
    let r = f(&mut content);
    let pad = PANEL_PAD * s;
    let r = Rect {
        x: r.x - pad,
        y: r.y - pad,
        w: r.w + 2.0 * pad,
        h: r.h + 2.0 * pad,
    };
    push_rect(v, r.x, r.y, r.w, r.h, PLATE, vp);
    outline(v, r, s, LINE, vp);
    v.append(&mut content);
    r
}

fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect {
        x,
        y,
        w: a.right().max(b.right()) - x,
        h: (a.y + a.h).max(b.y + b.h) - y,
    }
}

fn rows(
    v: &mut Vec<UiVertex>,
    lines: &[Vec<Seg>],
    x: f32,
    top: f32,
    right: bool,
    s: f32,
    vp: [f32; 2],
) -> Rect {
    let mut bounds: Option<Rect> = None;
    for (i, segs) in lines.iter().enumerate() {
        let r = row(v, segs, x, top + (i as f32 + 0.5) * ROW * s, right, s, vp);
        bounds = Some(bounds.map_or(r, |b| union(b, r)));
    }
    bounds.unwrap_or_default()
}

fn on(active: bool) -> [f32; 4] {
    if active { ACCENT } else { MUTED }
}

fn legend(state: &HudState<'_>) -> Vec<Vec<Seg>> {
    let item = |key, label: &str, active| [Seg::Key(key), text(label, on(active)), Seg::Gap(6.0)];
    let mut first = Vec::new();
    first.extend(item("M", if state.puzzle { "maps" } else { "menu" }, false));
    if state.puzzle {
        first.extend(item("Tab", "theory", false));
    }
    first.extend(item("P", "settings", false));
    first.extend([Seg::Key("H"), text("hide HUD", MUTED)]);
    let mut second = Vec::new();
    if !state.puzzle {
        second.extend(item("L", "labels", state.labels.show));
        second.extend(item("G", "targets", state.targets_open));
    }
    let mut lines = Vec::new();
    if !state.puzzle {
        lines.push(vec![text("OBSERVATORY / EXPLORE", ACCENT)]);
    }
    lines.push(first);
    if !second.is_empty() {
        lines.push(second);
    }
    if !state.puzzle {
        let mut third = Vec::new();
        third.extend(item("B", &format!("ground: {}", state.ground), false));
        third.extend([Seg::Key("J"), text("air", on(state.air_open))]);
        lines.push(third);
    }
    let mut view = Vec::new();
    if let Some((samples, _, converging)) = state.samples {
        view.extend([
            text(
                format!(
                    "{samples} spp · {}",
                    if converging { "converging" } else { "ready" }
                ),
                MUTED,
            ),
            text("·", MUTED),
        ]);
    }
    view.extend([
        text(format!("FOV {:.3}°", state.fov_deg), INK),
        Seg::Gap(4.0),
        Seg::Mouse(Mouse::Wheel),
        text("zoom", MUTED),
    ]);
    lines.push(view);
    lines
}

/// Bottom edge of the top-right key legend; panels open below it.
pub fn legend_bottom(vp: [f32; 2], requested: f32, puzzle: bool) -> f32 {
    let s = scale(vp[0], vp[1], requested);
    let lines = if puzzle { 2.0 } else { 5.0 };
    (MARGIN + lines * ROW + 6.0) * s
}

fn speed_text(minutes_per_second: f64) -> String {
    if minutes_per_second == 0.0 {
        return "0 min/s".into();
    }
    let magnitude = minutes_per_second.abs();
    let value = if magnitude.round() == 1000.0 {
        "1,000".into()
    } else {
        format!("{magnitude:.0}")
    };
    let sign = if minutes_per_second < 0.0 { "−" } else { "+" };
    format!("{sign}{value} min/s")
}

/// The full corner HUD. Returns each cluster's bounds: legend, time, exposure and lock.
pub fn build(
    v: &mut Vec<UiVertex>,
    vp: [f32; 2],
    requested: f32,
    state: &HudState<'_>,
) -> [Rect; 4] {
    let [w, h] = vp;
    let s = scale(w, h, requested);
    let m = MARGIN * s;
    let left = m + PANEL_PAD * s;
    let right = w - m - PANEL_PAD * s;

    // Top right: view controls, key legend and convergence readout.
    let legend_bounds = plated(v, s, vp, |v| {
        rows(
            v,
            &legend(state),
            right,
            m + (PANEL_PAD + 9.0 - 0.5 * ROW) * s,
            true,
            s,
            vp,
        )
    });

    // Bottom left: time.
    let step = crate::WARPS
        .iter()
        .position(|n| (*n * 1440.0 - state.minutes_per_second).abs() < 0.001)
        .unwrap_or(crate::STOP_WARP);
    let minutes = (state.sim_time * 1440.0).floor() as i64;
    let day = if state.day_step_failed {
        text("no day: locked to its star", WARN)
    } else {
        text(
            format!(
                "day {} · {:02}:{:02}",
                minutes.div_euclid(1440),
                minutes.rem_euclid(1440) / 60,
                minutes.rem_euclid(60)
            ),
            INK,
        )
    };
    let readout = [
        day,
        text("·", MUTED),
        text(speed_text(state.minutes_per_second), INK),
    ];
    let indicators = [
        vec![text("−1 day", MUTED)],
        vec![Seg::Meter(step)],
        vec![text("+1 day", MUTED)],
    ];
    let controls = [
        vec![Seg::Key("PgDn")],
        vec![
            Seg::Key("←"),
            Seg::Playback(state.minutes_per_second != 0.0),
            Seg::Key("→"),
        ],
        vec![Seg::Key("PgUp")],
    ];
    let bottom = h - m - (2.5 * ROW + 9.0 + PANEL_PAD) * s;
    let time_bounds = plated(v, s, vp, |v| {
        let mut bounds = row(v, &readout, left, bottom + 0.5 * ROW * s, false, s, vp);
        let mut x = left;
        for (indicator, control) in indicators.iter().zip(&controls) {
            let column_width = row_width(indicator, s).max(row_width(control, s));
            for (segs, line) in [(indicator, 1.5), (control, 2.5)] {
                let r = row(
                    v,
                    segs,
                    x + (column_width - row_width(segs, s)) * 0.5,
                    bottom + line * ROW * s,
                    false,
                    s,
                    vp,
                );
                bounds = union(bounds, r);
            }
            x += column_width + 14.0 * s;
        }
        bounds
    });

    // Bottom right: exposure.
    let f = ((state.ev_bias - MIN_EV) / (MAX_EV - MIN_EV)).clamp(0.0, 1.0);
    let ev = if state.auto_exposure { MUTED } else { INK };
    let headings = [
        vec![text("−", MUTED)],
        vec![text("exposure", MUTED)],
        vec![text("+", MUTED)],
    ];
    let adjustments = [
        vec![Seg::Key(",")],
        vec![Seg::Slider(f, 150.0)],
        vec![Seg::Key(".")],
    ];
    let exposure_footer = [
        text(format!("{:+.2} EV", state.ev_bias), ev),
        Seg::Gap(8.0),
        text("[A]uto", on(state.auto_exposure)),
        Seg::Switch(state.auto_exposure),
    ];
    let exposure_bounds = plated(v, s, vp, |v| {
        let widths: Vec<_> = headings
            .iter()
            .zip(&adjustments)
            .map(|(heading, adjustment)| row_width(heading, s).max(row_width(adjustment, s)))
            .collect();
        let total = widths.iter().sum::<f32>() + 2.0 * 8.0 * s;
        let mut x = right - total;
        let mut bounds = row(
            v,
            &exposure_footer,
            right,
            bottom + 2.5 * ROW * s,
            true,
            s,
            vp,
        );
        for ((heading, adjustment), column_width) in headings.iter().zip(&adjustments).zip(widths) {
            for (segs, line) in [(heading, 0.5), (adjustment, 1.5)] {
                let r = row(
                    v,
                    segs,
                    x + (column_width - row_width(segs, s)) * 0.5,
                    bottom + line * ROW * s,
                    false,
                    s,
                    vp,
                );
                bounds = union(bounds, r);
            }
            x += column_width + 8.0 * s;
        }
        bounds
    });

    // Bottom centre: the lock.
    let lock_bounds = lock(v, vp, s, state.lock_label, state.lock_icon);
    [legend_bounds, time_bounds, exposure_bounds, lock_bounds]
}

fn lock_glyph(v: &mut Vec<UiVertex>, center: [f32; 2], icon: LockIcon, s: f32, vp: [f32; 2]) {
    use crate::ui::{line, triangle};
    let star = |v: &mut Vec<UiVertex>, p: [f32; 2], radius: f32| {
        disk(v, p, radius * 0.32, OK, vp);
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0;
            let direction = [angle.cos(), angle.sin()];
            line(
                v,
                [
                    p[0] + direction[0] * radius * 0.55,
                    p[1] + direction[1] * radius * 0.55,
                ],
                [p[0] + direction[0] * radius, p[1] + direction[1] * radius],
                s,
                OK,
                vp,
            );
        }
    };
    match icon {
        LockIcon::Star => star(v, center, 6.0 * s),
        LockIcon::Background => {
            for (x, y, radius) in [(-4.0, 2.5, 2.8), (0.0, -3.5, 3.5), (4.5, 2.0, 2.5)] {
                star(v, [center[0] + x * s, center[1] + y * s], radius * s);
            }
        }
        LockIcon::Planet => {
            let radius = 6.0 * s;
            disk(
                v,
                center,
                radius,
                [OK[0] * 0.25, OK[1] * 0.25, OK[2] * 0.25, 1.0],
                vp,
            );
            for i in 0..24 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 24.0;
                let b = a + std::f32::consts::PI / 24.0;
                triangle(
                    v,
                    [
                        center,
                        [center[0] + radius * a.cos(), center[1] + radius * a.sin()],
                        [center[0] + radius * b.cos(), center[1] + radius * b.sin()],
                    ],
                    OK,
                    vp,
                );
            }
        }
    }
}

/// The lock reticle, bottom centre. Also drawn with the rest of the HUD hidden.
fn lock(
    v: &mut Vec<UiVertex>,
    vp: [f32; 2],
    s: f32,
    target: Option<&str>,
    icon: Option<LockIcon>,
) -> Rect {
    let [w, h] = vp;
    let r = Rect {
        x: (w - 260.0 * s) * 0.5,
        y: h - MARGIN * s - 52.0 * s,
        w: 260.0 * s,
        h: 52.0 * s,
    };
    let color = if target.is_some() { OK } else { MUTED };
    push_rect(
        v,
        r.x,
        r.y,
        r.w,
        r.h,
        if target.is_some() {
            [OK[0] * 0.15, OK[1] * 0.15, OK[2] * 0.15, 0.7]
        } else {
            PLATE
        },
        vp,
    );
    let center = [r.x + 19.0 * s, r.center_y()];
    outline(v, r, s, LINE, vp);
    let reticle = Rect {
        x: center[0] - 7.0 * s,
        y: center[1] - 7.0 * s,
        w: 14.0 * s,
        h: 14.0 * s,
    };
    brackets(v, reticle, color, s * 0.5, vp);
    if let Some(icon) = icon {
        lock_glyph(v, center, icon, s, vp);
    }
    let x = r.x + 36.0 * s;
    let top = r.y + 15.0 * s;
    let bottom = r.y + 35.0 * s;
    match target {
        Some(name) => {
            eyebrow(v, [x, top - 4.5 * s], 8.5 * s, "FOLLOWING", OK, vp);
            row(
                v,
                &[Seg::Mouse(Mouse::Left), text("release", MUTED)],
                r.right() - 12.0 * s,
                top,
                true,
                s,
                vp,
            );
            let px = TEXT_PX * s;
            let available = r.right() - x - 12.0 * s;
            let mut label = name.to_string();
            if typeface::width(Face::Mono, &label, px) > available {
                while !label.is_empty()
                    && typeface::width(Face::Mono, &format!("{label}…"), px) > available
                {
                    label.pop();
                }
                label.push('…');
            }
            font_text(v, [x, bottom - px * 0.5], px, Face::Mono, &label, INK, vp);
        }
        None => {
            eyebrow(v, [x, top - 4.5 * s], 8.5 * s, "FREE LOOK", MUTED, vp);
            row(
                v,
                &[Seg::Mouse(Mouse::Right), text("right-click to lock", MUTED)],
                x,
                bottom,
                false,
                s,
                vp,
            );
        }
    }
    r
}

/// With the HUD hidden: the lock (only while following) and how to come back.
pub fn build_hidden(
    v: &mut Vec<UiVertex>,
    vp: [f32; 2],
    requested: f32,
    target: Option<&str>,
    icon: Option<LockIcon>,
) {
    let [w, h] = vp;
    let s = scale(w, h, requested);
    if target.is_some() {
        lock(v, vp, s, target, icon);
    }
    let m = MARGIN * s;
    plated(v, s, vp, |v| {
        row(
            v,
            &[Seg::Key("H"), text("show HUD", MUTED)],
            w - m,
            h - m - 0.5 * ROW * s,
            true,
            s,
            vp,
        )
    });
}

/// A panel opened from the legend: top-right, below the legend.
pub fn panel(vp: [f32; 2], requested: f32, puzzle: bool, w: f32, h: f32) -> (Rect, f32) {
    let s = scale(vp[0], vp[1], requested);
    let m = MARGIN * s;
    (
        Rect {
            x: vp[0] - m - w * s,
            y: legend_bottom(vp, requested, puzzle) + 8.0 * s,
            w: w * s,
            h: h * s,
        },
        s,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::LabelOptions;

    fn state(puzzle: bool, lock: Option<&'static str>, time: f64) -> HudState<'static> {
        HudState {
            labels: LabelOptions::default(),
            lock_label: lock,
            lock_icon: lock.map(|_| LockIcon::Planet),
            sim_time: time,
            day_step_failed: time < 0.0,
            auto_exposure: true,
            ev_bias: MAX_EV,
            minutes_per_second: -1000.0,
            targets_open: false,
            ground: "Atacama",
            air_open: false,
            puzzle,
            fov_deg: 0.001,
            samples: Some((4096, 4096, false)),
        }
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn corner_clusters_stay_on_screen_and_apart() {
        let long = Box::leak("A very long body name ".repeat(6).into_boxed_str());
        for (w, h, requested) in [
            (3840.0, 2160.0, 2.0),
            (1920.0, 1080.0, 1.0),
            (1280.0, 720.0, 1.0),
            (1280.0, 480.0, 1.0),
            (800.0, 600.0, 1.0),
            (640.0, 480.0, 2.0),
        ] {
            for puzzle in [false, true] {
                for (lock, time) in [(None, 0.0), (Some(&*long), 1e9), (Some("Calyx"), -1e6)] {
                    let mut v = Vec::new();
                    let bounds = build(&mut v, [w, h], requested, &state(puzzle, lock, time));
                    for r in bounds {
                        assert!(r.x >= 0.0 && r.right() <= w, "{w}x{h}: {r:?}");
                        assert!(r.y >= 0.0 && r.y + r.h <= h, "{w}x{h}: {r:?}");
                    }
                    for (i, a) in bounds.iter().enumerate() {
                        for b in &bounds[i + 1..] {
                            assert!(!overlaps(*a, *b), "{w}x{h} puzzle={puzzle}: {a:?} {b:?}");
                        }
                    }
                    assert!(v.iter().all(|p| {
                        p.pos
                            .iter()
                            .all(|c| c.is_finite() && (-1.001..=1.001).contains(c))
                    }));
                    // Panels open below the legend.
                    assert!(
                        legend_bottom([w, h], requested, puzzle) + 0.001
                            >= bounds[0].y + bounds[0].h
                    );
                    let inset = MARGIN * scale(w, h, requested);
                    let near = |a: f32, b: f32| (a - b).abs() < 0.001;
                    assert!(near(bounds[0].y, inset));
                    assert!(near(w - bounds[0].right(), inset));
                    assert!(near(bounds[1].x, inset));
                    assert!(near(w - bounds[2].right(), inset));
                    for r in &bounds[1..] {
                        assert!(near(h - r.y - r.h, inset));
                    }
                }
            }
        }
    }

    #[test]
    fn every_keycap_maps_to_its_key() {
        for key in [
            "M", "P", "H", "L", "G", "E", "T", "B", "J", "A", "Tab", "Space", "PgDn", "PgUp", "←",
            "→", ",", ".", "=", "-", "Home", "[", "]", ";", "'", "Bksp",
        ]
        .into_iter()
        .chain(DIGITS)
        {
            assert!(code(key).is_some(), "{key}");
        }
    }
}
