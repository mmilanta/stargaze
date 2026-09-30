//! Native Celestial Blueprint main menu, system browser, levels and controls.
use crate::ui::{self, Rect, UiVertex};
use anyhow::Result;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub struct Entry {
    pub path: PathBuf,
    pub name: String,
}

impl Entry {
    /// Puzzle titles avoid revealing body names, moon counts, or filenames.
    pub fn puzzle_title(&self, index: usize) -> String {
        match self.path.file_stem().and_then(|s| s.to_str()) {
            Some("puzzle") => "First field study".into(),
            Some("halo") => "Amber horizon".into(),
            Some("median-resonance") => "Clockwork sky".into(),
            Some("solar-system") => "Distant lights".into(),
            Some("vesper") => "Bright wanderers".into(),
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    Title,
    #[default]
    Explore,
    Levels,
    Controls,
}

#[derive(Default)]
pub struct Menu {
    pub open: bool,
    pub page: Page,
    pub entries: Vec<Entry>,
    pub offset: usize,
    pub selected: usize,
    pub title_selected: usize,
    pub error: Option<String>,
    pub search: String,
    pub searching: bool,
    pub favorites_only: bool,
    pub favorites: BTreeSet<String>,
    pub progress: BTreeMap<String, (usize, usize, Option<f32>)>,
    pub invalid: BTreeMap<PathBuf, String>,
    pub can_resume: bool,
}

impl Menu {
    pub fn refresh(&mut self) {
        self.open = true;
        self.offset = 0;
        self.selected = 0;
        self.error = None;
        match discover(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/configs"))) {
            Ok(entries) => self.entries = entries,
            Err(err) => {
                self.entries.clear();
                self.error = Some(format!("Cannot read configs: {err}"));
            }
        }
        self.invalid = self
            .entries
            .iter()
            .filter_map(|e| {
                crate::config::load(&e.path)
                    .err()
                    .map(|err| (e.path.clone(), format!("{err:#}")))
            })
            .collect();
    }
    pub fn visible(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                self.page != Page::Explore
                    || ((self.search.is_empty()
                        || e.name.to_lowercase().contains(&self.search.to_lowercase())
                        || e.path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&self.search.to_lowercase()))
                        && (!self.favorites_only
                            || self.favorites.contains(&crate::progress::key(&e.path))))
            })
            .map(|(i, _)| i)
            .collect()
    }
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.visible()
            .get(self.selected)
            .map(|&i| self.entries[i].path.clone())
    }
    pub fn move_selection(&mut self, forward: bool, page: usize) {
        let count = self.visible().len();
        if count == 0 {
            self.selected = 0;
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % count
        } else {
            (self.selected + count - 1) % count
        };
        self.offset = self.selected / page * page;
    }
    pub fn summary(&self, path: &Path) -> Option<(usize, usize, Option<f32>)> {
        self.progress.get(&crate::progress::key(path)).copied()
    }
}

pub struct Layout {
    #[cfg(test)]
    pub home: Rect,
    pub rows: Vec<Rect>,
    pub play: Vec<Rect>,
    pub discard: Vec<Rect>,
    pub titles: [Rect; 4],
    pub previous: Rect,
    pub next: Rect,
    pub resume: Rect,
    pub graphics: Rect,
    pub controls: Rect,
    pub switch: Rect,
    pub search: Rect,
    pub favorites: Rect,
    pub scale: f32,
}

impl Layout {
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 1060.0).min(h / 760.0);
        let width = 900.0 * s;
        let x = (w - width) * 0.5;
        let count = ((h / s - 290.0) / 92.0).floor().clamp(1.0, 9.0) as usize;
        let rows: Vec<Rect> = (0..count)
            .map(|i| Rect {
                x,
                y: (164.0 + i as f32 * 92.0) * s,
                w: width,
                h: 80.0 * s,
            })
            .collect();
        let play = rows
            .iter()
            .map(|r| Rect {
                x: r.x + r.w - 298.0 * s,
                y: r.y + 18.0 * s,
                w: 158.0 * s,
                h: 44.0 * s,
            })
            .collect();
        let discard = rows
            .iter()
            .map(|r| Rect {
                x: r.x + r.w - 128.0 * s,
                y: r.y + 18.0 * s,
                w: 110.0 * s,
                h: 44.0 * s,
            })
            .collect();
        let nav = |nx, nw| Rect {
            x: nx,
            y: h - 72.0 * s,
            w: nw * s,
            h: 34.0 * s,
        };
        Self {
            #[cfg(test)]
            home: ui::layout(w, h, scale).home,
            rows,
            play,
            discard,
            titles: std::array::from_fn(|i| Rect {
                x: (w - 360.0 * s) * 0.5,
                y: h * 0.43 + i as f32 * 68.0 * s,
                w: 360.0 * s,
                h: 60.0 * s,
            }),
            previous: nav(x, 110.0),
            next: nav(x + width - 110.0 * s, 110.0),
            graphics: nav(x + 124.0 * s, 140.0),
            controls: nav(x + 278.0 * s, 140.0),
            switch: nav(x + 432.0 * s, 160.0),
            resume: Rect {
                x: x + width - 158.0 * s,
                y: 40.0 * s,
                w: 158.0 * s,
                h: 34.0 * s,
            },
            search: Rect {
                x: x + 430.0 * s,
                y: 110.0 * s,
                w: 310.0 * s,
                h: 30.0 * s,
            },
            favorites: Rect {
                x: x + 750.0 * s,
                y: 110.0 * s,
                w: 150.0 * s,
                h: 30.0 * s,
            },
            scale: s,
        }
    }
}

