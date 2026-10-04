//! Screen routing and overlays for the native Blueprint interface.
use crate::{App, Leave, menu, ui};
use std::time::Instant;
use ui::{Rect, UiVertex};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

struct Modal {
    panel: Rect,
    keep: Rect,
    discard: Rect,
    cancel: Rect,
    scale: f32,
}
impl Modal {
    fn new(vp: [f32; 2], scale: f32) -> Self {
        let s = scale.min(vp[0] / 800.0).min(vp[1] / 500.0);
        let panel = Rect {
            x: (vp[0] - 680.0 * s) * 0.5,
            y: (vp[1] - 240.0 * s) * 0.5,
            w: 680.0 * s,
            h: 240.0 * s,
        };
        let rect = |x, w| Rect {
            x: panel.x + x * s,
            y: panel.y + 178.0 * s,
            w: w * s,
            h: 38.0 * s,
        };
        Self {
            panel,
            keep: rect(442.0, 214.0),
            discard: rect(216.0, 210.0),
            cancel: rect(24.0, 176.0),
            scale: s,
        }
    }
}

impl App {
    pub(crate) fn open_settings(&mut self) {
        self.graphics.begin();
        self.state.last = Instant::now();
        self.exposure_dragging = false;
        self.state.end_sky_press();
    }

    pub(crate) fn settings_key(&mut self, key: KeyCode) {
        if self.graphics.key(key) {
            self.sync_graphics();
        }
        self.state.last = Instant::now();
    }

