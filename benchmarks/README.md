# Renderer performance and image comparisons

**Current app default:** guided ground sampling. For reproducibility, benchmark
`reference` retains the original unguided sampler with 8/4 atmosphere integration;
use `--variant guided-ground` to measure the accepted app default. Existing
captures keep their original labels and source provenance. The app accepts
`--unguided-ground` for direct comparison.

The [October 7 follow-up investigation](optimizations/followup-2026-10-07/README.md)
adds raw and rendering-quality candidates, desert day/night fixtures, and a
bracketed study runner. It records completed equivalence checks and the current
hardware-timing limitation; no new speedup is claimed without measurements.

This suite measures the existing production path tracer, exposure meter, and
display shader on a hardware Vulkan GPU. It also captures deterministic images
so a faster implementation can be checked for visual changes. It does not modify
the rendering algorithm or solve the star-flicker issue.

The [RX 7600 XT baseline](baselines/rx7600xt-2026-10-04/summary.md) contains actual
GPU timestamp measurements, raw timings, provenance, and reference PNGs. Open its
[index.html](baselines/rx7600xt-2026-10-04/index.html) locally to browse the images.
GitHub displays individual PNGs but does not execute the HTML gallery.

The [first optimization report](optimizations/rx7600xt-2026-10-04/README.md)
compares the faster renderer against that baseline, with repeated timings,
image differences, incremental experiments, memory costs and GPU test results.

## Run

Requires Rust/Cargo, Python 3 (standard library only), and a Vulkan hardware GPU
with timestamp-query support. The default selects the first discrete GPU and
refuses software adapters. Use `--adapter 'RX 7600'` to select by name substring.
There is no display-server or window requirement.

```sh
python3 scripts/benchmark.py run target/benchmarks/before
# After implementing ONE optimization:
python3 scripts/benchmark.py run target/benchmarks/after
python3 scripts/benchmark.py compare \
  target/benchmarks/before target/benchmarks/after target/benchmarks/comparison
```

Open `target/benchmarks/comparison/index.html` in a browser. It links to a table
of timing changes and shows baseline, candidate, and an 8× amplified absolute
image difference side by side. All output directories must be new: previous
measurements are never overwritten. Failed runs keep their logs and partial
artifacts and are not valid baselines.

Compare against the checked-in baseline using the default settings:

```sh
python3 scripts/benchmark.py run target/benchmarks/candidate
python3 scripts/benchmark.py compare \
  benchmarks/baselines/rx7600xt-2026-10-04 \
  target/benchmarks/candidate target/benchmarks/baseline-comparison
```

The checked-in baseline omits large raw image buffers, so comparisons against it
report displayed-image errors only. Keep the full local `before` output for
linear HDR comparisons as well. Each full run includes compressed RGBA8 and
RGBA32F buffers; RGB alone is used for HDR errors (W is metering data). These
files can be shared as a separate benchmark artifact without putting them all
in Git. The suite can also decode its PNGs with no Python imaging dependency.

Defaults: **1280×720 display, 75% render scale (960×540 actual tracing), 4 bounces,
70% vegetation, 1 new sample per submission, automatic exposure while timing**.
Each case/mode has 8 warm-up submissions followed by 3 repetitions of 16 measured
frames. Paused images are captured at 1 and 32 spp; advancing images at sequence
frames 8 and 15. Every submission completes before the next, avoiding large
queued batches. The sample limit never truncates the run.

Other workloads are configurable; compare identical arguments:

```sh
# 1080p display, Balanced render quality
python3 scripts/benchmark.py run target/benchmarks/1080p --width 1920 --height 1080
# Native resolution; more measurements and a higher-sample image reference
python3 scripts/benchmark.py run target/benchmarks/native \
  --scale 100 --frames 32 --repeats 5 --image-samples 256
# Small smoke test, not a publishable performance baseline
python3 scripts/benchmark.py run target/benchmarks/smoke \
  --width 320 --height 180 --scale 100 --warmup 1 --frames 2 --repeats 1 --image-samples 8
```

