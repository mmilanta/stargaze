//! Minimal analytic (stateless) solar-system simulation.
//!
//! Every body stores Keplerian orbital elements relative to its parent. The
//! position at any time is computed on demand from those elements, so time is
//! fully scrubbable and nothing drifts.

use glam::{DMat3, DQuat, DVec3};

#[path = "solar.rs"]
mod solar;

/// Kilometres per astronomical unit.
pub const AU_KM: f64 = 149_597_870.7;

#[derive(Clone, Copy, Debug)]
pub struct Elements {
    /// Semi-major axis (AU).
    pub a: f64,
    /// Eccentricity.
    pub e: f64,
    /// Inclination (rad).
    pub inc: f64,
    /// Longitude of the ascending node (rad).
    pub node: f64,
    /// Argument of periapsis (rad).
    pub peri: f64,
    /// Mean anomaly at `epoch` (rad).
    pub m0: f64,
    /// Reference epoch (days).
    pub epoch: f64,
}

impl Elements {
    pub fn circular(a: f64, m0: f64) -> Self {
        Self {
            a,
            e: 0.0,
            inc: 0.0,
            node: 0.0,
            peri: 0.0,
            m0,
            epoch: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Invisible point that other bodies orbit; emits no light, casts none.
    Anchor,
    Star,
    Planet,
    Moon,
}

/// An equatorial, infinitesimally thin particulate disc. Radii are multiples
/// of the parent body's radius; optical depth is measured normal to the plane.
#[derive(Clone, Copy, Debug)]
pub struct Rings {
    pub inner_radius: f64,
    pub outer_radius: f64,
    pub optical_depth: f32,
    pub albedo: [f32; 3],
    /// Saturn-like C/B/A bands, Cassini division and narrow outer gaps.
    /// Disable for a uniform disc (also useful for transport tests).
    pub banded: bool,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub name: String,
    pub parent: Option<usize>,
    pub elements: Elements,
    /// Orbital period around the parent (days). Each body owns its period, so
    /// systems can be tuned freely; it is not derived from the parent's mass.
    /// Unused (0) for the root body.
    pub period_days: f64,
    /// Physical radius (AU), used for ray intersections and light transport.
    pub radius: f64,
    pub kind: BodyKind,
    /// Surface albedo for lit bodies.
    pub albedo: [f32; 3],
    /// Radiated colour and strength for stars.
    pub emission: [f32; 3],
    pub luminosity: f32,
    /// Axial tilt (rad) and sidereal spin period (days).
    pub obliquity: f64,
    pub spin_period: f64,
    pub spin_phase: f64,
    pub rings: Option<Rings>,
    /// Align spin with the orbital plane and synchronize it with mean anomaly.
    pub tidally_locked: bool,
}

impl Body {
    /// Position relative to the parent body (AU).
    pub fn relative_position(&self, t: f64) -> DVec3 {
        let n = std::f64::consts::TAU / self.period_days;
        let m = self.elements.m0 + n * (t - self.elements.epoch);
        let e = self.elements.e;

        let ea = solve_kepler(m, e);
        let r = self.elements.a * (1.0 - e * ea.cos());
        let half = ea * 0.5;
        let nu = 2.0 * (((1.0 + e).sqrt() * half.sin()).atan2((1.0 - e).sqrt() * half.cos()));

        let in_plane = DVec3::new(r * nu.cos(), r * nu.sin(), 0.0);
        let rot = DMat3::from_rotation_z(self.elements.node)
            * DMat3::from_rotation_x(self.elements.inc)
            * DMat3::from_rotation_z(self.elements.peri);
        rot * in_plane
    }

    /// Heliocentric (or root-relative) position. `positions` must already
    /// contain the parent's world position.
    pub fn world_position(&self, t: f64, positions: &[DVec3]) -> DVec3 {
        match self.parent {
            Some(i) => positions[i] + self.relative_position(t),
            None => DVec3::ZERO,
        }
    }

    /// Orientation of the body's fixed frame in world space.
    pub fn spin_quat(&self, t: f64) -> DQuat {
        if self.tidally_locked {
            let orbit = DQuat::from_rotation_z(self.elements.node)
                * DQuat::from_rotation_x(self.elements.inc)
                * DQuat::from_rotation_z(self.elements.peri);
            let phase = self.elements.m0
                + std::f64::consts::TAU * (t - self.elements.epoch) / self.period_days;
            return orbit * DQuat::from_rotation_z(phase);
        }
        let theta = self.spin_phase + std::f64::consts::TAU * (t / self.spin_period);
        // Body-local Z is the spin axis. Tilt that entire frame relative to
        // ecliptic north; rotating around a tilted axis without tilting the
        // frame makes latitude drift and gives the wrong day/night cycle.
        DQuat::from_rotation_y(self.obliquity) * DQuat::from_rotation_z(theta)
    }
}

/// Solve `E - e sin E = M` with Newton's method.
fn solve_kepler(m: f64, e: f64) -> f64 {
    let m = m.rem_euclid(std::f64::consts::TAU);
    let mut ea = if e < 0.8 { m } else { std::f64::consts::PI };
    for _ in 0..16 {
        let f = ea - e * ea.sin() - m;
        let fp = 1.0 - e * ea.cos();
        let step = f / fp;
        ea -= step;
        if step.abs() < 1e-13 {
            break;
        }
    }
    ea
}

/// A body's atmosphere: single-scattering Rayleigh + Mie.
#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    /// Height at which density falls to 1/e (AU).
    pub scale_height: f64,
    /// Top of the modelled atmosphere above the surface (AU).
    pub thickness: f64,
    /// Rayleigh scattering coefficients at the surface, per RGB (1/AU).
    pub rayleigh: [f64; 3],
    /// Mie scattering coefficient at the surface (1/AU).
    pub mie: f64,
    /// Henyey-Greenstein asymmetry of the Mie lobe (0 isotropic, ~0.76 haze).
    pub mie_g: f64,
}

impl Atmosphere {
    /// Earth-like Rayleigh scattering plus a light forward-scattering haze.
    pub fn earthlike() -> Self {
        // Convert sea-level coefficients from 1/m to 1/AU.
        let au_metres = AU_KM * 1_000.0;
        let per_au = |beta: f64| beta * au_metres;
        Self {
            scale_height: 8_500.0 / au_metres,
            thickness: 80_000.0 / au_metres,
            rayleigh: [per_au(5.8e-6), per_au(13.5e-6), per_au(33.1e-6)],
            mie: per_au(21.0e-6),
            mie_g: 0.76,
        }
    }
}

/// A star system plus the index of the body the telescope stands on.
pub struct Scene {
    pub bodies: Vec<Body>,
    /// The body the observatory is attached to (a planet, or a moon).
    pub host: usize,
    /// Atmosphere of the host body, if it has one.
    pub atmosphere: Option<Atmosphere>,
    /// Body to aim at on startup.
    pub default_target: usize,
    /// Field of view, in degrees, for the startup view.
    pub default_fov_deg: f64,
    /// Bodies the number keys jump to, in order.
    pub targets: Vec<usize>,
    /// Observatory latitude and longitude on the host body (degrees).
    pub observer_lat_deg: f64,
    pub observer_lon_deg: f64,
    pub observer_height_m: f64,
    pub initial_time_days: Option<f64>,
}

impl Scene {
    /// World positions of every body at time `t` (days).
    pub fn positions(&self, t: f64) -> Vec<DVec3> {
        let mut out = vec![DVec3::ZERO; self.bodies.len()];
        for (i, body) in self.bodies.iter().enumerate() {
            out[i] = body.world_position(t, &out);
        }
        out
    }

