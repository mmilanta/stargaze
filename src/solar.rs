//! True-scale solar-system preset. Planet elements are approximate J2000
//! values (JPL's approximate planetary positions), held fixed rather than
//! propagated as an ephemeris: https://ssd.jpl.nasa.gov/planets/approx_pos.html
//! Radii, sidereal periods and moon distances are rounded physical values.
//! Satellite phases are illustrative; precession and perturbations are absent.

use glam::{DQuat, EulerRot};

use super::{AU_KM, Body, BodyKind, Elements, Scene};

struct Orbiter {
    name: &'static str,
    parent: usize,
    /// Semi-major axis (AU), eccentricity, then inclination, ascending node,
    /// argument of periapsis and mean anomaly (degrees).
    orbit: [f64; 6],
    /// Satellite elements relative to the parent's equator, not the ecliptic.
    equatorial: bool,
    period_days: f64,
    radius_km: f64,
    albedo: [f32; 3],
    tilt_deg: f64,
    spin_days: f64,
}

impl Orbiter {
    fn body(self, bodies: &[Body]) -> Body {
        let [a, e, inc, node, peri, anomaly] = self.orbit;
        let mut elements = Elements {
            a,
            e,
            inc: inc.to_radians(),
            node: node.to_radians(),
            peri: peri.to_radians(),
            m0: anomaly.to_radians(),
            epoch: 0.0,
        };
        if self.equatorial {
            // The same static pole tilt used for the parent's rotation.
            let orientation = DQuat::from_rotation_y(bodies[self.parent].obliquity)
                * DQuat::from_euler(EulerRot::ZXZ, elements.node, elements.inc, elements.peri);
            (elements.node, elements.inc, elements.peri) = orientation.to_euler(EulerRot::ZXZ);
        }
        Body {
            name: self.name.into(),
            parent: Some(self.parent),
            elements,
            period_days: self.period_days,
            radius: self.radius_km / AU_KM,
            kind: if self.parent == 0 {
                BodyKind::Planet
            } else {
                BodyKind::Moon
            },
            albedo: self.albedo,
            emission: [0.0; 3],
            luminosity: 0.0,
            obliquity: self.tilt_deg.to_radians(),
            spin_period: self.spin_days,
            spin_phase: 0.0,
            tidally_locked: false,
            rings: None,
        }
    }
}

pub fn scene() -> Scene {
    crate::config::parse(include_str!("../configs/solar-system.yaml"))
        .expect("bundled solar configuration must be valid")
}

/// Observatory at 20 degrees north on Saturn's modeled spherical surface.
pub fn saturn_observatory() -> Scene {
    let mut scene = scene();
    let saturn = 8;
    // Cassini estimate: orbit ~117,000 km; height ~300 m. As with the other
    // moons, use a spherical body and illustrative orbital phase. The orbit
    // is circular/equatorial and rotation is assumed synchronous.
    // https://science.nasa.gov/photojournal/a-small-find-near-equinox/
    let distance_km: f64 = 117_000.0;
    // Kepler period from Saturn's GM (km^3/s^2), JPL SAT441:
    // https://ssd.jpl.nasa.gov/sats/phys_par/sep.html
    let period_days =
        std::f64::consts::TAU * (distance_km.powi(3) / 37_931_206.23).sqrt() / 86_400.0;
    let mut moon = Orbiter {
        name: "S/2009 S 1",
        parent: saturn,
        orbit: [distance_km / AU_KM, 0.0, 0.0, 0.0, 0.0, 0.0],
        equatorial: true,
        period_days,
        radius_km: 0.15,
        albedo: [0.6, 0.57, 0.52],
        tilt_deg: scene.bodies[saturn].obliquity.to_degrees(),
        spin_days: period_days,
    }
    .body(&scene.bodies);
    // Longitude zero faces Saturn throughout the circular synchronous orbit.
    moon.spin_phase = std::f64::consts::PI;
    let moon_index = scene.bodies.len();
    scene.bodies.push(moon);
    scene.host = saturn;
    // Saturn's atmosphere is not modeled by the Earth-like scattering shell.
    scene.atmosphere = None;
    scene.default_target = moon_index;
    // Key 7 looks toward the inner rings instead of aiming into the host.
    scene.targets[6] = moon_index;
    scene.default_fov_deg = 80.0;
    scene.observer_lat_deg = 20.0;
    scene.observer_lon_deg = 0.0;
    scene
}
