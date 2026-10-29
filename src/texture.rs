//! Texture editor (`--texture`): one planet, a movable light and live
//! sliders for every procedural surface parameter.
//!
//! The scene is a sunlike star, the planet, and a tiny airless "eye" moon
//! carrying the camera. The eye is tidally locked, so the planet stays fixed
//! in its sky; moving the eye around the planet changes the light angle.
//! The eye stands at 60 degrees latitude, which keeps the planet's spin axis
//! vertical on screen and the planet well above the eye's horizon.
use crate::sim::{Scene, Surface, SurfaceStyle};
use crate::ui::{self, Rect, UiVertex};
use std::time::Instant;
use winit::keyboard::KeyCode;

const PLANET: &str = "Planet";
const EYE: &str = "Eye";
/// Planet radius and eye distance (km): the planet spans about 29 degrees.
const RADIUS_KM: f64 = 6000.0;
const DISTANCE_KM: f64 = 4.0 * RADIUS_KM;
/// Automatic rotation speed (degrees per second).
const SPIN_SPEED: f64 = 30.0;

/// A tunable value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Param {
    Rotation,
    Light,
    Tilt,
    Seed,
    Albedo(usize),
    Secondary(usize),
    Bands,
    Turbulence,
    Storms,
    Craters,
    IceCaps,
    Ocean,
    Clouds,
}

impl Param {
    fn label(self) -> String {
        const RGB: [&str; 3] = ["R", "G", "B"];
        match self {
            Param::Rotation => "Rotation".into(),
            Param::Light => "Light".into(),
            Param::Tilt => "Axis tilt".into(),
            Param::Seed => "Seed".into(),
            Param::Albedo(c) => format!("Main {}", RGB[c]),
            Param::Secondary(c) => format!("Second {}", RGB[c]),
            Param::Bands => "Bands".into(),
            Param::Turbulence => "Turbulence".into(),
            Param::Storms => "Storms".into(),
            Param::Craters => "Craters".into(),
            Param::IceCaps => "Ice caps".into(),
            Param::Ocean => "Ocean".into(),
            Param::Clouds => "Clouds".into(),
        }
    }

    /// (minimum, maximum, integer steps).
    fn range(self) -> (f64, f64, bool) {
        match self {
            Param::Rotation => (0.0, 360.0, false),
            Param::Light => (-180.0, 180.0, false),
            Param::Tilt => (0.0, 90.0, false),
            Param::Seed => (0.0, 99.0, true),
            Param::Bands => (1.0, 30.0, false),
            Param::Storms => (0.0, 3.0, true),
            _ => (0.0, 1.0, false),
        }
    }
}

/// Something clickable in the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Control {
    Slider(Param),
    Play,
    StylePrev,
    StyleNext,
    Print,
    Reset,
}

/// One laid-out panel row.
enum Row {
    Header(&'static str, f32),
    Slider(Param, Rect),
    Button(Control, Rect),
    Style(f32),
}

pub struct Editor {
    /// None: plain mottled albedo; otherwise a procedural style.
    style: Option<SurfaceStyle>,
    albedo: [f32; 3],
    surface: Surface,
    rotation_deg: f64,
    /// Phase angle: 0 lights the face we see, +-180 backlights it.
    light_deg: f64,
    tilt_deg: f64,
    pub playing: bool,
    pub fov_deg: f64,
    drag: Option<Param>,
    last_tick: Option<Instant>,
    message: Option<String>,
}

/// A good-looking starting colour for each style.
fn style_albedo(style: Option<SurfaceStyle>) -> [f32; 3] {
    match style {
        None => [0.6, 0.58, 0.55],
        Some(SurfaceStyle::GasGiant) => [0.86, 0.76, 0.6],
        Some(SurfaceStyle::Cratered) => [0.62, 0.6, 0.57],
        Some(SurfaceStyle::Rocky) => [0.72, 0.45, 0.28],
        Some(SurfaceStyle::Icy) => [0.61, 0.66, 0.67],
        Some(SurfaceStyle::Volcanic) => [0.62, 0.54, 0.28],
        Some(SurfaceStyle::Terran) => [0.72, 0.62, 0.45],
    }
}

/// Parse a style name from the command line ("plain" for none).
pub fn parse_style(name: &str) -> Option<Option<SurfaceStyle>> {
    if name == "plain" {
        return Some(None);
    }
    SurfaceStyle::ALL
        .into_iter()
        .find(|s| s.name() == name)
        .map(Some)
}

impl Editor {
    pub fn new(style: Option<SurfaceStyle>) -> Self {
        let mut editor = Self {
            style,
            albedo: [0.0; 3],
            surface: crate::config::default_surface(SurfaceStyle::Cratered, [0.5; 3]),
            rotation_deg: 0.0,
            light_deg: 60.0,
            tilt_deg: 0.0,
            playing: false,
            fov_deg: 40.0,
            drag: None,
            last_tick: None,
            message: None,
        };
        editor.set_style(style);
        editor
    }

