//! Runtime graphics controls and scheduling for progressive observation.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

use crate::ui::{self, Rect, UiVertex};

pub const MAX_SAMPLES: u32 = 16_777_216;

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
            resolution_percent: 50,
            max_bounces: 2,
            samples_per_frame: 1,
            still_samples: 128,
            hud_scale_percent: 100,
        },
        Self {
            max_fps: 30,
            resolution_percent: 75,
            max_bounces: 4,
            samples_per_frame: 1,
            still_samples: 256,
            hud_scale_percent: 100,
        },
        Self {
            max_fps: 60,
            resolution_percent: 100,
            max_bounces: 8,
            samples_per_frame: 1,
            still_samples: 1024,
            hud_scale_percent: 100,
        },
    ];

    pub fn normalized(mut self) -> Self {
        self.max_fps = self.max_fps.min(240);
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
            0 => step(&mut self.max_fps, &[15, 30, 60, 90, 120, 240, 0], forward),
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
            4 => step(
                &mut self.still_samples,
                &[64, 128, 256, 512, 1024, 4096, 0],
                forward,
            ),
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
        self.open = true;
        self.dragging = None;
    }
    pub fn cancel(&mut self) {
        self.draft = self.settings;
        self.open = false;
        self.dragging = None;
    }
    fn preset(&mut self, index: usize) {
        let hud = self.draft.hud_scale_percent;
        self.draft = Settings::PRESETS[index];
        self.draft.hud_scale_percent = hud;
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
        let options: &[u32] = match row {
            0 => &[15, 30, 60, 90, 120, 240, 0],
            1 => &[25, 50, 75, 100],
            2 => &[1, 2, 4, 8, 12, 16, 32, 64],
            3 => &[1, 2, 4, 8, 16, 32, 64],
            4 => &[64, 128, 256, 512, 1024, 4096, 0],
            _ => &[75, 100, 125, 150, 175, 200],
        };
        let value = options[(f * (options.len() - 1) as f32).round() as usize];
        match row {
            0 => self.draft.max_fps = value,
            1 => self.draft.resolution_percent = value,
            2 => self.draft.max_bounces = value,
            3 => self.draft.samples_per_frame = value,
            4 => self.draft.still_samples = value,
            _ => self.draft.hud_scale_percent = value,
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
        for (i, r) in layout.presets.iter().enumerate() {
            if r.contains(cursor.0, cursor.1) {
                self.preset(i);
            }
        }
        for (i, [decrease, increase]) in layout.arrows.iter().enumerate() {
            if decrease.contains(cursor.0, cursor.1) {
                self.selected = i;
                self.draft.change(i, false);
            } else if increase.contains(cursor.0, cursor.1) {
                self.selected = i;
                self.draft.change(i, true);
            } else if layout.tracks[i].contains(cursor.0, cursor.1) {
                self.selected = i;
                self.dragging = Some(i);
                self.slider(layout, i, cursor.0);
            }
        }
        false
    }
    pub fn key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape | KeyCode::KeyM | KeyCode::KeyG => self.cancel(),
            KeyCode::Enter => return self.apply(),
            KeyCode::ArrowUp => self.selected = self.selected.saturating_sub(1),
            KeyCode::ArrowDown => self.selected = (self.selected + 1).min(5),
            KeyCode::ArrowLeft => self.draft.change(self.selected, false),
            KeyCode::ArrowRight => self.draft.change(self.selected, true),
            KeyCode::Digit1 => self.preset(0),
            KeyCode::Digit2 => self.preset(1),
            KeyCode::Digit3 => self.preset(2),
            _ => {}
        }
        false
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
    pub arrows: [[Rect; 2]; 6],
    pub tracks: [Rect; 6],
    pub back: Rect,
    pub apply: Rect,
}

impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 1060.0).min(h / 760.0);
        let x = (w - 900.0 * s) * 0.5;
        Self {
            scale: s,
            x,
            presets: std::array::from_fn(|i| Rect {
                x: x + (480.0 + i as f32 * 134.0) * s,
                y: 128.0 * s,
                w: 124.0 * s,
                h: 36.0 * s,
            }),
            arrows: std::array::from_fn(|i| {
                std::array::from_fn(|j| Rect {
                    x: x + (480.0 + j as f32 * 374.0) * s,
                    y: (196.0 + i as f32 * 68.0) * s,
                    w: 36.0 * s,
                    h: 32.0 * s,
                })
            }),
            tracks: std::array::from_fn(|i| Rect {
                x: x + 536.0 * s,
                y: (196.0 + i as f32 * 68.0) * s,
                w: 294.0 * s,
                h: 32.0 * s,
            }),
            back: Rect {
                x: x + 582.0 * s,
                y: 652.0 * s,
                w: 144.0 * s,
                h: 42.0 * s,
            },
            apply: Rect {
                x: x + 742.0 * s,
                y: 652.0 * s,
                w: 144.0 * s,
                h: 42.0 * s,
            },
        }
    }
}