`--cases` accepts one or more names from the table below. The runner removes
`STARGAZE_*` overrides and does not read saved graphics preferences.

## Fixed views

Definitions live in `src/render_benchmark.rs` and use the same frame assembly and
scene data as the application. Camera/geometry/time input fingerprints are saved
for every frame; comparisons reject changed inputs, settings, suite versions,
and GPU/backend/driver combinations. If an experiment intentionally changes scene
inputs, define a new fixture/version instead of silently comparing unlike work.

| Case | Fixed setup | Exercises |
|---|---|---|
| `halo-rings` | Halo observatory, day 0, initial Calyx view | Binary illumination, Calyx rings, occultation/shadows, host atmosphere |
| `earth-daylight` | Earth, day 0.599, north, altitude 3°, FOV 55° | Continuous terrain, conifers, grass, cottage, daylight scattering and indirect light |
| `earth-twilight` | Same ground view, day 1.04; sun about 1.4° below horizon | Twilight integration, dark ground, faint light and background |
| `median-dense` | Cadence, day 0.25; bright Lumen near local noon, north, altitude 20°, FOV 65° | Dense atmosphere, two stellar sources, ground near horizon |
| `vantus-eclipse` | Binary system, day 444.478; telescope tracks Vantus, FOV 2.5× angular radius | Multiple moon transits/shadows and atmospheric transport at deep zoom |
| `vantus-airless` | Exactly the same Vantus geometry/time/camera, atmosphere disabled | Controlled airless counterpart, isolates atmospheric workload differences |
| `dual-eclipse` | Synthetic ringed sphere, two finite suns and two aligned occluders | Overlapping soft eclipse shadows, colored lighting, ring transmission, indirect light without atmosphere |
| `moonlit-air` (optional) | Synthetic large moon illuminated by a sun below the observer horizon | Reflected atmospheric glow |
| `eclipse-umbra-edge` (optional) | Nearly point-like sun, observer just inside totality, transverse shadow motion | Sharp atmospheric illumination boundary: bright sky to the left, dark sky to the right and dark ground |

Advancing mode uses **one simulated second per sequence frame**, not elapsed
wall time. Each repetition replays the same sequence. Telescope cases track their
target; ground views hold their local azimuth/altitude. The synthetic occluders
move 0.003 scene units per step. This is a short, slow-motion workload; it does not
cover fast camera pans, large time jumps, resize, or every application state.

## What is measured

- GPU timestamps bracket production `PathTracer::encode` and the actual display
  pass into an sRGB texture. This includes skylight-cache generation when history
  is invalid, path tracing, automatic exposure, and upscaled presentation.
- Wall timing starts before command encoding and stops after GPU completion. It
  includes buffer uploads/submission and the tiny timestamp resolve/copy, but
  excludes mapping the timestamp result. GPU timing excludes CPU work and uploads
  queued before the encoded commands.
- Shader compilation, scene/frame construction, terrain generation, image
  readback, image compression, and report generation are outside timing.
- No window, HUD, compositor, VSync, application frame cap, or adaptive scheduler
  is involved. **1000 / milliseconds is a renderer throughput equivalent, not
  a measured application FPS.**
- Each paused repetition starts fresh: its first sample builds the sky cache,
  then accumulation continues through 16 spp. Each advancing frame currently
  invalidates history and stays at 1 spp. The suite checks this expected behavior;
  a future temporal-reuse implementation must deliberately update that check and
  document its sample semantics, while preserving these input trajectories.
- Raw per-frame results are retained. Reports give median, interpolated p95,
  wall median, and each repetition's median. Speedup is baseline GPU median /
  candidate GPU median. Small changes comparable to repeat/run variation are
  inconclusive; repeat measurements before claiming them.

Close Stargaze and other GPU-heavy applications, keep the same display/power
configuration, and compare on the same driver. The harness warms each workload
but does not lock GPU clocks or control other processes. The baseline includes
its observed same-code rerun spread; it is not a controlled laboratory claim.

## Image comparisons

