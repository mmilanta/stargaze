# First renderer optimization results

Measured on the **Radeon RX 7600 XT**, Vulkan RADV / Mesa 26.2.2-arch1.1.
The optimized renderer is commit `058f4a5`. The original renderer is `e7cb173`,
with its reproducible benchmark published in `c3dad2d`.

The atmospheric workloads are **1.09×–2.83× faster at the same settings**.
All **28 displayed RGB and linear HDR RGB images match exactly**: zero MAE and
RMSE, including one-sample, 32-sample and two advancing-time checkpoints.
This measures the tested scenes and hardware; it does not establish equality
for every possible custom scene or performance on other drivers.

## Measurements

1280×720 display, 960×540 tracing, four bounces, 70% vegetation, one sample per
submission. Eight warmups, three repetitions of sixteen measured submissions
per case/mode. GPU timestamps include tracing, exposure and offscreen display.
These are renderer timings, **not measured application FPS**.

| Advancing view | Original GPU ms | Optimized GPU ms | Throughput gain |
|---|---:|---:|---:|
| halo-rings | 73.638 | 34.218 | 2.15× |
| earth-daylight | 131.301 | 120.813 | 1.09× |
| earth-twilight | 114.843 | 40.558 | 2.83× |
| median-dense | 104.692 | 74.474 | 1.41× |
| vantus-eclipse | 67.533 | 31.490 | 2.14× |
| vantus-airless | 2.607 | 2.549 | 1.02× |
| dual-eclipse | 0.946 | 0.960 | 0.99× |

The complete [comparison](comparison/comparison.md) includes paused results and
all image metrics. [Raw timings](results.json), [summary](summary.md),
[provenance](manifest.json) and [side-by-side gallery](comparison/index.html)
are included. Open HTML locally; GitHub can display the individual PNG files.

A separate [initial full run](initial/summary.md) agreed within
0.73% for all atmospheric medians; the largest difference across every
case/mode was 2.65%. The initial run precedes final test/comment
cleanup; both full runs use the retained production algorithm. Each run's
manifest records its source hashes. Paused repeat medians and p95 values are
included so warmup/cache variation remains visible.

Airless cases did not materially benefit. The paused synthetic `dual-eclipse`
case **regressed from 1.480 to 1.668 ms (+0.188 ms, 12.7%)**; the initial run
also showed a regression. This bundle trades that small absolute cost for much
larger savings in the expensive atmosphere workloads. Daylight terrain still
costs about 121 ms per sample and needs further work for smooth interaction.

## What changed and what it costs

- Shadow rays first find the distance to the sampled light, then stop at the
  first intervening opaque object. Terrain and landscape queries have an
  occlusion path that skips closest-hit completion and unused normals. Ring
  transmission, finite lights, eclipses and equal-distance body ordering remain.
- Terrain traversal stores a node index and entry distance instead of complete
  node coordinates: **256 rather than 640 bytes per ray**. Tile coordinates are
  reconstructed with integer operations when a node is visited.
- Curved terrain bounds, vertices and normals are computed once per host radius
  and reused across animation and accumulation resets. This costs **8.02 MiB
  additional GPU memory**, plus a compute pass at first use or radius change.
  Warmed benchmark timings do not measure first-entry cache construction latency.
- A 16-byte uniform caches up to three emissive-body indices. Inner lighting
  loops skip non-emissive bodies without changing light/sample order. Custom
  systems with more than three stars retain the full scan.

Resolution, samples, bounces, atmosphere step counts and vegetation are
unchanged. Star-flicker handling and temporal sample reuse are deferred.
[Incremental screening results](screening/README.md) record each experiment's
measurements, image checks and compromises. A separate atmosphere pass was
rejected because its lower GPU time came with large measured wall-time overhead.

## Validation

- Release build and all **105 ordinary Rust tests** pass; shader validation and
  CPU/GPU buffer-layout checks are included. All four Python benchmark tests pass.
- The optimized GPU regression run has **32 passes and three failures**. A fresh
  build of baseline `c3dad2d` has **31 passes and the same three failures**.
  The additional passing test compares shadow queries against closest-hit
  visibility, including terrain, props, rings, coincident spheres, host targets,
  and zero through five emissive sources with order/fallback checks.
- Known failures on both versions: the night-ground test observes maximum
  1.2892441e-10 instead of its <1e-12 threshold; the Mercury silhouette assertion
  fails; the terrain oracle misses its 5 km-host boundary ray #7 (reference
  distance 18.525682249 m). They were not hidden or relaxed. Full logs:
  [baseline](validation/baseline-gpu-regressions.log),
  [optimized](validation/optimized-gpu-regressions.log),
  [ordinary tests](validation/optimized-unit-tests.log).
- Full local HDR buffers produced the published zero-error metrics;
  [HDR checksums](hdr-checksums.json) preserve their hashes. As with the original
  compact baseline, large raw buffers are omitted from Git. Candidate PNGs are
  included; identical files reuse Git's existing image objects.

Reproduce using the [benchmark instructions](../../README.md), running the same
arguments on both commits and comparing the complete output directories.
