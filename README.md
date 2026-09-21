# stargaze

An observatory on a planet, looking out at a star system. Stand still and watch
Keplerian orbits through an alt-azimuth telescope—or pause time and let the
**progressive, fully ray-traced image** converge.

The default scene is a fictitious binary-star system. Set
`STARGAZE_SYSTEM=solar` for Sun, Terra, Luna, Mars and Phobos.

## Rendering

All scene visibility and illumination are traced in a WGSL compute shader:

- **Primary rays**, jittered within each pixel, intersect analytic spheres.
  There are no rasterized body meshes, minimum-size planet impostors, shadow
  maps, or screen-space eclipse masks.
- **Finite stars:** emissive spheres appear as discs. At each diffuse surface
  hit, the integrator samples a direction uniformly over each star's apparent
  solid-angle disc, then traces a visibility ray. Blocked samples contribute
  nothing. **Umbras, penumbras and annular eclipses emerge from these samples**,
  not from a smoothed shadow-cylinder approximation.
- **Indirect illumination:** cosine-weighted Lambertian bounces, power-heuristic
  multiple importance sampling (MIS), and Russian roulette. MIS combines disc
  samples and bounce rays that hit stars without double-counting sunlight.
- **Consistent emission:** the same stellar radiance is used for visible stars
  and surface illumination. Uniform-radiance stellar discs are assumed; no
  limb darkening is currently modeled. Inverse-square dilution follows from
  the star's decreasing solid angle, not an additional falloff multiplier.
- **Real ground:** the observer's planet is included in intersections. The
  camera stands two metres above its surface; the horizon is not a sky mask.
- **Background stars:** fixed angular emissive discs at infinity, intersected
  by miss-ray directions. A conservative spherical grid accelerates catalogue
  lookup. They do not fade with the Sun's altitude or grow to a minimum pixel
  size. Background light can also be received by indirect paths.
- **No atmosphere:** no scattering, refraction, haze, stellar halo, artificial
  ambient lighting, or red lunar-eclipse glow. Space is black; a fully eclipsed
  Moon receives only indirect light. Surfaces retain the procedural albedo
  mottling, clamped to energy-conserving diffuse reflectances.

Each frame adds fresh Monte Carlo samples to a linear HDR running average.
The window title shows **samples per pixel (spp)**. Pause time and stop moving
for convergence; motion/time changes reset the average. HUD and label toggles
preserve it. Resize, material, camera, or integrator changes invalidate it.
At low sample counts, noise is expected, particularly in indirect lighting.
The average stops at 16,777,216 spp to avoid counter/float precision loss.

Only tone-mapped presentation and the HUD use ordinary rasterization. No
hardware ray-tracing extension or RT-capable GPU is needed: a native wgpu
compute-capable Metal, Vulkan or DX12 device is sufficient.

> Images under `docs/` show the **previous rasterized renderer**, including its
> atmospheric and red-umbra approximations; they are not current reference images.

## Build & run

Requires a recent stable Rust toolchain compatible with wgpu 30
(tested with Rust 1.98.1).

```sh
cargo run --release
STARGAZE_SYSTEM=solar cargo run --release
```

If installing Rust using rustup for the first time, load its environment with
`source "$HOME/.cargo/env"` before running Cargo.

### Quality settings

```sh
# Eight new samples per pixel per frame; up to 12 surface vertices per path.
STARGAZE_SPP=8 STARGAZE_BOUNCES=12 cargo run --release

# Lower exposure to inspect the bright solar disc rather than lit planets.
STARGAZE_SYSTEM=solar STARGAZE_AIM=0 STARGAZE_EXPOSURE=0.00001 cargo run --release
```

| Variable | Default | Meaning |
| --- | --- | --- |
| `STARGAZE_SPP` | `1` | Samples per pixel per frame, clamped to 1–64 |
| `STARGAZE_BOUNCES` | `8` | Maximum surface vertices per path, clamped to 1–64; `1` gives direct-only lighting |
| `STARGAZE_EXPOSURE` | `1` | Positive finite exposure multiplier, applied after accumulation |

