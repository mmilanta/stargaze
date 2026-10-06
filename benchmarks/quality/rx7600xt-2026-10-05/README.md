# Rendering-quality experiments for visual review

**All six choices are retained for review. No variant is selected, discarded,
or enabled in the application.** The experimental shader construction is compiled
only into the benchmark tests, on branch `perf/render-quality-experiments`.
The previously accepted production optimizations remain on `main` at `0994342`.

Open [the interactive image review](index.html) locally. Choose a variant and
scene, drag the divider, toggle the whole image, or show the absolute difference
amplified eight times. The page links to the original PNGs and measurements.
GitHub can display PNGs but does not execute the HTML viewer.

A new [eclipse-at-totality-edge comparison](../eclipse-umbra-edge/index.html) adds
the tiny-star, one-sided atmospheric illumination stress case.

## Provisional timing results

**Another application was using the RX 7600 XT during these timing runs.**
The results below are screening evidence under shared GPU load, not isolated
performance measurements. Small differences should not be interpreted as gains.
The airless control timings varied too. Rerun the retained variants and reference
with the GPU otherwise idle before selecting on performance.

These gains are relative to the already optimized current renderer, rather than
the original slow baseline. Reference advancing times were 125.43 ms for daylight
and 33.12 ms for Vantus. Settings were 1280×720 display, 960×540 tracing, four
bounces, 70% vegetation, one sample per submission, eight warmups, and three
repeats of sixteen frames. These are GPU renderer timings, not application FPS.

| Variant | Change | Observed throughput range across atmospheric views | Daylight | Vantus eclipse |
|---|---|---:|---:|---:|
| `sun6` | 24 view / 6 light-path steps | 1.00–1.01× | 124.03 ms | 33.05 ms |
| `view12` | 12 view / 12 light-path steps | 1.37–1.82× | 76.34 ms | 18.42 ms |
| `balanced` | 12 view / 6 light-path steps | 1.46–1.91× | 72.85 ms | 17.34 ms |
| `fast` | 8 view / 4 light-path steps | 1.67–2.67× | 56.32 ms | 12.62 ms |
| `planetshine-quarter` | Quarter-rate reflected-light sampling | 1.01–1.25× | 119.80 ms | 27.85 ms |
| `no-planetshine` | Omit reflected atmospheric light | 1.73–4.18× | 63.94 ms | 9.99 ms |

## What to examine

- **`sun6`:** Small attenuation and colour changes; inspect daylight and dense atmosphere.
- **`view12`:** Changes view integration, haze and sampling noise; inspect twilight and eclipse edges.
- **`balanced`:** Combines the two integration reductions; inspect sky brightness and colour.
- **`fast`:** Larger integration changes; inspect daylight sky and dense haze.
- **`planetshine-quarter`:** Retains reflected atmospheric glow in expectation; increases noise, most visible in moonlit air.
- **`no-planetshine`:** Little visible change in several daylight cases, but removes much of the moonlit sky glow.

Start with daylight and dense atmosphere to compare colour and haze; use Halo
and Vantus for rings/eclipses; use twilight for faint illumination. The synthetic
`moonlit-air` case deliberately places a sun below the observer's horizon while
it illuminates a large moon. Its fixed exposure of 64 makes reflected atmospheric
glow visible. Other images use exposure 1, or 8 for Vantus.

Use **1 spp** to judge interactive noise and **32/256 spp** to examine how the
appearance settles. The 256-spp images use a smaller 640×360 display and 480×270
trace size, compared within that same resolution. They help distinguish sampling
noise from persistent changes, but are not completely converged physical ground
truth. Changing view-step count also changes later random samples, so an image
difference includes sampling changes as well as integration error. Extra samples
cannot restore omitted light or remove quadrature bias.

The global HDR relative error can understate an obvious sky change when a bright
moon dominates the image's energy. For example, omitted planetshine changes the
256-spp moonlit displayed image by a mean **74.50 levels out of 255**, despite
having little effect on several daylight views. Inspect the sky surrounding the
moon as well as the moon itself. These metrics measure differences, not aesthetic
acceptability. No visual acceptance threshold has been applied.

No memory buffers are added to the production renderer. The integration-step
variants reduce numerical work; the quarter-rate variant randomly selects six
of 24 reflected-light cells and weights them by four; omission removes that
lighting contribution. The quarter-rate approach does not guarantee a fourfold
speedup because other tracing work and GPU execution behavior still matter.
Atmospheric direct sunlight, surface lighting, rings and geometry remain present
in all variants. Star-flicker handling and temporal reuse are outside this study.

## Evidence and reproduction

`data/full/<variant>/` contains request parameters, source hashes, raw timings,
summary and comparisons for eight views: the original seven plus moonlit air.
`data/quality/<variant>/` contains a separate 256-spp comparison for the six
atmospheric views. The viewer includes all six candidates at all three sample
counts; airless control results remain in the data. Full local galleries also
retain advancing-time snapshots under `target/benchmarks/quality-study/` and
`target/benchmarks/quality-refinement/`.

The report bundle retains PNGs and numeric HDR comparisons. Large compressed
HDR buffers remain in the local output directories. Every comparison validates
matching settings, GPU/driver, camera/geometry fingerprints and image settings.
The benchmark-only source is in `src/render_experiments.rs` (commit `db70f07`).
All six shader variants passed WGSL validation and completed hardware runs;
106 ordinary Rust tests and four Python comparison tests passed. The reference
variant is tested to reproduce the production shader source exactly.

```sh
# Repeat for each variant, keeping all other arguments identical.
python3 scripts/benchmark.py run target/benchmarks/review-reference --variant reference \
  --cases halo-rings earth-daylight earth-twilight median-dense vantus-eclipse vantus-airless dual-eclipse moonlit-air
python3 scripts/benchmark.py run target/benchmarks/review-view12 --variant view12 \
  --cases halo-rings earth-daylight earth-twilight median-dense vantus-eclipse vantus-airless dual-eclipse moonlit-air
python3 scripts/benchmark.py compare target/benchmarks/review-reference \
  target/benchmarks/review-view12 target/benchmarks/review-view12-comparison
```

For the longer-refined set, use `--width 640 --height 360 --warmup 2 --frames 4
--repeats 1 --image-samples 256` and the six atmospheric cases. Its few timing
samples are not used for the performance table. Use fresh output directories.
The standalone review can be regenerated with `scripts/benchmark_review.py`.

The review's JavaScript controls, statistics and file links were checked across
all 108 variant/scene/sample combinations. A headless-browser launch was blocked
by the current sandbox's socket restrictions, so browser-level visual validation
was unavailable. The report and viewer are included alongside the benchmark
sources for local review.