    pub(crate) fn title_action(&mut self, i: usize, event_loop: Option<&ActiveEventLoop>) {
        self.menu.title_selected = i;
        match i {
            0 => self.open_page(menu::Page::Levels),
            1 => self.open_page(menu::Page::Explore),
            2 => self.open_settings(),
            3 => {
                if self.game.as_ref().is_some_and(|g| g.has_progress())
                    && self.active_path.is_some()
                {
                    self.request_leave(Leave::Quit);
                } else {
                    let saved = self.save_progress();
                    if saved || !self.study_started {
                        if let Some(event_loop) = event_loop {
                            event_loop.exit();
                        }
                    } else {
                        self.leave = Some(Leave::Quit);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn modal_key(&mut self, key: KeyCode, event_loop: &ActiveEventLoop) -> bool {
        if let Some(to) = self.leave {
            match key {
                KeyCode::Escape => self.leave = None,
                KeyCode::Enter | KeyCode::KeyK => {
                    let done = self.finish_leave(to, true);
                    if done && matches!(to, Leave::Quit) {
                        event_loop.exit();
                    }
                }
                KeyCode::KeyD => {
                    let done = self.finish_leave(to, false);
                    if done && matches!(to, Leave::Quit) {
                        event_loop.exit();
                    }
                }
                _ => {}
            }
            self.state.last = Instant::now();
            return true;
        }
        if self.discard.is_some() {
            match key {
                KeyCode::Escape => self.discard = None,
                KeyCode::Enter | KeyCode::KeyD => self.confirm_discard(),
                _ => {}
            }
            return true;
        }
        false
    }

    pub(crate) fn modal_click(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if self.leave.is_none() && self.discard.is_none() {
            return false;
        }
        let Some(r) = &self.renderer else { return true };
        let (w, h) = r.size();
        let m = Modal::new([w as f32, h as f32], self.scale);
        let (x, y) = self.cursor;
        if m.cancel.contains(x, y) {
            self.leave = None;
            self.discard = None;
            self.state.last = Instant::now();
        } else if let Some(to) = self.leave {
            if m.keep.contains(x, y) || m.discard.contains(x, y) {
                let done = self.finish_leave(to, m.keep.contains(x, y));
                if done && matches!(to, Leave::Quit) {
                    event_loop.exit();
                }
            }
        } else if m.keep.contains(x, y) {
            self.confirm_discard();
        }
        true
    }

    pub(crate) fn menu_key(
        &mut self,
        key: KeyCode,
        text: Option<&str>,
        event_loop: Option<&ActiveEventLoop>,
    ) -> bool {
        if !self.menu.open {
            return false;
        }
        if self.menu.searching {
            match key {
                KeyCode::Escape | KeyCode::Enter => self.menu.searching = false,
                KeyCode::Backspace => {
                    self.menu.search.pop();
                }
                _ => {
                    if let Some(text) = text {
                        for c in text.chars().filter(|c| !c.is_control()) {
                            if self.menu.search.chars().count() < 80 {
                                self.menu.search.push(c);
                            }
                        }
                    }
                }
            }
            self.menu.selected = 0;
            self.menu.offset = 0;
            return true;
        }
        if self.menu.page == menu::Page::Title {
            match key {
                KeyCode::ArrowUp => self.menu.title_selected = (self.menu.title_selected + 3) % 4,
                KeyCode::ArrowDown => self.menu.title_selected = (self.menu.title_selected + 1) % 4,
                KeyCode::Enter | KeyCode::ArrowRight => {
                    self.title_action(self.menu.title_selected, event_loop)
                }
                KeyCode::Escape if self.menu.can_resume => self.toggle_menu(),
                KeyCode::KeyC => {
                    self.controls_back = menu::Page::Title;
                    self.menu.page = menu::Page::Controls;
                }
                _ => {}
            }
            return true;
        }
        if self.menu.page == menu::Page::Controls {
            if matches!(key, KeyCode::PageUp | KeyCode::PageDown)
                && let Some(l) = self.menu_layout()
            {
                let h = self.renderer.as_ref().unwrap().size().1 as f32;
                let delta = if key == KeyCode::PageDown {
                    h / l.scale - 162.0
                } else {
                    -(h / l.scale - 162.0)
                };
                self.menu.controls_scroll = (self.menu.controls_scroll + delta)
                    .clamp(0.0, menu::controls_max_scroll(&l, h));
                return true;
            }
            if matches!(key, KeyCode::Escape | KeyCode::ArrowLeft | KeyCode::KeyC) {
                self.menu.page = self.controls_back;
            } else if key == KeyCode::KeyT {
                self.open_page(menu::Page::Title);
            }
            return true;
        }
        let page = self.menu_layout().map_or(5, |l| l.rows.len());
        match key {
            KeyCode::Escape | KeyCode::KeyM if self.menu.can_resume => self.toggle_menu(),
            KeyCode::Escape | KeyCode::ArrowLeft | KeyCode::KeyT => {
                self.open_page(menu::Page::Title)
            }
            KeyCode::KeyG => self.open_settings(),
            KeyCode::KeyC => {
                self.controls_back = self.menu.page;
                self.menu.page = menu::Page::Controls;
            }
            KeyCode::KeyP => self.open_page(if self.menu.page == menu::Page::Levels {
                menu::Page::Explore
            } else {
                menu::Page::Levels
            }),
            KeyCode::ArrowUp => self.menu.move_selection(false, page),
            KeyCode::ArrowDown => self.menu.move_selection(true, page),
            KeyCode::Enter | KeyCode::ArrowRight => {
                if let Some(path) = self.menu.selected_path()
                    && !self.menu.invalid.contains_key(&path)
                {
                    self.select_map(&path);
                }
            }
            KeyCode::PageUp => {
                self.menu.offset = self.menu.offset.saturating_sub(page);
                self.menu.selected = self.menu.offset;
            }
            KeyCode::PageDown if self.menu.offset + page < self.menu.visible().len() => {
                self.menu.offset += page;
                self.menu.selected = self.menu.offset;
            }
            KeyCode::KeyD if self.menu.page == menu::Page::Levels => self.discard_selected(),
            KeyCode::Slash if self.menu.page == menu::Page::Explore => self.menu.searching = true,
            KeyCode::KeyF if self.menu.page == menu::Page::Explore => self.favorite(),
            KeyCode::KeyB if self.menu.page == menu::Page::Explore => {
                self.menu.favorites_only = !self.menu.favorites_only;
                self.menu.selected = 0;
                self.menu.offset = 0;
            }
            _ => {
                let keys = [
                    KeyCode::Digit1,
                    KeyCode::Digit2,
                    KeyCode::Digit3,
                    KeyCode::Digit4,
                    KeyCode::Digit5,
                    KeyCode::Digit6,
                    KeyCode::Digit7,
                    KeyCode::Digit8,
                    KeyCode::Digit9,
                ];
                if let Some(row) = keys
                    .iter()
                    .position(|&k| k == key)
                    .filter(|&r| r < page && self.menu.offset + r < self.menu.visible().len())
                {
                    self.menu.selected = self.menu.offset + row;
                }
            }
        }
        true
    }

    pub(crate) fn build_dialogs(&self, v: &mut Vec<UiVertex>, vp: [f32; 2]) {
        if self.leave.is_some() || self.discard.is_some() {
            let m = Modal::new(vp, self.scale);
            let s = m.scale;
            let d = m.panel;
            ui::push_rect(v, 0.0, 0.0, vp[0], vp[1], [0.0, 0.001, 0.01, 0.75], vp);
            ui::push_rect(v, d.x, d.y, d.w, d.h, ui::BG, vp);
            ui::outline(v, d, s, ui::LINE, vp);
            let text = |v: &mut Vec<UiVertex>, y: f32, value: &str, color| {
                ui::push_text(v, d.x + 24.0 * s, d.y + y * s, 0.85 * s, value, color, vp)
            };
            text(
                v,
                22.0,
                if self.leave.is_some() {
                    "FIELD STUDY"
                } else {
                    "SAVED PROGRESS"
                },
                ui::ACCENT,
            );
            let discard_title = self
                .discard
                .as_ref()
                .and_then(|path| {
                    self.menu
                        .entries
                        .iter()
                        .enumerate()
                        .find(|(_, e)| e.path == *path)
                        .map(|(i, e)| format!("Discard {}?", e.puzzle_title(i)))
                })
                .unwrap_or_else(|| "Discard this level's progress?".into());
            ui::heading(
                v,
                d.x + 24.0 * s,
                d.y + 54.0 * s,
                30.0 * s,
                if self.leave.is_some() {
                    "Keep your progress before leaving?"
                } else {
                    &discard_title
                },
                ui::INK,
                vp,
            );
            text(
                v,
                110.0,
                if self.leave.is_some() {
                    "Keep your theory, score history and observation for next time."
                } else {
                    "This removes the saved theory, checks and best score."
                },
                ui::MUTED,
            );
            text(
                v,
                137.0,
                if self.leave.is_some() {
                    "Discard starts this field study fresh."
                } else {
                    "This cannot be undone. Other levels stay saved."
                },
                ui::MUTED,
            );
            let button = |v: &mut Vec<UiVertex>, r: Rect, label: &str, primary: bool| {
                ui::button(
                    v,
                    r,
                    label,
                    s,
                    primary,
                    r.contains(self.cursor.0, self.cursor.1),
                    vp,
                )
            };
            if let Some(path) = &self.discard
                && let Some((objects, checks, best)) = self.menu.summary(path)
            {
                text(
                    v,
                    160.0,
                    &format!(
                        "{objects} objects · {checks} checks · best {}",
                        best.map_or("—".into(), |b| format!("{b:.1}"))
                    ),
                    ui::INK,
                );
            }
            button(v, m.cancel, "Cancel [Esc]", false);
            if self.leave.is_some() {
                button(v, m.discard, "Discard & leave [D]", false);
                button(v, m.keep, "Keep & leave [Enter]", true);
            } else {
                let start = v.len();
                button(v, m.keep, "Discard [Enter]", true);
                for p in &mut v[start..] {
                    if p.color == ui::ACCENT {
                        p.color = ui::WARN;
                    }
                }
            }
        }
        if let Some((message, started)) = &self.toast
            && started.elapsed().as_secs_f32() < 4.0
        {
            let s = self.scale.min(vp[0] / 1300.0);
            let size = 0.85 * s;
            let width = (ui::text_width(message, size) + 32.0 * s).min(vp[0] - 32.0 * s);
            let r = Rect {
                x: (vp[0] - width) * 0.5,
                y: 24.0 * s,
                w: width,
                h: 38.0 * s,
            };
            ui::push_rect(v, r.x, r.y, r.w, r.h, ui::DEEP, vp);
            ui::outline(v, r, s, ui::LINE, vp);
            ui::push_text(
                v,
                r.x + 16.0 * s,
                r.y + 12.0 * s,
                size,
                message,
                ui::INK,
                vp,
            );
        }
    }

    pub(crate) fn build_observation_extras(&self, v: &mut Vec<UiVertex>, vp: [f32; 2]) {
        if self.menu.open || self.graphics.open || self.game.as_ref().is_some_and(|g| g.open) {
            return;
        }
        let s = self.scale.min(vp[0] / 1280.0);
        let text = |v: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, text: &str, color| {
            ui::push_text(v, x, y, size * s, text, color, vp)
        };
        if !self.show_hud {
            return;
        }
        if self.game.is_some() {
            let title = self
                .active_path
                .as_ref()
                .and_then(|p| {
                    self.menu
                        .entries
                        .iter()
                        .enumerate()
                        .find(|(_, e)| e.path == *p)
                        .map(|(i, e)| e.puzzle_title(i))
                })
                .unwrap_or_else(|| "Field study".into());
            let r = Rect {
                x: 16.0 * s,
                y: 24.0 * s,
                w: 320.0 * s,
                h: 130.0 * s,
            };
            ui::push_rect(
                v,
                r.x,
                r.y,
                r.w,
                r.h,
                [ui::BG[0], ui::BG[1], ui::BG[2], 0.84],
                vp,
            );
            ui::outline(v, r, s, ui::LINE, vp);
            text(
                v,
                r.x + 14.0 * s,
                r.y + 12.0 * s,
                0.7,
                "FIELD STUDY / OBSERVE THE UNKNOWN",
                ui::ACCENT,
            );
            ui::heading(
                v,
                r.x + 14.0 * s,
                r.y + 32.0 * s,
                22.0 * s,
                &title,
                ui::INK,
                vp,
            );
            for y in [69.0, 91.0, 113.0] {
                let p = [r.x + 17.0 * s, r.y + y * s];
                let points = [
                    [p[0], p[1] - 3.0 * s],
                    [p[0] + 3.0 * s, p[1]],
                    [p[0], p[1] + 3.0 * s],
                    [p[0] - 3.0 * s, p[1]],
                ];
                for i in 0..4 {
                    ui::line(v, points[i], points[(i + 1) % 4], s, ui::MUTED, vp);
                }
            }
            text(
                v,
                r.x + 28.0 * s,
                r.y + 65.0 * s,
                0.75,
                "Watch which lights move together",
                ui::MUTED,
            );
            text(
                v,
                r.x + 28.0 * s,
                r.y + 87.0 * s,
                0.75,
                "Draw their orbits in Theory [Tab]",
                ui::MUTED,
            );
            text(
                v,
                r.x + 28.0 * s,
                r.y + 109.0 * s,
                0.75,
                "Place yourself, then check [Enter]",
                ui::MUTED,
            );
        } else {
            text(
                v,
                16.0 * s,
                16.0 * s,
                0.8,
                "OBSERVATORY / EXPLORE",
                ui::ACCENT,
            );
            text(
                v,
                16.0 * s,
                36.0 * s,
                0.75,
                &format!(
                    "FOV {:.3}° / [G] targets / [Home] reset",
                    self.state.observer.fov_y.to_degrees()
                ),
                ui::MUTED,
            );
            if self.targets_open {
                let r = Self::targets_panel(vp, s, self.state.scene.targets.len());
                ui::push_rect(
                    v,
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    [ui::BG[0], ui::BG[1], ui::BG[2], 0.88],
                    vp,
                );
                ui::outline(v, r, s, ui::LINE, vp);
                text(
                    v,
                    r.x + 12.0 * s,
                    r.y + 12.0 * s,
                    0.8,
                    "TARGETS [G]",
                    ui::ACCENT,
                );
                for (i, &target) in self.state.scene.targets.iter().take(9).enumerate() {
                    let row = Self::target_row(r, s, i);
                    ui::button(
                        v,
                        row,
                        &format!("{} [F{}]", self.state.scene.body(target).name, i + 1),
                        s,
                        false,
                        row.contains(self.cursor.0, self.cursor.1),
                        vp,
                    );
                }
                let row = Self::target_row(r, s, self.state.scene.targets.len().min(9));
                ui::button(v, row, "Eclipse [E] / Transit [T]", s, false, false, vp);
            }
        }
        if let Some(r) = &self.renderer {
            let samples = r.samples();
            let limit = self.graphics.settings.sample_limit();
            let x = vp[0] - 190.0 * s;
            text(
                v,
                x,
                18.0 * s,
                0.8,
                &format!(
                    "{samples} spp · {}",
                    if r.needs_redraw() {
                        "converging"
                    } else {
                        "ready"
                    }
                ),
                ui::MUTED,
            );
            ui::push_rect(v, x, 37.0 * s, 174.0 * s, 3.0 * s, ui::LINE, vp);
            ui::push_rect(
                v,
                x,
                37.0 * s,
                174.0 * s * (samples as f32 / limit as f32).min(1.0),
                3.0 * s,
                ui::OK,
                vp,
            );
        }
    }

    fn targets_panel(vp: [f32; 2], s: f32, n: usize) -> Rect {
        Rect {
            x: vp[0] - 254.0 * s,
            y: 60.0 * s,
            w: 238.0 * s,
            h: (70.0 + n.min(9) as f32 * 34.0) * s,
        }
    }
    fn target_row(r: Rect, s: f32, i: usize) -> Rect {
        Rect {
            x: r.x + 8.0 * s,
            y: r.y + (36.0 + i as f32 * 34.0) * s,
            w: r.w - 16.0 * s,
            h: 28.0 * s,
        }
    }
    pub(crate) fn targets_click(&mut self) -> bool {
        if !self.targets_open || self.game.is_some() || !self.show_hud {
            return false;
        }
        let Some(r) = &self.renderer else {
            return false;
        };
        let (w, h) = r.size();
        let vp = [w as f32, h as f32];
        let s = self.scale.min(vp[0] / 1280.0);
        let panel = Self::targets_panel(vp, s, self.state.scene.targets.len());
        let (x, y) = self.cursor;
        if !panel.contains(x, y) {
            return false;
        }
        for i in 0..self.state.scene.targets.len().min(9) {
            if Self::target_row(panel, s, i).contains(x, y) {
                self.state.aim_target(i);
                return true;
            }
        }
        if Self::target_row(panel, s, self.state.scene.targets.len().min(9)).contains(x, y) {
            if x < (panel.x + panel.w * 0.5) as f64 {
                self.find_event(false);
            } else {
                self.find_event(true);
            }
        }
        true
    }
    pub(crate) fn find_event(&mut self, transit: bool) {
        let found = if transit {
            self.state.next_moon_transit()
        } else {
            self.state.next_eclipse()
        };
        self.toast = Some((
            found.map_or_else(
                || "No visible event found in the search interval.".into(),
                |what| format!("Found {what} / day {:.3}", self.state.sim_time),
            ),
            Instant::now(),
        ));
    }
}