    /// Switch style, resetting its colours and parameters to the defaults.
    fn set_style(&mut self, style: Option<SurfaceStyle>) {
        self.style = style;
        self.albedo = style_albedo(style);
        let seed = self.surface.seed;
        if let Some(style) = style {
            self.surface = crate::config::default_surface(style, self.albedo);
            self.surface.seed = seed;
        }
    }

    fn cycle_style(&mut self, forward: bool) {
        let all: Vec<Option<SurfaceStyle>> = std::iter::once(None)
            .chain(SurfaceStyle::ALL.into_iter().map(Some))
            .collect();
        let i = all.iter().position(|s| *s == self.style).unwrap_or(0);
        let n = all.len();
        self.set_style(
            all[if forward {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            }],
        );
    }

    fn style_name(&self) -> &'static str {
        self.style.map_or("plain", SurfaceStyle::name)
    }

    /// The sliders shown for the current style, in panel order.
    fn surface_params(&self) -> Vec<Param> {
        let mut p = vec![Param::Seed];
        p.extend((0..3).map(Param::Albedo));
        match self.style {
            None => p.retain(|q| *q != Param::Seed),
            Some(style) => {
                p.extend((0..3).map(Param::Secondary));
                match style {
                    SurfaceStyle::GasGiant => {
                        p.extend([Param::Bands, Param::Turbulence, Param::Storms])
                    }
                    SurfaceStyle::Terran => {
                        p.extend([Param::Ocean, Param::Clouds, Param::IceCaps, Param::Craters])
                    }
                    _ => p.extend([Param::Craters, Param::IceCaps]),
                }
            }
        }
        p
    }

    fn get(&self, p: Param) -> f64 {
        let s = &self.surface;
        match p {
            Param::Rotation => self.rotation_deg,
            Param::Light => self.light_deg,
            Param::Tilt => self.tilt_deg,
            Param::Seed => f64::from(s.seed),
            Param::Albedo(c) => f64::from(self.albedo[c]),
            Param::Secondary(c) => f64::from(s.secondary_albedo[c]),
            Param::Bands => f64::from(s.bands),
            Param::Turbulence => f64::from(s.turbulence),
            Param::Storms => f64::from(s.storms),
            Param::Craters => f64::from(s.craters),
            Param::IceCaps => f64::from(s.ice_caps),
            Param::Ocean => f64::from(s.ocean),
            Param::Clouds => f64::from(s.clouds),
        }
    }

    fn set(&mut self, p: Param, value: f64) {
        let (lo, hi, integer) = p.range();
        let v = if integer { value.round() } else { value }.clamp(lo, hi);
        let s = &mut self.surface;
        match p {
            Param::Rotation => self.rotation_deg = v,
            Param::Light => self.light_deg = v,
            Param::Tilt => self.tilt_deg = v,
            Param::Seed => s.seed = v as u32,
            Param::Albedo(c) => self.albedo[c] = v as f32,
            Param::Secondary(c) => s.secondary_albedo[c] = v as f32,
            Param::Bands => s.bands = v as f32,
            Param::Turbulence => s.turbulence = v as f32,
            Param::Storms => s.storms = v as u32,
            Param::Craters => s.craters = v as f32,
            Param::IceCaps => s.ice_caps = v as f32,
            Param::Ocean => s.ocean = v as f32,
            Param::Clouds => s.clouds = v as f32,
        }
    }