Increasing samples per frame makes each frame slower, not intrinsically more
accurate than accumulating the same total sample count over more frames. The
bounce limit truncates long light paths; increase it for higher-albedo scenes.
Russian roulette starts after three diffuse vertices. The pseudo-random
sequence is deterministic for a given pixel and sample index.

### Controls

| Input | Action |
| --- | --- |
| left-drag (sky) | look around |
| scroll / `=` / `-` | zoom, down to 0.001° |
| `-1d` `-1h` `-1m` `+1m` `+1h` `+1d` (bar) | step simulation time |
| Labels toggle (bar) | show / hide names |
| space | play / pause; pause to accumulate a clean image |
| `[` `]` | slower / faster time |
| `1`–`8` | aim at scene targets |
| `E` | search for an eclipse |
| `T` | search for a moon transit (Phobos/Mars in the solar scene) |
| `R` | reset view |
| `H` | show / hide HUD |
| `L` | toggle labels |
| esc | quit |

Startup helpers (body indices depend on the selected system):

```sh
STARGAZE_SYSTEM=solar STARGAZE_FIND_ECLIPSE=1 cargo run --release
STARGAZE_SYSTEM=solar STARGAZE_FIND_PHOBOS=1 cargo run --release
STARGAZE_SYSTEM=solar STARGAZE_TIME=245.92 STARGAZE_AIM=2 STARGAZE_NO_ADVANCE=1 cargo run --release
STARGAZE_DEBUG=1 cargo run
```

`STARGAZE_FOV` overrides the vertical field of view in degrees.

### Distant-planet eclipse preset

```sh
./scripts/vantus-eclipse.sh
```

Starts paused at **day 444.478**, aimed tightly at **Vantus** from about
**2.72 AU (407 million km)** away. Its moon **Iri** casts a visible shadow by
blocking **Aur**; the second star still illuminates the shadowed surface.
Leave the view stationary to converge, or step time with the HUD's minute
buttons. The launcher fixes the time, target and field of view without changing
the ordinary startup defaults.

## Implementation

- Rust + wgpu + winit. The analytic, stateless orbit simulation is unchanged.
- Orbital positions, camera subtraction and rotation into telescope space use
  CPU `f64`. Primary rays stay in telescope space so extreme-zoom pixel jitter
  is not rounded away in world-space unit vectors. Sphere centres are uploaded
  as high/low `f32` parts. Secondary ray origins are body-local, and
  intersections use closest-approach cross products rather than subtracting
  nearly equal squared astronomical distances. Ray offsets scale with radius.
  GPU direction/intersection arithmetic is still `f32`, so extreme zooms and
  near-tangent hits have finite precision; more samples do not fix that error.
- Up to 256 scene spheres; all are tested directly, including every luminous
  star and every potential blocker. No BVH is needed for the current scenes.
- One compute invocation per pixel; accumulation uses 16 bytes per pixel.
- ACES tone mapping followed by sRGB presentation. No denoiser or adaptive
  sampling is currently applied.

| File | Purpose |
| --- | --- |
| `src/sim.rs` | Kepler solver and scene definitions |
| `src/camera.rs` | observer anchored to the rotating host planet |
| `src/main.rs` | events, time, frame assembly, labels |
| `src/pathtracer.rs` | compute pipeline, accumulation and invalidation |
| `src/shaders/pathtrace.wgsl` | sphere intersections and light transport |
| `src/shaders/display.wgsl` | HDR tone mapping |
| `src/renderer.rs` | window presentation and HUD |
| `src/stars.rs` | procedural catalogue and spherical lookup grid |
| `src/ui.rs` | HUD geometry and font atlas |

## Tests

```sh
cargo test
# Opt-in, actual shader execution on a GPU; no window required:
cargo test gpu_ -- --ignored --nocapture
# Also save small reference renders as PPM images:
STARGAZE_TEST_IMAGE_DIR=/tmp/stargaze-renders cargo test gpu_scene_smoke -- --ignored --nocapture
```

Tests cover WGSL validation and buffer layouts, accumulation invalidation,
star-grid seam/pole coverage, actual GPU emission, finite-disc irradiance,
umbras, penumbras, annular eclipses, multiple stars, MIS, indirect reflected
light, resize, presentation, and both built-in scenes.