fn trim(value: &str, width: f32, size: f32) -> String {
    let mut text = String::new();
    for ch in value.chars() {
        if ui::text_width(&(text.clone() + &ch.to_string() + "…"), size) > width {
            text.push('…');
            break;
        }
        text.push(ch);
    }
    text
}

pub fn build(
    v: &mut Vec<UiVertex>,
    vp: [f32; 2],
    scale: f32,
    menu: &Menu,
    cursor: (f64, f64),
    _puzzle: bool,
) {
    if !menu.open {
        return;
    }
    let [w, h] = vp;
    let l = Layout::new(w, h, scale);
    let s = l.scale;
    ui::menu_background(v, vp, s);
    let button = |v: &mut Vec<UiVertex>, r: Rect, label: &str, primary: bool| {
        ui::button(v, r, label, s, primary, r.contains(cursor.0, cursor.1), vp)
    };
    if menu.page == Page::Title {
        let px = 132.0 * s;
        let text = "stargaze";
        let tw = crate::typeface::width(crate::typeface::Face::Display, text, px);
        let x = (w - tw) * 0.5;
        ui::heading(v, x, h * 0.23, px, text, ui::INK, vp);
        ui::heading(v, x + tw, h * 0.23, px, ".", ui::ACCENT, vp);
        for (i, (label, desc)) in [
            ("Play", "Your levels"),
            ("Explore", "Any system, freely"),
            ("Settings", "Rendering and interface"),
            ("Quit", ""),
        ]
        .iter()
        .enumerate()
        {
            let r = l.titles[i];
            let active = menu.title_selected == i;
            let hover = r.contains(cursor.0, cursor.1);
            if active || hover {
                ui::push_rect(v, r.x, r.y, r.w, r.h, [0.6, 0.7, 1.0, 0.09], vp);
                ui::outline(v, r, s, ui::LINE, vp);
            }
            if active {
                ui::brackets(v, r, ui::OK, s, vp);
                ui::push_text(
                    v,
                    r.x + 14.0 * s,
                    r.y + 15.0 * s,
                    1.0 * s,
                    "▸",
                    ui::ACCENT,
                    vp,
                );
            }
            ui::heading(
                v,
                r.x + 40.0 * s,
                r.y + 8.0 * s,
                26.0 * s,
                label,
                if active { ui::ACCENT } else { ui::INK },
                vp,
            );
            ui::push_text(
                v,
                r.x + 40.0 * s,
                r.y + 39.0 * s,
                0.75 * s,
                desc,
                ui::MUTED,
                vp,
            );
        }
        if menu.can_resume {
            button(v, l.resume, "Resume [Esc]", false);
        }
        ui::push_text(
            v,
            (w - 220.0 * s) * 0.5,
            h - 36.0 * s,
            0.75 * s,
            "[↑ ↓] choose   [Enter] open",
            ui::MUTED,
            vp,
        );
    } else {
        let x = l.rows[0].x;
        let (title, sub) = match menu.page {
            Page::Explore => ("Explore", "ANY SYSTEM / LABELS AND TARGETS ON"),
            Page::Levels => ("Levels", "PLAY / RECONSTRUCT UNKNOWN SKIES"),
            _ => ("Controls", "EVERY KEY / ONE REFERENCE"),
        };
        ui::push_text(v, x, 44.0 * s, 0.75 * s, sub, ui::ACCENT, vp);
        ui::heading(v, x, 73.0 * s, 48.0 * s, title, ui::INK, vp);
        if menu.can_resume {
            button(v, l.resume, "Resume [Esc]", false);
        }
        if menu.page == Page::Controls {
            build_controls(v, vp, &l);
        } else {
            let indices = menu.visible();
            if menu.page == Page::Explore {
                button(
                    v,
                    l.search,
                    &format!(
                        "{}{}",
                        if menu.searching { "> " } else { "Search [/] " },
                        menu.search
                    ),
                    menu.searching,
                );
                button(
                    v,
                    l.favorites,
                    if menu.favorites_only {
                        "Favorites [B]"
                    } else {
                        "All / favs [B]"
                    },
                    menu.favorites_only,
                );
            }
            if indices.is_empty() {
                ui::heading(
                    v,
                    x + 24.0 * s,
                    190.0 * s,
                    26.0 * s,
                    "No systems found",
                    ui::INK,
                    vp,
                );
                ui::push_text(
                    v,
                    x + 24.0 * s,
                    230.0 * s,
                    0.9 * s,
                    "Clear the search or add a YAML system and reopen the menu.",
                    ui::MUTED,
                    vp,
                );
            }
            for (row, &index) in indices
                .iter()
                .skip(menu.offset)
                .take(l.rows.len())
                .enumerate()
            {
                let entry = &menu.entries[index];
                let r = l.rows[row];
                let active = menu.selected == menu.offset + row;
                ui::push_rect(
                    v,
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    if active {
                        [0.013, 0.063, 0.52, 0.88]
                    } else {
                        [ui::BG[0], ui::BG[1], ui::BG[2], 0.5]
                    },
                    vp,
                );
                ui::outline(v, r, s, if active { ui::INK } else { ui::LINE }, vp);
                if active {
                    ui::brackets(v, r, ui::OK, s, vp);
                }
                ui::push_text(
                    v,
                    r.x + 18.0 * s,
                    r.y + 20.0 * s,
                    0.8 * s,
                    &format!("{:02}", row + 1),
                    if active { ui::ACCENT } else { ui::MUTED },
                    vp,
                );
                let label = if menu.page == Page::Levels {
                    entry.puzzle_title(index)
                } else {
                    entry.name.clone()
                };
                let label = trim(&label, 460.0 * s, 1.45 * s);
                ui::heading(
                    v,
                    r.x + 62.0 * s,
                    r.y + 13.0 * s,
                    25.0 * s,
                    &label,
                    ui::INK,
                    vp,
                );
                let invalid = menu.invalid.get(&entry.path);
                if menu.page == Page::Levels {
                    let summary = menu.summary(&entry.path);
                    let (objects, attempts, best) = summary.unwrap_or_default();
                    let status = if best.is_some_and(|b| b >= 99.9) {
                        "Solved"
                    } else if summary.is_some() {
                        "In progress"
                    } else {
                        "Not started"
                    };
                    let text = if invalid.is_some() {
                        "Could not load this map".into()
                    } else {
                        format!(
                            "{status}  {}  {objects} objects / {attempts} checks",
                            best.map_or(String::new(), |b| format!("best {b:.1}"))
                        )
                    };
                    ui::push_rect(
                        v,
                        r.x + 62.0 * s,
                        r.y + 60.0 * s,
                        92.0 * s,
                        3.0 * s,
                        ui::LINE,
                        vp,
                    );
                    ui::push_rect(
                        v,
                        r.x + 62.0 * s,
                        r.y + 60.0 * s,
                        92.0 * s * best.unwrap_or(0.0) / 100.0,
                        3.0 * s,
                        ui::ACCENT,
                        vp,
                    );
                    ui::push_text(
                        v,
                        r.x + 168.0 * s,
                        r.y + 52.0 * s,
                        0.65 * s,
                        &text,
                        if invalid.is_some() {
                            ui::WARN
                        } else {
                            ui::MUTED
                        },
                        vp,
                    );
                    if summary.is_some() {
                        button(v, l.discard[row], "Discard [D]", false);
                    }
                    button(
                        v,
                        l.play[row],
                        if invalid.is_some() {
                            "Unavailable"
                        } else if summary.is_some() {
                            "Resume [Enter]"
                        } else {
                            "Play [Enter]"
                        },
                        invalid.is_none(),
                    );
                } else {
                    let detail = invalid
                        .map(|_| "Invalid configuration — select to see the error".to_owned())
                        .unwrap_or_else(|| {
                            format!(
                                "{}{}",
                                entry.path.file_name().unwrap_or_default().to_string_lossy(),
                                if menu.favorites.contains(&crate::progress::key(&entry.path)) {
                                    "  / favorite"
                                } else {
                                    ""
                                }
                            )
                        });
                    ui::push_text(
                        v,
                        r.x + 62.0 * s,
                        r.y + 50.0 * s,
                        0.75 * s,
                        &trim(&detail, 450.0 * s, 0.75 * s),
                        if invalid.is_some() {
                            ui::WARN
                        } else {
                            ui::MUTED
                        },
                        vp,
                    );
                    button(
                        v,
                        l.play[row],
                        if invalid.is_some() {
                            "View error [Enter]"
                        } else {
                            "Explore [Enter]"
                        },
                        invalid.is_none(),
                    );
                    button(
                        v,
                        l.discard[row],
                        "Favorite [F]",
                        menu.favorites.contains(&crate::progress::key(&entry.path)),
                    );
                }
            }
            button(v, l.previous, "Prev [PgUp]", false);
            button(v, l.next, "Next [PgDn]", false);
            button(
                v,
                l.switch,
                if menu.page == Page::Levels {
                    "Explore [P]"
                } else {
                    "Levels [P]"
                },
                false,
            );
        }
        button(v, l.graphics, "Settings [G]", false);
        button(v, l.controls, "Controls [C]", false);
        ui::push_text(
            v,
            x,
            h - 24.0 * s,
            0.75 * s,
            "[↑ ↓] select   [Enter] open   [T] main menu   [Esc] back",
            ui::MUTED,
            vp,
        );
    }
    if let Some(err) = &menu.error {
        let r = Rect {
            x: (w - 700.0 * s) * 0.5,
            y: h - 132.0 * s,
            w: 700.0 * s,
            h: 42.0 * s,
        };
        ui::push_rect(v, r.x, r.y, r.w, r.h, ui::DEEP, vp);
        ui::outline(v, r, s, ui::WARN, vp);
        ui::push_text(
            v,
            r.x + 12.0 * s,
            r.y + 12.0 * s,
            0.75 * s,
            &trim(err, 676.0 * s, 0.75 * s),
            ui::WARN,
            vp,
        );
    }
}