    fn value_text(&self, p: Param) -> String {
        let v = self.get(p);
        match p {
            Param::Rotation | Param::Light | Param::Tilt => format!("{v:.0}°"),
            Param::Seed | Param::Storms => format!("{v:.0}"),
            Param::Bands => format!("{v:.1}"),
            _ => format!("{v:.2}"),
        }
    }

    /// The scene: star, planet and eye, before the editor's values apply.
    pub fn scene(&self) -> Scene {
        let yaml = format!(
            "version: 1
name: Texture editor
time_days: 0
camera:
  body: eye
  latitude_deg: 60
  longitude_deg: 180
  height_m: 2
  look_at: planet
  fov_deg: {fov}
bodies:
  - id: sun
    name: Sun
    kind: star
    radius_km: 696340
    luminosity: 1
    emission: [1.0, 0.97, 0.92]
  - id: planet
    name: {PLANET}
    kind: planet
    parent: sun
    radius_km: {RADIUS_KM}
    orbit: {{semi_major_axis_au: 1, period_days: 1.0e9}}
    rotation: {{mode: free, period_days: 1.0e9}}
  - id: eye
    name: {EYE}
    kind: moon
    parent: planet
    radius_km: 1
    albedo: [0, 0, 0]
    orbit: {{semi_major_axis_km: {DISTANCE_KM}, period_days: 1.0e9}}
    rotation: {{mode: tidal_lock}}
",
            fov = self.fov_deg
        );
        crate::config::parse(&yaml).expect("texture editor scene is valid")
    }

    /// Copy the editor's values into the scene (cheap; done every frame).
    pub fn apply(&self, scene: &mut Scene) {
        let find = |name: &str| scene.bodies.iter().position(|b| b.name == name).unwrap();
        let (planet, eye) = (find(PLANET), find(EYE));
        let p = &mut scene.bodies[planet];
        p.albedo = self.albedo;
        p.surface = self.style.map(|style| Surface {
            style,
            ..self.surface
        });
        p.spin_phase = self.rotation_deg.to_radians();
        p.obliquity = self.tilt_deg.to_radians();
        // The star lies along -X from the planet; the eye sits at angle
        // 180 - phase around it, so phase 0 lights the face we see.
        scene.bodies[eye].elements.m0 = (180.0 - self.light_deg).to_radians();
    }

    pub fn planet_index(scene: &Scene) -> usize {
        scene.bodies.iter().position(|b| b.name == PLANET).unwrap()
    }

    /// Advance the automatic rotation. Returns true while playing.
    pub fn tick(&mut self, now: Instant) -> bool {
        let dt = self
            .last_tick
            .map_or(0.0, |t| now.duration_since(t).as_secs_f64().min(0.25));
        self.last_tick = Some(now);
        if self.playing {
            self.rotation_deg = (self.rotation_deg + SPIN_SPEED * dt).rem_euclid(360.0);
        }
        self.playing
    }

    /// The config lines for the current planet, ready to paste into a body.
    pub fn yaml(&self) -> String {
        let rgb = |c: [f32; 3]| format!("[{:.2}, {:.2}, {:.2}]", c[0], c[1], c[2]);
        let mut out = format!("    albedo: {}\n", rgb(self.albedo));
        if let Some(style) = self.style {
            let s = &self.surface;
            let mut fields = vec![
                format!("style: {}", style.name()),
                format!("seed: {}", s.seed),
                format!("secondary_albedo: {}", rgb(s.secondary_albedo)),
            ];
            match style {
                SurfaceStyle::GasGiant => fields.extend([
                    format!("bands: {:.1}", s.bands),
                    format!("turbulence: {:.2}", s.turbulence),
                    format!("storms: {}", s.storms),
                ]),
                SurfaceStyle::Terran => fields.extend([
                    format!("ocean: {:.2}", s.ocean),
                    format!("clouds: {:.2}", s.clouds),
                    format!("ice_caps: {:.2}", s.ice_caps),
                    format!("craters: {:.2}", s.craters),
                ]),
                _ => fields.extend([
                    format!("craters: {:.2}", s.craters),
                    format!("ice_caps: {:.2}", s.ice_caps),
                ]),
            }
            out += &format!("    surface: {{{}}}\n", fields.join(", "));
        }
        out
    }

    fn print(&mut self) {
        let yaml = self.yaml();
        println!("\n# Texture editor: paste into a planet or moon\n{yaml}");
        self.message = Some("YAML printed to the terminal".into());
    }