    pub fn body(&self, index: usize) -> &Body {
        &self.bodies[index]
    }

    pub fn star_indices(&self) -> Vec<usize> {
        self.bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BodyKind::Star)
            .map(|(i, _)| i)
            .collect()
    }
}

/// True-scale solar system, observed from Earth.
pub fn default_scene() -> Scene {
    solar::scene()
}

/// Solar system observed from Saturn at 20 degrees north.
pub fn saturn_scene() -> Scene {
    solar::saturn_observatory()
}

/// A fictitious binary system.
///
/// Two stars orbit their common barycentre — `Aur`, a large yellow star, and
/// `Igni`, a smaller, redder one. Three planets circle the pair, with one, two
/// and three moons. The observatory stands on `Calyx`, watching its two moons,
/// `Halo` and `Umbra`, rise and set as the planet rotates.
pub fn binary_scene() -> Scene {
    let deg = std::f64::consts::PI / 180.0;
    let star = |name: &str,
                a: f64,
                m0: f64,
                radius_au: f64,
                emission: [f32; 3],
                luminosity: f32,
                spin: f64| Body {
        name: name.into(),
        parent: Some(0),
        elements: Elements {
            a,
            e: 0.0,
            inc: 0.0,
            node: 0.0,
            peri: 0.0,
            m0,
            epoch: 0.0,
        },
        period_days: 120.0,
        radius: radius_au,
        kind: BodyKind::Star,
        albedo: [0.0; 3],
        emission,
        luminosity,
        obliquity: 0.0,
        spin_period: spin,
        spin_phase: 0.0,
        tidally_locked: false,
        rings: None,
    };

    let bodies = vec![
        // 0 — the barycentre the two stars and every planet orbit.
        Body {
            name: "Barycentre".into(),
            parent: None,
            elements: Elements::circular(0.0, 0.0),
            period_days: 0.0,
            radius: 0.0,
            kind: BodyKind::Anchor,
            albedo: [0.0; 3],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 0.0,
            spin_period: 1.0,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 1 — Aur: larger, yellow. The radii are inflated for a thicker disc
        // while `luminosity` (the total light output) is unchanged: emitted
        // radiance scales as luminosity / radius^2, so a bigger sphere is
        // dimmer per unit area, preserving far-field irradiance.
        star("Aur", 0.30, 0.0, 0.030, [1.0, 0.96, 0.86], 2.0, 22.0),
        // 2 — Igni: smaller and redder.
        star(
            "Igni",
            0.50,
            std::f64::consts::PI,
            0.018,
            [1.0, 0.42, 0.20],
            0.7,
            30.0,
        ),
        // 3 — Nerid, one moon.
        Body {
            name: "Nerid".into(),
            parent: Some(0),
            elements: Elements {
                a: 2.6,
                e: 0.03,
                inc: 1.5 * deg,
                node: 0.4,
                peri: 1.1,
                m0: 2.0,
                epoch: 0.0,
            },
            period_days: 1530.0,
            radius: 5_734.0 / AU_KM,
            kind: BodyKind::Planet,
            albedo: [0.45, 0.55, 0.65],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 18.0 * deg,
            spin_period: 0.85,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 4 — Pip (moon of Nerid).
        Body {
            name: "Pip".into(),
            parent: Some(3),
            elements: Elements {
                a: 2.2e-4,
                e: 0.01,
                inc: 2.0 * deg,
                node: 0.0,
                peri: 0.5,
                m0: 1.0,
                epoch: 0.0,
            },
            period_days: 2.5,
            radius: 1_200.0 / AU_KM,
            kind: BodyKind::Moon,
            albedo: [0.60, 0.60, 0.62],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 2.0 * deg,
            spin_period: 2.5,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 5 — Calyx: the two-mooned planet and observatory host.
        Body {
            name: "Calyx".into(),
            parent: Some(0),
            elements: Elements {
                a: 4.2,
                e: 0.02,
                inc: 1.0 * deg,
                node: 2.0,
                peri: 3.0,
                m0: 4.5,
                epoch: 0.0,
            },
            period_days: 3000.0,
            radius: 9_560.0 / AU_KM,
            kind: BodyKind::Planet,
            albedo: [0.72, 0.60, 0.42],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 22.0 * deg,
            spin_period: 0.60,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 6 — Halo: inner moon of Calyx and the startup target.
        Body {
            name: "Halo".into(),
            parent: Some(5),
            elements: Elements {
                a: 1.6e-4,
                e: 0.002,
                inc: 0.5 * deg,
                node: 0.0,
                peri: 0.0,
                m0: 0.0,
                epoch: 0.0,
            },
            period_days: 3.5,
            radius: 1_500.0 / AU_KM,
            kind: BodyKind::Moon,
            albedo: [0.55, 0.57, 0.60],
            emission: [0.0; 3],
            luminosity: 0.0,
            // Nearly ecliptic-normal spin for the synchronous inner moon.
            obliquity: 0.5 * deg,
            spin_period: 3.5,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 7 — Umbra: outer moon of Calyx.
        Body {
            name: "Umbra".into(),
            parent: Some(5),
            elements: Elements {
                a: 3.2e-4,
                e: 0.01,
                inc: 1.5 * deg,
                node: 1.0,
                peri: 2.0,
                m0: 3.0,
                epoch: 0.0,
            },
            period_days: 8.0,
            radius: 1_000.0 / AU_KM,
            kind: BodyKind::Moon,
            albedo: [0.35, 0.33, 0.32],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 1.5 * deg,
            spin_period: 8.0,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 8 — Vantus, three moons.
        Body {
            name: "Vantus".into(),
            parent: Some(0),
            elements: Elements {
                a: 6.8,
                e: 0.05,
                inc: 2.5 * deg,
                node: 1.5,
                peri: 0.8,
                m0: 1.2,
                epoch: 0.0,
            },
            period_days: 6200.0,
            radius: 14_016.0 / AU_KM,
            kind: BodyKind::Planet,
            albedo: [0.35, 0.42, 0.55],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 12.0 * deg,
            spin_period: 0.40,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        // 9, 10, 11 — Iri, Kesh, Coll (moons of Vantus).
        Body {
            name: "Iri".into(),
            parent: Some(8),
            elements: Elements {
                a: 1.8e-4,
                e: 0.005,
                inc: 1.0 * deg,
                node: 0.0,
                peri: 1.0,
                m0: 0.5,
                epoch: 0.0,
            },
            period_days: 5.0,
            radius: 800.0 / AU_KM,
            kind: BodyKind::Moon,
            albedo: [0.50, 0.50, 0.55],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 1.0 * deg,
            spin_period: 5.0,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        Body {
            name: "Kesh".into(),
            parent: Some(8),
            elements: Elements {
                a: 3.0e-4,
                e: 0.01,
                inc: 2.0 * deg,
                node: 0.5,
                peri: 2.0,
                m0: 2.5,
                epoch: 0.0,
            },
            period_days: 11.0,
            radius: 600.0 / AU_KM,
            kind: BodyKind::Moon,
            albedo: [0.45, 0.40, 0.35],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 2.0 * deg,
            spin_period: 11.0,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
        Body {
            name: "Coll".into(),
            parent: Some(8),
            elements: Elements {
                a: 4.5e-4,
                e: 0.02,
                inc: 3.0 * deg,
                node: 0.2,
                peri: 4.0,
                m0: 5.0,
                epoch: 0.0,
            },
            period_days: 18.0,
            radius: 500.0 / AU_KM,
            kind: BodyKind::Moon,
            albedo: [0.30, 0.32, 0.38],
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: 3.0 * deg,
            spin_period: 18.0,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        },
    ];

    Scene {
        bodies,
        host: 5, // Calyx
        atmosphere: Some(Atmosphere::earthlike()),
        default_target: 6, // Halo
        default_fov_deg: 20.0,
        targets: vec![6, 1, 2, 7, 3, 8, 4, 9],
        observer_lat_deg: 35.0,
        observer_lon_deg: 0.0,
        observer_height_m: 2.0,
        initial_time_days: None,
    }
}

/// Halo's near-side observatory, facing ringed Calyx in the binary system.
pub fn halo_scene() -> Scene {
    crate::config::parse(include_str!("../configs/halo.yaml"))
        .expect("bundled Halo configuration must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(scene: &Scene, name: &str) -> usize {
        scene
            .bodies
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("no body named {name}"))
    }

    #[test]
    fn earthlike_atmosphere_uses_consistent_units() {
        let a = Atmosphere::earthlike();
        let au_metres = 149_597_870_700.0;
        assert!((a.scale_height * au_metres - 8_500.0).abs() < 1e-9);
        assert!((a.thickness * au_metres - 80_000.0).abs() < 1e-9);
        // Vertical sea-level optical depth is dimensionless and order unity,
        // not the hundreds produced by mixing kilometres and metres.
        let optical_depth = (a.rayleigh[2] + a.mie) * a.scale_height;
        assert!((optical_depth - 0.45985).abs() < 1e-6);
    }

    #[test]
    fn moons_remain_outside_their_parents() {
        for scene in [default_scene(), binary_scene()] {
            for moon in scene.bodies.iter().filter(|b| b.kind == BodyKind::Moon) {
                let parent = scene.body(moon.parent.unwrap());
                assert!(moon.elements.a * (1.0 - moon.elements.e) > parent.radius + moon.radius);
            }
        }
    }

    #[test]
    fn solar_system_has_real_scales_and_all_observable_planets() {
        let scene = default_scene();
        assert_eq!(scene.body(scene.host).name, "Earth");
        assert_eq!(
            scene
                .bodies
                .iter()
                .filter(|b| b.kind == BodyKind::Planet)
                .count(),
            8
        );
        assert_eq!(
            scene
                .bodies
                .iter()
                .filter(|b| b.kind == BodyKind::Moon)
                .count(),
            10
        );
        for (name, radius_km, distance_au, period_days) in [
            ("Mercury", 2_439.7, 0.38709927, 87.9691),
            ("Venus", 6_051.8, 0.72333566, 224.701),
            ("Earth", 6_371.0, 1.00000261, 365.256),
            ("Mars", 3_389.5, 1.52371034, 686.980),
            ("Jupiter", 69_911.0, 5.202887, 4_332.589),
            ("Saturn", 58_232.0, 9.53667594, 10_759.22),
            ("Uranus", 25_362.0, 19.18916464, 30_688.5),
            ("Neptune", 24_622.0, 30.06992276, 60_182.0),
        ] {
            let i = index(&scene, name);
            let body = scene.body(i);
            assert!((body.radius * AU_KM - radius_km).abs() < 1e-6);
            assert!((body.elements.a - distance_au).abs() < 1e-9);
            assert!((body.period_days - period_days).abs() < 1e-6);
            assert_eq!(scene.targets.contains(&i), i != scene.host);
            let start = body.relative_position(0.0);
            let end = body.relative_position(body.period_days);
            assert!(
                (start - end).length() < 1e-10,
                "{name}: orbit failed to close"
            );
        }
        assert_eq!(scene.targets.len(), 9);
        assert!((scene.body(index(&scene, "Phobos")).elements.a * AU_KM - 9_376.0).abs() < 1e-6);
        for (i, body) in scene.bodies.iter().enumerate() {
            if let Some(parent) = body.parent {
                assert!(parent < i, "parents must precede children");
            }
        }
        for t in [-365.25, 0.0, 365.25, 60_182.0] {
            assert!(scene.positions(t).iter().all(|p| p.is_finite()));
        }
    }

    #[test]
    fn spin_keeps_latitude_fixed_and_respects_axial_tilt() {
        let scene = default_scene();
        for body in &scene.bodies {
            let expected_pole = DQuat::from_rotation_y(body.obliquity) * DVec3::Z;
            let latitude = 35_f64.to_radians();
            let local = DVec3::new(latitude.cos(), 0.0, latitude.sin());
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let q = body.spin_quat(body.spin_period * fraction);
                assert!((q * DVec3::Z - expected_pole).length() < 1e-12);
                assert!(((q * local).dot(expected_pole) - latitude.sin()).abs() < 1e-12);
            }
        }
        for name in ["Venus", "Uranus"] {
            assert!((scene.body(index(&scene, name)).spin_quat(0.0) * DVec3::Z).z < 0.0);
        }
        for (name, inclination) in [
            ("Phobos", 1.075_f64),
            ("Titania", 0.079),
            ("Triton", 156.865),
        ] {
            let moon = scene.body(index(&scene, name));
            let planet = scene.body(moon.parent.unwrap());
            let pole = planet.spin_quat(0.0) * DVec3::Z;
            let normal = DMat3::from_rotation_z(moon.elements.node)
                * DMat3::from_rotation_x(moon.elements.inc)
                * DVec3::Z;
            assert!((pole.dot(normal) - inclination.to_radians().cos()).abs() < 1e-12);
        }
    }

    #[test]
    fn find_eclipse_times() {
        use crate::camera::Observer;
        let scene = default_scene();
        let obs = Observer::new(scene.host);
        let terra_r = scene.bodies[1].radius;
        let luna_r = scene.bodies[2].radius;
        let sun_r = scene.bodies[0].radius;

        let (mut solar, mut lunar) = (Vec::new(), Vec::new());
        let step = 0.01;
        let mut t = 0.0;
        while t < 1500.0 {
            let p = scene.positions(t);
            let vf = obs.frame(&scene, t, &p);

            // Solar eclipse, visible only while the Sun is up.
            let dm = (p[2] - vf.position).normalize();
            let ds = (p[0] - vf.position).normalize();
            let sep = dm.angle_between(ds);
            let rm = (luna_r / (p[2] - vf.position).length()).asin();
            let rs = (sun_r / (p[0] - vf.position).length()).asin();
            if vf.sun_altitude > 0.1 && sep < rm + rs {
                solar.push((t, sep / (rm + rs), rm / rs));
            }

            // Lunar eclipse, visible only while the Moon is up.
            let l = (p[0] - p[2]).normalize();
            let oc = p[1] - p[2];
            let tca = oc.dot(l);
            if tca > 0.0 && dm.dot(vf.zenith) > 0.1 {
                let perp = (oc.length_squared() - tca * tca).max(0.0).sqrt();
                if perp < terra_r {
                    lunar.push((t, perp / terra_r));
                }
            }
            t += step;
        }

        let best = |v: &[(f64, f64, f64)]| {
            v.iter()
                .cloned()
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        };
        let best_lunar = lunar
            .iter()
            .cloned()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        println!("SOLAR windows visible: {}", solar.len());
        println!("  deepest (t, sep/(rm+rs), rm/rs): {:?}", best(&solar));
        println!("LUNAR windows visible: {}", lunar.len());
        println!("  deepest (t, perp/Rterra): {best_lunar:?}");

        assert!(!solar.is_empty(), "no solar eclipse in 1500 days");
        assert!(!lunar.is_empty(), "no lunar eclipse in 1500 days");
        // With the real Earth radius, a deep solar eclipse is not guaranteed
        // at this particular observatory within four years.
        assert!(best(&solar).unwrap().1 < 1.0, "no solar-disc overlap");
        assert!(best_lunar.unwrap().1 < 0.8, "no deep (total) lunar eclipse");
    }

    #[test]
    fn find_phobos_events() {
        use crate::camera::Observer;
        let scene = default_scene();
        let obs = Observer::new(scene.host);
        let mars = 3usize;
        let phobos = 4usize;
        let mars_r = scene.bodies[mars].radius;

        let (mut transits, mut shadowed) = (Vec::new(), Vec::new());
        let step = 0.002; // days (~3 min; Phobos orbits in 0.319 d)
        let mut t = 0.0;
        while t < 60.0 {
            let p = scene.positions(t);
            let vf = obs.frame(&scene, t, &p);
            let d_mars = p[mars] - vf.position;
            let d_phobos = p[phobos] - vf.position;
            let mr = (mars_r / d_mars.length()).asin();
            let sep = d_mars.normalize().angle_between(d_phobos.normalize());
            let mars_up = d_mars.normalize().dot(vf.zenith) > 0.2;
            let night = vf.sun_altitude < -0.1;
            if mars_up && night {
                if sep < mr && d_phobos.length() < d_mars.length() {
                    transits.push((t, sep / mr));
                }
                // Phobos inside Mars's shadow.
                let l = (p[0] - p[phobos]).normalize();
                let oc = p[mars] - p[phobos];
                let tca = oc.dot(l);
                if tca > 0.0 {
                    let perp = (oc.length_squared() - tca * tca).max(0.0).sqrt();
                    if perp < mars_r {
                        shadowed.push((t, perp / mars_r));
                    }
                }
            }
            t += step;
        }
        println!(
            "Phobos transits of Mars (visible): {}  first {:?}",
            transits.len(),
            transits.first()
        );
        println!(
            "Phobos eclipsed by Mars (visible):   {}  first {:?}",
            shadowed.len(),
            shadowed.first()
        );
        assert!(
            !transits.is_empty() || !shadowed.is_empty(),
            "no Phobos/Mars events"
        );
    }

    #[test]
    fn earth_and_mars_have_distinct_periods() {
        let scene = default_scene();
        let terra = scene.bodies[index(&scene, "Earth")].period_days;
        let mars = scene.bodies[index(&scene, "Mars")].period_days;
        assert!((terra - 365.256).abs() < 0.01, "terra period {terra}");
        assert!((mars - 686.98).abs() < 0.5, "mars period {mars}");
        assert!((mars - terra).abs() > 100.0, "periods must differ");
    }

    #[test]
    fn mars_closes_its_orbit_only_after_its_own_period() {
        let scene = default_scene();
        let mars = index(&scene, "Mars");
        let start = scene.positions(0.0)[mars];

        // One Mars period later it is essentially back where it began...
        let after_mars = scene.positions(686.980)[mars];
        assert!(
            (after_mars - start).length() < 0.02,
            "Mars did not close its orbit"
        );

        // ...but one Terra period later it is nowhere near.
        let after_terra = scene.positions(365.256)[mars];
        assert!(
            (after_terra - start).length() > 0.3,
            "Mars appears to share Terra's period"
        );
    }

    #[test]
    fn binary_scene_is_well_formed() {
        let scene = binary_scene();

        // Two stars, both orbiting the barycentre and visibly different colours.
        let stars = scene.star_indices();
        assert_eq!(stars.len(), 2);
        for &s in &stars {
            assert_eq!(scene.body(s).parent, Some(0));
        }
        let (a, b) = (scene.body(stars[0]), scene.body(stars[1]));
        assert!(
            a.emission != b.emission,
            "the two suns should differ in colour"
        );

        // The observatory stands on Calyx, which has exactly two moons.
        let planet = scene.host;
        assert_eq!(scene.body(planet).name, "Calyx");
        assert_eq!(scene.body(planet).kind, BodyKind::Planet);
        let moons: Vec<usize> = (0..scene.bodies.len())
            .filter(|&i| {
                scene.bodies[i].parent == Some(planet) && scene.bodies[i].kind == BodyKind::Moon
            })
            .collect();
        assert_eq!(moons.len(), 2, "Calyx should have two moons");

        // Halo is the startup target; the host is not an aiming shortcut.
        let target = scene.default_target;
        assert_eq!(scene.body(target).name, "Halo");
        assert!(moons.contains(&target));
        assert_eq!(scene.targets[0], target);
        assert!(!scene.targets.contains(&scene.host));

        // Three planets, six moons in total.
        let planets = scene
            .bodies
            .iter()
            .filter(|b| b.kind == BodyKind::Planet)
            .count();
        let moon_count = scene
            .bodies
            .iter()
            .filter(|b| b.kind == BodyKind::Moon)
            .count();
        assert_eq!(planets, 3);
        assert_eq!(moon_count, 6);
    }
}
