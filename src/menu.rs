//! Runtime config browser and its immediate-mode overlay.
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::ui::{Rect, UiVertex, push_rect, push_text};

pub struct Entry {
    pub path: PathBuf,
    pub name: String,
}

impl Entry {
    /// Puzzle titles avoid revealing body names, moon counts, or filenames.
    fn puzzle_title(&self, index: usize) -> String {
        match self.path.file_stem().and_then(|s| s.to_str()) {
            Some("puzzle") => "First field study".into(),
            Some("halo") => "Amber horizon".into(),
            Some("median-resonance") => "Clockwork sky".into(),
            Some("solar-system") => "Distant lights".into(),
            _ => format!("Uncharted map {:02}", index + 1),
        }
    }
}

pub fn discover(dir: &Path) -> Result<Vec<Entry>> {
    #[derive(Deserialize)]
    struct Name {
        name: String,
    }
    let mut entries = Vec::new();
    for file in std::fs::read_dir(dir)? {
        let path = file?.path();
        if !path.is_file()
            || !path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml")
            })
        {
            continue;
        }
        let name = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_saphyr::from_str::<Name>(&text).ok())
            .map(|meta| meta.name)
            .unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into()
            });
        entries.push(Entry { path, name });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

#[derive(Default)]
pub struct Menu {
    pub open: bool,
    pub entries: Vec<Entry>,
    pub offset: usize,
    pub error: Option<String>,
}

impl Menu {
    pub fn refresh(&mut self) {
        self.open = true;
        self.offset = 0;
        self.error = None;
        match discover(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/configs"))) {
            Ok(entries) => self.entries = entries,
            Err(error) => {
                self.entries.clear();
                self.error = Some(format!("Cannot read configs: {error}"));
            }
        }
    }
}

pub struct Layout {
    pub home: Rect,
    pub rows: Vec<Rect>,
    pub previous: Rect,
    pub next: Rect,
    pub resume: Rect,
    pub scale: f32,
}

impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 360.0).min(h / 300.0);
        let x = (w - (680.0 * s).min(w - 40.0 * s)) * 0.5;
        let width = w - x * 2.0;
        let count = ((h / s - 240.0) / 64.0).floor().clamp(1.0, 9.0) as usize;
        let rows = (0..count)
            .map(|i| Rect {
                x,
                y: (100.0 + i as f32 * 64.0) * s,
                w: width,
                h: 54.0 * s,
            })
            .collect();
        Self {
            home: crate::ui::layout(w, h, scale).home,
            rows,
            previous: Rect {
                x,
                y: h - 125.0 * s,
                w: 120.0 * s,
                h: 32.0 * s,
            },
            next: Rect {
                x: x + width - 120.0 * s,
                y: h - 125.0 * s,
                w: 120.0 * s,
                h: 32.0 * s,
            },
            resume: Rect {
                x: x + width - 140.0 * s,
                y: 28.0 * s,
                w: 140.0 * s,
                h: 28.0 * s,
            },
            scale: s,
        }
    }
}

fn text(
    verts: &mut Vec<UiVertex>,
    viewport: [f32; 2],
    rect: Rect,
    s: f32,
    value: &str,
    color: [f32; 4],
) {
    let max = (rect.w / (8.0 * s)).floor() as usize;
    let mut value: String = value
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    if value.len() > max {
        value.truncate(max.saturating_sub(3));
        value.push_str("...");
    }
    push_text(verts, rect.x, rect.y, s, &value, color, viewport);
}