    // --- Layout and drawing -------------------------------------------

    pub fn panel(vp: [f32; 2], s: f32) -> Rect {
        let w = 360.0 * s;
        Rect {
            x: vp[0] - w - 12.0 * s,
            y: 12.0 * s,
            w,
            h: vp[1] - 24.0 * s,
        }
    }

    fn rows(&self, vp: [f32; 2], s: f32) -> Vec<Row> {
        let p = Self::panel(vp, s);
        let mut y = p.y + 44.0 * s;
        let mut rows = Vec::new();
        let row_h = 26.0 * s;
        let track = |y: f32| Rect {
            x: p.x + 112.0 * s,
            y: y + 9.0 * s,
            w: p.w - 190.0 * s,
            h: 6.0 * s,
        };
        rows.push(Row::Header("VIEW", y));
        y += 22.0 * s;
        for param in [Param::Rotation, Param::Light, Param::Tilt] {
            rows.push(Row::Slider(param, track(y)));
            y += row_h;
        }
        rows.push(Row::Button(
            Control::Play,
            Rect {
                x: p.x + 12.0 * s,
                y,
                w: p.w - 24.0 * s,
                h: 26.0 * s,
            },
        ));
        y += 42.0 * s;
        rows.push(Row::Header("SURFACE", y));
        y += 22.0 * s;
        rows.push(Row::Style(y));
        y += 36.0 * s;
        for param in self.surface_params() {
            rows.push(Row::Slider(param, track(y)));
            y += row_h;
        }
        y += 12.0 * s;
        let half = (p.w - 36.0 * s) / 2.0;
        rows.push(Row::Button(
            Control::Print,
            Rect {
                x: p.x + 12.0 * s,
                y,
                w: half,
                h: 26.0 * s,
            },
        ));
        rows.push(Row::Button(
            Control::Reset,
            Rect {
                x: p.x + 24.0 * s + half,
                y,
                w: half,
                h: 26.0 * s,
            },
        ));
        rows
    }

    fn style_buttons(p: Rect, y: f32, s: f32) -> (Rect, Rect) {
        let b = |x| Rect {
            x,
            y,
            w: 30.0 * s,
            h: 26.0 * s,
        };
        (b(p.x + 112.0 * s), b(p.right() - 42.0 * s))
    }

    fn controls(&self, vp: [f32; 2], s: f32) -> Vec<(Control, Rect)> {
        let p = Self::panel(vp, s);
        let mut out = Vec::new();
        for row in self.rows(vp, s) {
            match row {
                Row::Slider(param, t) => out.push((
                    Control::Slider(param),
                    Rect {
                        x: t.x - 8.0 * s,
                        y: t.y - 10.0 * s,
                        w: t.w + 16.0 * s,
                        h: t.h + 20.0 * s,
                    },
                )),
                Row::Button(c, r) => out.push((c, r)),
                Row::Style(y) => {
                    let (prev, next) = Self::style_buttons(p, y, s);
                    out.push((Control::StylePrev, prev));
                    out.push((Control::StyleNext, next));
                }
                Row::Header(..) => {}
            }
        }
        out
    }

    fn track_of(&self, param: Param, vp: [f32; 2], s: f32) -> Option<Rect> {
        self.rows(vp, s).into_iter().find_map(|r| match r {
            Row::Slider(p, t) if p == param => Some(t),
            _ => None,
        })
    }