fn build_controls(v: &mut Vec<UiVertex>, vp: [f32; 2], l: &Layout) {
    let s = l.scale;
    let x = l.rows[0].x;
    let groups: [(&str, &[(&str, &str)]); 3] = [
        (
            "MENUS",
            &[
                ("↑ ↓", "Select"),
                ("Enter / →", "Open"),
                ("Esc / ←", "Back / resume"),
                ("1–9", "Choose entry"),
                ("PgUp / PgDn", "Change page"),
                ("/", "Search"),
                ("F / B", "Favorite / filter"),
                ("P", "Explore / levels"),
                ("G / C", "Settings / controls"),
                ("T", "Main menu"),
            ],
        ),
        (
            "OBSERVATION",
            &[
                ("Drag", "Look around"),
                ("Right-click", "Follow body / sky"),
                ("Click / U", "Release lock"),
                ("Scroll / + −", "Zoom"),
                ("← →", "Signed speed"),
                ("Space", "Stop / resume"),
                ("PgDn / PgUp", "−1 / +1 local day"),
                ("R", "Horizon / stars"),
                ("Home", "Reset view"),
                (", . / A", "Exposure / auto"),
                ("L / H / G", "Labels / HUD / targets"),
                ("F1–F9", "Aim at target"),
                ("E / T", "Eclipse / transit"),
            ],
        ),
        (
            "THEORY",
            &[
                ("Tab", "Observe / theory"),
                ("Right-click / N", "Add orbiting body"),
                ("Drag", "Order orbits"),
                ("S", "Is star"),
                ("R", "Has rings"),
                ("V", "Viewer here"),
                ("Del", "Delete branch"),
                ("Z / Y", "Undo / redo"),
                ("X", "Clear theory"),
                ("Enter", "Check theory"),
                ("M / Esc", "Leave field study"),
            ],
        ),
    ];
    for (column, (title, keys)) in groups.iter().enumerate() {
        let cx = x + column as f32 * 304.0 * s;
        ui::push_text(v, cx, 165.0 * s, 0.85 * s, title, ui::ACCENT, vp);
        for (row, (key, desc)) in keys.iter().enumerate() {
            let y = (200.0 + row as f32 * 27.0) * s;
            ui::push_text(v, cx, y, 0.75 * s, key, ui::INK, vp);
            ui::push_text(v, cx + 130.0 * s, y, 0.68 * s, desc, ui::MUTED, vp);
        }
    }
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
                .chain([&l.home, &l.previous, &l.next, &l.resume, &l.graphics])
            {
                assert!(r.x >= 0.0 && r.y >= 0.0);
                assert!(r.x + r.w <= w && r.y + r.h <= h);
            }
            assert!(l.resume.y + l.resume.h < l.rows[0].y);
            assert!(l.previous.x + l.previous.w <= l.graphics.x);
            assert!(l.graphics.x + l.graphics.w <= l.next.x);
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