pub fn build(
    verts: &mut Vec<UiVertex>,
    viewport: [f32; 2],
    scale: f32,
    menu: &Menu,
    cursor: (f64, f64),
    puzzle: bool,
) {
    if puzzle && !menu.open {
        return;
    }
    let [w, h] = viewport;
    let l = Layout::new(w, h, scale);
    let s = l.scale;
    let fg = [0.83, 0.90, 1.0, 1.0];
    let button = |verts: &mut Vec<UiVertex>, r: Rect| {
        let color = if r.contains(cursor.0, cursor.1) {
            [0.13, 0.26, 0.40, 1.0]
        } else {
            [0.055, 0.10, 0.17, 1.0]
        };
        push_rect(verts, r.x, r.y, r.w, r.h, color, viewport);
    };
    if menu.open {
        push_rect(verts, 0.0, 0.0, w, h, [0.008, 0.015, 0.03, 1.0], viewport);
        let x = l.rows[0].x;
        let width = l.rows[0].w;
        text(
            verts,
            viewport,
            Rect {
                x,
                y: 28.0 * s,
                w: width,
                h: 24.0 * s,
            },
            2.5 * s,
            if puzzle { "MAPS" } else { "STARGAZE" },
            fg,
        );
        text(
            verts,
            viewport,
            Rect {
                x,
                y: 65.0 * s,
                w: width,
                h: 16.0 * s,
            },
            1.5 * s,
            if puzzle {
                "Choose a map to start a new puzzle"
            } else {
                "Choose a solar system"
            },
            fg,
        );
        button(verts, l.resume);
        text(
            verts,
            viewport,
            Rect {
                x: l.resume.x + 10.0 * s,
                y: l.resume.y + 9.0 * s,
                ..l.resume
            },
            1.1 * s,
            "Resume [Esc]",
            fg,
        );
        for (i, (r, entry)) in l
            .rows
            .iter()
            .zip(menu.entries.iter().skip(menu.offset))
            .enumerate()
        {
            button(verts, *r);
            text(
                verts,
                viewport,
                Rect {
                    x: r.x + 12.0 * s,
                    y: r.y + 9.0 * s,
                    w: r.w - 24.0 * s,
                    h: r.h,
                },
                1.7 * s,
                &format!(
                    "[{}] {}",
                    i + 1,
                    if puzzle {
                        entry.puzzle_title(menu.offset + i)
                    } else {
                        entry.name.clone()
                    }
                ),
                fg,
            );
            text(
                verts,
                viewport,
                Rect {
                    x: r.x + 12.0 * s,
                    y: r.y + 32.0 * s,
                    w: r.w - 24.0 * s,
                    h: r.h,
                },
                s,
                &if puzzle {
                    "Start a fresh theory".into()
                } else {
                    entry.path.file_name().unwrap_or_default().to_string_lossy()
                },
                [0.5, 0.65, 0.8, 1.0],
            );
        }
        for (r, label, enabled) in [
            (l.previous, "< [PgUp]", menu.offset > 0),
            (
                l.next,
                "[PgDn] >",
                menu.offset + l.rows.len() < menu.entries.len(),
            ),
        ] {
            if enabled {
                button(verts, r);
                text(
                    verts,
                    viewport,
                    Rect {
                        x: r.x + 8.0 * s,
                        y: r.y + 10.0 * s,
                        ..r
                    },
                    1.2 * s,
                    label,
                    fg,
                );
            }
        }
        let message = menu.error.as_deref().unwrap_or(if menu.entries.is_empty() {
            "No YAML systems found in configs."
        } else if puzzle {
            "Choosing a map clears your theory. Resume keeps your current puzzle."
        } else {
            "Esc or home: resume current view"
        });
        // Wrap errors so validation details remain readable.
        let columns = (width / (8.0 * s)).floor().max(1.0) as usize;
        let chars: Vec<_> = message.chars().collect();
        for (i, line) in chars.chunks(columns).take(5).enumerate() {
            text(
                verts,
                viewport,
                Rect {
                    x,
                    y: h - (82.0 - i as f32 * 12.0) * s,
                    w: width,
                    h: 12.0 * s,
                },
                s,
                &line.iter().collect::<String>(),
                fg,
            );
        }
    }
    if puzzle {
        return;
    }
    button(verts, l.home);
    let hud_scale = crate::ui::layout(w, h, scale).scale;
    text(
        verts,
        viewport,
        Rect {
            x: l.home.x + 5.0 * hud_scale,
            y: l.home.y + 12.0 * hud_scale,
            ..l.home
        },
        1.1 * hud_scale,
        "[M]enu",
        fg,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_yaml_names_and_keeps_invalid_files_selectable() {
        let dir = std::env::temp_dir().join(format!("stargaze-menu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.yaml"), "name: Halo observatory\nversion: 1\n").unwrap();
        std::fs::write(dir.join("b.YML"), "invalid: [").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a config").unwrap();
        std::fs::create_dir_all(dir.join("folder.yaml")).unwrap();
        let entries = discover(&dir).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Halo observatory");
        assert_eq!(entries[1].name, "b");
        std::fs::write(dir.join("c.yml"), "name: New system").unwrap();
        assert_eq!(discover(&dir).unwrap().len(), 3);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn puzzle_menu_titles_hide_config_names_and_filenames() {
        let known = Entry {
            path: PathBuf::from("median-resonance.yaml"),
            name: "Secret planet and three moons".into(),
        };
        assert_eq!(known.puzzle_title(0), "Clockwork sky");
        let custom = Entry {
            path: PathBuf::from("three-stars-seven-moons.yaml"),
            name: "Solution hints".into(),
        };
        assert_eq!(custom.puzzle_title(4), "Uncharted map 05");
    }

    #[test]
    fn menu_controls_fit_small_and_hidpi_windows() {
        for (w, h, scale) in [
            (1280.0, 720.0, 1.0),
            (640.0, 480.0, 2.0),
            (320.0, 240.0, 1.0),
        ] {
            let l = Layout::new(w, h, scale);
            for r in l
                .rows
                .iter()
                .chain([&l.home, &l.previous, &l.next, &l.resume])
            {
                assert!(r.x >= 0.0 && r.y >= 0.0);
                assert!(r.x + r.w <= w && r.y + r.h <= h);
            }
            assert!(l.resume.y + l.resume.h < l.rows[0].y);
            let last = l.rows.last().unwrap();
            assert!(last.y + last.h < l.previous.y);
            let hud = crate::ui::layout(w, h, scale);
            assert!(l.home.y >= hud.bar.y);
            assert!(l.home.y + l.home.h <= hud.bar.y + hud.bar.h);
            assert!(l.home.x + l.home.w < hud.toggle.x);
            assert_eq!(l.home.y, hud.toggle.y);
        }
    }
}
