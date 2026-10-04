//! Runtime graphics controls and scheduling for progressive observation.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

use crate::ui::{self, Rect, UiVertex};

pub const MAX_SAMPLES: u32 = 16_777_216;
const FRAME_RATES: &[u32] = &[15, 30, 60, 90, 120, 240, 0];
const STILL_LIMITS: &[u32] = &[64, 128, 256, 512, 1024, 4096, 0];
pub const ACTIONS_ROW: usize = 8;

fn choice_fraction(choices: &[u32], value: u32) -> f32 {
    let order = |v: u32| if v == 0 { u32::MAX } else { v };
    let index = choices
        .iter()
        .position(|&v| order(v) >= order(value))
        .unwrap_or(choices.len() - 1);
    index as f32 / (choices.len() - 1) as f32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Zero means unlimited (presentation still follows the display).
    pub max_fps: u32,
    pub resolution_percent: u32,
    pub max_bounces: u32,
    pub samples_per_frame: u32,
    /// Zero means continuous refinement, up to the integrator's precision limit.
    pub still_samples: u32,
    pub hud_scale_percent: u32,
    pub vegetation_percent: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self::PRESETS[1]
    }
}

impl Settings {
    pub const PRESETS: [Self; 3] = [
        Self {
            max_fps: 15,
            vegetation_percent: 35,
            resolution_percent: 50,
            max_bounces: 2,
            samples_per_frame: 1,
            still_samples: 128,
            hud_scale_percent: 100,
        },
        Self {
            max_fps: 30,
            vegetation_percent: 70,
            resolution_percent: 75,
            max_bounces: 4,
            samples_per_frame: 1,
            still_samples: 256,
            hud_scale_percent: 100,
        },
        Self {
            max_fps: 60,
            vegetation_percent: 100,
            resolution_percent: 100,
            max_bounces: 8,
            samples_per_frame: 1,
            still_samples: 1024,
            hud_scale_percent: 100,
        },
    ];

    pub fn normalized(mut self) -> Self {
        self.max_fps = self.max_fps.min(240);
        self.vegetation_percent = self.vegetation_percent.min(100);
        self.resolution_percent = self.resolution_percent.clamp(25, 100);
        self.max_bounces = self.max_bounces.clamp(1, 64);
        self.samples_per_frame = self.samples_per_frame.clamp(1, 64);
        self.still_samples = self.still_samples.min(MAX_SAMPLES);
        self.hud_scale_percent = self.hud_scale_percent.clamp(75, 200);
        self
    }

    pub fn render_size(self, size: (u32, u32)) -> (u32, u32) {
        let scale = self.normalized().resolution_percent as u64;
        let scaled = |v: u32| ((v as u64 * scale).div_ceil(100) as u32).max(1);
        (scaled(size.0), scaled(size.1))
    }

    pub fn sample_limit(self) -> u32 {
        if self.still_samples == 0 {
            MAX_SAMPLES
        } else {
            self.still_samples.min(MAX_SAMPLES)
        }
    }

    fn change(&mut self, row: usize, forward: bool) {
        fn step(current: &mut u32, values: &[u32], forward: bool) {
            // Zero is the final, unlimited choice; keep both ends bounded.
            let order = |v: u32| if v == 0 { u32::MAX } else { v };
            let next = if forward {
                values.iter().copied().find(|v| order(*v) > order(*current))
            } else {
                values
                    .iter()
                    .copied()
                    .rev()
                    .find(|v| order(*v) < order(*current))
            };
            if let Some(next) = next {
                *current = next;
            }
        }
        match row {
            0 => step(&mut self.max_fps, FRAME_RATES, forward),
            1 => step(&mut self.resolution_percent, &[25, 50, 75, 100], forward),
            2 => step(
                &mut self.max_bounces,
                &[1, 2, 4, 8, 12, 16, 32, 64],
                forward,
            ),
            3 => step(
                &mut self.samples_per_frame,
                &[1, 2, 4, 8, 16, 32, 64],
                forward,
            ),
            4 => step(&mut self.still_samples, STILL_LIMITS, forward),
            6 => {
                self.vegetation_percent = if forward {
                    ((self.vegetation_percent / 25 + 1) * 25).min(100)
                } else {
                    self.vegetation_percent.saturating_sub(1) / 25 * 25
                }
            }
            5 => step(
                &mut self.hud_scale_percent,
                &[75, 100, 125, 150, 175, 200],
                forward,
            ),
            _ => {}
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        Ok(serde_saphyr::from_str::<Self>(&std::fs::read_to_string(path)?)?.normalized())
    }

    pub fn save(self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_saphyr::to_string(&self.normalized())?)?;
        std::fs::rename(&temporary, path).context("saving graphics settings")
    }
}