pub fn build(v: &mut Vec<UiVertex>, vp: [f32; 2], scale: f32, menu: &Menu, cursor: (f64, f64)) {
    let [w, h] = vp;
    let l = Layout::new(w, h, scale);
    let s = l.scale;
    let x = l.x;
    ui::menu_background(v, vp, s);
    let text = |v: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, value: &str, color| {
        ui::push_text(v, x, y, size * s, value, color, vp)
    };
    let button = |v: &mut Vec<UiVertex>, r: Rect, label: &str, primary: bool| {
        ui::button(v, r, label, s, primary, r.contains(cursor.0, cursor.1), vp)
    };
    text(v, x, 42.0 * s, 0.75, "CALIBRATE THE TELESCOPE", ui::ACCENT);
    ui::heading(v, x, 72.0 * s, 48.0 * s, "Settings", ui::INK, vp);
    ui::heading(v, x, 136.0 * s, 21.0 * s, "Quality preset", ui::INK, vp);
    let st = menu.draft;
    let preset = Settings::PRESETS.iter().position(|p| {
        p.max_fps == st.max_fps
            && p.resolution_percent == st.resolution_percent
            && p.max_bounces == st.max_bounces
            && p.samples_per_frame == st.samples_per_frame
            && p.still_samples == st.still_samples
    });
    for (i, label) in ["Eco [1]", "Balanced [2]", "High [3]"].iter().enumerate() {
        button(v, l.presets[i], label, preset == Some(i));
    }
    if preset.is_none() {
        text(v, x + 210.0 * s, 143.0 * s, 0.85, "Custom", ui::ACCENT);
    }
    let values = [
        if st.max_fps == 0 {
            "Unlimited".into()
        } else {
            format!("{} FPS", st.max_fps)
        },
        format!("{}%", st.resolution_percent),
        format!("{}", st.max_bounces),
        format!("{} spp", st.samples_per_frame),
        if st.still_samples == 0 {
            "Continuous".into()
        } else {
            format!("{} spp", st.still_samples)
        },
        format!("{}%", st.hud_scale_percent),
    ];
    let rows = [
        ("Frame rate", "Give the sky a frame budget."),
        (
            "Render resolution",
            "Menus and labels stay at native resolution.",
        ),
        (
            "Maximum bounces",
            "More light paths add indirect illumination.",
        ),
        ("Samples per frame", "More samples make each frame heavier."),
        (
            "Still image limit",
            "Pause to converge, then let the GPU rest.",
        ),
        ("HUD scale", "Size of the interface on your display."),
    ];
    for (i, (label, help)) in rows.iter().enumerate() {
        let y = (190.0 + i as f32 * 68.0) * s;
        let r = Rect {
            x,
            y: y - 2.0 * s,
            w: 900.0 * s,
            h: 58.0 * s,
        };
        if menu.selected == i {
            ui::push_rect(v, r.x, r.y, r.w, r.h, [0.6, 0.7, 1.0, 0.04], vp);
            ui::brackets(v, r, ui::OK, s, vp);
        }
        ui::heading(v, x + 12.0 * s, y + 4.0 * s, 21.0 * s, label, ui::INK, vp);
        text(v, x + 12.0 * s, y + 35.0 * s, 0.75, help, ui::MUTED);
        button(v, l.arrows[i][0], "←", false);
        button(v, l.arrows[i][1], "→", false);
        let track = l.tracks[i];
        ui::push_rect(
            v,
            track.x,
            track.y + 26.0 * s,
            track.w,
            2.0 * s,
            ui::LINE,
            vp,
        );
        text(
            v,
            track.x + 100.0 * s,
            track.y + 3.0 * s,
            0.9,
            &values[i],
            ui::INK,
        );
    }
    if st != menu.settings {
        text(
            v,
            x + 12.0 * s,
            668.0 * s,
            0.8,
            "Unsaved changes",
            ui::ACCENT,
        );
    }
    button(v, l.back, "Back [Esc]", false);
    button(v, l.apply, "Apply [Enter]", true);
    text(
        v,
        x,
        h - 30.0 * s,
        0.75,
        "[↑ ↓] select   [← →] adjust   [1–3] preset",
        ui::MUTED,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn controls_work_at_small_and_hidpi_sizes_without_overlap() {
        for (w, h, scale) in [(1280., 720., 1.), (640., 480., 2.), (320., 240., 1.)] {
            let layout = Layout::new(w, h, scale);
            let controls: Vec<_> = layout
                .presets
                .iter()
                .chain(layout.arrows.iter().flatten())
                .chain([&layout.back])
                .collect();
            for (i, a) in controls.iter().enumerate() {
                assert!(a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= w && a.y + a.h <= h);
                for b in controls.iter().skip(i + 1) {
                    assert!(
                        a.x + a.w <= b.x
                            || b.x + b.w <= a.x
                            || a.y + a.h <= b.y
                            || b.y + b.h <= a.y
                    );
                }
            }
            let mut menu = Menu {
                open: true,
                ..Menu::default()
            };
            let center = |r: Rect| ((r.x + r.w * 0.5) as f64, (r.y + r.h * 0.5) as f64);
            menu.begin();
            let committed = menu.settings;
            assert!(!menu.click(&layout, center(layout.presets[0])));
            assert_eq!(menu.draft, Settings::PRESETS[0]);
            assert_eq!(menu.settings, committed);
            assert!(!menu.click(&layout, center(layout.arrows[1][1])));
            assert_eq!(menu.draft.resolution_percent, 75);
            assert!(!menu.key(KeyCode::ArrowRight));
            assert_eq!(menu.draft.resolution_percent, 100);
            assert!(!menu.key(KeyCode::Digit3));
            assert_eq!(menu.draft, Settings::PRESETS[2]);
            assert!(!menu.click(&layout, center(layout.back)));
            assert!(!menu.open);
            assert_eq!(menu.settings, committed, "Back discards the draft");
            menu.begin();
            menu.key(KeyCode::Digit3);
            assert!(menu.key(KeyCode::Enter));
            assert_eq!(menu.settings, Settings::PRESETS[2]);
            assert!(!menu.open);
        }
    }
}
