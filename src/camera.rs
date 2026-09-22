//! The telescope: an observer anchored to a point on a planet's surface.

use glam::DVec3;

use crate::sim::Scene;

pub struct Observer {
    /// Index of the body we stand on.
    pub body: usize,
    /// Latitude / longitude on that body (rad).
    pub lat: f64,
    pub lon: f64,
    /// Alt-azimuth mount angles (rad). Azimuth is measured from local north,
    /// increasing towards local east.
    pub az: f64,
    pub alt: f64,
    /// Vertical field of view (rad).
    pub fov_y: f64,
    pub height_m: f64,
}

impl Observer {
    pub fn new(body: usize) -> Self {
        Self {
            body,
            height_m: 2.0,
            lat: 35.0_f64.to_radians(),
            lon: 0.0,
            az: 0.0,
            alt: 45.0_f64.to_radians(),
            fov_y: 10.0_f64.to_radians(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ViewFrame {
    pub position: DVec3,
    pub forward: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    pub zenith: DVec3,
    pub north: DVec3,
    pub east: DVec3,
    /// Sine of the primary star's altitude above the local horizon.
    pub sun_altitude: f64,
}

impl ViewFrame {
    /// Rotate a camera-relative vector into telescope space (forward is -Z).
    pub fn world_to_view(&self, relative: DVec3) -> DVec3 {
        DVec3::new(
            relative.dot(self.right),
            relative.dot(self.up),
            -relative.dot(self.forward),
        )
    }
}

impl Observer {
    pub fn frame(&self, scene: &Scene, t: f64, positions: &[DVec3]) -> ViewFrame {
        let host = scene.body(self.body);
        let center = positions[self.body];
        let q = host.spin_quat(t);

        let (slat, clat) = self.lat.sin_cos();
        let (slon, clon) = self.lon.sin_cos();

        // Surface frame expressed in the body's rotating frame (Z = spin axis).
        let up_local = DVec3::new(clat * clon, clat * slon, slat);
        let north_local = DVec3::new(-slat * clon, -slat * slon, clat);
        let east_local = DVec3::new(-slon, clon, 0.0);

        let zenith = (q * up_local).normalize();
        let north = q * north_local;
        let east = q * east_local;

        // Configured height above the actual surface. The host is now intersectable
        // geometry, so a camera exactly on its boundary is numerically unsafe.
        let observer_height_au = self.height_m / (crate::sim::AU_KM * 1000.0);
        let position = center + zenith * (host.radius + observer_height_au);

        let (salt, calt) = self.alt.sin_cos();
        let (saz, caz) = self.az.sin_cos();
        let forward = (calt * (caz * north + saz * east) + salt * zenith).normalize();
        // Derive right from azimuth, not forward × zenith: a tracked body
        // can pass through the zenith, where that cross product vanishes.
        let right = (caz * east - saz * north).normalize();
        let up = right.cross(forward);

        // `sun_altitude` refers to the primary (first) star; used only for "is
        // it day" heuristics in the aiming and event searches.
        let sun_altitude = match scene.star_indices().first() {
            Some(&si) => zenith.dot((positions[si] - position).normalize()),
            None => 0.0,
        };

        ViewFrame {
            position,
            forward,
            right,
            up,
            zenith,
            north,
            east,
            sun_altitude,
        }
    }
}