fn settings_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("STARGAZE_SETTINGS") {
        return Some(path.into());
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .map(|p| p.join("stargaze/graphics.yaml"))
}

#[derive(Default)]
pub struct Menu {
    pub settings: Settings,
    pub draft: Settings,
    pub dragging: Option<usize>,
    pub open: bool,
    pub selected: usize,
    pub scroll: f32,
    pub action_back: bool,
    pub message: String,
    path: Option<PathBuf>,
}

impl Menu {
    pub fn load() -> Self {
        let path = settings_path();
        let mut menu = Self {
            path,
            ..Self::default()
        };
        if let Some(path) = &menu.path {
            match Settings::load(path) {
                Ok(settings) => menu.settings = settings,
                Err(error) if path.exists() => {
                    log::warn!("could not load graphics settings: {error:#}");
                    menu.message = "Could not read saved settings. Using Balanced.".into();
                }
                Err(_) => {}
            }
        }
        let options = crate::pathtracer::Options::from_env();
        if std::env::var_os("STARGAZE_SPP").is_some() {
            menu.settings.samples_per_frame = options.samples_per_frame;
        }
        if std::env::var_os("STARGAZE_BOUNCES").is_some() {
            menu.settings.max_bounces = options.max_bounces;
        }
        menu
    }

    pub fn persist(&mut self) {
        self.message = match self.path.as_ref().map(|p| self.settings.save(p)) {
            Some(Ok(())) => "Settings saved. Changes apply when observing.".into(),
            Some(Err(error)) => {
                log::warn!("could not save graphics settings: {error:#}");
                "Settings applied for this session; could not save them.".into()
            }
            None => "Settings applied for this session.".into(),
        };
    }

    pub fn begin(&mut self) {
        self.draft = self.settings;
        self.selected = 0;
        self.scroll = 0.0;
        self.action_back = false;
        self.open = true;
        self.dragging = None;
    }
    pub fn cancel(&mut self) {
        self.draft = self.settings;
        self.open = false;
        self.dragging = None;
    }
    fn preset(&mut self, index: usize) {
        let p = Settings::PRESETS[index];
        self.draft.resolution_percent = p.resolution_percent;
        self.draft.samples_per_frame = p.samples_per_frame;
        self.draft.max_bounces = p.max_bounces;
        self.draft.vegetation_percent = p.vegetation_percent;
    }
    fn preset_index(&self) -> Option<usize> {
        Settings::PRESETS.iter().position(|p| {
            p.resolution_percent == self.draft.resolution_percent
                && p.samples_per_frame == self.draft.samples_per_frame
                && p.max_bounces == self.draft.max_bounces
                && p.vegetation_percent == self.draft.vegetation_percent
        })
    }

