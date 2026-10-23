//! Versioned, human-editable scene files. All external distances have explicit units.
use crate::sim::{
    AU_KM, Atmosphere, Body, BodyKind, Elements, Rings, Scene, Surface, SurfaceStyle,
};
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
    /// Multiplier on the preset's density (Rayleigh and Mie); 1 if omitted.
    #[serde(default)]
    atmosphere_density: Option<f64>,
    #[serde(default)]
    ground: crate::ground::Theme,
    #[serde(default)]
    direction: Option<Direction>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Direction {
    azimuth_deg: f64,
    altitude_deg: f64,
}
fn camera_height() -> f64 {
    2.0
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AtmosphereDef {
    Earthlike,
    Dense,
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
    #[serde(default)]
    surface: Option<SurfaceDef>,
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
    TidalLock {
        /// Lock to this moon (a direct child) instead of to the parent.
        #[serde(default)]
        to: Option<String>,
    },
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

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum StyleDef {
    GasGiant,
    Cratered,
    Rocky,
    Icy,
    Volcanic,
    Terran,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceDef {
    style: StyleDef,
    #[serde(default)]
    seed: u32,
    #[serde(default, alias = "belt_albedo")]
    secondary_albedo: Option<[f32; 3]>,
    #[serde(default)]
    bands: Option<f32>,
    #[serde(default)]
    turbulence: Option<f32>,
    #[serde(default)]
    storms: Option<u32>,
    #[serde(default)]
    craters: Option<f32>,
    #[serde(default)]
    ice_caps: Option<f32>,
    #[serde(default)]
    ocean: Option<f32>,
    #[serde(default)]
    clouds: Option<f32>,
}

impl SurfaceDef {
    fn build(&self, albedo: [f32; 3]) -> Result<Surface> {
        let style = match self.style {
            StyleDef::GasGiant => SurfaceStyle::GasGiant,
            StyleDef::Cratered => SurfaceStyle::Cratered,
            StyleDef::Rocky => SurfaceStyle::Rocky,
            StyleDef::Icy => SurfaceStyle::Icy,
            StyleDef::Volcanic => SurfaceStyle::Volcanic,
            StyleDef::Terran => SurfaceStyle::Terran,
        };
        let gas = style == SurfaceStyle::GasGiant;
        let terran = style == SurfaceStyle::Terran;
        ensure!(
            gas || (self.bands.is_none() && self.turbulence.is_none() && self.storms.is_none()),
            "surface bands, turbulence and storms apply only to gas_giant"
        );
        ensure!(
            !gas || (self.craters.is_none() && self.ice_caps.is_none()),
            "surface craters and ice_caps do not apply to gas_giant"
        );
        ensure!(
            terran || (self.ocean.is_none() && self.clouds.is_none()),
            "surface ocean and clouds apply only to terran"
        );
        // Default second colours keep the body's hue where that is natural:
        // darker, more saturated belts, maria and dark regions.
        let peak = albedo.into_iter().fold(1.0e-6_f32, f32::max);
        let deeper = |scale: f32, power: f32| albedo.map(|c| scale * c * (c / peak).powf(power));
        let secondary_albedo = self.secondary_albedo.unwrap_or(match style {
            SurfaceStyle::GasGiant => deeper(0.8, 1.6),
            SurfaceStyle::Cratered => deeper(0.55, 0.6),
            SurfaceStyle::Rocky => deeper(0.6, 1.0),
            SurfaceStyle::Icy => [albedo[0] * 0.75, albedo[1] * 0.5, albedo[2] * 0.35],
            SurfaceStyle::Volcanic => [albedo[0] * 0.85, albedo[1] * 0.45, albedo[2] * 0.2],
            SurfaceStyle::Terran => [0.03, 0.09, 0.2],
        });
        rgb(secondary_albedo, "surface secondary_albedo")?;
        let unit = |value: Option<f32>, default: f32, name: &str| -> Result<f32> {
            let v = value.unwrap_or(default);
            ensure!(
                v.is_finite() && (0.0..=1.0).contains(&v),
                "surface {name} must be between 0 and 1"
            );
            Ok(v)
        };
        let bands = self.bands.unwrap_or(7.0);
        ensure!(
            bands.is_finite() && (1.0..=30.0).contains(&bands),
            "surface bands must be between 1 and 30"
        );
        let storms = self.storms.unwrap_or(1);
        ensure!(storms <= 3, "surface storms must be at most 3");
        let (craters, ice_caps) = match style {
            SurfaceStyle::GasGiant => (0.0, 0.0),
            SurfaceStyle::Cratered => (0.8, 0.0),
            SurfaceStyle::Rocky => (0.5, 0.06),
            SurfaceStyle::Icy => (0.15, 0.0),
            SurfaceStyle::Volcanic => (0.5, 0.0),
            SurfaceStyle::Terran => (0.0, 0.1),
        };
        Ok(Surface {
            style,
            seed: self.seed,
            secondary_albedo,
            bands,
            turbulence: unit(self.turbulence, 0.5, "turbulence")?,
            storms,
            craters: unit(self.craters, craters, "craters")?,
            ice_caps: unit(self.ice_caps, ice_caps, "ice_caps")?,
            ocean: unit(self.ocean, 0.65, "ocean")?,
            clouds: unit(self.clouds, 0.4, "clouds")?,
        })
    }
}

/// A style's default surface for a body of the given albedo, exactly as a
/// config file naming only the style would produce.
pub fn default_surface(style: SurfaceStyle, albedo: [f32; 3]) -> Surface {
    SurfaceDef {
        style: match style {
            SurfaceStyle::GasGiant => StyleDef::GasGiant,
            SurfaceStyle::Cratered => StyleDef::Cratered,
            SurfaceStyle::Rocky => StyleDef::Rocky,
            SurfaceStyle::Icy => StyleDef::Icy,
            SurfaceStyle::Volcanic => StyleDef::Volcanic,
            SurfaceStyle::Terran => StyleDef::Terran,
        },
        seed: 0,
        secondary_albedo: None,
        bands: None,
        turbulence: None,
        storms: None,
        craters: None,
        ice_caps: None,
        ocean: None,
        clouds: None,
    }
    .build(albedo)
    .expect("style defaults are valid")
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
        let mut locks = Vec::new();
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
            if let Rotation::TidalLock { to: Some(moon) } = &def.rotation {
                locks.push((bodies.len(), def.id.clone(), moon.clone()));
            }
            indices.insert(def.id, bodies.len());
            bodies.push(body);
        }
        // Locks to a moon resolve once every body (and so the moon) exists.
        for (index, id, moon) in locks {
            let target = *indices
                .get(&moon)
                .with_context(|| format!("body '{id}': unknown tidal_lock moon '{moon}'"))?;
            ensure!(
                bodies[target].parent == Some(index),
                "body '{id}': tidal_lock to '{moon}' requires '{moon}' to orbit '{id}'"
            );
            bodies[index].lock_to = Some((bodies[target].elements, bodies[target].period_days));
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
            c.atmosphere_density
                .is_none_or(|d| d.is_finite() && (0.0..=10.0).contains(&d)),
            "camera.atmosphere_density must be between 0 and 10"
        );
        ensure!(
            c.fov_deg.is_finite() && (0.001..=90.0).contains(&c.fov_deg),
            "camera.fov_deg must be in [0.001, 90]"
        );
        if let Some(direction) = &c.direction {
            ensure!(
                direction.azimuth_deg.is_finite() && (0.0..360.0).contains(&direction.azimuth_deg),
                "camera.direction.azimuth_deg must be in [0, 360)"
            );
            ensure!(
                direction.altitude_deg.is_finite()
                    && (-90.0..=90.0).contains(&direction.altitude_deg),
                "camera.direction.altitude_deg must be in [-90, 90]"
            );
        }
        Ok(Scene {
            bodies,
            host,
            default_target,
            targets,
            atmosphere: c.atmosphere.map(|preset| {
                match preset {
                    AtmosphereDef::Earthlike => Atmosphere::earthlike(),
                    AtmosphereDef::Dense => Atmosphere::dense(),
                }
                .tuned(c.atmosphere_density.unwrap_or(1.0), None)
            }),
            ground: c.ground,
            default_fov_deg: c.fov_deg,
            initial_direction_deg: c.direction.map(|d| [d.azimuth_deg, d.altitude_deg]),
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
            Rotation::TidalLock { ref to } => {
                ensure!(
                    parent.is_some() || to.is_some(),
                    "tidal_lock requires an orbit and parent, or a moon to lock to"
                );
                (elements.inc, period_days, elements.m0, true)
            }
        };
        let rings = self
            .rings
            .as_ref()
            .map(|r| -> Result<Rings> {
                ensure!(
                    matches!(kind, BodyKind::Star | BodyKind::Planet | BodyKind::Moon),
                    "rings require a star, planet or moon"
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
        let surface = self
            .surface
            .as_ref()
            .map(|d| -> Result<Surface> {
                ensure!(
                    matches!(kind, BodyKind::Planet | BodyKind::Moon),
                    "surface requires a planet or moon"
                );
                d.build(self.albedo)
            })
            .transpose()?;
        Ok(Body {
            surface,
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
            lock_to: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HALO: &str = include_str!("../configs/legacy/halo.yaml");
    const SOLAR: &str = include_str!("../configs/legacy/solar-system.yaml");
    const CORONA: &str = include_str!("../configs/3-corona-valley.yaml");

    #[test]
    fn gas_giant_surface_defaults_and_validation() {
        let scene = parse(CORONA).unwrap();
        let tormenta = scene.bodies.iter().find(|b| b.name == "Tormenta").unwrap();
        let surface = tormenta.surface.unwrap();
        assert_eq!(surface.style, SurfaceStyle::GasGiant);
        assert_eq!((surface.seed, surface.storms), (21, 3));
        // Default belts: same hue as the zones, darker and more saturated.
        let (zone, belt) = (tormenta.albedo, surface.secondary_albedo);
        assert!((0..3).all(|c| belt[c] < zone[c]));
        assert!(belt[2] / belt[0] < zone[2] / zone[0]);
        let line = "surface: {style: gas_giant, seed: 21, bands: 8, turbulence: 0.7, storms: 3}";
        assert!(CORONA.contains(line));
        for bad in [
            "surface: {style: gas_giant, bands: 0}",
            "surface: {style: gas_giant, turbulence: 2}",
            "surface: {style: gas_giant, storms: 4}",
            "surface: {style: gas_giant, belt_albedo: [2, 0, 0]}",
            "surface: {style: marble}",
            "surface: {style: gas_giant, colour: 1}",
            "surface: {style: gas_giant, craters: 0.5}",
        ] {
            assert!(parse(&CORONA.replace(line, bad)).is_err(), "{bad}");
        }
        // Stars cannot take a surface style.
        let star = CORONA.replacen(
            "    rotation: {mode: free, period_days: 9, obliquity_deg: 25}",
            "    rotation: {mode: free, period_days: 9, obliquity_deg: 25}\n    surface: {style: gas_giant}",
            1,
        );
        assert!(parse(&star).is_err());
    }

    #[test]
    fn solid_surface_styles_defaults_and_field_checks() {
        let scene = parse(CORONA).unwrap();
        let surface = |name: &str| {
            scene
                .bodies
                .iter()
                .find(|b| b.name == name)
                .unwrap()
                .surface
                .unwrap()
        };
        let ocre = surface("Ocre");
        assert_eq!(ocre.style, SurfaceStyle::Rocky);
        assert!(ocre.craters > 0.0 && ocre.ice_caps > 0.0);
        assert_eq!(surface("Chispa").style, SurfaceStyle::Cratered);
        assert_eq!(surface("Ceniza").style, SurfaceStyle::Volcanic);
        assert_eq!(surface("Alba").style, SurfaceStyle::Icy);
        let puna = surface("Puna");
        assert_eq!(puna.style, SurfaceStyle::Terran);
        assert!(puna.ocean > 0.5 && puna.clouds > 0.0);
        // The host keeps the plain albedo; its ground is the local terrain.
        assert!(scene.body(scene.host).surface.is_none());
        let line = "surface: {style: rocky, seed: 1}";
        assert!(CORONA.contains(line));
        assert!(
            parse(&CORONA.replace(line, "surface: {style: rocky, craters: 1, ice_caps: 0}"))
                .is_ok()
        );
        for bad in [
            "surface: {style: rocky, ocean: 0.5}",
            "surface: {style: rocky, clouds: 0.5}",
            "surface: {style: rocky, bands: 4}",
            "surface: {style: rocky, storms: 1}",
            "surface: {style: rocky, craters: 1.5}",
            "surface: {style: rocky, ice_caps: -0.1}",
        ] {
            assert!(parse(&CORONA.replace(line, bad)).is_err(), "{bad}");
        }
        let terran = "surface: {style: terran, seed: 7}";
        assert!(
            parse(&CORONA.replace(terran, "surface: {style: terran, ocean: 0.3, clouds: 0}"))
                .is_ok()
        );
        assert!(parse(&CORONA.replace(terran, "surface: {style: terran, ocean: 2}")).is_err());
    }

    #[test]
    fn gemma_and_gemmi_are_mutually_tidally_locked() {
        const ERG: &str = include_str!("../configs/2-erg-dunes.yaml");
        let scene = parse(ERG).unwrap();
        let id = |name: &str| scene.bodies.iter().position(|b| b.name == name).unwrap();
        let (gemma, gemmi) = (id("Gemma"), id("Gemmi"));
        for t in [0.0, 0.7, 1.6, 2.9, 40.0] {
            let p = scene.positions(t);
            let towards = (p[gemmi] - p[gemma]).normalize();
            // Gemma's longitude 0 faces Gemmi; Gemmi's longitude 180 faces Gemma.
            let gemma_x = scene.body(gemma).spin_quat(t) * glam::DVec3::X;
            let gemmi_x = scene.body(gemmi).spin_quat(t) * glam::DVec3::X;
            assert!(gemma_x.dot(towards) > 0.9999, "t={t}");
            assert!(gemmi_x.dot(towards) > 0.9999, "t={t}");
        }
        let lock = "rotation: {mode: tidal_lock, to: gemmi}";
        assert!(ERG.contains(lock));
        // The target must exist and orbit the locked body.
        assert!(parse(&ERG.replace(lock, "rotation: {mode: tidal_lock, to: nowhere}")).is_err());
        assert!(parse(&ERG.replace(lock, "rotation: {mode: tidal_lock, to: simoom}")).is_err());
    }

    #[test]
    fn corona_level_opens_before_puna_occults_the_ringed_star_at_night() {
        let scene = parse(CORONA).unwrap();
        assert_eq!(scene.ground, crate::ground::Theme::Desert);
        let id = |name: &str| scene.bodies.iter().position(|b| b.name == name).unwrap();
        let (corona, brasa, puna) = (id("Corona"), id("Brasa"), id("Puna"));
        assert_eq!(scene.body(corona).kind, BodyKind::Star);
        assert!(
            scene.body(corona).rings.is_some(),
            "the main star is ringed"
        );
        assert!(!scene.body(scene.host).tidally_locked);
        assert_eq!(scene.body(scene.host).parent, Some(brasa));
        assert_eq!(scene.body(puna).parent, Some(brasa));
        let start = scene.initial_time_days.unwrap();
        let state = crate::State::with_scene(parse(CORONA).unwrap());
        // Within two hours of the start, Puna fully covers Corona's disc while
        // Corona is above the western ridges and Brasa is below the horizon.
        let total = (0..2400).map(|i| start + i as f64 * 0.00005).find(|&t| {
            let p = scene.positions(t);
            let vf = state.observer.frame(&scene, t, &p);
            let (dc, dp) = (p[corona] - vf.position, p[puna] - vf.position);
            let rc = (scene.body(corona).radius / dc.length()).asin();
            let rp = (scene.body(puna).radius / dp.length()).asin();
            let up = |d: glam::DVec3| d.normalize().dot(vf.zenith).asin().to_degrees();
            dc.angle_between(dp) < rp - rc && up(dc) > 20.0 && up(p[brasa] - vf.position) < -10.0
        });
        assert!(total.is_some(), "no total occultation after day {start}");
    }
    #[test]
    fn desert_level_has_middle_free_moon_submoon_and_visible_wanderers() {
        let scene = parse(include_str!("../configs/legacy/atacama.yaml")).unwrap();
        assert_eq!(scene.ground, crate::ground::Theme::Desert);
        assert_eq!(parse(HALO).unwrap().ground, crate::ground::Theme::Forest);
        assert!(
            parse(&HALO.replace(
                "  atmosphere: earthlike",
                "  ground: unknown\n  atmosphere: earthlike"
            ))
            .is_err()
        );
        let host = scene.body(scene.host);
        assert!(!host.tidally_locked);
        assert_ne!(host.spin_period, host.period_days);
        let giant = host.parent.unwrap();
        let mut moons: Vec<_> = scene
            .bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.parent == Some(giant))
            .collect();
        moons.sort_by(|a, b| a.1.elements.a.total_cmp(&b.1.elements.a));
        assert_eq!(moons.len(), 3);
        assert_eq!(moons[1].0, scene.host);
        let ring = scene.body(giant).rings.unwrap();
        assert!(ring.outer_radius < 1.5);
        assert!(
            ring.outer_radius * scene.body(giant).radius
                < moons[0].1.elements.a - moons[0].1.radius
        );
        let submoons: Vec<_> = scene
            .bodies
            .iter()
            .filter(|b| b.parent == Some(scene.host))
            .collect();
        assert_eq!(submoons.len(), 1);
        assert!(submoons[0].radius < host.radius * 0.1);
        let others: Vec<_> = scene
            .bodies
            .iter()
            .enumerate()
            .filter(|(i, b)| *i != giant && b.kind == BodyKind::Planet)
            .collect();
        assert_eq!(others.len(), 3);
        assert_eq!(
            others
                .iter()
                .filter(|(_, b)| b.elements.a < scene.body(giant).elements.a)
                .count(),
            1
        );
        let s = crate::State::with_scene(scene);
        let positions = s.scene.positions(s.sim_time);
        let view = s.observer.frame(&s.scene, s.sim_time, &positions);
        assert!(
            (0.2..0.4).contains(&view.sun_altitude),
            "morning light should reveal the road"
        );
        for (i, b) in s
            .scene
            .bodies
            .iter()
            .enumerate()
            .filter(|(i, b)| *i != giant && b.kind == BodyKind::Planet)
        {
            assert!(
                (positions[i] - view.position).normalize().dot(view.zenith) > 0.2,
                "{} below arrival horizon",
                b.name
            );
            assert_eq!(b.luminosity, 0.0);
        }
        let frame = s.build_frame(960, 540);
        assert_eq!(frame.globals.ground_counts[2], 1);
        assert!(
            frame
                .ground
                .iter()
                .all(|p| p.center[3] < 0.0 || p.center[3] == crate::ground::TUMBLED_ROCK)
        );
    }
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
    fn bundled_levels_have_atmospheres_and_airless_configs_still_load() {
        for yaml in [
            HALO,
            SOLAR,
            include_str!("../configs/legacy/puzzle.yaml"),
            include_str!("../configs/legacy/median-resonance.yaml"),
            include_str!("../configs/legacy/vesper.yaml"),
        ] {
            assert!(parse(yaml).unwrap().atmosphere.is_some());
        }
        let dense = parse(include_str!("../configs/legacy/median-resonance.yaml"))
            .unwrap()
            .atmosphere
            .unwrap();
        assert!(dense.mie > Atmosphere::earthlike().mie);
        let airless = HALO.replace("  atmosphere: earthlike\n", "");
        assert!(parse(&airless).unwrap().atmosphere.is_none());
        assert!(parse(&HALO.replace("earthlike", "unknown")).is_err());
    }
    #[test]
    fn target_lists_can_span_multiple_number_key_pages() {
        let mut system: System = serde_saphyr::from_str(SOLAR).unwrap();
        system.targets = system
            .bodies
            .iter()
            .filter(|body| body.id != system.camera.body && !matches!(body.kind, Kind::Anchor))
            .map(|body| body.id.clone())
            .collect();
        let count = system.targets.len();
        assert!(count > 10);
        let scene = system.into_scene().unwrap();
        assert_eq!(scene.targets.len(), count);
        assert!(
            scene
                .targets
                .iter()
                .all(|&i| i < scene.bodies.len() && i != scene.host)
        );
    }

    #[test]
    fn validates_optional_camera_direction() {
        assert!(parse(HALO).unwrap().initial_direction_deg.is_none());
        assert_eq!(
            parse(include_str!("../configs/legacy/vesper.yaml"))
                .unwrap()
                .initial_direction_deg,
            Some([0.0, 25.0])
        );
        for azimuth in [-1.0, 360.0, f64::NAN] {
            rejected(
                |s| {
                    s.camera.direction = Some(Direction {
                        azimuth_deg: azimuth,
                        altitude_deg: 25.0,
                    })
                },
                "direction.azimuth_deg",
            );
        }
        for altitude in [-91.0, 91.0, f64::INFINITY] {
            rejected(
                |s| {
                    s.camera.direction = Some(Direction {
                        azimuth_deg: 0.0,
                        altitude_deg: altitude,
                    })
                },
                "direction.altitude_deg",
            );
        }
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
