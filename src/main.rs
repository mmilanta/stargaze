//! stargaze — an observatory on a planet, looking out at a solar system.

mod camera;
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

/// Simulation speeds in days per real second.
const WARPS: [f64; 8] = [0.001, 0.005, 0.02, 0.1, 0.5, 2.0, 10.0, 40.0];
const START_WARP: usize = 2;
/// Telescope magnification limits and the per-notch zoom factor.
const MIN_FOV_DEG: f64 = 0.001;
const MAX_FOV_DEG: f64 = 90.0;
const ZOOM_STEP: f64 = 0.78;
/// Exposure-scale normalization: a white Lambertian surface facing a unit
/// luminosity star at 1 AU has outgoing radiance 2.5. The same stellar radiance
/// is used for camera hits AND illumination (no independent bright-disc gain).
const SUN_LIGHT: f64 = 2.5;

/// Which system to load at startup. Defaults to the fictitious binary system;
/// set `STARGAZE_SYSTEM=solar` for the original solar system.
fn scene_for_startup() -> Scene {
    match std::env::var("STARGAZE_SYSTEM").ok().as_deref() {
        Some("solar") | Some("sol") => sim::default_scene(),
        _ => sim::binary_scene(),
    }
}

struct State {
    scene: Scene,
    observer: Observer,
    sim_time: f64,
    warp: usize,
    paused: bool,
    last: Instant,
    dragging: bool,
    last_cursor: Option<(f64, f64)>,
    /// Body the view is locked onto, if any. While locked the telescope keeps
    /// pointing at it even as simulation time advances.
    tracked: Option<usize>,
    /// Cursor position where the current left-button press began (sky only).
    press: Option<(f64, f64)>,
    /// Accumulated cursor travel since the press, in physical pixels.
    drag_distance: f64,
}

impl State {
    fn new() -> Self {
        Self::with_scene(scene_for_startup())
    }

    fn with_scene(scene: Scene) -> Self {
        let mut observer = Observer::new(scene.host);
        observer.lat = scene.observer_lat_deg.to_radians();
        observer.lon = scene.observer_lon_deg.to_radians();
        let mut state = Self {
            scene,
            observer,
            sim_time: 0.0,
            warp: START_WARP,
            paused: true,
            last: Instant::now(),
            dragging: false,
            last_cursor: None,
            tracked: None,
            press: None,
            drag_distance: 0.0,
        };
        state.find_good_start();
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
        if std::env::var("STARGAZE_FIND_PHOBOS").is_ok()
            && let Some(what) = state.next_phobos_event()
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
        self.tracked = None;
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
        self.tracked = None;
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let dir = (positions[target] - vf.position).normalize();
        self.observer.alt = dir.dot(vf.zenith).asin();
        self.observer.az = dir.dot(vf.east).atan2(dir.dot(vf.north));
        self.observer.fov_y = fov.clamp(MIN_FOV_DEG.to_radians(), MAX_FOV_DEG.to_radians());
    }

    /// Fast-forward to the next time a moon transits the planet it orbits, and
    /// frame the planet closely. Phobos on Mars in the solar system; one of
    /// Calyx's or Vantus's moons in the binary.
    fn next_phobos_event(&mut self) -> Option<&'static str> {
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
                if planet.kind != BodyKind::Planet {
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
        for i in 1..60_000 {
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

    /// Aim at the k-th entry of the scene's target list (number keys).
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
        let mut max_ang = body.radius / dist;
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
        if !self.paused {
            self.sim_time += dt * WARPS[self.warp];
        }
    }

    fn reset_view(&mut self) {
        self.aim_at(self.scene.default_target);
    }

    /// Keep the telescope pointed at the locked body as time advances.
    fn track(&mut self) {
        let Some(target) = self.tracked else {
            return;
        };
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let dir = (positions[target] - vf.position).normalize();
        self.observer.alt = dir.dot(vf.zenith).asin();
        self.observer.az = dir.dot(vf.east).atan2(dir.dot(vf.north));
    }

    /// Return the nearest body under a physical-pixel cursor position, if any.
    ///
    /// Unlike the label pass this is a true ray/sphere test, so overlapping
    /// bodies resolve to whichever is actually in front.
    fn pick_body(&self, x: f64, y: f64, width: u32, height: u32) -> Option<usize> {
        if width == 0 || height == 0 {
            return None;
        }
        let positions = self.scene.positions(self.sim_time);
        let vf = self.observer.frame(&self.scene, self.sim_time, &positions);
        let aspect = width as f64 / height as f64;
        let tan_half = (self.observer.fov_y * 0.5).tan();
        let ndc_x = 2.0 * x / width as f64 - 1.0;
        let ndc_y = 1.0 - 2.0 * y / height as f64;
        // Ray in telescope space (forward is -Z), then rotated to world space.
        let view = DVec3::new(ndc_x * tan_half * aspect, ndc_y * tan_half, -1.0).normalize();
        let dir = (vf.right * view.x + vf.up * view.y - vf.forward * view.z).normalize();
        // The host body is kept as an occluder so a click on the ground cannot
        // select a body hidden on the far side of the planet, but it is never
        // itself selectable.
        let mut best: Option<(f64, usize)> = None;
        for (i, body) in self.scene.bodies.iter().enumerate() {
            if body.kind == BodyKind::Anchor || body.radius <= 0.0 {
                continue;
            }
            let oc = positions[i] - vf.position;
            let along = oc.dot(dir);
            if along <= 0.0 {
                continue;
            }
            let perpendicular2 = oc.length_squared() - along * along;
            let r2 = body.radius * body.radius;
            if perpendicular2 > r2 {
                continue;
            }
            let distance = along - (r2 - perpendicular2).sqrt();
            if distance > 0.0 && best.is_none_or(|(bd, _)| distance < bd) {
                best = Some((distance, i));
            }
        }
        match best {
            Some((_, i)) if i != self.scene.host => Some(i),
            _ => None,
        }
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
            });
        }

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
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    stars: Vec<CatalogueStar>,
    last_title: String,
    scale: f32,
    cursor: (f64, f64),
    show_labels: bool,
    show_hud: bool,
}