    pub fn apply(&mut self) -> bool {
        let changed = self.settings != self.draft;
        self.settings = self.draft.normalized();
        self.open = false;
        self.dragging = None;
        changed
    }
    fn slider(&mut self, l: &Layout, row: usize, x: f64) {
        let r = l.tracks[row];
        let f = ((x as f32 - r.x) / r.w).clamp(0.0, 1.0);
        match row {
            0 => self.draft.resolution_percent = 25 + 25 * (f * 3.0).round() as u32,
            1 => self.draft.samples_per_frame = 1 + (f * 63.0).round() as u32,
            2 => self.draft.max_bounces = 1 + (f * 63.0).round() as u32,
            3 => self.draft.max_fps = FRAME_RATES[(f * 6.0).round() as usize],
            4 => self.draft.still_samples = STILL_LIMITS[(f * 6.0).round() as usize],
            5 => self.draft.vegetation_percent = (f * 4.0).round() as u32 * 25,
            _ => self.draft.hud_scale_percent = 75 + 25 * (f * 5.0).round() as u32,
        }
    }
    pub fn motion(&mut self, l: &Layout, x: f64) {
        if let Some(row) = self.dragging {
            self.slider(l, row, x);
        }
    }
    pub fn click(&mut self, layout: &Layout, cursor: (f64, f64)) -> bool {
        if layout.back.contains(cursor.0, cursor.1) {
            self.cancel();
            return false;
        }
        if layout.apply.contains(cursor.0, cursor.1) {
            return self.apply();
        }
        if !layout.viewport.contains(cursor.0, cursor.1) {
            return false;
        }
        for (i, r) in layout.presets.iter().enumerate() {
            if r.contains(cursor.0, cursor.1) {
                self.selected = 0;
                self.preset(i);
            }
        }
        for (i, r) in layout.tracks.iter().enumerate() {
            if r.contains(cursor.0, cursor.1) {
                self.selected = i + 1;
                self.dragging = Some(i);
                self.slider(layout, i, cursor.0);
            }
        }
        false
    }
    pub fn key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => self.cancel(),
            KeyCode::Enter if self.selected == ACTIONS_ROW && self.action_back => self.cancel(),
            KeyCode::Enter => return self.apply(),
            KeyCode::ArrowUp => self.selected = (self.selected + ACTIONS_ROW) % (ACTIONS_ROW + 1),
            KeyCode::ArrowDown => self.selected = (self.selected + 1) % (ACTIONS_ROW + 1),
            KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                let forward = key == KeyCode::ArrowRight;
                if self.selected == 0 {
                    let i = self.preset_index().unwrap_or(1);
                    self.preset(if forward { (i + 1) % 3 } else { (i + 2) % 3 });
                } else if self.selected == ACTIONS_ROW {
                    self.action_back = !forward;
                } else {
                    match self.selected {
                        1 => self.draft.change(1, forward),
                        2 => {
                            self.draft.samples_per_frame = if forward {
                                (self.draft.samples_per_frame + 1).min(64)
                            } else {
                                self.draft.samples_per_frame.saturating_sub(1).max(1)
                            }
                        }
                        3 => {
                            self.draft.max_bounces = if forward {
                                (self.draft.max_bounces + 1).min(64)
                            } else {
                                self.draft.max_bounces.saturating_sub(1).max(1)
                            }
                        }
                        4 => self.draft.change(0, forward),
                        5 => self.draft.change(4, forward),
                        6 => self.draft.change(6, forward),
                        _ => self.draft.change(5, forward),
                    }
                }
            }
            KeyCode::Digit1 => self.preset(0),
            KeyCode::Digit2 => self.preset(1),
            KeyCode::Digit3 => self.preset(2),
            _ => {}
        }
        false
    }
    pub fn reveal_selection(&mut self, w: f32, h: f32, scale: f32) {
        let l = Layout::new(w, h, scale).scrolled(self.scroll);
        let r = if self.selected == 0 {
            l.rows[0]
        } else if self.selected < ACTIONS_ROW {
            l.rows[self.selected]
        } else {
            l.apply
        };
        if r.y < l.viewport.y {
            self.scroll -= (l.viewport.y - r.y) / l.scale;
        }
        if r.y + r.h > l.viewport.y + l.viewport.h {
            self.scroll += (r.y + r.h - l.viewport.y - l.viewport.h) / l.scale;
        }
        self.scroll = self.scroll.clamp(0.0, l.max_scroll);
    }
}

/// Input also goes through the frame cap. Hidden UI screens repaint immediately.
pub struct FramePacer {
    last_frame: Option<Instant>,
    pending: bool,
}

impl Default for FramePacer {
    fn default() -> Self {
        Self {
            last_frame: None,
            pending: true,
        }
    }
}

impl FramePacer {
    pub fn request(&mut self) {
        self.pending = true;
    }
    pub fn rendered(&mut self, started: Instant) {
        self.last_frame = Some(started);
        self.pending = false;
    }
    pub fn deadline(&self, now: Instant, continuous: bool, fps: u32) -> Option<Instant> {
        if !self.pending && !continuous {
            return None;
        }
        let interval = if fps == 0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(1.0 / fps as f64)
        };
        Some(
            self.last_frame
                .map_or(now, |last| (last + interval).max(now)),
        )
    }
}

