//! stargaze — an observatory on a planet, looking out at a solar system.

mod camera;
mod config;
mod game;
mod menu;
mod pathtracer;
mod renderer;
mod sim;
mod stars;
mod ui;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use glam::DVec3;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use camera::{Observer, ViewFrame};
use renderer::{Frame, Globals, Renderer, Sphere};
use sim::{BodyKind, Scene};
use stars::CatalogueStar;
use ui::Label;

/// Signed simulation speeds in days per real second, including a true stop.
const WARPS: [f64; 17] = [
    -40.0, -10.0, -2.0, -0.5, -0.1, -0.02, -0.005, -0.001, 0.0, 0.001, 0.005, 0.02, 0.1, 0.5, 2.0,
    10.0, 40.0,
];
const STOP_WARP: usize = 8;
/// Telescope magnification limits and the per-notch zoom factor.
const MIN_FOV_DEG: f64 = 0.001;
const MAX_FOV_DEG: f64 = 90.0;
const ZOOM_STEP: f64 = 0.78;
/// Exposure-scale normalization: a white Lambertian surface facing a unit
/// luminosity star at 1 AU has outgoing radiance 2.5. The same stellar radiance
/// is used for camera hits AND illumination (no independent bright-disc gain).
const SUN_LIGHT: f64 = 2.5;

/// Explicit CLI files take precedence over environment and built-in presets.
/// Validation exits before creating a window or requesting a GPU.
fn scene_for_startup() -> Result<Option<Scene>> {
    let mut args = std::env::args_os().skip(1);
    let mut path = None;
    let mut check_only = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                println!(
                    "Stargaze — an observatory in a configurable star system\n\nUsage: stargaze [--game] [--config FILE | --check-config FILE]\n\n--game               Play the anonymous system reconstruction puzzle\n--config FILE        Load a YAML system and its camera\n--check-config FILE  Validate without opening a window\n\nDefaults to configs/halo.yaml, or configs/puzzle.yaml with --game. STARGAZE_CONFIG also selects a file."
                );
                return Ok(None);
            }
            Some("--game") => {}
            Some("--config" | "--check-config") => {
                anyhow::ensure!(path.is_none(), "specify only one configuration file");
                check_only = arg == "--check-config";
                path =
                    Some(std::path::PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("{arg:?} requires a file path")
                    })?));
            }
            _ => anyhow::bail!("unknown argument {arg:?}; use --help"),
        }
    }
    let scene =
        if let Some(path) = path.or_else(|| std::env::var_os("STARGAZE_CONFIG").map(Into::into)) {
            config::load(path)?
        } else {
            let preset = std::env::var("STARGAZE_SYSTEM").ok();
            match preset.as_deref() {
                None if game_requested() => {
                    config::load(concat!(env!("CARGO_MANIFEST_DIR"), "/configs/puzzle.yaml"))?
                }
                None | Some("halo" | "binary") => {
                    config::load(concat!(env!("CARGO_MANIFEST_DIR"), "/configs/halo.yaml"))?
                }
                Some("earth") => config::load(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/configs/solar-system.yaml"
                ))?,
                Some("calyx" | "solar" | "sol" | "saturn") => scene_for_system(preset.as_deref()),
                Some(other) => anyhow::bail!("unknown STARGAZE_SYSTEM '{other}'"),
            }
        };
    if check_only {
        println!(
            "Valid system: {} bodies; camera on {}, looking at {}",
            scene.bodies.len(),
            scene.body(scene.host).name,
            scene.body(scene.default_target).name
        );
        return Ok(None);
    }
    Ok(Some(scene))
}

fn game_requested() -> bool {
    std::env::args_os().any(|arg| arg == "--game")
}