impl App {
    fn ui_layout(&self) -> Option<ui::Layout> {
        let r = self.renderer.as_ref()?;
        let (w, h) = r.size();
        if w == 0 || h == 0 {
            return None;
        }
        Some(ui::layout(w as f32, h as f32, self.scale))
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
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.state.advance();
                self.state.track();
                if let (Some(r), Some(window)) = (self.renderer.as_mut(), self.window.as_ref()) {
                    let (w, h) = r.size();
                    if w > 0 && h > 0 {
                        let mut frame = self.state.build_frame(w, h);
                        if self.show_hud {
                            let layout = ui::layout(w as f32, h as f32, self.scale);
                            ui::build(
                                &mut frame.ui,
                                &layout,
                                w as f32,
                                h as f32,
                                ui::LabelOptions {
                                    show: self.show_labels,
                                    locked: self.state.tracked,
                                },
                                self.state.sim_time,
                                &frame.labels,
                            );
                        }
                        r.render(&frame);
                    }
                    let rate = if self.state.paused {
                        "paused".to_string()
                    } else {
                        format!("{:.3} day/s", WARPS[self.state.warp])
                    };
                    let lock = match self.state.tracked {
                        Some(i) => format!("  ·  locked: {}", self.state.scene.body(i).name),
                        None => String::new(),
                    };
                    let title = format!(
                        "stargaze  ·  t = {:.2} d ({:.3} yr)  ·  {}  ·  {} spp{lock}  ·  click body=lock  drag=look  scroll=zoom  space=play/pause  [ ]=speed  R=reset  E=eclipse  T=phobos  H=hud  L=labels",
                        self.state.sim_time,
                        self.state.sim_time / 365.256,
                        rate,
                        r.samples(),
                    );
                    if title != self.last_title {
                        window.set_title(&title);
                        self.last_title = title;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button != MouseButton::Left {
                    return;
                }
                match state {
                    ElementState::Pressed => {
                        let (cx, cy) = self.cursor;
                        let mut consumed = false;
                        if self.show_hud
                            && let Some(layout) = self.ui_layout()
                        {
                            if layout.toggle.contains(cx, cy) {
                                self.show_labels = !self.show_labels;
                                consumed = true;
                            } else {
                                for (rect, (_, delta)) in
                                    layout.buttons.iter().zip(ui::TIME_BUTTONS.iter())
                                {
                                    if rect.contains(cx, cy) {
                                        self.state.sim_time += delta;
                                        consumed = true;
                                        break;
                                    }
                                }
                                if !consumed && layout.bar.contains(cx, cy) {
                                    consumed = true;
                                }
                            }
                        }
                        if consumed {
                            self.state.press = None;
                            self.state.dragging = false;
                            self.state.last_cursor = None;
                        } else {
                            self.state.press = Some((cx, cy));
                            self.state.drag_distance = 0.0;
                            self.state.dragging = true;
                            self.state.last_cursor = Some((cx, cy));
                        }
                    }
                    ElementState::Released => {
                        self.state.dragging = false;
                        self.state.last_cursor = None;
                        // A press that barely moved is a click: lock the body
                        // under the cursor (or unlock when the sky was hit).
                        if self.state.press.take().is_some()
                            && self.state.drag_distance <= 4.0 * self.scale as f64
                        {
                            let picked = self.renderer.as_ref().and_then(|r| {
                                let (w, h) = r.size();
                                self.state.pick_body(self.cursor.0, self.cursor.1, w, h)
                            });
                            self.state.tracked = picked;
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = (position.x, position.y);
                self.cursor = pos;
                if self.state.dragging
                    && let Some(prev) = self.state.last_cursor
                {
                    let dx = pos.0 - prev.0;
                    let dy = pos.1 - prev.1;
                    self.state.drag_distance += (dx * dx + dy * dy).sqrt();
                    if self.state.drag_distance > 4.0 * self.scale as f64 {
                        // A real drag becomes free look, releasing any lock so
                        // the tracking does not fight the cursor.
                        self.state.tracked = None;
                    }
                    // Grab-the-sky dragging: the content follows the cursor,
                    // at a rate set by the current field of view, so zooming
                    // also changes how fast looking feels.
                    let height = self
                        .renderer
                        .as_ref()
                        .map(|r| r.size().1)
                        .unwrap_or(1)
                        .max(1) as f64;
                    let rad_per_px = self.state.observer.fov_y / height;
                    self.state.observer.az -= dx * rad_per_px;
                    self.state.observer.alt += dy * rad_per_px;
                    self.state.observer.alt = self
                        .state
                        .observer
                        .alt
                        .clamp(-5.0_f64.to_radians(), 89.0_f64.to_radians());
                }
                self.state.last_cursor = Some(pos);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 50.0,
                };
                let fov = self.state.observer.fov_y * ZOOM_STEP.powf(y);
                self.state.observer.fov_y =
                    fov.clamp(MIN_FOV_DEG.to_radians(), MAX_FOV_DEG.to_radians());
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::Space) => {
                        self.state.paused = !self.state.paused;
                    }
                    PhysicalKey::Code(KeyCode::BracketRight) => {
                        self.state.warp = (self.state.warp + 1).min(WARPS.len() - 1);
                        self.state.paused = false;
                    }
                    PhysicalKey::Code(KeyCode::BracketLeft) => {
                        self.state.warp = self.state.warp.saturating_sub(1);
                        self.state.paused = false;
                    }
                    PhysicalKey::Code(KeyCode::KeyR) => self.state.reset_view(),
                    PhysicalKey::Code(KeyCode::KeyE) => {
                        if let Some(what) = self.state.next_eclipse() {
                            log::info!("found {what} at t = {:.3} d", self.state.sim_time);
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyT) => {
                        if let Some(what) = self.state.next_phobos_event() {
                            log::info!("found {what} at t = {:.3} d", self.state.sim_time);
                        }
                    }
                    PhysicalKey::Code(KeyCode::KeyH) => self.show_hud = !self.show_hud,
                    PhysicalKey::Code(KeyCode::KeyL) => self.show_labels = !self.show_labels,
                    PhysicalKey::Code(KeyCode::Digit1) => self.state.aim_target(0),
                    PhysicalKey::Code(KeyCode::Digit2) => self.state.aim_target(1),
                    PhysicalKey::Code(KeyCode::Digit3) => self.state.aim_target(2),
                    PhysicalKey::Code(KeyCode::Digit4) => self.state.aim_target(3),
                    PhysicalKey::Code(KeyCode::Digit5) => self.state.aim_target(4),
                    PhysicalKey::Code(KeyCode::Digit6) => self.state.aim_target(5),
                    PhysicalKey::Code(KeyCode::Digit7) => self.state.aim_target(6),
                    PhysicalKey::Code(KeyCode::Digit8) => self.state.aim_target(7),
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

    let stars = stars::generate(3500, 2500, 0x5EED_1234);
    log::info!("generated {} background stars", stars.len());

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        state: State::new(),
        window: None,
        renderer: None,
        stars,
        last_title: String::new(),
        scale: 1.0,
        cursor: (0.0, 0.0),
        show_labels: true,
        show_hud: true,
    };

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
    if std::env::var("STARGAZE_DEBUG").is_ok() {
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
        // A far-off corner is empty sky (the host planet is never pickable).
        assert_eq!(state.pick_body(1.0, 1.0, 800, 600), None);

        // Lock it, advance time, and re-track: the body stays centred.
        state.tracked = Some(2);
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
    fn eclipse_search_finds_something() {
        let mut state = State::with_scene(sim::default_scene());
        let what = state.next_eclipse();
        assert!(what.is_some(), "no eclipse found");
        let t = state.sim_time;
        assert!(t > 0.0);
    }

    #[test]
    fn phobos_search_finds_a_transit_in_front_of_mars() {
        let mut state = State::with_scene(sim::default_scene());
        let what = state.next_phobos_event();
        assert!(what.is_some(), "no Phobos transit found");

        // Re-derive the geometry at the chosen time: Phobos must be nearer
        // than Mars (a transit) and on Mars's disc.
        let p = state.scene.positions(state.sim_time);
        let vf = state.observer.frame(&state.scene, state.sim_time, &p);
        let (mars, phobos) = (3usize, 4usize);
        let d_mars = p[mars] - vf.position;
        let d_phobos = p[phobos] - vf.position;
        assert!(
            d_phobos.length() < d_mars.length(),
            "Phobos should be in front of Mars"
        );
        let sep = d_mars.normalize().angle_between(d_phobos.normalize());
        let mr = (state.scene.body(mars).radius / d_mars.length()).asin();
        assert!(sep < mr, "Phobos should be on Mars's disc");
        assert!(vf.sun_altitude < 0.0, "it should be night");
    }
}