Images use a **fixed manual exposure** (8 for Vantus, 1 elsewhere), no automatic
adaptation, and the production display shader. Timing still includes automatic
exposure. Fixed seeds, samples, camera, and exposure prevent exposure adaptation
from concealing lighting differences. One-sample, 32-sample, and two advancing
checkpoints exercise both interactive and refining images. **32 spp is a noisy
reference, not converged ground truth.** Use `--image-samples 256` or more for
sensitive quality evaluation, running both versions with the same setting.

Metrics include displayed RGB MAE/RMSE in 0–255 units, maximum error, percentage
of pixels changing by more than 1/255, and PSNR (JSON null means identical images).
With full artifacts, also report linear RGB RMSE, relative RMSE normalized by
baseline RMS, and log(1+RGB) RMSE. Linear metrics detect lighting changes hidden
by tone mapping; the logarithmic metric reduces domination by bright emitters.
Relative error is undefined for an all-black reference and reported as null.

These are difference metrics, not a universal quality score. A better sampler
can legitimately change every pixel. Inspect edges, shadow positions, ringing,
energy/brightness, and fine detail, and compare higher-sample references where
needed. Two animation checkpoints do not constitute a temporal-flicker metric.
Keep correctness regression tests alongside the benchmark.

## Artifacts and validation

Each output contains `request.json`, `manifest.json` (revision, dirty status,
source hashes, OS, CPU, toolchain), `results.json` (adapter/driver and raw timings),
`summary.json`, `summary.md`, `run.log`, PNGs, raw compressed images, and an HTML
gallery. Benchmark code is compiled only for tests; production shaders and
renderer behavior are unchanged.

```sh
cargo test --release
python3 -m unittest discover -s scripts -p 'test_benchmark.py'
```

Ordinary tests validate deterministic fixtures, day/twilight conditions and
occluder alignment, plus comparison statistics, errors, and compatibility checks.
The ignored `render_benchmark` test is invoked by the Python runner with `--release`.
The checked-in baseline and repeatability report document the hardware runs.

## Rendering-quality investigation

[Review all six atmosphere approximations](quality/rx7600xt-2026-10-05/README.md)
with [interactive image comparisons](quality/rx7600xt-2026-10-05/index.html).
The reviewed `fast` integration counts are used by the app: 8 view steps and
4 light-path steps. `reference` and `fast` retain the original unguided ground
sampler and are identical aliases; `guided-ground` selects the app default.
Other named experiments retain their reviewed settings,
including 24/12 steps for the two planetshine experiments. Archived review
images and timings retain the original 24/12 reference and their capture-time
source hashes. Their timing results are provisional because another application
shared the GPU. Compare against those captures when measuring the change from
the previous default.

## Atmospheric eclipse boundary

The `eclipse-umbra-edge` stress case captures a recently eclipsed tiny star:
the observer is in totality while dense atmosphere to the left still sees the
star. [Inspect all six variants](quality/eclipse-umbra-edge/index.html), including
1-, 32-, and 256-sample images. See the [geometry and validation notes](quality/eclipse-umbra-edge/README.md).

```sh
python3 scripts/benchmark.py run target/benchmarks/umbra-reference --cases eclipse-umbra-edge
python3 scripts/benchmark.py run target/benchmarks/umbra-fast --cases eclipse-umbra-edge --variant fast
python3 scripts/benchmark.py compare target/benchmarks/umbra-reference target/benchmarks/umbra-fast target/benchmarks/umbra-comparison
```

The seven default cases are preserved for comparisons against historical runs.
Select the new case explicitly with `--cases eclipse-umbra-edge`, or append it to
your case list. An independent finite-disc geometry test and the ignored GPU test
`gpu_umbra_edge_contrast` check totality, lit neighbouring air and dark ground.
The ignored `render_umbra_edge_study` captures all seven shader variants (including
reference) at both image settings into a new `target/benchmarks/umbra-edge-study`
directory; it refuses to overwrite previous runs. Standard Python invocations
above are the recommended repeatable route and also create their reports.