fn scene_for_system(system: Option<&str>) -> Scene {
    match system {
        Some("calyx") => sim::binary_scene(),
        Some("earth") => sim::default_scene(),
        Some("solar" | "sol" | "saturn") => sim::saturn_scene(),
        _ => sim::halo_scene(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ViewLock {
    Body(usize),
    /// Fixed inertial direction, independent of the observer's orbit and spin.
    Background(DVec3),
}

struct State {
    scene: Scene,
    observer: Observer,
    sim_time: f64,
    warp: usize,
    last: Instant,
    /// True only after a sky press crosses the drag threshold.
    dragging: bool,
    last_cursor: Option<(f64, f64)>,
    /// Either a moving scene body or a fixed direction against distant stars.
    view_lock: Option<ViewLock>,
    /// Cursor position where the current left-button press began (sky only).
    press: Option<(f64, f64)>,
}

impl State {
    fn with_scene(scene: Scene) -> Self {
        let mut observer = Observer::new(scene.host);
        observer.lat = scene.observer_lat_deg.to_radians();
        observer.lon = scene.observer_lon_deg.to_radians();
        observer.height_m = scene.observer_height_m;
        let mut state = Self {
            scene,
            observer,
            sim_time: 0.0,
            warp: STOP_WARP,
            last: Instant::now(),
            dragging: false,
            last_cursor: None,
            view_lock: None,
            press: None,
        };
        if let Some(t) = state.scene.initial_time_days {
            state.sim_time = t;
        } else {
            state.find_good_start();
        }
        if let Ok(t) = std::env::var("STARGAZE_TIME")
            && let Ok(v) = t.parse::<f64>()
        {
            state.sim_time = v;
        }
        let aim_env = std::env::var("STARGAZE_AIM")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        match aim_env {
            Some(target) => state.aim_at(target),
            None => {
                let target = state.scene.default_target;
                let fov = state.scene.default_fov_deg.to_radians();
                state.point_at_fov(target, fov);
            }
        }
        if let Ok(f) = std::env::var("STARGAZE_FOV")
            && let Ok(v) = f.parse::<f64>()
        {
            state.observer.fov_y = v
                .to_radians()
                .clamp(MIN_FOV_DEG.to_radians(), MAX_FOV_DEG.to_radians());
        }
        if std::env::var("STARGAZE_FIND_ECLIPSE").is_ok()
            && let Some(what) = state.next_eclipse()
        {
            log::info!("found {what} at t = {:.3} d", state.sim_time);
        }
        if (std::env::var("STARGAZE_FIND_TRANSIT").is_ok()
            || std::env::var("STARGAZE_FIND_PHOBOS").is_ok())
            && let Some(what) = state.next_moon_transit()
        {
            log::info!("found {what} at t = {:.3} d", state.sim_time);
        }
        state
    }

    /// Advance to a time when the default target is well placed (and, for
    /// non-stars, the sky is dark).
    fn find_good_start(&mut self) {
        let target = self.scene.default_target;
        let is_star = self.scene.body(target).kind == BodyKind::Star;
        let stars = self.scene.star_indices();
        let step = 0.01; // days
        let mut t = 0.0;
        for _ in 0..40_000 {
            let positions = self.scene.positions(t);
            let vf = self.observer.frame(&self.scene, t, &positions);
            let dir = (positions[target] - vf.position).normalize();
            let alt = dir.dot(vf.zenith).asin();
            if alt > 0.2 {
                let night = stars
                    .iter()
                    .all(|&si| (positions[si] - vf.position).normalize().dot(vf.zenith) < -0.1);
                if is_star || night {
                    self.sim_time = t;
                    return;
                }
            }
            t += step;
        }
    }

    /// Slew to a body and frame it, choosing a good moment to look: the body
    /// above the horizon, a dark sky, and (for planets and moons) a well-lit
    /// phase. Zooms to fit the body together with its moons.
    fn aim_at(&mut self, target: usize) {
        self.unlock();
        let is_star = self.scene.body(target).kind == BodyKind::Star;

        // Park exactly on the requested moment instead of searching.
        if std::env::var("STARGAZE_NO_ADVANCE").is_ok() {
            self.point_at(target);
            return;
        }

        let step = 10.0 / 1440.0; // days
        let max_iter = if is_star { 144 } else { 57_600 }; // ~1 day / ~400 days

        let mut fallback: Option<(f64, DVec3, ViewFrame)> = None;
        let mut chosen: Option<(f64, DVec3, ViewFrame)> = None;

        for _ in 0..max_iter {
            let positions = self.scene.positions(self.sim_time);
            let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
            let dir = (positions[target] - vf.position).normalize();
            let alt = dir.dot(vf.zenith).asin();
            if alt > 0.15 {
                if fallback.is_none() {
                    fallback = Some((self.sim_time, dir, vf));
                }
                let good = if is_star {
                    true
                } else {
                    // Cosine of the phase angle: body->Sun vs body->observer.
                    let to_sun = (positions[0] - positions[target]).normalize();
                    let illum = (-dir).dot(to_sun);
                    vf.sun_altitude < -0.1 && illum > 0.3
                };
                if good {
                    chosen = Some((self.sim_time, dir, vf));
                    break;
                }
            }
            self.sim_time += step;
        }

        let (time, dir, vf) = chosen.or(fallback).unwrap_or_else(|| {
            let positions = self.scene.positions(self.sim_time);
            let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
            let dir = (positions[target] - vf.position).normalize();
            (self.sim_time, dir, vf)
        });

        self.sim_time = time;
        self.observer.alt = dir.dot(vf.zenith).asin();
        self.observer.az = dir.dot(vf.east).atan2(dir.dot(vf.north));
        self.observer.fov_y = self.fit_fov(target, &self.scene.positions(time), &vf);
    }

    /// Aim at a body using the current time, without searching.
    fn point_at(&mut self, target: usize) {
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let fov = self.fit_fov(target, &positions, &vf);
        self.point_at_fov(target, fov);
    }

    /// Aim at a body with an explicit field of view.
    fn point_at_fov(&mut self, target: usize, fov: f64) {
        self.unlock();
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let dir = (positions[target] - vf.position).normalize();
        self.observer.alt = dir.dot(vf.zenith).asin();
        self.observer.az = dir.dot(vf.east).atan2(dir.dot(vf.north));
        self.observer.fov_y = fov.clamp(MIN_FOV_DEG.to_radians(), MAX_FOV_DEG.to_radians());
    }

    /// Fast-forward to the next time a moon transits the planet it orbits, and
    /// frame the planet closely (e.g. Phobos on Mars or Io on Jupiter).
    /// Moons of the observer's host cannot transit their parent from here.
    fn next_moon_transit(&mut self) -> Option<&'static str> {
        let host = self.scene.host;
        let step = 0.001; // days
        let start = self.sim_time;
        for i in 1..40_000 {
            let t = start + i as f64 * step;
            let p = self.scene.positions(t);
            let vf = self.observer.frame(&self.scene, t, &p);
            for (mi, moon) in self.scene.bodies.iter().enumerate() {
                if moon.kind != BodyKind::Moon || mi == host {
                    continue;
                }
                let Some(parent) = moon.parent else { continue };
                let planet = self.scene.body(parent);
                if parent == host || planet.kind != BodyKind::Planet {
                    continue;
                }
                let d_planet = p[parent] - vf.position;
                let d_moon = p[mi] - vf.position;
                let planet_dist = d_planet.length();
                if d_planet.normalize().dot(vf.zenith) < 0.2 || vf.sun_altitude > -0.1 {
                    continue;
                }
                if d_moon.length() >= planet_dist {
                    continue;
                }
                let planet_ang = (planet.radius / planet_dist).asin();
                let sep = d_planet.normalize().angle_between(d_moon.normalize());
                if sep < planet_ang * 0.7 {
                    self.sim_time = t;
                    self.point_at_fov(parent, 2.2 * planet_ang);
                    return Some("moon transiting its planet");
                }
            }
        }
        None
    }

    /// Fast-forward to the next time one of the stars is occulted (from the
    /// observatory) by another body: the Moon blotting out the Sun in the solar
    /// system, or a planet eclipsing a star in the binary.
    fn next_eclipse(&mut self) -> Option<&'static str> {
        let stars = self.scene.star_indices();
        let host = self.scene.host;
        let step = 0.01; // days
        let start = self.sim_time;
        // A true-size Earth does not see a local solar eclipse every year.
        // Search up to ten model years instead of the old inflated-scene limit.
        for i in 1..365_250 {
            let t = start + i as f64 * step;
            let p = self.scene.positions(t);
            let vf = self.observer.frame(&self.scene, t, &p);
            for &si in &stars {
                let d_star = p[si] - vf.position;
                let star_dist = d_star.length();
                let star_dir = d_star / star_dist;
                if star_dir.dot(vf.zenith) < 0.05 {
                    continue; // star below the horizon
                }
                let star_ang = (self.scene.body(si).radius / star_dist).asin();
                for (bi, body) in self.scene.bodies.iter().enumerate() {
                    if bi == host
                        || bi == si
                        || matches!(body.kind, BodyKind::Star | BodyKind::Anchor)
                    {
                        continue;
                    }
                    let d_body = p[bi] - vf.position;
                    let body_dist = d_body.length();
                    if body_dist >= star_dist {
                        continue; // must be in front of the star
                    }
                    let body_ang = (body.radius / body_dist).asin();
                    if body_ang < star_ang * 0.3 {
                        continue; // too small to be a real eclipse
                    }
                    let sep = star_dir.angle_between(d_body / body_dist);
                    if sep < star_ang + body_ang {
                        self.sim_time = t;
                        self.point_at(si);
                        return Some("eclipse of a star");
                    }
                }
            }
        }
        None
    }

    /// Aim at the k-th entry of the scene's target list (function keys).
    fn aim_target(&mut self, k: usize) {
        if let Some(&target) = self.scene.targets.get(k) {
            self.aim_at(target);
        }
    }

    /// Choose a field of view that comfortably frames `target` together with
    /// any moons orbiting it.
    fn fit_fov(&self, target: usize, positions: &[DVec3], vf: &ViewFrame) -> f64 {
        let body = self.scene.body(target);
        let dist = (positions[target] - vf.position).length().max(1.0e-9);
        let mut max_ang = body.radius * body.rings.map_or(1.0, |r| r.outer_radius) / dist;
        if body.kind != BodyKind::Star {
            for (i, child) in self.scene.bodies.iter().enumerate() {
                if child.parent == Some(target) {
                    let r = (positions[i] - positions[target]).length();
                    max_ang = max_ang.max(r / dist);
                }
            }
        }
        (4.0 * max_ang).clamp(MIN_FOV_DEG.to_radians(), MAX_FOV_DEG.to_radians())
    }

    fn advance(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f64();
        self.last = now;
        self.advance_by(dt);
    }

    fn advance_by(&mut self, seconds: f64) {
        self.sim_time += seconds * WARPS[self.warp];
    }

    fn change_speed(&mut self, faster: bool) {
        self.warp = if faster {
            (self.warp + 1).min(WARPS.len() - 1)
        } else {
            self.warp.saturating_sub(1)
        };
    }

    fn time_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::ArrowLeft => self.change_speed(false),
            KeyCode::ArrowRight => self.change_speed(true),
            KeyCode::Space => self.warp = STOP_WARP,
            _ => return false,
        }
        true
    }

    fn reset_view(&mut self) {
        self.aim_at(self.scene.default_target);
    }

    fn unlock(&mut self) {
        self.view_lock = None;
    }

    fn toggle_star_orientation(&mut self) {
        if self.observer.star_orientation.is_some() {
            self.observer.star_orientation = None;
        } else {
            let positions = self.scene.positions(self.sim_time);
            let frame = self.observer.frame(&self.scene, self.sim_time, &positions);
            self.observer.star_orientation = Some((frame.forward, frame.up));
        }
    }

    fn locked_body(&self) -> Option<usize> {
        match self.view_lock {
            Some(ViewLock::Body(i)) => Some(i),
            _ => None,
        }
    }

    fn lock_label(&self, show_names: bool) -> Option<String> {
        self.view_lock.map(|target| match target {
            ViewLock::Background(_) => "Background sky".into(),
            ViewLock::Body(i) if show_names => self.scene.body(i).name.clone(),
            ViewLock::Body(i) if self.scene.body(i).kind == BodyKind::Star => "Star".into(),
            ViewLock::Body(_) => "Planet / moon".into(),
        })
    }

    /// Update the local telescope angles to maintain the selected world direction.
    fn track(&mut self) {
        let Some(target) = self.view_lock else {
            return;
        };
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let dir = match target {
            ViewLock::Body(i) => (positions[i] - vf.position).normalize(),
            ViewLock::Background(dir) => dir,
        };
        self.observer.alt = dir.dot(vf.zenith).clamp(-1.0, 1.0).asin();
        self.observer.az = dir.dot(vf.east).atan2(dir.dot(vf.north));
    }

    fn cursor_ray(&self, x: f64, y: f64, width: u32, height: u32) -> Option<DVec3> {
        if width == 0
            || height == 0
            || !(0.0..width as f64).contains(&x)
            || !(0.0..height as f64).contains(&y)
        {
            return None;
        }
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let aspect = width as f64 / height as f64;
        let tan_half = (self.observer.fov_y * 0.5).tan();
        let ndc_x = 2.0 * x / width as f64 - 1.0;
        let ndc_y = 1.0 - 2.0 * y / height as f64;
        Some(
            (vf.forward + vf.right * (ndc_x * tan_half * aspect) + vf.up * (ndc_y * tan_half))
                .normalize(),
        )
    }

    fn lock_at(&mut self, x: f64, y: f64, width: u32, height: u32) {
        let Some(dir) = self.cursor_ray(x, y, width, height) else {
            return;
        };
        self.view_lock = match self.pick_body(x, y, width, height) {
            // The ground occludes the sky, so it is not a background-star target.
            Some(i) if i == self.scene.host => return,
            Some(i) => Some(ViewLock::Body(i)),
            None => Some(ViewLock::Background(dir)),
        };
        self.track();
    }

    fn begin_sky_press(&mut self, cursor: (f64, f64)) {
        self.unlock();
        self.press = Some(cursor);
        self.last_cursor = Some(cursor);
        self.dragging = false;
    }

    /// Do not pan until the cursor leaves the click tolerance.
    fn drag_sky(&mut self, cursor: (f64, f64), scale: f64, height: u32) {
        let Some(start) = self.press else { return };
        if !self.dragging {
            if (cursor.0 - start.0).hypot(cursor.1 - start.1) <= 4.0 * scale {
                return;
            }
            self.dragging = true;
            self.unlock();
        }
        let previous = self.last_cursor.unwrap_or(start);
        let rad_per_px = self.observer.fov_y / height.max(1) as f64;
        if self.observer.star_orientation.is_some() {
            let positions = self.scene.positions(self.sim_time);
            let frame = self.observer.frame(&self.scene, self.sim_time, &positions);
            let dir = (frame.forward - frame.right * ((cursor.0 - previous.0) * rad_per_px)
                + frame.up * ((cursor.1 - previous.1) * rad_per_px))
                .normalize();
            self.observer.alt = dir.dot(frame.zenith).clamp(-1.0, 1.0).asin();
            self.observer.az = dir.dot(frame.east).atan2(dir.dot(frame.north));
        } else {
            self.observer.az -= (cursor.0 - previous.0) * rad_per_px;
            self.observer.alt = (self.observer.alt + (cursor.1 - previous.1) * rad_per_px)
                .clamp(-5.0_f64.to_radians(), 89.0_f64.to_radians());
        }
        self.last_cursor = Some(cursor);
    }

    /// Finish or cancel a gesture, returning whether it was a sky click.
    fn end_sky_press(&mut self) -> bool {
        let click = self.press.take().is_some() && !self.dragging;
        self.dragging = false;
        self.last_cursor = None;
        click
    }

    /// Return the nearest body under a physical-pixel cursor position, if any.
    ///
    /// Unlike the label pass this uses sphere and ring-annulus intersections,
    /// so overlapping bodies resolve to whichever is actually in front.
    /// Ring picking uses the annulus envelope, not its individual ringlets.
    fn pick_body(&self, x: f64, y: f64, width: u32, height: u32) -> Option<usize> {
        let dir = self.cursor_ray(x, y, width, height)?;
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        // The host body is kept as an occluder so a click on the ground cannot
        // select a body hidden on the far side of the planet. Locking rejects
        // the host rather than interpreting the ground as background sky.
        let mut best: Option<(f64, usize)> = None;
        for (i, body) in self.scene.bodies.iter().enumerate() {
            if body.kind == BodyKind::Anchor || body.radius <= 0.0 {
                continue;
            }
            let oc = positions[i] - vf.position;
            let along = oc.dot(dir);
            if let Some(rings) = body.rings {
                let pole = body.spin_quat(self.sim_time) * DVec3::Z;
                let denom = dir.dot(pole);
                if denom.abs() >= 1.0e-7 && rings.optical_depth > 0.0 {
                    let distance = oc.dot(pole) / denom;
                    let radius = (dir * distance - oc).length() / body.radius;
                    if distance > 0.0
                        && (rings.inner_radius..=rings.outer_radius).contains(&radius)
                        && best.is_none_or(|(bd, _)| distance < bd)
                    {
                        best = Some((distance, i));
                    }
                }
            }
            // Match the shader's stable test for tiny, distant bodies.
            let perpendicular2 = oc.cross(dir).length_squared();
            let r2 = body.radius * body.radius;
            if perpendicular2 > r2 {
                continue;
            }
            let root = (r2 - perpendicular2).sqrt();
            let near = along - root;
            let distance = if near > 0.0 { near } else { along + root };
            if distance > 0.0 && best.is_none_or(|(bd, _)| distance < bd) {
                best = Some((distance, i));
            }
        }
        best.map(|(_, i)| i)
    }

    fn build_frame(&self, width: u32, height: u32) -> Frame {
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);

        let aspect = width as f64 / height.max(1) as f64;
        let tan_half = (self.observer.fov_y * 0.5).tan();

        let mut bodies = Vec::new();
        for (i, body) in self.scene.bodies.iter().enumerate() {
            if body.kind == BodyKind::Anchor {
                continue; // invisible orbit anchors, not the observer's planet
            }
            let relative = positions[i] - vf.position;
            // Rotate in f64 as well as subtracting the camera. In telescope
            // space, near-axis ray x/y stay tiny instead of being rounded away
            // when added to a large world-space direction at extreme zoom.
            let view_center = vf.world_to_view(relative);
            let center = view_center.as_vec3();
            let low = (view_center - center.as_dvec3()).as_vec3();
            let star = body.kind == BodyKind::Star;
            let radiance = (SUN_LIGHT * body.luminosity as f64 / body.radius.powi(2)) as f32;
            let (ring_plane, ring_params, ring_color) = match body.rings {
                Some(rings) => {
                    let pole = vf.world_to_view(body.spin_quat(self.sim_time) * DVec3::Z);
                    (
                        [
                            pole.x as f32,
                            pole.y as f32,
                            pole.z as f32,
                            rings.inner_radius as f32,
                        ],
                        [
                            rings.outer_radius as f32,
                            rings.optical_depth,
                            if rings.banded { 1.0 } else { 0.0 },
                            0.0,
                        ],
                        [rings.albedo[0], rings.albedo[1], rings.albedo[2], 0.0],
                    )
                }
                None => ([0.0; 4], [0.0; 4], [0.0; 4]),
            };
            bodies.push(Sphere {
                center: center.into(),
                radius: body.radius as f32,
                color: if star {
                    body.emission.map(|c| c * radiance)
                } else {
                    body.albedo
                },
                emissive: if star { 1.0 } else { 0.0 },
                center_low: [low.x, low.y, low.z, if star { 0.0 } else { 1.0 }],
                ring_plane,
                ring_params,
                ring_color,
            });
        }

        // The host's atmosphere shell in telescope space. The camera sits just
        // above the surface, so the host-centre vector is short and needs no
        // high/low split.
        let host = self.scene.body(self.scene.host);
        let host_view = vf.world_to_view(positions[self.scene.host] - vf.position);
        let (atmo_center, atmo_rayleigh, atmo_params) = match self.scene.atmosphere {
            Some(a) => (
                [
                    host_view.x as f32,
                    host_view.y as f32,
                    host_view.z as f32,
                    host.radius as f32,
                ],
                [
                    a.rayleigh[0] as f32,
                    a.rayleigh[1] as f32,
                    a.rayleigh[2] as f32,
                    a.mie as f32,
                ],
                [
                    a.mie_g as f32,
                    a.scale_height as f32,
                    a.thickness as f32,
                    1.0,
                ],
            ),
            None => ([0.0; 4], [0.0; 4], [0.0; 4]),
        };

        let globals = Globals {
            cam_right: [vf.right.x as f32, vf.right.y as f32, vf.right.z as f32, 0.0],
            cam_up: [vf.up.x as f32, vf.up.y as f32, vf.up.z as f32, 0.0],
            cam_forward: [
                vf.forward.x as f32,
                vf.forward.y as f32,
                vf.forward.z as f32,
                tan_half as f32,
            ],
            viewport: [width as f32, height as f32, 1.0, 0.0],
            atmo_center,
            atmo_rayleigh,
            atmo_params,
        };

        // Screen-space anchors for the optional name labels.
        let mut labels = Vec::new();
        {
            let w = width as f32;
            let h = height as f32;
            for (i, body) in self.scene.bodies.iter().enumerate() {
                if i == self.scene.host || body.kind == BodyKind::Anchor {
                    continue;
                }
                let relative = positions[i] - vf.position;
                let dist = relative.length();
                if dist < 1.0e-9 {
                    continue;
                }
                // Skip anything below the local horizon.
                if relative.dot(vf.zenith) < 0.0 {
                    continue;
                }
                // Labels use the same f64 telescope coordinates as the traced
                // geometry; no raster projection or depth range is needed.
                let center = vf.world_to_view(relative);
                if center.z >= 0.0 {
                    continue;
                }
                let ndc_x = center.x / (-center.z * tan_half * aspect);
                let ndc_y = center.y / (-center.z * tan_half);
                if !(-1.0..=1.0).contains(&ndc_x) || !(-1.0..=1.0).contains(&ndc_y) {
                    continue;
                }
                let sx = (ndc_x * 0.5 + 0.5) as f32 * w;
                let sy = (1.0 - (ndc_y * 0.5 + 0.5)) as f32 * h;
                let ang = body.radius / dist;
                let px_r = (ang / tan_half) as f32 * (h * 0.5);
                labels.push(Label {
                    x: sx,
                    y: sy,
                    radius: px_r.max(4.0),
                    text: body.name.clone(),
                    body: i,
                });
            }
        }

        Frame {
            globals,
            bodies,
            scene_time: self.sim_time,
            labels,
            ui: Vec::new(),
        }
    }
}

