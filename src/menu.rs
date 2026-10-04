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
    pub controls_scroll: f32,
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
    pub fn hover(&mut self, l: &Layout, cursor: (f64, f64)) {
        if self.page == Page::Title {
            if let Some(i) = l.titles.iter().position(|r| r.contains(cursor.0, cursor.1)) {
                self.title_selected = i;
            }
        } else if matches!(self.page, Page::Explore | Page::Levels)
            && let Some(i) = l.rows.iter().position(|r| r.contains(cursor.0, cursor.1))
            && self.offset + i < self.visible().len()
        {
            self.selected = self.offset + i;
        }
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
    pub scale: f32,
}

impl Layout {
    pub fn play_for(&self, row: usize, page: Page) -> Rect {
        let r = self.play[row];
        if page == Page::Explore {
            Rect {
                x: self.rows[row].x + self.rows[row].w - 178.0 * self.scale,
                ..r
            }
        } else {
            r
        }
    }
    pub fn new(w: f32, h: f32, scale: f32) -> Self {
        let s = scale.min(w / 1000.0).min(h / 600.0);
        let width = 872.0 * s;
        let x = (w - 1000.0 * s) * 0.5 + 64.0 * s;
        let count = ((h / s - 190.0) / 98.0).floor().clamp(1.0, 9.0) as usize;
        let rows: Vec<Rect> = (0..count)
            .map(|i| Rect {
                x,
                y: (158.0 + i as f32 * 98.0) * s,
                w: width,
                h: 86.0 * s,
            })
            .collect();
        let play = rows
            .iter()
            .map(|r| Rect {
                x: r.x + r.w - 336.0 * s,
                y: r.y + 20.0 * s,
                w: 160.0 * s,
                h: 46.0 * s,
            })
            .collect();
        let discard = rows
            .iter()
            .map(|r| Rect {
                x: r.x + r.w - 158.0 * s,
                y: r.y + 20.0 * s,
                w: 140.0 * s,
                h: 46.0 * s,
            })
            .collect();
        let px = (w * 0.125).clamp(96.0 * s, 196.0 * s).min(h * 0.23);
        let top = (h - px - 368.0 * s) * 0.5;
        Self {
            #[cfg(test)]
            home: ui::layout(w, h, scale).home,
            rows,
            play,
            discard,
            titles: std::array::from_fn(|i| Rect {
                x: (w - 380.0 * s) * 0.5,
                y: top + px + 52.0 * s + i as f32 * 78.5 * s,
                w: 380.0 * s,
                h: if i == 3 { 56.0 * s } else { 70.5 * s },
            }),
            previous: Rect {
                x,
                y: h - 32.0 * s,
                w: 110.0 * s,
                h: 26.0 * s,
            },
            next: Rect {
                x: x + width - 110.0 * s,
                y: h - 32.0 * s,
                w: 110.0 * s,
                h: 26.0 * s,
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
        let word_start = v.len();
        let px = (w * 0.125).clamp(96.0 * s, 196.0 * s).min(h * 0.23);
        let top = (h - px - 368.0 * s) * 0.5;
        let tracking = -0.065 * px;
        let tw =
            crate::typeface::width(crate::typeface::Face::Display, "stargaze", px) + 7.0 * tracking;
        let total = tw + 0.70 * px;
        let x = (w - total) * 0.5;
        let p = [x + 0.20 * px, top + 0.50 * px];
        // The orbit mark is vector geometry, just like the prototype SVG.
        ui::ellipse(v, p, [0.07 * px, 0.07 * px], 1.7 * s, ui::ACCENT, vp);
        for i in 0..64 {
            let point = |j: usize| {
                let a = j as f32 * std::f32::consts::TAU / 64.0;
                let dx = 0.17 * px * a.cos();
                let dy = 0.075 * px * a.sin();
                [p[0] + dx * 0.80 + dy * 0.60, p[1] - dx * 0.60 + dy * 0.80]
            };
            ui::line(v, point(i), point(i + 1), 1.7 * s, ui::ACCENT, vp);
        }
        ui::tracked_heading(
            v,
            [x + 0.59 * px, top],
            px,
            "stargaze",
            tracking,
            ui::INK,
            vp,
        );
        ui::disk(
            v,
            [x + 0.59 * px + tw + 0.10 * px, top + 0.78 * px],
            0.061 * px,
            ui::ACCENT,
            vp,
        );
        crate::motion::rise(&mut v[word_start..], 0.9, 0.0);
        for (i, (label, desc)) in [
            ("Play", "Your levels"),
            ("Explore", "Any system, freely"),
            ("Settings", "Rendering, HUD scale"),
            ("Quit", ""),
        ]
        .iter()
        .enumerate()
        {
            let row_start = v.len();
            let r = l.titles[i];
            let active = menu.title_selected == i;
            if active {
                ui::push_rect(v, r.x, r.y, r.w, r.h, [0.871, 0.904, 1.0, 0.08], vp);
                ui::outline(v, r, s, ui::LINE, vp);
                ui::brackets(v, r, ui::OK, s, vp);
                ui::triangle(
                    v,
                    [
                        [r.x + 21.0 * s, r.y + r.h * 0.5 - 2.0 * s],
                        [r.x + 25.0 * s, r.y + r.h * 0.5],
                        [r.x + 21.0 * s, r.y + r.h * 0.5 + 2.0 * s],
                    ],
                    ui::ACCENT,
                    vp,
                );
            }
            let x = r.x + (if active { 53.0 } else { 50.0 }) * s;
            ui::tracked_heading(
                v,
                [x, r.y + 13.0 * s],
                26.0 * s,
                label,
                -0.6 * s,
                if active { ui::ACCENT } else { ui::INK },
                vp,
            );
            ui::push_text(
                v,
                r.x + 50.0 * s,
                r.y + 46.0 * s,
                0.75 * s,
                desc,
                ui::MUTED,
                vp,
            );
            crate::motion::rise(&mut v[row_start..], 0.5, 0.35 + i as f32 * 0.07);
        }
    } else {
        let x = l.rows[0].x;
        let (title, sub) = match menu.page {
            Page::Explore => ("Explore", "ANY SYSTEM · LABELS AND TARGETS ON"),
            Page::Levels => ("Levels", "PLAY · RECONSTRUCT UNKNOWN SKIES"),
            _ => ("Controls", "EVERY KEY, ONE PAGE"),
        };
        ui::eyebrow(v, [x, 48.0 * s], 9.0 * s, sub, ui::ACCENT, vp);
        ui::tracked_heading(v, [x, 71.0 * s], 50.0 * s, title, -2.0 * s, ui::INK, vp);
        if menu.page == Page::Controls {
            build_controls(v, vp, &l, menu.controls_scroll);
        } else {
            let indices = menu.visible();
            if indices.is_empty() {
                let r = Rect {
                    x,
                    y: 158.0 * s,
                    w: 872.0 * s,
                    h: 170.0 * s,
                };
                ui::outline(v, r, s, ui::LINE, vp);
                ui::heading(
                    v,
                    x + 60.0 * s,
                    r.y + 55.0 * s,
                    26.0 * s,
                    "No systems found",
                    ui::INK,
                    vp,
                );
                ui::push_text(
                    v,
                    x + 60.0 * s,
                    r.y + 100.0 * s,
                    0.825 * s,
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
                let row_start = v.len();
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
                        [0.0137, 0.063, 0.521, 0.88]
                    } else {
                        [ui::BG[0], ui::BG[1], ui::BG[2], 0.5]
                    },
                    vp,
                );
                ui::outline(
                    v,
                    r,
                    s,
                    if active {
                        ui::INK
                    } else {
                        [ui::LINE[0], ui::LINE[1], ui::LINE[2], 0.14]
                    },
                    vp,
                );
                if active {
                    ui::brackets(v, r, ui::OK, s, vp);
                }
                ui::push_text(
                    v,
                    r.x + 18.0 * s,
                    r.y + 37.0 * s,
                    0.825 * s,
                    &format!("{:02}", menu.offset + row + 1),
                    if active { ui::ACCENT } else { ui::MUTED },
                    vp,
                );
                let label = if menu.page == Page::Levels {
                    entry.puzzle_title(index)
                } else {
                    entry.name.clone()
                };
                let info_x = r.x + 76.0 * s;
                let play = if menu.page == Page::Explore {
                    Rect {
                        x: r.x + r.w - 178.0 * s,
                        ..l.play[row]
                    }
                } else {
                    l.play[row]
                };
                let available = play.x - info_x - 18.0 * s;
                let mut title = label.clone();
                while crate::typeface::width(crate::typeface::Face::Sans, &title, 25.0 * s)
                    > available
                    && title.len() > 3
                {
                    title.pop();
                }
                if title != label {
                    title.pop();
                    title.push('…');
                }
                ui::tracked_heading(
                    v,
                    [info_x, r.y + 16.0 * s],
                    25.0 * s,
                    &title,
                    -0.6 * s,
                    ui::INK,
                    vp,
                );
                let invalid = menu.invalid.get(&entry.path);
                if menu.page == Page::Levels {
                    let summary = menu.summary(&entry.path);
                    let (objects, _, best) = summary.unwrap_or_default();
                    let solved = best.is_some_and(|b| b >= 99.9);
                    let status = if solved {
                        "Solved"
                    } else if summary.is_some() {
                        "In progress"
                    } else {
                        "Not started"
                    };
                    ui::push_rect(v, info_x, r.y + 59.0 * s, 180.0 * s, 4.0 * s, ui::LINE, vp);
                    ui::push_rect(
                        v,
                        info_x,
                        r.y + 59.0 * s,
                        180.0 * s * best.unwrap_or(0.0) / 100.0,
                        4.0 * s,
                        if solved { ui::OK } else { ui::ACCENT },
                        vp,
                    );
                    let detail = format!(
                        "{status}{}{}",
                        best.map_or(String::new(), |b| format!("  best {b:.1}")),
                        if summary.is_some() && objects > 0 {
                            format!(" / {objects} objects drawn")
                        } else {
                            String::new()
                        }
                    );
                    ui::push_text(
                        v,
                        info_x + 192.0 * s,
                        r.y + 55.0 * s,
                        0.69 * s,
                        &trim(&detail, available - 192.0 * s, 0.69 * s),
                        if solved { ui::OK } else { ui::MUTED },
                        vp,
                    );
                    let start = v.len();
                    button(
                        v,
                        l.discard[row],
                        if active {
                            "× Discard [D]"
                        } else {
                            "× Discard"
                        },
                        false,
                    );
                    for vertex in &mut v[start..] {
                        if vertex.color[..3] == ui::INK[..3] || vertex.color[..3] == ui::LINE[..3] {
                            vertex.color[..3].copy_from_slice(&ui::WARN[..3]);
                        }
                        if summary.is_none() {
                            vertex.color[3] *= 0.25;
                        }
                    }
                    button(
                        v,
                        play,
                        if invalid.is_some() {
                            "Invalid"
                        } else if summary.is_some() {
                            if active { "Resume [Enter]" } else { "Resume" }
                        } else if active {
                            "Play [Enter]"
                        } else {
                            "Play"
                        },
                        invalid.is_none(),
                    );
                } else {
                    ui::push_text(
                        v,
                        info_x,
                        r.y + 51.0 * s,
                        0.75 * s,
                        &trim(
                            &entry.path.file_name().unwrap_or_default().to_string_lossy(),
                            available,
                            0.75 * s,
                        ),
                        ui::MUTED,
                        vp,
                    );
                    if let Some(error) = invalid {
                        ui::push_text(
                            v,
                            info_x,
                            r.y + 67.0 * s,
                            0.675 * s,
                            &trim(error.lines().next().unwrap_or(error), available, 0.675 * s),
                            ui::WARN,
                            vp,
                        );
                    }
                    let start = v.len();
                    button(
                        v,
                        play,
                        if invalid.is_some() {
                            "Invalid"
                        } else if active {
                            "Explore [Enter]"
                        } else {
                            "Explore"
                        },
                        invalid.is_none(),
                    );
                    if invalid.is_some() {
                        for vertex in &mut v[start..] {
                            vertex.color[3] *= 0.25;
                        }
                    }
                }
                crate::motion::rise(&mut v[row_start..], 0.45, 0.12 + row as f32 * 0.06);
            }
            if indices.len() > l.rows.len() {
                button(v, l.previous, "Prev [PgUp]", false);
                button(v, l.next, "Next [PgDn]", false);
            }
            if menu.searching || !menu.search.is_empty() || menu.favorites_only {
                ui::push_text(
                    v,
                    x,
                    131.0 * s,
                    0.75 * s,
                    &format!(
                        "Search: {}{}",
                        menu.search,
                        if menu.favorites_only {
                            " / favorites"
                        } else {
                            ""
                        }
                    ),
                    ui::MUTED,
                    vp,
                );
            }
        }
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

pub fn controls_max_scroll(l: &Layout, h: f32) -> f32 {
    (740.0 - h / l.scale).max(0.0)
}
fn build_controls(v: &mut Vec<UiVertex>, vp: [f32; 2], l: &Layout, scroll: f32) {
    let s = l.scale;
    let x = l.rows[0].x;
    let offset = scroll.clamp(0.0, controls_max_scroll(l, vp[1])) * s;
    let start = v.len();
    let groups: [(&str, &[(&str, &str)]); 3] = [
        (
            "MENUS",
            &[
                ("M Esc", "open / resume"),
                ("1–9", "choose a system or map"),
                ("PgUp PgDn", "change page"),
                ("Enter →", "open selected"),
                ("Esc ←", "back"),
                ("/", "search"),
                ("F", "favorite"),
                ("P", "Explore ↔ Levels"),
                ("T", "title screen"),
                ("G C", "settings / controls"),
                ("B", "favorite filter"),
            ],
        ),
        (
            "OBSERVATION",
            &[
                ("drag", "look around"),
                ("right-click", "lock body / sky direction"),
                ("left-click U", "release lock"),
                ("scroll = -", "zoom (to 0.001°)"),
                ("← →", "signed speed"),
                ("Space", "stop / resume"),
                ("PgDn PgUp", "−1 / +1 local day"),
                ("R", "horizon level / stars fixed"),
                (", .", "exposure ±0.25 EV"),
                ("A", "auto exposure"),
                ("L", "labels"),
                ("H", "hide HUD"),
                ("F1–F9", "aim at targets"),
                ("E", "find eclipse"),
                ("T", "find transit"),
                ("Home", "reset view"),
                ("G", "targets panel"),
                ("F11", "settings"),
            ],
        ),
        (
            "THEORY EDITOR",
            &[
                ("Tab", "observe ↔ theory"),
                ("right-click", "add orbiting object"),
                ("drag", "reorder orbits"),
                ("S", "is star"),
                ("R", "has rings"),
                ("V", "viewer here"),
                ("Del", "delete branch"),
                ("Z Y", "undo / redo"),
                ("X", "clear all"),
                ("Enter", "check theory"),
                ("M Esc", "maps"),
            ],
        ),
    ];
    for (column, (title, keys)) in groups.iter().enumerate() {
        let cx = x + column as f32 * 299.0 * s;
        ui::eyebrow(v, [cx, 160.0 * s - offset], 9.0 * s, title, ui::ACCENT, vp);
        ui::push_rect(v, cx, 179.0 * s - offset, 273.0 * s, s, ui::LINE, vp);
        for (row, (key, desc)) in keys.iter().enumerate() {
            let y = (194.0 + row as f32 * 31.0) * s - offset;
            let mut kx = cx;
            for k in key.split_whitespace() {
                let width = ui::text_width(k, 0.7875 * s) + 10.0 * s;
                ui::keycap(
                    v,
                    Rect {
                        x: kx,
                        y: y - 1.0 * s,
                        w: width,
                        h: 17.0 * s,
                    },
                    k,
                    s * 1.05,
                    ui::INK,
                    vp,
                );
                kx += width + 4.0 * s;
            }
            ui::push_text(
                v,
                cx + 273.0 * s - ui::text_width(desc, 0.7875 * s),
                y,
                0.7875 * s,
                desc,
                ui::MUTED,
                vp,
            );
            ui::push_rect(
                v,
                cx,
                y + 25.0 * s,
                273.0 * s,
                s,
                [ui::LINE[0], ui::LINE[1], ui::LINE[2], 0.14],
                vp,
            );
        }
    }
    ui::clip(
        v,
        start,
        Rect {
            x,
            y: 150.0 * s,
            w: 872.0 * s,
            h: vp[1] - 162.0 * s,
        },
        vp,
    );
    let max = controls_max_scroll(l, vp[1]);
    if max > 0.0 {
        let h = vp[1] - 162.0 * s;
        let thumb = h * h / (h + max * s);
        let tx = x + 880.0 * s;
        ui::push_rect(v, tx, 150.0 * s, 3.0 * s, h, ui::LINE, vp);
        ui::push_rect(
            v,
            tx,
            150.0 * s + (h - thumb) * scroll / max,
            3.0 * s,
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
    fn mouse_hover_and_keyboard_use_the_same_filtered_selection() {
        let mut m = Menu {
            page: Page::Title,
            ..Menu::default()
        };
        let l = Layout::new(1280.0, 720.0, 1.0);
        let r = l.titles[2];
        m.hover(&l, ((r.x + 10.0) as f64, (r.y + 10.0) as f64));
        assert_eq!(m.title_selected, 2);
        m.page = Page::Explore;
        m.refresh();
        m.search = "vesper".into();
        m.hover(&l, ((l.rows[0].x + 5.0) as f64, (l.rows[0].y + 5.0) as f64));
        assert!(m.selected_path().unwrap().ends_with("vesper.yaml"));
        m.move_selection(true, l.rows.len());
        assert_eq!(m.selected, 0);
        let play = l.play_for(0, Page::Explore);
        assert!(play.x > l.play_for(0, Page::Levels).x);
    }
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
            for r in l.rows.iter().chain([&l.home, &l.previous, &l.next]) {
                assert!(r.x >= 0.0 && r.y >= 0.0);
                assert!(r.x + r.w <= w && r.y + r.h <= h);
            }
            assert!(l.previous.x + l.previous.w < l.next.x);
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
