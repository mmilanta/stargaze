//! Versioned, human-editable scene files. All external distances have explicit units.
use crate::sim::{AU_KM, Atmosphere, Body, BodyKind, Elements, Rings, Scene};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct System {
    version: u32,
    name: String,
    #[serde(default)]
    time_days: Option<f64>,
    camera: Camera,
    #[serde(default)]
    targets: Vec<String>,
    bodies: Vec<BodyDef>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Camera {
    body: String,
    latitude_deg: f64,
    longitude_deg: f64,
    #[serde(default = "camera_height")]
    height_m: f64,
    look_at: String,
    fov_deg: f64,
    #[serde(default)]
    atmosphere: Option<AtmosphereDef>,
}
fn camera_height() -> f64 {
    2.0
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AtmosphereDef {
    Earthlike,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Anchor,
    Star,
    Planet,
    Moon,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyDef {
    id: String,
    #[serde(default)]
    name: Option<String>,
    kind: Kind,
    #[serde(default)]
    parent: Option<String>,
    radius_km: f64,
    #[serde(default = "default_albedo")]
    albedo: [f32; 3],
    #[serde(default)]
    luminosity: f32,
    #[serde(default = "white")]
    emission: [f32; 3],
    #[serde(default)]
    orbit: Option<Orbit>,
    #[serde(default)]
    rotation: Rotation,
    #[serde(default)]
    rings: Option<RingDef>,
}
fn default_albedo() -> [f32; 3] {
    [0.5; 3]
}
fn white() -> [f32; 3] {
    [1.0; 3]
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Orbit {
    #[serde(default)]
    semi_major_axis_au: Option<f64>,
    #[serde(default)]
    semi_major_axis_km: Option<f64>,
    period_days: f64,
    #[serde(default)]
    eccentricity: f64,
    #[serde(default)]
    inclination_deg: f64,
    #[serde(default)]
    ascending_node_deg: f64,
    #[serde(default)]
    periapsis_deg: f64,
    #[serde(default)]
    mean_anomaly_deg: f64,
    #[serde(default)]
    epoch_days: f64,
}
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum Rotation {
    Free {
        period_days: f64,
        #[serde(default)]
        obliquity_deg: f64,
        #[serde(default)]
        phase_deg: f64,
    },
    TidalLock,
}
impl Default for Rotation {
    fn default() -> Self {
        Self::Free {
            period_days: 1.0,
            obliquity_deg: 0.0,
            phase_deg: 0.0,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RingDef {
    inner_radius: f64,
    outer_radius: f64,
    optical_depth: f32,
    albedo: [f32; 3],
    #[serde(default)]
    banded: bool,
}

pub fn load(path: impl AsRef<Path>) -> Result<Scene> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).with_context(|| format!("invalid system in {}", path.display()))
}
pub fn parse(text: &str) -> Result<Scene> {
    let system: System = serde_saphyr::from_str(text).context("YAML schema error")?;
    system.into_scene()
}
fn rgb(value: [f32; 3], field: &str) -> Result<()> {
    ensure!(
        value
            .iter()
            .all(|x| x.is_finite() && (0.0..=1.0).contains(x)),
        "{field} must contain three values in [0, 1]"
    );
    Ok(())
}
impl System {
    fn into_scene(self) -> Result<Scene> {
        ensure!(
            self.version == 1,
            "unsupported version {}; expected 1",
            self.version
        );
        ensure!(!self.name.trim().is_empty(), "system name cannot be empty");
        ensure!(
            (2..=256).contains(&self.bodies.len()),
            "a system needs 2..=256 bodies"
        );
        ensure!(
            self.time_days.is_none_or(f64::is_finite),
            "time_days must be finite"
        );
        let mut ids = HashSet::new();
        for b in &self.bodies {
            ensure!(
                !b.id.trim().is_empty() && ids.insert(b.id.clone()),
                "empty or duplicate body id '{}'",
                b.id
            );
        }
        ensure!(
            self.bodies.iter().filter(|b| b.parent.is_none()).count() == 1,
            "exactly one root body is required (use an anchor for a binary system)"
        );
        for b in &self.bodies {
            if let Some(parent) = &b.parent {
                ensure!(
                    ids.contains(parent),
                    "body '{}': unknown parent '{parent}'",
                    b.id
                );
            }
        }
        // Stable topological ordering lets authors place a moon before its planet.
        let mut pending = self.bodies;
        let mut indices = HashMap::new();
        let mut bodies: Vec<Body> = Vec::new();
        while !pending.is_empty() {
            let Some(next) = pending
                .iter()
                .position(|b| b.parent.as_ref().is_none_or(|id| indices.contains_key(id)))
            else {
                bail!(
                    "cyclic parent references: {}",
                    pending
                        .iter()
                        .map(|b| b.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            };
            let def = pending.remove(next);
            let body = def
                .build(&indices, &bodies)
                .with_context(|| format!("body '{}'", def.id))?;
            indices.insert(def.id, bodies.len());
            bodies.push(body);
        }
        ensure!(
            bodies.iter().any(|b| b.kind == BodyKind::Star
                && b.luminosity > 0.0
                && b.emission.iter().any(|x| *x > 0.0)),
            "at least one luminous star is required"
        );
        let lookup = |id: &str| {
            indices
                .get(id)
                .copied()
                .with_context(|| format!("unknown body '{id}'"))
        };
        let host = lookup(&self.camera.body).context("camera.body")?;
        ensure!(
            matches!(bodies[host].kind, BodyKind::Planet | BodyKind::Moon),
            "camera.body must be a planet or moon"
        );
        let default_target = lookup(&self.camera.look_at).context("camera.look_at")?;
        let check_target = |index: usize| -> Result<()> {
            ensure!(
                index != host && bodies[index].kind != BodyKind::Anchor,
                "target '{}' must be a visible body other than the camera host",
                bodies[index].name
            );
            Ok(())
        };
        check_target(default_target)?;
        ensure!(
            self.targets.len() <= 9,
            "targets supports at most nine number-key shortcuts"
        );
        let targets = if self.targets.is_empty() {
            vec![default_target]
        } else {
            self.targets
                .iter()
                .map(|id| {
                    let i = lookup(id)?;
                    check_target(i)?;
                    Ok(i)
                })
                .collect::<Result<Vec<_>>>()?
        };
        let c = self.camera;
        ensure!(
            c.latitude_deg.is_finite() && (-90.0..=90.0).contains(&c.latitude_deg),
            "camera.latitude_deg must be in [-90, 90]"
        );
        ensure!(
            c.longitude_deg.is_finite() && (-180.0..=180.0).contains(&c.longitude_deg),
            "camera.longitude_deg must be in [-180, 180]"
        );
        ensure!(
            c.height_m.is_finite() && c.height_m >= 0.1,
            "camera.height_m must be at least 0.1 metres"
        );
        ensure!(
            c.fov_deg.is_finite() && (0.001..=90.0).contains(&c.fov_deg),
            "camera.fov_deg must be in [0.001, 90]"
        );
        Ok(Scene {
            bodies,
            host,
            default_target,
            targets,
            atmosphere: c.atmosphere.map(|_| Atmosphere::earthlike()),
            default_fov_deg: c.fov_deg,
            observer_lat_deg: c.latitude_deg,
            observer_lon_deg: c.longitude_deg,
            observer_height_m: c.height_m,
            initial_time_days: self.time_days,
        })
    }
}
impl BodyDef {
    fn build(&self, indices: &HashMap<String, usize>, bodies: &[Body]) -> Result<Body> {
        let kind = match self.kind {
            Kind::Anchor => BodyKind::Anchor,
            Kind::Star => BodyKind::Star,
            Kind::Planet => BodyKind::Planet,
            Kind::Moon => BodyKind::Moon,
        };
        ensure!(
            self.radius_km.is_finite()
                && (if kind == BodyKind::Anchor {
                    self.radius_km == 0.0
                } else {
                    self.radius_km > 0.0
                }),
            "radius_km must be positive (zero for anchors)"
        );
        rgb(self.albedo, "albedo")?;
        rgb(self.emission, "emission")?;
        ensure!(
            self.luminosity.is_finite() && self.luminosity >= 0.0,
            "luminosity must be finite and nonnegative"
        );
        ensure!(
            kind == BodyKind::Star || self.luminosity == 0.0,
            "only stars may have luminosity"
        );
        let parent = self.parent.as_ref().map(|id| indices[id]);
        ensure!(
            parent.is_some() == self.orbit.is_some(),
            "parent and orbit must be provided together"
        );
        let (elements, period_days) = if let Some(o) = &self.orbit {
            let a = match (o.semi_major_axis_au, o.semi_major_axis_km) {
                (Some(a), None) => a,
                (None, Some(km)) => km / AU_KM,
                _ => {
                    bail!("orbit requires exactly one of semi_major_axis_au or semi_major_axis_km")
                }
            };
            ensure!(
                a.is_finite() && a > 0.0,
                "semi-major axis must be positive and finite"
            );
            ensure!(
                o.period_days.is_finite() && o.period_days > 0.0,
                "orbital period_days must be positive and finite"
            );
            ensure!(
                o.eccentricity.is_finite() && (0.0..1.0).contains(&o.eccentricity),
                "eccentricity must be in [0, 1)"
            );
            ensure!(
                [
                    o.inclination_deg,
                    o.ascending_node_deg,
                    o.periapsis_deg,
                    o.mean_anomaly_deg,
                    o.epoch_days
                ]
                .iter()
                .all(|x| x.is_finite()),
                "orbit angles and epoch must be finite"
            );
            ensure!(
                a * (1.0 - o.eccentricity)
                    > bodies[parent.unwrap()].radius + self.radius_km / AU_KM,
                "orbit intersects the parent at periapsis"
            );
            (
                Elements {
                    a,
                    e: o.eccentricity,
                    inc: o.inclination_deg.to_radians(),
                    node: o.ascending_node_deg.to_radians(),
                    peri: o.periapsis_deg.to_radians(),
                    m0: o.mean_anomaly_deg.to_radians(),
                    epoch: o.epoch_days,
                },
                o.period_days,
            )
        } else {
            (Elements::circular(0.0, 0.0), 0.0)
        };
        let (obliquity, spin_period, spin_phase, tidally_locked) = match self.rotation {
            Rotation::Free {
                period_days,
                obliquity_deg,
                phase_deg,
            } => {
                ensure!(
                    period_days.is_finite()
                        && period_days != 0.0
                        && obliquity_deg.is_finite()
                        && phase_deg.is_finite(),
                    "rotation period must be nonzero; rotation values must be finite"
                );
                (
                    obliquity_deg.to_radians(),
                    period_days,
                    phase_deg.to_radians(),
                    false,
                )
            }
            Rotation::TidalLock => {
                ensure!(parent.is_some(), "tidal_lock requires an orbit and parent");
                (elements.inc, period_days, elements.m0, true)
            }
        };
        let rings = self
            .rings
            .as_ref()
            .map(|r| -> Result<Rings> {
                ensure!(
                    matches!(kind, BodyKind::Planet | BodyKind::Moon),
                    "rings require a planet or moon"
                );
                ensure!(
                    r.inner_radius.is_finite()
                        && r.outer_radius.is_finite()
                        && r.inner_radius > 1.0
                        && r.outer_radius > r.inner_radius,
                    "ring radii must satisfy 1 < inner_radius < outer_radius"
                );
                ensure!(
                    r.optical_depth.is_finite() && r.optical_depth >= 0.0,
                    "ring optical_depth must be finite and nonnegative"
                );
                rgb(r.albedo, "ring albedo")?;
                Ok(Rings {
                    inner_radius: r.inner_radius,
                    outer_radius: r.outer_radius,
                    optical_depth: r.optical_depth,
                    albedo: r.albedo,
                    banded: r.banded,
                })
            })
            .transpose()?;
        Ok(Body {
            name: self.name.clone().unwrap_or_else(|| self.id.clone()),
            parent,
            elements,
            period_days,
            radius: self.radius_km / AU_KM,
            kind,
            albedo: self.albedo,
            emission: self.emission,
            luminosity: self.luminosity,
            obliquity,
            spin_period,
            spin_phase,
            rings,
            tidally_locked,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HALO: &str = include_str!("../configs/halo.yaml");
    const SOLAR: &str = include_str!("../configs/solar-system.yaml");
    fn definition() -> System {
        serde_saphyr::from_str(HALO).unwrap()
    }
    fn rejected(change: impl FnOnce(&mut System), expected: &str) {
        let mut system = definition();
        change(&mut system);
        let error = match system.into_scene() {
            Ok(_) => panic!("invalid config accepted"),
            Err(e) => format!("{e:#}"),
        };
        assert!(
            error.contains(expected),
            "{error:?} should contain {expected:?}"
        );
    }
    #[test]
    fn bundled_scenes_and_unordered_bodies_load() {
        let solar = parse(SOLAR).unwrap();
        assert_eq!(solar.bodies.len(), 19);
        assert_eq!(solar.body(solar.host).name, "Earth");
        let original = parse(HALO).unwrap();
        let mut def = definition();
        def.bodies.reverse();
        let reordered = def.into_scene().unwrap();
        for t in [0.0, 12.3, -15.0, 10_000.0] {
            let before = original.positions(t);
            let after = reordered.positions(t);
            for (i, b) in original.bodies.iter().enumerate() {
                let j = reordered
                    .bodies
                    .iter()
                    .position(|other| other.name == b.name)
                    .unwrap();
                assert!((before[i] - after[j]).length() < 1e-12);
            }
        }
        let halo = original.body(original.host);
        let calyx = original.body(original.default_target);
        assert!((halo.elements.a * AU_KM - 29919.57414).abs() < 1e-6);
        assert!(
            (halo.elements.a - halo.radius - calyx.radius * calyx.rings.unwrap().outer_radius)
                * AU_KM
                > 10_000.0
        );
    }
    #[test]
    fn rejects_bad_structure_and_references() {
        rejected(|s| s.version = 2, "unsupported version");
        rejected(
            |s| s.bodies[1].id = s.bodies[0].id.clone(),
            "duplicate body",
        );
        rejected(
            |s| s.bodies[6].parent = Some("missing".into()),
            "unknown parent",
        );
        rejected(
            |s| {
                s.bodies[5].parent = Some("halo".into());
            },
            "cyclic parent",
        );
        rejected(|s| s.camera.body = "missing".into(), "camera.body");
        rejected(
            |s| s.camera.look_at = s.camera.body.clone(),
            "other than the camera host",
        );
        rejected(|s| s.targets.push("missing".into()), "unknown body");
        assert!(parse(&format!("{HALO}\nmisspelled_setting: true\n")).is_err());
    }
    #[test]
    fn rejects_unsafe_numeric_values() {
        rejected(
            |s| s.bodies[6].orbit.as_mut().unwrap().eccentricity = 1.0,
            "eccentricity",
        );
        rejected(
            |s| s.bodies[6].orbit.as_mut().unwrap().period_days = 0.0,
            "orbital period",
        );
        rejected(
            |s| s.bodies[6].orbit.as_mut().unwrap().semi_major_axis_km = Some(100.0),
            "intersects the parent",
        );
        rejected(
            |s| s.bodies[6].orbit.as_mut().unwrap().semi_major_axis_au = Some(1.0),
            "exactly one",
        );
        rejected(|s| s.bodies[6].radius_km = f64::NAN, "radius_km");
        rejected(
            |s| s.bodies[5].rings.as_mut().unwrap().outer_radius = 1.0,
            "ring radii",
        );
        rejected(|s| s.bodies[5].albedo[0] = 2.0, "albedo");
        rejected(|s| s.camera.latitude_deg = 91.0, "latitude_deg");
        rejected(|s| s.camera.fov_deg = f64::INFINITY, "fov_deg");
        rejected(|s| s.camera.height_m = 0.0, "height_m");
        rejected(|s| s.time_days = Some(f64::NAN), "time_days");
    }
    #[test]
    fn tidal_rotation_aligns_with_arbitrary_orbit_and_epoch() {
        let mut def = definition();
        let orbit = def.bodies[6].orbit.as_mut().unwrap();
        orbit.inclination_deg = 47.0;
        orbit.ascending_node_deg = 123.0;
        orbit.periapsis_deg = 71.0;
        orbit.mean_anomaly_deg = 23.0;
        orbit.epoch_days = 52.0;
        def.time_days = Some(-42.0);
        def.camera.height_m = 25.0;
        let scene = def.into_scene().unwrap();
        let moon = scene.body(scene.host);
        for t in [-100.0, 0.0, 52.0, 1000.0] {
            let facing = moon.spin_quat(t) * -glam::DVec3::X;
            assert!(facing.dot(-moon.relative_position(t).normalize()) > 1.0 - 1e-12);
        }
        let state = crate::State::with_scene(scene);
        assert_eq!(state.sim_time, -42.0);
        assert_eq!(state.observer.height_m, 25.0);
        let positions = state.scene.positions(state.sim_time);
        let frame = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        let altitude = ((frame.position - positions[state.scene.host]).length()
            - state.scene.body(state.scene.host).radius)
            * AU_KM
            * 1000.0;
        assert!((altitude - 25.0).abs() < 0.001);
    }
}
