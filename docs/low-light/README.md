# Low-light ground sampling study

These are the original reviewed captures, before PR #2 added atmospheric
extinction to ground lighting. See the [combined-renderer rerun](with-extinction/README.md)
for measurements and images with that fix included. Both sets are retained for review.

The Atacama night view has bright ground speckles because much of its illumination
comes from Bruma. The current renderer explicitly samples emissive stars, but a
ground bounce discovers a reflecting planet only when its random direction hits
that planet. Rare hits carry large contributions.

The benchmark-only `guided-ground` prototype mixes ordinary cosine-weighted
directions with directions aimed at reflecting bodies, in equal proportions on
the first ground bounce. Contributions use the full mixture probability,
including overlapping planetary discs. Direct-star MIS weights also account for
the changed continuation distribution. This targets the existing lighting integral
without adding a blur or changing materials. The production shader is unchanged.

## Images and measurements

Open [the side-by-side comparison](index.html) in a browser. It includes current
and guided renders at 1, 16, 64 and 1,024 samples per pixel, with identical exposure.

| Measurement | Current | Guided ground |
|---|---:|---:|
| Foreground luminance RMSE at 64 samples | 0.0117405 | 0.0037581 |
| Mean foreground luminance at 1,024 samples | 0.00823247 | 0.00825808 |
| Median GPU time, advancing simulation | 6.520 ms | 6.432 ms |

This is **3.12× lower measured foreground error** at 64 samples. Mean brightness
at 1,024 samples differs by **0.31%**. GPU cost is comparable; the small timing
difference should not be treated as an established speedup.

The diagnostic omissions in the comparison remove real lighting and are not
candidate fixes. Disabling ground continuation leaves only about 18% of the
original mean foreground luminance at 64 samples. Disabling its sky-light term
retains about 82%, with prominent speckles still present. This supports focusing
on sampling the reflected planet light first.

## Method and limits

- AMD Radeon RX 7600 XT, Vulkan, RADV Mesa 26.2.2-arch1.1; details and individual
  measurements are in [advancing.json](advancing.json).
- `configs/atacama.yaml`, day 0, initial camera, 480×270, four bounces, fixed
  exposure 1. Images use accumulated one-sample submissions.
- Error is measured in linear Rec.709 luminance over foreground rectangle
  `[60, 211, 420, 260)` against the current renderer's 1,024-sample image.
  That reference remains noisy, and the current renderer's lower-sample images
  share its sample prefix. This is a diagnostic comparison, not a ground-truth
  accuracy measurement or proof of convergence.
- GPU timings use timestamp queries around rendering and display. Three pairs
  of alternating variant blocks each measure 32 frames after 12 warm-up frames.
  Simulation advances from seconds 1 through 32; each measured frame is checked
  to have only one accumulated sample. Medians aggregate 96 frames per variant.
  Readback and submission wall time are excluded. GPU clocks and background load
  were not controlled.
- This is one scene at low resolution. Eclipse transitions, other planetary
  arrangements, forest ground and moving-camera visual stability still need
  validation before enabling the prototype by default.
- Atmosphere noise, the ground's sky-light cache and ring sampling are unchanged.
  Visible noise remains, particularly in the sky. The result does not demonstrate
  a fix for star flickering.

[summary.json](summary.json) contains the image metrics. [manifest.json](manifest.json)
records source provenance after capture; this study was run from an uncommitted
working tree. Raw linear captures remain under `target/lowlight-study` and can be
regenerated with the commands below.

## Reproduce

```sh
cargo test --release gpu_lowlight_study -- --ignored --nocapture --test-threads=1
python3 scripts/lowlight_report.py target/lowlight-study docs/low-light
```

The test requires a discrete Vulkan GPU with timestamp query support and refuses
software rendering. It overwrites its own files under `target/lowlight-study`.
The report script uses only the Python standard library. The Rust prototype lives
in `src/lowlight_diagnostics.rs`, included only in test builds.

## Further options

1. Validate guided sampling across the existing benchmark scenes. Its extra work
   is selecting a reflecting body and evaluating a mixture PDF over bodies.
   The first experiment suggests a large noise benefit at similar frame cost.
2. Refine or average the 16×8 ground sky-light cache. It currently receives one
   atmosphere sample per direction when history resets and does not refine while
   paused. More samples cost additional atmosphere evaluations, amortized across
   ground pixels; they address a separate source of persistent error.
3. Try a depth/normal-guided ground denoiser. An extra GPU pass and auxiliary
   buffers can smooth residual noise, with a risk of softened rocks and shadows.
   Keep stars and sky outside that filter for the first experiment.
4. Reproject ground history while simulation advances. This can reuse samples
   across frames, but needs history rejection for motion, disocclusion and changing
   eclipses to avoid ghosting. It adds memory and GPU work and changes temporal
   appearance, so should have a separate moving-image comparison.

Background: [PBRT's path-tracing and MIS discussion](https://pbr-book.org/4ed/Light_Transport_I_Surface_Reflection/A_Better_Path_Tracer)
and the [SVGF paper](https://research.nvidia.com/labs/rtr/publication/schied2017spatiotemporal/)
describe the sampling and temporal/spatial filtering approaches behind these options.