pub struct Layout {
    pub scale: f32,
    pub x: f32,
    pub presets: [Rect; 3],
    pub tracks: [Rect; 7],
    pub rows: [Rect; 8],
    pub back: Rect,
    pub apply: Rect,
    pub viewport: Rect,
    pub max_scroll: f32,
}
impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 1000.0).min(h / 600.0);
        let x = (w - 1000.0 * s) * 0.5 + 64.0 * s;
        let ys = [227.0, 331.5, 436.0, 524.5, 613.0, 701.5, 790.0, 984.0];
        let hs = [104.5, 104.5, 88.5, 88.5, 88.5, 88.5, 88.5, 88.5];
        Self {
            scale: s,
            x,
            presets: std::array::from_fn(|i| Rect {
                x: x + (484.0 + i as f32 * 64.0) * s,
                y: 261.0 * s,
                w: 64.0 * s,
                h: 38.0 * s,
            }),
            tracks: std::array::from_fn(|i| Rect {
                x: x + 448.0 * s,
                y: (ys[i + 1] + hs[i + 1] * 0.5 - 14.0) * s,
                w: 166.0 * s,
                h: 28.0 * s,
            }),
            rows: std::array::from_fn(|i| Rect {
                x,
                y: ys[i] * s,
                w: 760.0 * s,
                h: hs[i] * s,
            }),
            back: Rect {
                x: x + 432.0 * s,
                y: 1112.5 * s,
                w: 144.0 * s,
                h: 46.0 * s,
            },
            apply: Rect {
                x: x + 592.0 * s,
                y: 1112.5 * s,
                w: 168.0 * s,
                h: 46.0 * s,
            },
            viewport: Rect {
                x: x - 8.0 * s,
                y: 150.0 * s,
                w: 890.0 * s,
                h: h - 162.0 * s,
            },
            max_scroll: (1170.5 - h / s).max(0.0),
        }
    }
    pub fn scrolled(mut self, scroll: f32) -> Self {
        let dy = scroll.clamp(0.0, self.max_scroll) * self.scale;
        for r in self
            .presets
            .iter_mut()
            .chain(self.tracks.iter_mut())
            .chain(self.rows.iter_mut())
            .chain([&mut self.back, &mut self.apply])
        {
            r.y -= dy;
        }
        self
    }
}

