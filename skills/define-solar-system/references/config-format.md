# Stargaze YAML format, version 1

## Loading and validation

From the Stargaze project root:

```sh
cargo run --release -- --config configs/halo.yaml
cargo run --release -- --config configs/solar-system.yaml
cargo run --release -- --check-config configs/halo.yaml
```

`--check-config` exits with status 0 for a valid file and nonzero with a contextual error otherwise; it needs no window or GPU. `--config FILE` takes precedence over `STARGAZE_CONFIG`, which takes precedence over `STARGAZE_SYSTEM`. With none set, the application reads `configs/halo.yaml` from its build-time project directory. Edits take effect on the next launch, without recompilation. Arbitrary config paths may be absolute or relative to the current working directory.

The bundled `halo.yaml` defines the fictional Aur/Igni binary with Calyx, rings, and the airless Halo observatory. `solar-system.yaml` defines the Sun, all eight planets and selected moons, with an Earth observatory looking at the Moon. Its fixed, approximate J2000 elements and illustrative moon phases are not a current ephemeris. Legacy `STARGAZE_SYSTEM=solar` still selects the Saturn surface view, `earth` selects the solar YAML, and `calyx` selects the historical planet-side binary view.

## Top level

| Field | Meaning |
| --- | --- |
| `version` | Required integer `1` |
| `name` | Required nonempty descriptive system name |
| `time_days` | Optional finite starting day, relative to the system's epoch; omission/null searches days 0–400 for a suitable dark view |
| `camera` | Required surface observatory, described below |
| `targets` | Optional list of up to nine visible body IDs for keys 1–9; defaults to `camera.look_at` |
| `bodies` | Required list of 2–256 bodies, with unique nonempty IDs and exactly one root |

Body ordering is arbitrary; the loader resolves parents before children and rejects cycles. Unknown fields and unknown IDs are errors. Numbers must be finite. Values use AU, km, metres, degrees, or days as stated in their field names. One AU is 149,597,870.7 km. All angles are degrees in YAML, including negative retrograde tilts or phases.

`STARGAZE_TIME`, `STARGAZE_AIM`, `STARGAZE_FOV`, and event-search environment variables remain runtime overrides. An aim override can search for a visible time. Use a clean environment when verifying exact YAML startup time and framing. The config checker validates the file alone.

## Camera

| Field | Meaning |
| --- | --- |
| `body` | Required planet or moon ID; camera follows this body's rotation |
| `latitude_deg` | Required, −90 to +90 |
| `longitude_deg` | Required, −180 to +180, increasing toward body-local east |
| `height_m` | Optional, default `2`; must be at least `0.1` above the modeled sphere |
| `look_at` | Required visible body ID, different from the host; sets initial pointing without enabling tracking |
| `fov_deg` | Required vertical FOV, 0.001–90 |
| `atmosphere` | Optional `earthlike`; omission/null gives an airless view |

The atmosphere is only rendered around the camera host: an 8.5 km exponential scale height and 80 km top with Earth-like Rayleigh/Mie scattering. Custom atmospheres are not supported by version 1. The camera is attached to a spherical surface, including on gas giants; clouds and gas layers are not modeled.

A target may be below the horizon at `time_days`; the validator does not change explicit time. If omitted, startup searches for the target above the horizon and, unless aiming at a star, all stars below it, and falls back to day 0 if no suitable view is found. Click a body in the running app to track it. Exposure is controlled with the HUD slider (−8 to +8 EV in quarter stops) and Auto checkbox, immediately after the time buttons; exposure settings are not part of scene YAML.

## Bodies

| Field | Meaning |
| --- | --- |
| `id` | Required stable string used by references |
| `name` | Optional display name, defaults to `id` |
| `kind` | Required: `anchor`, `star`, `planet`, or `moon` |
| `parent` | Required for every non-root body; another body ID |
| `radius_km` | Required positive physical radius; anchors require `0` |
| `albedo` | Optional RGB reflectance in [0,1], default `[0.5, 0.5, 0.5]` |
| `luminosity` | Stars only, nonnegative relative luminosity; default `0`; the system needs at least one luminous star |
| `emission` | Optional stellar RGB in [0,1], default `[1, 1, 1]`; tints the light |
| `orbit` | Required with `parent`, absent for the root |
| `rotation` | Optional free or synchronous rotation, described below |
| `rings` | Optional ring annulus on a planet or moon |

An `anchor` is invisible, casts no shadows and emits no light. Use one as the root of a binary system. The root remains at the origin. Radii are physical; orbit distances are centre-to-centre.

### Orbit

- Exactly one of `semi_major_axis_au` or `semi_major_axis_km`, positive.
- `period_days`: required positive orbital period, independent of radius; masses are not modeled.
- `eccentricity`: default 0; range `[0,1)`.
- `inclination_deg`, `ascending_node_deg`, `periapsis_deg`, `mean_anomaly_deg`: default 0.
- `epoch_days`: default 0, reference time for the mean anomaly.

Every orbit uses the same world/ecliptic frame, **including moon orbits**. Its rotation is `Rz(ascending_node) * Rx(inclination) * Rz(periapsis)`. The parent contributes position, not orientation. Periapsis distance must exceed parent radius plus child radius. Mutual body collisions, ring crossings and long-term stability are not checked; ring moonlets may intentionally lie inside a ring.

### Rotation

```yaml
rotation:
  mode: free
  period_days: 1.0       # nonzero; negative gives retrograde spin
  obliquity_deg: 23.4   # optional, default 0
  phase_deg: 0          # optional, default 0
```

Omitting rotation is equivalent to `free` with period 1 day and zero angles. Free rotation is `Ry(obliquity) * Rz(phase + 360 * time / period)`. Periods use simulated days.

```yaml
rotation:
  mode: tidal_lock
```

Tidal locking requires a parent and orbit. Its pole is aligned with the orbital normal; spin period, phase and epoch follow that orbit. Longitude 180° faces the parent exactly in a circular orbit. Eccentric orbits use uniform synchronous spin and therefore librate. Do not specify free-rotation fields with `tidal_lock`.

### Rings

```yaml
rings:
  inner_radius: 1.25    # multiples of the body's physical radius, NOT km
  outer_radius: 1.9
  optical_depth: 1.0    # normal incidence; finite and nonnegative
  albedo: [0.88, 0.80, 0.66]
  banded: true         # optional, default false
```

Require `1 < inner_radius < outer_radius`. The infinitesimally thin ring lies in the body's equator and follows its rotation frame. `banded: true` applies Saturn-like bands and gaps across this annulus; false gives a uniform disc. To keep rings clear of a moon, compare the ring's outer edge to the moon's periapsis distance **minus its radius**.

## Minimal complete example

A deliberately simple Earth-sized planet orbiting a Sun-sized star, seen from its day side. The camera looks at the star at an explicit time:

```yaml
version: 1
name: A simple observatory
# Day zero is an arbitrary epoch in this illustrative system.
time_days: 0
camera:
  body: haven
  latitude_deg: 20
  longitude_deg: 180
  height_m: 2
  look_at: sun
  fov_deg: 60
targets: [sun]
bodies:
  - id: sun
    kind: star
    radius_km: 696340
    luminosity: 1
    emission: [1, 0.95, 0.88]
  - id: haven
    kind: planet
    parent: sun
    radius_km: 6371
    albedo: [0.3, 0.45, 0.6]
    orbit:
      semi_major_axis_au: 1
      period_days: 365.256
    rotation:
      mode: free
      period_days: 1
```