struct App {
    state: State,
    menu: menu::Menu,
    game: Option<game::Game>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    stars: Vec<CatalogueStar>,
    last_title: String,
    scale: f32,
    cursor: (f64, f64),
    show_labels: bool,
    show_hud: bool,
    auto_exposure: bool,
    ev_bias: f32,
    exposure_dragging: bool,
}

impl App {
    fn right_click(&mut self) {
        if self.menu.open {
            return;
        }
        let Some(r) = self.renderer.as_ref() else {
            return;
        };
        let (w, h) = r.size();
        if w == 0 || h == 0 {
            return;
        }
        if let Some(game) = self.game.as_mut()
            && game.open
        {
            game.right_click(&game.layout(w as f32, h as f32, self.scale), self.cursor);
            return;
        }
        let (x, y) = self.cursor;
        let hud = ui::layout(w as f32, h as f32, self.scale).with_puzzle(self.game.is_some());
        if hud.lock.contains(x, y)
            || (self.show_hud && hud.bar.contains(x, y))
            || (self.game.is_none() && hud.home.contains(x, y))
        {
            return;
        }
        self.state.end_sky_press();
        self.exposure_dragging = false;
        self.state.lock_at(x, y, w, h);
    }

    fn toggle_notebook(&mut self) {
        if let Some(game) = self.game.as_mut() {
            game.toggle();
        }
        self.state.end_sky_press();
        self.state.last = Instant::now();
        self.exposure_dragging = false;
    }