pub fn build(v: &mut Vec<UiVertex>, vp: [f32; 2], scale: f32, menu: &Menu, cursor: (f64, f64)) {
    let [w, h] = vp;
    let l = Layout::new(w, h, scale).scrolled(menu.scroll);
    let s = l.scale;
    let x = l.x;
    ui::menu_background(v, vp, s);
    ui::eyebrow(
        v,
        [x, 48.0 * s],
        9.0 * s,
        "CALIBRATE THE TELESCOPE",
        ui::ACCENT,
        vp,
    );
    ui::tracked_heading(
        v,
        [x, 71.0 * s],
        50.0 * s,
        "Settings",
        -2.0 * s,
        ui::INK,
        vp,
    );
    if !menu.message.is_empty() {
        ui::push_text(v, x, 131.0 * s, 0.75 * s, &menu.message, ui::MUTED, vp);
    }
    let start = v.len();
    let offset = menu.scroll.clamp(0.0, l.max_scroll) * s;
    let text = |v: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, value: &str, color| {
        ui::push_text(v, x, y, size * s, value, color, vp)
    };
    for (title, help, y) in [
        (
            "Rendering",
            "Every frame adds fresh samples to a running average. Stop time and stop moving to converge.",
            158.0,
        ),
        (
            "Interface",
            "Changing the HUD never resets the image.",
            914.5,
        ),
    ] {
        ui::tracked_heading(
            v,
            [x, y * s - offset],
            24.0 * s,
            title,
            -0.5 * s,
            ui::INK,
            vp,
        );
        text(v, x, (y + 39.0) * s - offset, 0.80, help, ui::MUTED);
    }
    let info = [
        (
            "Quality preset",
            "Sets rendering quality and vegetation density.",
            "Eco favors motion; High favors detail.",
            "graphics · presets",
        ),
        (
            "Render resolution",
            "Resolution of the traced night sky only.",
            "Menus, HUD and labels stay sharp.",
            "graphics · resolution_percent · 25–100%",
        ),
        (
            "Samples per frame",
            "Maximum new samples per frame.",
            "Batches shrink when rendering is slow.",
            "STARGAZE_SPP · 1–64",
        ),
        (
            "Maximum bounces",
            "Surface vertices per path.",
            "1 gives direct light only.",
            "STARGAZE_BOUNCES · 1–64",
        ),
        (
            "Frame rate",
            "Caps frames while moving or refining.",
            "Choose 60 or higher for smoother motion.",
            "Uncapped follows your display.",
        ),
        (
            "Still image limit",
            "Stop refining after this many samples.",
            "Higher limits reduce grain in a paused view.",
            "Continuous keeps refining while visible.",
        ),
        (
            "Vegetation density",
            "Trees and ground cover around the clearing.",
            "Fewer plants can improve frame rate.",
            "0% bare · 100% full forest",
        ),
        (
            "HUD scale",
            "Bar and menu scale on top of the",
            "display scale factor.",
            "window scale factor",
        ),
    ];
    for (i, ((label, help, help2, env), r)) in info.iter().zip(l.rows).enumerate() {
        if menu.selected == i {
            ui::push_rect(v, r.x, r.y, r.w, r.h, [0.871, 0.904, 1.0, 0.08], vp);
            ui::brackets(v, r, ui::OK, s, vp);
        }
        ui::push_rect(
            v,
            r.x,
            r.y + r.h - s,
            r.w,
            s,
            [ui::LINE[0], ui::LINE[1], ui::LINE[2], 0.18],
            vp,
        );
        text(v, x + 14.0 * s, r.y + 20.0 * s, 0.825, label, ui::INK);
        text(v, x + 14.0 * s, r.y + 42.0 * s, 0.75, help, ui::MUTED);
        text(v, x + 14.0 * s, r.y + 58.0 * s, 0.75, help2, ui::MUTED);
        text(
            v,
            x + 14.0 * s,
            r.y + r.h - 14.0 * s,
            0.675,
            env,
            [ui::MUTED[0], ui::MUTED[1], ui::MUTED[2], 0.7],
        );
    }
    for (i, label) in ["Eco", "Balanced", "High"].iter().enumerate() {
        let r = l.presets[i];
        let active = menu.preset_index() == Some(i);
        ui::button(v, r, label, s, false, r.contains(cursor.0, cursor.1), vp);
        if active {
            ui::push_rect(v, r.x, r.y, r.w, r.h, ui::INK, vp);
            text(v, r.x + 9.0 * s, r.y + 13.0 * s, 0.80, label, ui::BG);
        }
    }
    text(
        v,
        x + 689.0 * s,
        l.presets[0].y + 13.0 * s,
        0.75,
        "Custom",
        if menu.preset_index().is_none() {
            ui::INK
        } else {
            ui::MUTED
        },
    );
    let st = menu.draft;
    let (res_w, res_h) = st.render_size((w as u32, h as u32));
    let values = [
        format!("{}% · {}×{}", st.resolution_percent, res_w, res_h),
        format!("{} spp", st.samples_per_frame),
        st.max_bounces.to_string(),
        if st.max_fps == 0 {
            "Uncapped".into()
        } else {
            format!("{} fps", st.max_fps)
        },
        if st.still_samples == 0 {
            "Continuous".into()
        } else {
            format!("{} spp", st.still_samples)
        },
        format!("{}%", st.vegetation_percent),
        format!("{}%", st.hud_scale_percent),
    ];
    let f = [
        (st.resolution_percent - 25) as f32 / 75.0,
        (st.samples_per_frame - 1) as f32 / 63.0,
        (st.max_bounces - 1) as f32 / 63.0,
        choice_fraction(FRAME_RATES, st.max_fps),
        choice_fraction(STILL_LIMITS, st.still_samples),
        st.vegetation_percent as f32 / 100.0,
        (st.hud_scale_percent - 75) as f32 / 125.0,
    ];
    for (i, r) in l.tracks.iter().enumerate() {
        let y = r.y + r.h * 0.5;
        ui::push_rect(
            v,
            r.x,
            y - 3.0 * s,
            r.w,
            6.0 * s,
            [0.12, 0.14, 0.18, 1.0],
            vp,
        );
        ui::push_rect(v, r.x, y - 3.0 * s, r.w * f[i], 6.0 * s, ui::ACCENT, vp);
        ui::disk(v, [r.x + r.w * f[i], y], 7.0 * s, ui::ACCENT, vp);
        text(
            v,
            x + 746.0 * s - ui::text_width(&values[i], 0.825 * s),
            y - 5.5 * s,
            0.825,
            &values[i],
            ui::INK,
        );
    }
    if menu.draft != menu.settings {
        text(
            v,
            x,
            1041.0 * s - offset,
            0.825,
            "Unsaved changes",
            ui::ACCENT,
        );
    }
    ui::button(
        v,
        l.back,
        "Back [Esc]",
        s,
        false,
        l.back.contains(cursor.0, cursor.1),
        vp,
    );
    ui::button(
        v,
        l.apply,
        "Apply [Enter]",
        s,
        true,
        l.apply.contains(cursor.0, cursor.1),
        vp,
    );
    if menu.selected == ACTIONS_ROW {
        ui::brackets(
            v,
            if menu.action_back { l.back } else { l.apply },
            ui::OK,
            s,
            vp,
        );
    }
    ui::clip(v, start, l.viewport, vp);
    if l.max_scroll > 0.0 {
        let r = l.viewport;
        let track = Rect {
            x: r.x + r.w - 7.0 * s,
            y: r.y,
            w: 3.0 * s,
            h: r.h,
        };
        ui::push_rect(v, track.x, track.y, track.w, track.h, ui::LINE, vp);
        let thumb = track.h * track.h / (track.h + l.max_scroll * s);
        ui::push_rect(
            v,
            track.x,
            track.y + (track.h - thumb) * menu.scroll / l.max_scroll,
            track.w,
            thumb,
            ui::MUTED,
            vp,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vegetation_slider_keyboard_and_cancel_preserve_the_applied_scene() {
        let mut menu = Menu::default();
        menu.begin();
        let original = menu.settings;
        let l = Layout::new(1280.0, 720.0, 1.0);
        menu.slider(&l, 5, l.tracks[5].x as f64);
        assert_eq!(menu.draft.vegetation_percent, 0);
        assert_eq!(menu.settings, original);
        menu.selected = 6;
        menu.key(KeyCode::ArrowRight);
        assert_eq!(menu.draft.vegetation_percent, 25);
        menu.key(KeyCode::ArrowLeft);
        assert_eq!(menu.draft.vegetation_percent, 0);
        menu.cancel();
        assert_eq!(menu.settings, original);
        menu.begin();
        menu.slider(&l, 5, (l.tracks[5].x + l.tracks[5].w) as f64);
        menu.apply();
        assert_eq!(menu.settings.vegetation_percent, 100);
    }
    #[test]
    fn frame_cap_includes_input_but_ui_and_finished_views_can_rest() {
        let start = Instant::now();
        let mut pacer = FramePacer::default();
        assert_eq!(pacer.deadline(start, false, 30), Some(start));
        pacer.rendered(start);
        assert_eq!(pacer.deadline(start, false, 30), None);
        let next = start + Duration::from_secs_f64(1.0 / 30.0);
        assert_eq!(pacer.deadline(start, true, 30), Some(next));
        pacer.request();
        // Rapid input must not defeat the cap or be dropped after convergence.
        assert_eq!(
            pacer.deadline(start + Duration::from_millis(1), false, 30),
            Some(next)
        );
        assert_eq!(
            pacer.deadline(start + Duration::from_millis(1), false, 0),
            Some(start + Duration::from_millis(1))
        );
        assert_eq!(pacer.deadline(next, false, 30), Some(next));
        pacer.rendered(next);
        assert_eq!(pacer.deadline(next, false, 30), None);
        // A slow frame is not followed by a burst to catch up missed frames.
        let slow = start + Duration::from_secs(1);
        assert_eq!(pacer.deadline(slow, true, 30), Some(slow));
    }

    #[test]
    fn settings_round_trip_clamp_and_handle_partial_files() {
        let dir = std::env::temp_dir().join(format!("stargaze-graphics-{}", std::process::id()));
        let path = dir.join("nested/graphics.yaml");
        Settings::PRESETS[0].save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), Settings::PRESETS[0]);
        std::fs::write(&path, "max_fps: 60\n").unwrap();
        assert_eq!(
            Settings::load(&path).unwrap(),
            Settings {
                max_fps: 60,
                ..Settings::default()
            }
        );
        std::fs::write(
            &path,
            "resolution_percent: 999\nmax_bounces: 0\nsamples_per_frame: 999\nstill_samples: 0\n",
        )
        .unwrap();
        let settings = Settings::load(&path).unwrap();
        assert_eq!(settings.resolution_percent, 100);
        assert_eq!(settings.max_bounces, 1);
        assert_eq!(settings.samples_per_frame, 64);
        assert_eq!(settings.sample_limit(), MAX_SAMPLES);
        std::fs::write(&path, "unknown_option: true\n").unwrap();
        assert!(Settings::load(&path).is_err());
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(Settings::PRESETS[0].render_size((1920, 1080)), (960, 540));
        assert_eq!(Settings::default().render_size((801, 601)), (601, 451));
        assert_eq!(Settings::PRESETS[0].render_size((1, 1)), (1, 1));
    }

    #[test]
    fn settings_draft_keyboard_and_scroll_keep_committed_values() {
        for (w, h, scale) in [(1280., 720., 1.), (640., 480., 2.), (320., 240., 1.)] {
            let mut menu = Menu::default();
            menu.begin();
            let committed = menu.settings;
            menu.key(KeyCode::Digit1);
            assert_eq!(menu.draft.resolution_percent, 50);
            assert_eq!(menu.settings, committed);
            menu.key(KeyCode::ArrowDown);
            menu.key(KeyCode::ArrowRight);
            assert_eq!(menu.draft.resolution_percent, 75);
            menu.key(KeyCode::ArrowUp);
            menu.key(KeyCode::ArrowRight);
            assert_eq!(menu.draft.resolution_percent, 100);
            menu.key(KeyCode::Escape);
            assert_eq!(menu.settings, committed);
            menu.begin();
            menu.key(KeyCode::Digit3);
            menu.selected = ACTIONS_ROW;
            menu.reveal_selection(w, h, scale);
            let l = Layout::new(w, h, scale).scrolled(menu.scroll);
            assert!(
                l.viewport
                    .contains(l.apply.x as f64, (l.apply.y + l.apply.h) as f64)
            );
            assert!(menu.key(KeyCode::Enter));
            assert_eq!(menu.settings.resolution_percent, 100);
            menu.begin();
            menu.key(KeyCode::Digit1);
            menu.selected = ACTIONS_ROW;
            menu.key(KeyCode::ArrowLeft);
            menu.key(KeyCode::Enter);
            assert_eq!(menu.settings.resolution_percent, 100);
        }
    }

    #[test]
    fn motion_convergence_and_quality_controls_have_independent_drafts() {
        let mut menu = Menu::default();
        menu.begin();
        let committed = menu.settings;
        menu.key(KeyCode::Digit1);
        menu.selected = 4;
        menu.key(KeyCode::ArrowRight);
        assert_eq!(menu.draft.max_fps, 60);
        assert_eq!(menu.draft.resolution_percent, 50);
        menu.selected = 2;
        for _ in 0..3 {
            menu.key(KeyCode::ArrowRight);
        }
        assert_eq!(menu.draft.samples_per_frame, 4);
        menu.selected = 5;
        menu.key(KeyCode::ArrowRight);
        assert_eq!(menu.draft.still_samples, 512);
        assert_eq!(menu.settings, committed);
        menu.key(KeyCode::Digit3);
        assert_eq!(menu.draft.render_size((3840, 2160)), (3840, 2160));
        assert_eq!(menu.draft.max_bounces, 8);
        assert_eq!(menu.draft.max_fps, 60);
        assert_eq!(menu.draft.still_samples, 512);

        let l = Layout::new(1280.0, 720.0, 1.0);
        menu.slider(&l, 3, (l.tracks[3].x + l.tracks[3].w) as f64);
        menu.slider(&l, 4, (l.tracks[4].x + l.tracks[4].w) as f64);
        assert_eq!(menu.draft.max_fps, 0);
        assert_eq!(menu.draft.sample_limit(), MAX_SAMPLES);
        assert!(menu.apply());
        assert_eq!(menu.settings.max_fps, 0);
        assert_eq!(menu.settings.still_samples, 0);
        menu.begin();
        menu.selected = 4;
        menu.key(KeyCode::ArrowLeft);
        assert_eq!(menu.draft.max_fps, 240);
        menu.selected = 5;
        menu.key(KeyCode::ArrowLeft);
        assert_eq!(menu.draft.still_samples, 4096);
        menu.cancel();
        assert_eq!(menu.settings.max_fps, 0);
        assert_eq!(menu.settings.still_samples, 0);
    }
}