    pub fn build(&self, v: &mut Vec<UiVertex>, vp: [f32; 2], s: f32, cursor: (f64, f64)) {
        let p = Self::panel(vp, s);
        ui::push_rect(
            v,
            p.x,
            p.y,
            p.w,
            p.h,
            [ui::BG[0], ui::BG[1], ui::BG[2], 0.88],
            vp,
        );
        ui::outline(v, p, s, ui::LINE, vp);
        let text = |v: &mut Vec<UiVertex>, x: f32, y: f32, size: f32, t: &str, c| {
            ui::push_text(v, x, y, size * s, t, c, vp)
        };
        text(
            v,
            p.x + 12.0 * s,
            p.y + 14.0 * s,
            0.9,
            "TEXTURE EDITOR",
            ui::ACCENT,
        );
        let hover = |r: &Rect| r.contains(cursor.0, cursor.1);
        for row in self.rows(vp, s) {
            match row {
                Row::Header(title, y) => text(v, p.x + 12.0 * s, y, 0.75, title, ui::MUTED),
                Row::Slider(param, t) => {
                    let (lo, hi, _) = param.range();
                    let f = ((self.get(param) - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
                    text(
                        v,
                        p.x + 12.0 * s,
                        t.y - 5.0 * s,
                        0.75,
                        &param.label(),
                        ui::INK,
                    );
                    let fill = match param {
                        Param::Albedo(c) | Param::Secondary(c) => {
                            let mut col = [0.25, 0.25, 0.25, 1.0];
                            col[c] = 0.95;
                            col
                        }
                        _ => ui::OK,
                    };
                    ui::push_rect(v, t.x, t.y, t.w, t.h, ui::LINE, vp);
                    ui::push_rect(v, t.x, t.y, t.w * f, t.h, fill, vp);
                    let knob = 7.0 * s;
                    ui::push_rect(
                        v,
                        t.x + t.w * f - knob,
                        t.y + t.h * 0.5 - knob,
                        2.0 * knob,
                        2.0 * knob,
                        if self.drag == Some(param) {
                            ui::ACCENT
                        } else {
                            ui::INK
                        },
                        vp,
                    );
                    text(
                        v,
                        t.right() + 14.0 * s,
                        t.y - 5.0 * s,
                        0.75,
                        &self.value_text(param),
                        ui::MUTED,
                    );
                    // Colour swatch after the blue slider of each colour.
                    let swatch = match param {
                        Param::Albedo(2) => Some(self.albedo),
                        Param::Secondary(2) => Some(self.surface.secondary_albedo),
                        _ => None,
                    };
                    if let Some(c) = swatch {
                        let r = Rect {
                            x: p.x + 12.0 * s,
                            y: t.y + 10.0 * s,
                            w: 84.0 * s,
                            h: 6.0 * s,
                        };
                        ui::push_rect(v, r.x, r.y, r.w, r.h, [c[0], c[1], c[2], 1.0], vp);
                    }
                }
                Row::Button(control, r) => {
                    let label = match control {
                        Control::Play if self.playing => "Pause rotation [Space]",
                        Control::Play => "Play rotation [Space]",
                        Control::Print => "Print YAML [P]",
                        Control::Reset => "Reset [R]",
                        _ => "",
                    };
                    ui::button(
                        v,
                        r,
                        label,
                        s,
                        control == Control::Play && self.playing,
                        hover(&r),
                        vp,
                    );
                }
                Row::Style(y) => {
                    text(v, p.x + 12.0 * s, y + 7.0 * s, 0.75, "Style [ ]", ui::INK);
                    let (prev, next) = Self::style_buttons(p, y, s);
                    ui::button(v, prev, "<", s, false, hover(&prev), vp);
                    ui::button(v, next, ">", s, false, hover(&next), vp);
                    let name = self.style_name();
                    let mid = (prev.right() + next.x) * 0.5;
                    text(
                        v,
                        mid - ui::text_width(name, 0.85 * s) * 0.5,
                        y + 6.0 * s,
                        0.85,
                        name,
                        ui::ACCENT,
                    );
                }
            }
        }
        let help = self
            .message
            .as_deref()
            .unwrap_or("Scroll: zoom  Arrows: rotate / light  Esc: quit");
        text(
            v,
            p.x + 12.0 * s,
            p.y + p.h - 24.0 * s,
            0.7,
            help,
            ui::MUTED,
        );
    }

    // --- Input ----------------------------------------------------------

    /// Mouse press; returns true if the panel used it.
    pub fn press(&mut self, x: f64, y: f64, vp: [f32; 2], s: f32) -> bool {
        if !Self::panel(vp, s).contains(x, y) {
            return false;
        }
        self.message = None;
        if let Some((control, _)) = self
            .controls(vp, s)
            .into_iter()
            .find(|(_, r)| r.contains(x, y))
        {
            match control {
                Control::Slider(param) => {
                    self.drag = Some(param);
                    self.drag_to(x, vp, s);
                }
                Control::Play => self.playing = !self.playing,
                Control::StylePrev => self.cycle_style(false),
                Control::StyleNext => self.cycle_style(true),
                Control::Print => self.print(),
                Control::Reset => self.reset(),
            }
        }
        true
    }

    pub fn drag_to(&mut self, x: f64, vp: [f32; 2], s: f32) {
        if let Some(param) = self.drag
            && let Some(t) = self.track_of(param, vp, s)
        {
            let (lo, hi, _) = param.range();
            let f = ((x as f32 - t.x) / t.w).clamp(0.0, 1.0) as f64;
            self.set(param, lo + f * (hi - lo));
        }
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    fn reset(&mut self) {
        let style = self.style;
        *self = Self {
            fov_deg: self.fov_deg,
            ..Self::new(style)
        };
    }

    pub fn zoom(&mut self, steps: f64) {
        self.fov_deg = (self.fov_deg * 0.85_f64.powf(steps)).clamp(2.0, 90.0);
    }

    /// Keyboard shortcuts; returns false for keys the editor does not use.
    pub fn key(&mut self, key: KeyCode) -> bool {
        self.message = None;
        match key {
            KeyCode::Space => self.playing = !self.playing,
            KeyCode::ArrowLeft => self.rotation_deg = (self.rotation_deg - 10.0).rem_euclid(360.0),
            KeyCode::ArrowRight => self.rotation_deg = (self.rotation_deg + 10.0).rem_euclid(360.0),
            KeyCode::ArrowUp => self.set(Param::Light, self.light_deg + 10.0),
            KeyCode::ArrowDown => self.set(Param::Light, self.light_deg - 10.0),
            KeyCode::BracketLeft => self.cycle_style(false),
            KeyCode::BracketRight => self.cycle_style(true),
            KeyCode::KeyP => self.print(),
            KeyCode::KeyR => self.reset(),
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_builds_a_valid_scene_and_yaml() {
        for style in std::iter::once(None).chain(SurfaceStyle::ALL.into_iter().map(Some)) {
            let editor = Editor::new(style);
            let mut scene = editor.scene();
            editor.apply(&mut scene);
            let planet = &scene.bodies[Editor::planet_index(&scene)];
            assert_eq!(planet.surface.map(|s| s.style), style);
            // The printed lines are a valid body: splice them into a config.
            let config = format!(
                "version: 1\nname: t\ncamera: {{body: p, latitude_deg: 0, longitude_deg: 0, look_at: s, fov_deg: 40}}\nbodies:\n  - id: s\n    kind: star\n    radius_km: 1000\n    luminosity: 1\n  - id: p\n    kind: planet\n    parent: s\n    radius_km: 1000\n    orbit: {{semi_major_axis_au: 1, period_days: 365}}\n{}",
                editor.yaml()
            );
            let parsed = crate::config::parse(&config).unwrap();
            let p = &parsed.bodies[1];
            assert_eq!(p.surface.map(|s| s.style), style);
        }
    }

    #[test]
    fn light_angle_zero_lights_the_visible_face() {
        let mut editor = Editor::new(Some(SurfaceStyle::Rocky));
        let mut scene = editor.scene();
        for (light, lit) in [(0.0, true), (180.0, false)] {
            editor.light_deg = light;
            editor.apply(&mut scene);
            let p = scene.positions(0.0);
            let planet = Editor::planet_index(&scene);
            let eye = scene.bodies.iter().position(|b| b.name == EYE).unwrap();
            let to_eye = (p[eye] - p[planet]).normalize();
            let to_sun = (p[0] - p[planet]).normalize();
            assert_eq!(to_eye.dot(to_sun) > 0.9, lit, "light {light}");
        }
    }

    #[test]
    fn sliders_clamp_and_round() {
        let mut editor = Editor::new(Some(SurfaceStyle::GasGiant));
        editor.set(Param::Storms, 7.6);
        assert_eq!(editor.surface.storms, 3);
        editor.set(Param::Seed, 12.4);
        assert_eq!(editor.surface.seed, 12);
        editor.set(Param::Turbulence, -1.0);
        assert_eq!(editor.surface.turbulence, 0.0);
        assert!(editor.surface_params().contains(&Param::Bands));
        editor.cycle_style(true);
        assert_eq!(editor.style, Some(SurfaceStyle::Cratered));
        assert!(!editor.surface_params().contains(&Param::Bands));
    }
}