    fn toggle_menu(&mut self) {
        self.state.end_sky_press();
        if let Some(game) = self.game.as_mut() {
            game.release();
        }
        self.exposure_dragging = false;
        if self.menu.open {
            self.menu.open = false;
            self.state.last = Instant::now();
        } else {
            self.menu.refresh();
            if self.game.is_some() {
                self.menu
                    .entries
                    .sort_by_key(|e| e.path.file_stem().is_none_or(|name| name != "puzzle"));
            }
        }
    }

    fn select_map(&mut self, path: &std::path::Path) {
        // Load successfully before replacing any of the player's current state.
        match config::load(path) {
            Ok(scene) => {
                self.state = State::with_scene(scene);
                if self.game.is_some() {
                    self.game = Some(game::Game::default());
                }
                self.menu.open = false;
                self.menu.error = None;
                self.exposure_dragging = false;
            }
            Err(error) => {
                self.menu.error = Some(if self.game.is_some() {
                    "Could not load this map. Your current puzzle is unchanged.".into()
                } else {
                    format!(
                        "Could not load {}: {error:#}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )
                });
            }
        }
    }

    fn menu_layout(&self) -> Option<menu::Layout> {
        let (w, h) = self.renderer.as_ref()?.size();
        (w > 0 && h > 0).then(|| menu::Layout::new(w as f32, h as f32, self.scale))
    }

    fn menu_click(&mut self, cx: f64, cy: f64) -> bool {
        let Some(layout) = self.menu_layout() else {
            return self.menu.open;
        };
        if self.game.as_ref().is_some_and(|g| g.confirm_clear) {
            return false;
        }
        if self.game.as_ref().is_some_and(|g| g.open) && !self.menu.open {
            if let Some(r) = self.renderer.as_ref() {
                let (w, h) = r.size();
                if game::Layout::new(w as f32, h as f32, self.scale)
                    .maps
                    .contains(cx, cy)
                {
                    self.toggle_menu();
                    return true;
                }
            }
            return false;
        }
        if (!self.menu.open && layout.home.contains(cx, cy))
            || (self.menu.open && layout.resume.contains(cx, cy))
        {
            self.toggle_menu();
            return true;
        }
        if !self.menu.open {
            return false;
        }
        let page = layout.rows.len();
        if layout.previous.contains(cx, cy) {
            self.menu.offset = self.menu.offset.saturating_sub(page);
        } else if layout.next.contains(cx, cy) && self.menu.offset + page < self.menu.entries.len()
        {
            self.menu.offset += page;
        } else if let Some(row) = layout.rows.iter().position(|r| r.contains(cx, cy))
            && let Some(entry) = self.menu.entries.get(self.menu.offset + row)
        {
            let path = entry.path.clone();
            self.select_map(&path);
        }
        true
    }

    /// UI clicks preserve tracking, except the explicit lock/unlock button.
    fn hud_click(&mut self, layout: &ui::Layout, x: f64, y: f64) -> bool {
        if layout.lock.contains(x, y) {
            self.state.unlock();
            return true;
        }
        if !self.show_hud || !layout.bar.contains(x, y) {
            return false;
        }
        if layout.orientation.contains(x, y) {
            self.state.toggle_star_orientation();
        } else if layout.slower.contains(x, y) {
            self.state.change_speed(false);
        } else if layout.faster.contains(x, y) {
            self.state.change_speed(true);
        } else if layout.stop.contains(x, y) {
            self.state.warp = STOP_WARP;
        } else if self.game.is_some() && layout.draw.contains(x, y) {
            self.toggle_notebook();
        } else if self.game.is_none() && layout.toggle.contains(x, y) {
            self.show_labels = !self.show_labels;
        } else if layout.exposure.contains(x, y) {
            self.exposure_dragging = true;
            self.adjust_exposure(layout, x);
        } else if layout.auto.contains(x, y) {
            self.auto_exposure = !self.auto_exposure;
            self.sync_exposure();
        }
        true
    }

    fn adjust_exposure(&mut self, layout: &ui::Layout, x: f64) {
        self.ev_bias = layout.exposure_bias_at(x);
        self.sync_exposure();
    }

    fn sync_exposure(&mut self) {
        if let Some(r) = self.renderer.as_mut() {
            r.set_exposure_controls(self.auto_exposure, self.ev_bias);
        }
    }

    fn ui_layout(&self) -> Option<ui::Layout> {
        let r = self.renderer.as_ref()?;
        let (w, h) = r.size();
        if w == 0 || h == 0 {
            return None;
        }
        Some(ui::layout(w as f32, h as f32, self.scale).with_puzzle(self.game.is_some()))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("stargaze")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        match Renderer::new(window.clone(), &self.stars) {
            Ok(renderer) => {
                log::info!("renderer ready: {:?}", window.inner_size());
                self.scale = window.scale_factor() as f32;
                self.auto_exposure = renderer.auto_exposure();
                self.ev_bias = renderer.ev_bias();
                self.renderer = Some(renderer);
                self.window = Some(window);
            }
            Err(err) => {
                log::error!("failed to initialise renderer: {err:?}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.menu.open || self.game.as_ref().is_some_and(|g| g.open) {
                    self.state.last = Instant::now();
                } else {
                    self.state.advance();
                    self.state.track();
                }
                let lock_label = self
                    .state
                    .lock_label(self.show_labels && self.show_hud && self.game.is_none());
                if let (Some(r), Some(window)) = (self.renderer.as_mut(), self.window.as_ref()) {
                    let (w, h) = r.size();
                    if w > 0 && h > 0 {
                        let mut frame = self.state.build_frame(w, h);
                        if self.show_hud && !self.menu.open {
                            let layout = ui::layout(w as f32, h as f32, self.scale)
                                .with_puzzle(self.game.is_some());
                            ui::build(
                                &mut frame.ui,
                                &layout,
                                w as f32,
                                h as f32,
                                ui::HudState {
                                    labels: ui::LabelOptions {
                                        show: self.show_labels && self.game.is_none(),
                                        locked: self.state.locked_body(),
                                    },
                                    lock_label: lock_label.as_deref(),
                                    sim_time: self.state.sim_time,
                                    auto_exposure: self.auto_exposure,
                                    ev_bias: self.ev_bias,
                                    steady_stars: self.state.observer.star_orientation.is_some(),
                                    minutes_per_second: WARPS[self.state.warp] * 1440.0,
                                },
                                &frame.labels,
                            );
                        }
                        if !self.show_hud
                            && !self.menu.open
                            && !self.game.as_ref().is_some_and(|g| g.open)
                        {
                            let layout = ui::layout(w as f32, h as f32, self.scale)
                                .with_puzzle(self.game.is_some());
                            ui::build_lock(
                                &mut frame.ui,
                                &layout,
                                [w as f32, h as f32],
                                lock_label.as_deref(),
                            );
                        }
                        if let Some(game) = &self.game {
                            game::build(
                                &mut frame.ui,
                                [w as f32, h as f32],
                                self.scale,
                                game,
                                self.cursor,
                                &format!(
                                    "{} | {:+.1} min/s{}",
                                    if self.state.warp == STOP_WARP {
                                        "STOPPED"
                                    } else {
                                        "PLAYING"
                                    },
                                    WARPS[self.state.warp] * 1440.0,
                                    if self.state.view_lock.is_some() {
                                        " | View locked"
                                    } else {
                                        ""
                                    }
                                ),
                            );
                        }
                        menu::build(
                            &mut frame.ui,
                            [w as f32, h as f32],
                            self.scale,
                            &self.menu,
                            self.cursor,
                            self.game.is_some(),
                        );
                        r.render(&frame);
                    }
                    let rate = format!("{:+.3} day/s", WARPS[self.state.warp]);
                    let lock = lock_label
                        .as_ref()
                        .map_or_else(String::new, |label| format!("  ·  locked: {label}"));
                    let exposure = if self.auto_exposure {
                        format!("auto exp {:+.2} EV", self.ev_bias)
                    } else {
                        format!("exp {:.3} {:+.2} EV", r.manual_exposure(), self.ev_bias)
                    };
                    let title = if self.game.is_some() {
                        "stargaze · Reconstruction puzzle · M: menu · Tab: diagram · Space: stop"
                            .into()
                    } else if self.menu.open {
                        "stargaze  ·  Solar systems".to_string()
                    } else {
                        format!(
                            "stargaze  ·  t = {:.2} d ({:.3} yr)  ·  {}  ·  {} spp  ·  {exposure}{lock}  ·  right click=lock  left click=unlock  drag=look  scroll=zoom  space=stop  , . =exposure  A=auto  arrows=speed  R=reset  E=eclipse  T=transit  H=hud  L=labels",
                            self.state.sim_time,
                            self.state.sim_time / 365.256,
                            rate,
                            r.samples(),
                        )
                    };
                    if title != self.last_title {
                        window.set_title(&title);
                        self.last_title = title;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Right {
                    if state == ElementState::Pressed {
                        self.right_click();
                    }
                    return;
                }
                if button != MouseButton::Left {
                    return;
                }
                match state {
                    ElementState::Pressed => {
                        let (cx, cy) = self.cursor;
                        if self.menu_click(cx, cy) {
                            self.state.end_sky_press();
                            return;
                        }
                        if let (Some(game), Some(r)) = (self.game.as_mut(), self.renderer.as_ref())
                        {
                            let (w, h) = r.size();
                            let layout = game.layout(w.max(1) as f32, h.max(1) as f32, self.scale);
                            if game.open && game.click(&layout, self.cursor, &self.state.scene) {
                                self.state.end_sky_press();
                                self.exposure_dragging = false;
                                self.state.last = Instant::now();
                                return;
                            }
                        }
                        let consumed = self
                            .ui_layout()
                            .is_some_and(|layout| self.hud_click(&layout, cx, cy));
                        if consumed {
                            self.state.end_sky_press();
                        } else {
                            self.state.begin_sky_press((cx, cy));
                        }
                    }
                    ElementState::Released => {
                        if let Some(game) = self.game.as_mut() {
                            game.release();
                        }
                        self.exposure_dragging = false;
                        self.state.end_sky_press();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if self.menu.open {
                    return;
                }
                if let (Some(game), Some(r)) = (self.game.as_mut(), self.renderer.as_ref())
                    && game.open
                {
                    let (w, h) = r.size();
                    game.motion(
                        &game.layout(w.max(1) as f32, h.max(1) as f32, self.scale),
                        self.cursor,
                    );
                    return;
                }
                if self.exposure_dragging {
                    if let Some(layout) = self.ui_layout() {
                        self.adjust_exposure(&layout, position.x);
                    }
                } else {
                    let height = self.renderer.as_ref().map(|r| r.size().1).unwrap_or(1);
                    self.state.drag_sky(self.cursor, self.scale as f64, height);
                }
            }
            WindowEvent::Focused(false) | WindowEvent::CursorLeft { .. } => {
                if let Some(game) = self.game.as_mut() {
                    game.release();
                }
                // A release outside the window may never reach us.
                self.exposure_dragging = false;
                self.state.end_sky_press();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                if self.menu.open {
                    if let Some(layout) = self.menu_layout() {
                        let page = layout.rows.len();
                        if y > 0.0 {
                            self.menu.offset = self.menu.offset.saturating_sub(page);
                        } else if y < 0.0 && self.menu.offset + page < self.menu.entries.len() {
                            self.menu.offset += page;
                        }
                    }
                    return;
                }
                if self.game.as_ref().is_some_and(|g| g.open) {
                    return;
                }
                let fov = self.state.observer.fov_y * ZOOM_STEP.powf(y);
                self.state.observer.fov_y =
                    fov.clamp(MIN_FOV_DEG.to_radians(), MAX_FOV_DEG.to_radians());
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let PhysicalKey::Code(key) = event.physical_key else {
                    return;
                };
                if self.game.as_ref().is_some_and(|g| g.confirm_clear) {
                    if !event.repeat
                        && let (Some(game), Some(r)) = (self.game.as_mut(), self.renderer.as_ref())
                    {
                        let (w, h) = r.size();
                        game.key(
                            key,
                            &game.layout(w as f32, h as f32, self.scale),
                            self.cursor,
                            &self.state.scene,
                        );
                    }
                    return;
                }
                if matches!(key, KeyCode::Escape | KeyCode::KeyM) {
                    if !event.repeat {
                        self.toggle_menu();
                    }
                    return;
                }
                if self.menu.open {
                    if let Some(layout) = self.menu_layout() {
                        let page = layout.rows.len();
                        match key {
                            KeyCode::PageUp => {
                                self.menu.offset = self.menu.offset.saturating_sub(page)
                            }
                            KeyCode::PageDown
                                if self.menu.offset + page < self.menu.entries.len() =>
                            {
                                self.menu.offset += page
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
                                if let Some(row) =
                                    keys.iter().position(|&k| k == key).filter(|&i| i < page)
                                    && let Some(entry) =
                                        self.menu.entries.get(self.menu.offset + row)
                                {
                                    let path = entry.path.clone();
                                    if !event.repeat {
                                        self.select_map(&path);
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
                if self.game.is_some() {
                    if key == KeyCode::Tab {
                        if !event.repeat {
                            self.toggle_notebook();
                        }
                        return;
                    }
                    if self.game.as_ref().is_some_and(|g| g.open) {
                        if !event.repeat
                            && let (Some(game), Some(r)) =
                                (self.game.as_mut(), self.renderer.as_ref())
                        {
                            let (w, h) = r.size();
                            game.key(
                                key,
                                &game.layout(w as f32, h as f32, self.scale),
                                self.cursor,
                                &self.state.scene,
                            );
                        }
                        return;
                    }
                    // Named target and event shortcuts bypass the observation puzzle.
                    if matches!(
                        key,
                        KeyCode::KeyL
                            | KeyCode::KeyE
                            | KeyCode::KeyT
                            | KeyCode::F1
                            | KeyCode::F2
                            | KeyCode::F3
                            | KeyCode::F4
                            | KeyCode::F5
                            | KeyCode::F6
                            | KeyCode::F7
                            | KeyCode::F8
                            | KeyCode::F9
                    ) {
                        return;
                    }
                    if key == KeyCode::KeyR {
                        self.state.point_at_fov(
                            self.state.scene.default_target,
                            self.state.scene.default_fov_deg.to_radians(),
                        );
                        return;
                    }
                }
                if self.state.time_key(key) {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyS | KeyCode::KeyO) => {
                        if !event.repeat {
                            self.state.toggle_star_orientation();
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyU) => self.state.unlock(),
                    PhysicalKey::Code(KeyCode::KeyR) => self.state.reset_view(),
                    PhysicalKey::Code(KeyCode::KeyE) => {
                        if let Some(what) = self.state.next_eclipse() {
                            log::info!("found {what} at t = {:.3} d", self.state.sim_time);
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        if let Some(what) = self.state.next_moon_transit() {
                            log::info!("found {what} at t = {:.3} d", self.state.sim_time);
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyH) => {
                        self.show_hud = !self.show_hud;
                        self.exposure_dragging = false;
                    }
                    PhysicalKey::Code(KeyCode::KeyL) => self.show_labels = !self.show_labels,
                    PhysicalKey::Code(KeyCode::KeyA) => {
                        self.auto_exposure = !self.auto_exposure;
                        self.sync_exposure();
                    }
                    PhysicalKey::Code(KeyCode::Comma) => {
                        self.ev_bias = (self.ev_bias - 0.25).clamp(ui::MIN_EV, ui::MAX_EV);
                        self.sync_exposure();
                    }
                    PhysicalKey::Code(KeyCode::Period) => {
                        self.ev_bias = (self.ev_bias + 0.25).clamp(ui::MIN_EV, ui::MAX_EV);
                        self.sync_exposure();
                    }
                    PhysicalKey::Code(KeyCode::F1) => self.state.aim_target(0),
                    PhysicalKey::Code(KeyCode::F2) => self.state.aim_target(1),
                    PhysicalKey::Code(KeyCode::F3) => self.state.aim_target(2),
                    PhysicalKey::Code(KeyCode::F4) => self.state.aim_target(3),
                    PhysicalKey::Code(KeyCode::F5) => self.state.aim_target(4),
                    PhysicalKey::Code(KeyCode::F6) => self.state.aim_target(5),
                    PhysicalKey::Code(KeyCode::F7) => self.state.aim_target(6),
                    PhysicalKey::Code(KeyCode::F8) => self.state.aim_target(7),
                    PhysicalKey::Code(KeyCode::F9) => self.state.aim_target(8),
                    PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                        let fov = self.state.observer.fov_y * ZOOM_STEP;
                        self.state.observer.fov_y = fov.max(MIN_FOV_DEG.to_radians());
                    }
                    PhysicalKey::Code(KeyCode::Minus)
                    | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                        let fov = self.state.observer.fov_y / ZOOM_STEP;
                        self.state.observer.fov_y = fov.min(MAX_FOV_DEG.to_radians());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let Some(scene) = scene_for_startup()? else {
        return Ok(());
    };
    let stars = stars::generate(3500, 2500, 0x5EED_1234);
    log::info!("generated {} background stars", stars.len());

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        state: State::with_scene(scene),
        menu: menu::Menu::default(),
        game: game_requested().then(game::Game::default),
        window: None,
        renderer: None,
        stars,
        last_title: String::new(),
        scale: 1.0,
        cursor: (0.0, 0.0),
        show_labels: true,
        show_hud: true,
        auto_exposure: true,
        ev_bias: 0.0,
        exposure_dragging: false,
    };

    if app.game.is_some() {
        app.toggle_menu();
    }

    if app.game.is_none() {
        log::info!(
            "system: {}",
            app.state
                .scene
                .bodies
                .iter()
                .map(|b| format!("{} ({:.3} d)", b.name, b.period_days))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if app.game.is_none() && std::env::var("STARGAZE_DEBUG").is_ok() {
        let frame = app.state.build_frame(1280, 720);
        log::info!("forward={:?}", frame.globals.cam_forward);
        log::info!(
            "ray-traced bodies={} viewport={:?}",
            frame.bodies.len(),
            frame.globals.viewport
        );
        for (i, s) in app.stars.iter().take(4).enumerate() {
            log::info!(
                "star{i}: direction={:?} brightness={:.3} angular_radius={:.6} rad",
                s.dir,
                s.bright,
                s.angular_radius
            );
        }
        for b in &frame.bodies {
            log::info!(
                "body center={:?} r={:.3e} col={:?}",
                b.center,
                b.radius,
                b.color
            );
        }
    }

    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod app_tests {
    use super::*;

    #[test]
    fn time_controls_and_menus_preserve_lock_but_lock_button_releases_it() {
        let mut app = App {
            state: State::with_scene(sim::default_scene()),
            menu: menu::Menu::default(),
            game: None,
            window: None,
            renderer: None,
            stars: vec![],
            last_title: String::new(),
            scale: 1.0,
            cursor: (0.0, 0.0),
            show_labels: true,
            show_hud: true,
            auto_exposure: true,
            ev_bias: 0.0,
            exposure_dragging: false,
        };
        let layout = ui::layout(1280.0, 720.0, 1.0);
        let click = |app: &mut App, r: ui::Rect| {
            assert!(app.hud_click(&layout, (r.x + r.w * 0.5) as f64, (r.y + r.h * 0.5) as f64));
        };
        for lock in [ViewLock::Body(2), ViewLock::Background(DVec3::X)] {
            app.state.view_lock = Some(lock);
            let time = app.state.sim_time;
            app.state.warp = STOP_WARP;
            click(&mut app, layout.faster);
            assert_eq!(app.state.warp, STOP_WARP + 1);
            click(&mut app, layout.slower);
            assert_eq!(app.state.warp, STOP_WARP);
            assert_eq!(WARPS[app.state.warp], 0.0);
            click(&mut app, layout.faster);
            click(&mut app, layout.stop);
            assert_eq!(WARPS[app.state.warp], 0.0);
            assert_eq!(app.state.sim_time, time);
            assert_eq!(app.state.view_lock, Some(lock));
            for r in [layout.toggle, layout.exposure, layout.auto] {
                click(&mut app, r);
                assert_eq!(app.state.view_lock, Some(lock));
            }
            app.toggle_menu();
            app.toggle_menu();
            assert_eq!(app.state.view_lock, Some(lock));
            app.game = Some(game::Game::default());
            app.toggle_notebook();
            app.toggle_notebook();
            assert_eq!(app.state.view_lock, Some(lock));
            app.game = None;
            app.show_hud = false;
            click(&mut app, layout.lock);
            assert_eq!(app.state.view_lock, None);
            app.show_hud = true;
        }
    }

    #[test]
    fn speed_controls_cross_into_reverse_and_respect_bounds() {
        let mut state = State::with_scene(sim::default_scene());
        state.view_lock = Some(ViewLock::Body(2));
        let start = state.sim_time;
        assert_eq!(WARPS[state.warp], 0.0);
        assert!(state.time_key(KeyCode::ArrowLeft));
        assert_eq!(WARPS[state.warp], -0.001);
        assert_eq!(state.sim_time, start, "speed changes must not jump time");
        assert!(state.time_key(KeyCode::ArrowRight));
        assert_eq!(WARPS[state.warp], 0.0);
        assert!(state.time_key(KeyCode::ArrowRight));
        assert_eq!(WARPS[state.warp], 0.001);
        for _ in 0..30 {
            state.time_key(KeyCode::ArrowLeft);
        }
        assert_eq!(state.warp, 0);
        assert_eq!(WARPS[state.warp], -40.0);
        for _ in 0..30 {
            state.time_key(KeyCode::ArrowRight);
        }
        assert_eq!(state.warp, WARPS.len() - 1);
        assert_eq!(WARPS[state.warp], 40.0);
        assert_eq!(state.view_lock, Some(ViewLock::Body(2)));
        assert!(!state.time_key(KeyCode::Digit1));
        assert!(!state.time_key(KeyCode::Digit2));
    }

    #[test]
    fn reverse_playback_retraces_orbits_keeps_target_centered_and_stops_at_zero() {
        let mut state = State::with_scene(sim::default_scene());
        state.sim_time = 0.0;
        state.view_lock = Some(ViewLock::Body(2));
        let initial_positions = state.scene.positions(state.sim_time);
        state.warp = 7; // slowest reverse speed
        state.advance_by(10.0);
        state.track();
        assert!(
            (state.sim_time + 0.01).abs() < 1e-12,
            "reverse can cross time zero"
        );
        let positions = state.scene.positions(state.sim_time);
        let frame = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        assert!(
            frame
                .forward
                .dot((positions[2] - frame.position).normalize())
                > 1.0 - 1e-12
        );
        assert!(state.time_key(KeyCode::Space));
        assert_eq!(WARPS[state.warp], 0.0);
        assert!(state.time_key(KeyCode::Space)); // stopping again never resumes
        assert_eq!(WARPS[state.warp], 0.0);
        state.advance_by(10.0);
        assert!((state.sim_time + 0.01).abs() < 1e-12);
        state.change_speed(true); // first positive speed from zero
        state.advance_by(10.0);
        assert!(state.sim_time.abs() < 1e-12);
        assert_eq!(state.scene.positions(state.sim_time), initial_positions);
        assert_eq!(state.view_lock, Some(ViewLock::Body(2)));
    }

    #[test]
    fn halo_faces_calyx_without_tracking_and_clears_its_rings() {
        for system in [None, Some("halo"), Some("binary")] {
            let mut state = State::with_scene(scene_for_system(system));
            let host = state.scene.host;
            let target = state.scene.default_target;
            let moon = state.scene.body(host);
            let planet = state.scene.body(target);
            assert_eq!(moon.name, "Halo");
            assert_eq!(planet.name, "Calyx");
            assert_eq!(moon.parent, Some(target));
            let rings = planet.rings.unwrap();
            let clearance = moon.elements.a * (1.0 - moon.elements.e)
                - moon.radius
                - rings.outer_radius * planet.radius;
            assert!(clearance * sim::AU_KM > 4_000.0);
            assert!(!state.scene.targets.contains(&host));
            assert_eq!(state.view_lock, None);
            assert_eq!(state.pick_body(400.0, 300.0, 800, 600), Some(target));
            let start = state.sim_time;
            let period = moon.period_days;
            // Do not call track or re-aim: the physical spin/orbit must keep
            // the planet fixed, including across the orbit's angular wrap.
            for step in 0..65 {
                state.sim_time = start + period * step as f64 / 16.0;
                let positions = state.scene.positions(state.sim_time);
                let view = state
                    .observer
                    .frame(&state.scene, state.sim_time, &positions);
                let to_planet = (positions[target] - view.position).normalize();
                assert!(view.forward.dot(to_planet) > 1.0 - 1e-10);
                assert!(to_planet.dot(view.zenith) > 0.7);
            }
        }
    }

    #[test]
    fn saturn_observatory_stays_at_twenty_degrees_north() {
        for system in [Some("solar"), Some("sol"), Some("saturn")] {
            let mut state = State::with_scene(scene_for_system(system));
            let host = state.scene.host;
            assert_eq!(state.observer.body, host);
            assert_eq!(state.scene.body(host).name, "Saturn");
            assert_eq!(state.observer.lat.to_degrees(), 20.0);
            assert!(state.scene.atmosphere.is_none());
            assert!(!state.scene.targets.contains(&host));
            assert!(
                state.observer.alt > 0.2,
                "initial target must be above the horizon"
            );
            assert_eq!(
                state.pick_body(400.0, 300.0, 800, 600),
                Some(state.scene.default_target)
            );
            let period = state.scene.body(host).spin_period;
            for _ in 0..8 {
                state.sim_time += period / 8.0;
                let positions = state.scene.positions(state.sim_time);
                let view = state
                    .observer
                    .frame(&state.scene, state.sim_time, &positions);
                let surface_offset = view.position - positions[host];
                let body = state.scene.body(host);
                let height_m = (surface_offset.length() - body.radius) * sim::AU_KM * 1000.0;
                assert!((height_m - 2.0).abs() < 0.001);
                let pole = body.spin_quat(state.sim_time) * DVec3::Z;
                assert!(
                    (surface_offset.normalize().dot(pole) - 20_f64.to_radians().sin()).abs() < 1e-8
                );
                assert!(view.forward.is_finite());
            }
        }
        assert_eq!(scene_for_system(Some("earth")).host, 1);
        let binary = scene_for_system(Some("calyx"));
        assert_eq!(binary.body(binary.host).name, "Calyx");
    }

    #[test]
    fn binary_observatory_starts_on_calyx_looking_at_halo() {
        let state = State::with_scene(sim::binary_scene());
        assert_eq!(state.observer.body, state.scene.host);
        assert_eq!(state.scene.body(state.observer.body).name, "Calyx");
        let positions = state.scene.positions(state.sim_time);
        let vf = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        let height = (vf.position - positions[state.scene.host]).length()
            - state.scene.body(state.scene.host).radius;
        assert!((height * sim::AU_KM * 1_000.0 - 2.0).abs() < 0.001);
        let halo = state.scene.default_target;
        assert_eq!(state.pick_body(400.0, 300.0, 800, 600), Some(halo));
        let frame = state.build_frame(800, 600);
        assert!(frame.labels.iter().any(|label| label.body == halo));
        assert!(
            !frame
                .labels
                .iter()
                .any(|label| label.body == state.scene.host)
        );
    }

    #[test]
    fn labels_stay_aligned_with_traced_geometry_at_deep_zoom() {
        let mut state = State::with_scene(sim::binary_scene());
        state.sim_time = 444.478;
        state.point_at_fov(8, 0.005323985_f64.to_radians());
        let centered = state.build_frame(801, 601);
        let label = centered.labels.iter().find(|l| l.text == "Vantus").unwrap();
        assert!((label.x - 400.5).abs() < 0.01);
        assert!((label.y - 300.5).abs() < 0.01);

        state.observer.az += state.observer.fov_y * 0.1;
        let offset = state.build_frame(801, 601);
        let label = offset.labels.iter().find(|l| l.text == "Vantus").unwrap();
        // Binary scene index 0 is an invisible anchor, hence sphere index 7.
        let sphere = &offset.bodies[7];
        let center = DVec3::from_array(sphere.center.map(f64::from))
            + DVec3::new(
                sphere.center_low[0] as f64,
                sphere.center_low[1] as f64,
                sphere.center_low[2] as f64,
            );
        let tan_half = offset.globals.cam_forward[3] as f64;
        let x = (center.x / (-center.z * tan_half * 801.0 / 601.0) * 0.5 + 0.5) * 801.0;
        let y = (0.5 - center.y / (-center.z * tan_half) * 0.5) * 601.0;
        assert!((label.x as f64 - x).abs() < 0.01);
        assert!((label.y as f64 - y).abs() < 0.01);
    }

    #[test]
    fn clicking_a_body_picks_it_and_tracking_keeps_it_centered() {
        let mut state = State::with_scene(sim::default_scene());
        // Aim at Luna (2), which `find_good_start` guarantees is above the
        // horizon; a click at the exact centre must hit it.
        state.point_at(2);
        assert_eq!(
            state.pick_body(400.0, 300.0, 800, 600),
            Some(2),
            "centre click should pick Luna"
        );
        // A far-off corner is empty sky, suitable for an inertial direction lock.
        assert_eq!(state.pick_body(1.0, 1.0, 800, 600), None);

        // Right-click it, advance time, and re-track: the body stays centred.
        state.lock_at(400.0, 300.0, 800, 600);
        assert_eq!(state.view_lock, Some(ViewLock::Body(2)));
        state.sim_time += 37.0;
        state.track();
        let positions = state.scene.positions(state.sim_time);
        let vf = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        let dir = (positions[2] - vf.position).normalize();
        assert!(
            vf.forward.dot(dir) > 0.999_999,
            "locked body should stay centred as time advances"
        );
    }

    #[test]
    fn background_lock_keeps_clicked_world_direction_across_time_and_rotation() {
        let mut state = State::with_scene(sim::default_scene());
        state.point_at(2);
        let ray = state.cursor_ray(1.0, 1.0, 800, 600).unwrap();
        assert_eq!(state.pick_body(1.0, 1.0, 800, 600), None);
        let start = state.sim_time;
        state.lock_at(1.0, 1.0, 800, 600);
        assert_eq!(state.view_lock, Some(ViewLock::Background(ray)));
        for offset in [0.0, 0.25, 7.0, 91.0, -4.0] {
            state.sim_time = start + offset;
            state.track();
            let positions = state.scene.positions(state.sim_time);
            let vf = state
                .observer
                .frame(&state.scene, state.sim_time, &positions);
            assert!(
                vf.forward.dot(ray) > 1.0 - 1e-12,
                "fixed star direction moved at {offset} days"
            );
            assert!(vf.right.is_finite() && vf.up.is_finite());
        }
        state.begin_sky_press((100.0, 100.0));
        assert_eq!(state.view_lock, None);
        state.end_sky_press();
        state.sim_time += 0.1;
        state.track();
        let p = state.scene.positions(state.sim_time);
        let vf = state.observer.frame(&state.scene, state.sim_time, &p);
        assert!(
            vf.forward.dot(ray) < 0.99,
            "unlocked telescope should follow the host again"
        );
    }

    #[test]
    fn steady_stars_hold_background_roll_and_leave_the_lock_unchanged() {
        let mut state = State::with_scene(sim::default_scene());
        state.point_at(2);
        state.lock_at(1.0, 1.0, 800, 600);
        let lock = state.view_lock;
        let start = state.sim_time;
        let p = state.scene.positions(start);
        let before = state.observer.frame(&state.scene, start, &p);
        state.toggle_star_orientation();
        for offset in [0.0, 0.25, 0.5, 5.0, -1.0, 0.0] {
            state.sim_time = start + offset;
            state.track();
            let p = state.scene.positions(state.sim_time);
            let frame = state.observer.frame(&state.scene, state.sim_time, &p);
            assert!(frame.forward.dot(before.forward) > 1.0 - 1e-12);
            assert!(frame.up.dot(before.up) > 1.0 - 1e-12);
            assert!(frame.right.dot(before.right) > 1.0 - 1e-12);
            assert_eq!(state.view_lock, lock);
        }
        state.toggle_star_orientation();
        assert!(state.observer.star_orientation.is_none());
        assert_eq!(state.view_lock, lock);
    }

    #[test]
    fn steady_stars_follow_planet_without_roll_or_changing_position_or_zoom() {
        let mut state = State::with_scene(sim::default_scene());
        state.point_at(3); // Mars: the roll toggle also works with a planet lock.
        state.view_lock = Some(ViewLock::Body(3));
        state.track();
        let p = state.scene.positions(state.sim_time);
        let before = state.observer.frame(&state.scene, state.sim_time, &p);
        let fov = state.observer.fov_y;
        state.toggle_star_orientation();
        let start = state.sim_time;
        for offset in [0.0, 0.25, 7.0, 90.0, -10.0] {
            state.sim_time = start + offset;
            state.track();
            let p = state.scene.positions(state.sim_time);
            let frame = state.observer.frame(&state.scene, state.sim_time, &p);
            let direction = (p[3] - frame.position).normalize();
            assert!(frame.forward.dot(direction) > 1.0 - 1e-12);
            // Minimal transport back to the initial aim must show no accumulated roll.
            let back = glam::DQuat::from_rotation_arc(frame.forward, before.forward);
            assert!((back * frame.up).dot(before.up) > 1.0 - 1e-10);
            assert!((frame.right.cross(frame.up) + frame.forward).length() < 1e-10);
            assert_eq!(state.observer.fov_y, fov);
            let reference = state.observer.star_orientation.take();
            let natural = state.observer.frame(&state.scene, state.sim_time, &p);
            assert!((frame.position - natural.position).length() < 1e-12);
            state.observer.star_orientation = reference;
        }
    }

    #[test]
    fn lock_descriptions_hide_names_when_labels_are_off() {
        let mut state = State::with_scene(sim::default_scene());
        state.view_lock = Some(ViewLock::Body(2));
        assert_eq!(
            state.lock_label(true).as_deref(),
            Some(state.scene.body(2).name.as_str())
        );
        assert_eq!(state.lock_label(false).as_deref(), Some("Planet / moon"));
        state.view_lock = Some(ViewLock::Body(0));
        assert_eq!(state.lock_label(false).as_deref(), Some("Star"));
        state.view_lock = Some(ViewLock::Background(DVec3::X));
        for show_names in [false, true] {
            assert_eq!(
                state.lock_label(show_names).as_deref(),
                Some("Background sky")
            );
        }
        state.unlock();
        assert_eq!(state.lock_label(true), None);
        assert_eq!(state.locked_body(), None);
    }

    #[test]
    fn sky_click_unlocks_immediately_but_jitter_does_not_pan() {
        let mut state = State::with_scene(sim::default_scene());
        state.view_lock = Some(ViewLock::Body(2));
        let angles = (state.observer.az, state.observer.alt);
        state.begin_sky_press((400.0, 300.0));
        // Travel can accumulate indefinitely without ever leaving the click
        // tolerance. It must not move the telescope or turn into a drag.
        for _ in 0..10 {
            state.drag_sky((402.0, 300.0), 1.0, 600);
            state.drag_sky((400.0, 300.0), 1.0, 600);
        }
        assert_eq!((state.observer.az, state.observer.alt), angles);
        assert_eq!(state.view_lock, None);
        assert!(state.end_sky_press());

        state.begin_sky_press((400.0, 300.0));
        state.drag_sky((407.0, 300.0), 2.0, 600);
        assert_eq!(
            (state.observer.az, state.observer.alt),
            angles,
            "threshold scales with DPI"
        );
        state.drag_sky((410.0, 300.0), 2.0, 600);
        assert_eq!(state.view_lock, None);
        assert!(
            (state.observer.az - (angles.0 - 10.0 * state.observer.fov_y / 600.0)).abs() < 1e-12
        );
        state.drag_sky((400.0, 300.0), 2.0, 600);
        assert!(
            !state.end_sky_press(),
            "returning to the press is still a drag"
        );
        state.drag_sky((500.0, 400.0), 1.0, 600);
        assert!(!state.end_sky_press(), "cancelled gestures cannot click");
    }

    #[test]
    fn picking_tiny_bodies_is_stable_and_respects_occlusion() {
        let mut state = State::with_scene(sim::default_scene());
        state.point_at(2);
        assert_eq!(state.pick_body(-1.0, 300.0, 800, 600), None);
        assert_eq!(state.pick_body(400.0, 300.0, 0, 600), None);
        // Co-located geometry must pick the larger, nearer surface, not the
        // last matching body in the scene.
        let mut inner = state.scene.bodies[2].clone();
        inner.radius *= 0.5;
        state.scene.bodies.push(inner);
        assert_eq!(state.pick_body(400.0, 300.0, 800, 600), Some(2));
        state.scene.bodies.pop();
        state.scene.bodies[2].radius = 1e-12;
        assert_eq!(state.pick_body(400.0, 300.0, 800, 600), Some(2));
        state.observer.az += 1e-8;
        assert_eq!(state.pick_body(400.0, 300.0, 800, 600), None);

        // The host surface blocks targets below the horizon, even though it
        // is deliberately not selectable.
        state.scene.bodies[2].radius = 1_737.4 / sim::AU_KM;
        for step in 0..100 {
            state.sim_time = step as f64 * 0.01;
            state.point_at(2);
            if state.observer.alt < -0.1 {
                assert_eq!(
                    state.pick_body(400.0, 300.0, 800, 600),
                    Some(state.scene.host)
                );
                state.lock_at(400.0, 300.0, 800, 600);
                assert_eq!(state.view_lock, None, "ground must not lock background sky");
                return;
            }
        }
        panic!("test did not find a below-horizon Moon");
    }

    #[test]
    fn tracking_at_zenith_has_a_finite_camera_basis() {
        let mut state = State::with_scene(sim::default_scene());
        state.observer.alt = std::f64::consts::FRAC_PI_2;
        let positions = state.scene.positions(state.sim_time);
        let vf = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        assert!(vf.right.is_finite() && vf.up.is_finite());
        assert!(vf.right.dot(vf.forward).abs() < 1e-12);
        assert!((vf.right.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn reset_uses_the_scenes_default_target_and_releases_tracking() {
        let mut state = State::with_scene(sim::binary_scene());
        state.view_lock = Some(ViewLock::Body(1));
        state.reset_view();
        assert_eq!(state.view_lock, None);
        let positions = state.scene.positions(state.sim_time);
        let vf = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        let dir = (positions[state.scene.default_target] - vf.position).normalize();
        assert!(vf.forward.dot(dir) > 0.999_999);
    }

    #[test]
    fn eclipse_search_finds_something() {
        let mut state = State::with_scene(sim::default_scene());
        let what = state.next_eclipse();
        assert!(what.is_some(), "no eclipse found");
        let t = state.sim_time;
        assert!(t > 0.0);
    }

    #[test]
    fn transit_search_finds_a_visible_moon_in_front_of_its_planet() {
        let mut state = State::with_scene(sim::default_scene());
        assert!(state.next_moon_transit().is_some(), "no moon transit found");
        let p = state.scene.positions(state.sim_time);
        let vf = state.observer.frame(&state.scene, state.sim_time, &p);
        // With the expanded scene the first event may be a Galilean moon,
        // not Phobos. Verify the returned view against actual geometry.
        let visible_transit = state.scene.bodies.iter().enumerate().any(|(i, moon)| {
            let Some(parent) = moon.parent else {
                return false;
            };
            if moon.kind != BodyKind::Moon || parent == state.scene.host {
                return false;
            }
            let d_planet = p[parent] - vf.position;
            let d_moon = p[i] - vf.position;
            let angular_radius = (state.scene.body(parent).radius / d_planet.length()).asin();
            d_planet.normalize().dot(vf.forward) > 0.999_999
                && d_planet.normalize().dot(vf.zenith) > 0.2
                && d_moon.length() < d_planet.length()
                && d_planet.normalize().angle_between(d_moon.normalize()) < angular_radius
        });
        assert!(visible_transit, "view must center the transited planet");
        assert!(vf.sun_altitude < -0.1, "it should be night");
    }
}
