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
        },
        Self {
            max_fps: 30,
            resolution_percent: 75,
            max_bounces: 4,
            samples_per_frame: 1,
            still_samples: 256,
        },
        Self {
            max_fps: 60,
            resolution_percent: 100,
            max_bounces: 8,
            samples_per_frame: 1,
            still_samples: 1024,
        },
    ];

    pub fn normalized(mut self) -> Self {
        self.max_fps = self.max_fps.min(240);
        self.resolution_percent = self.resolution_percent.clamp(25, 100);
        self.max_bounces = self.max_bounces.clamp(1, 64);
        self.samples_per_frame = self.samples_per_frame.clamp(1, 64);
        self.still_samples = self.still_samples.min(MAX_SAMPLES);
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

    pub fn click(&mut self, layout: &Layout, cursor: (f64, f64)) -> bool {
        let before = self.settings;
        if layout.back.contains(cursor.0, cursor.1) {
            self.open = false;
        }
        for (i, rect) in layout.presets.iter().enumerate() {
            if rect.contains(cursor.0, cursor.1) {
                self.settings = Settings::PRESETS[i];
            }
        }
        for (i, [decrease, increase]) in layout.arrows.iter().enumerate() {
            if decrease.contains(cursor.0, cursor.1) {
                self.selected = i;
                self.settings.change(i, false);
            } else if increase.contains(cursor.0, cursor.1) {
                self.selected = i;
                self.settings.change(i, true);
            }
        }
        self.settings != before
    }

    pub fn key(&mut self, key: KeyCode) -> bool {
        let before = self.settings;
        match key {
            KeyCode::Escape | KeyCode::KeyM | KeyCode::KeyG | KeyCode::Enter => self.open = false,
            KeyCode::ArrowUp => self.selected = self.selected.saturating_sub(1),
            KeyCode::ArrowDown => self.selected = (self.selected + 1).min(4),
            KeyCode::ArrowLeft => self.settings.change(self.selected, false),
            KeyCode::ArrowRight => self.settings.change(self.selected, true),
            KeyCode::Digit1 => self.settings = Settings::PRESETS[0],
            KeyCode::Digit2 => self.settings = Settings::PRESETS[1],
            KeyCode::Digit3 => self.settings = Settings::PRESETS[2],
            _ => {}
        }
        self.settings != before
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
    pub arrows: [[Rect; 2]; 5],
    pub back: Rect,
}

impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 720.0).min(h / 620.0);
        let width = 680.0 * s;
        let x = (w - width) * 0.5;
        Self {
            scale: s,
            x,
            presets: std::array::from_fn(|i| Rect {
                x: x + i as f32 * 228.0 * s,
                y: 80.0 * s,
                w: 220.0 * s,
                h: 32.0 * s,
            }),
            arrows: std::array::from_fn(|i| {
                std::array::from_fn(|j| Rect {
                    x: x + (440.0 + j as f32 * 196.0) * s,
                    y: (142.0 + i as f32 * 78.0) * s,
                    w: 44.0 * s,
                    h: 32.0 * s,
                })
            }),
            back: Rect {
                x: x + width - 140.0 * s,
                y: 26.0 * s,
                w: 140.0 * s,
                h: 32.0 * s,
            },
        }
    }
}

pub fn build(
    vertices: &mut Vec<UiVertex>,
    viewport: [f32; 2],
    scale: f32,
    menu: &Menu,
    cursor: (f64, f64),
) {
    let [w, h] = viewport;
    let l = Layout::new(w, h, scale);
    let s = l.scale;
    ui::push_rect(
        vertices,
        0.0,
        0.0,
        w,
        h,
        [0.008, 0.015, 0.03, 1.0],
        viewport,
    );
    let text = |v: &mut Vec<UiVertex>, x, y, size, value: &str| {
        ui::push_text(v, x, y, size * s, value, [0.83, 0.90, 1.0, 1.0], viewport);
    };
    let button = |v: &mut Vec<UiVertex>, r: Rect, label, active| {
        ui::toolbar_button(
            v,
            r,
            label,
            s,
            active,
            r.contains(cursor.0, cursor.1),
            viewport,
        );
    };
    text(vertices, l.x, 28.0 * s, 2.5, "GRAPHICS");
    button(vertices, l.back, "Back [Esc]", false);
    for (i, label) in ["Eco [1]", "Balanced [2]", "High [3]"].iter().enumerate() {
        button(
            vertices,
            l.presets[i],
            label,
            menu.settings == Settings::PRESETS[i],
        );
    }
    let settings = menu.settings;
    let values = [
        if settings.max_fps == 0 {
            "Unlimited".into()
        } else {
            format!("{} FPS", settings.max_fps)
        },
        format!("{}%", settings.resolution_percent),
        format!("{} bounces", settings.max_bounces),
        settings.samples_per_frame.to_string(),
        if settings.still_samples == 0 {
            "Continuous".into()
        } else {
            format!("{} samples", settings.still_samples)
        },
    ];
    for (i, (label, help)) in [
        ("Frame rate", "Lower limits leave more time for other apps."),
        (
            "Render resolution",
            "Lower resolution reduces work. The UI stays sharp.",
        ),
        (
            "Light detail",
            "More bounces improve indirect light and cost more.",
        ),
        (
            "Samples per frame",
            "More samples refine faster but make each frame heavier.",
        ),
        (
            "Still image limit",
            "A stationary view finishes refining, then rests.",
        ),
    ]
    .iter()
    .enumerate()
    {
        let y = (142.0 + i as f32 * 78.0) * s;
        text(vertices, l.x, y + 7.0 * s, 1.6, label);
        text(vertices, l.x, y + 43.0 * s, 1.05, help);
        for j in 0..2 {
            button(
                vertices,
                l.arrows[i][j],
                if j == 0 { "<" } else { ">" },
                menu.selected == i,
            );
        }
        let value_scale = 1.25;
        let value_width = ui::text_width(&values[i], value_scale * s);
        text(
            vertices,
            l.x + 560.0 * s - value_width * 0.5,
            y + 10.0 * s,
            value_scale,
            &values[i],
        );
    }
    text(
        vertices,
        l.x,
        555.0 * s,
        1.05,
        "Up / Down: select   Left / Right: adjust   Esc: back",
    );
    text(vertices, l.x, 580.0 * s, 1.0, &menu.message);
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
            assert!(menu.click(&layout, center(layout.presets[0])));
            assert_eq!(menu.settings, Settings::PRESETS[0]);
            assert!(menu.click(&layout, center(layout.arrows[1][1])));
            assert_eq!(menu.settings.resolution_percent, 75);
            assert!(menu.key(KeyCode::ArrowRight));
            assert_eq!(menu.settings.resolution_percent, 100);
            assert!(!menu.key(KeyCode::ArrowRight));
            assert!(menu.key(KeyCode::Digit3));
            assert_eq!(menu.settings, Settings::PRESETS[2]);
            assert!(!menu.click(&layout, center(layout.back)));
            assert!(!menu.open);
        }
    }
}
