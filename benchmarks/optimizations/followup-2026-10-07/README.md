# Follow-up optimization investigation

Base: main `96f928c`, including atmospheric extinction for ground lighting.
Candidates remain **benchmark-only**, except the accepted guided ground sampler,
which is now the app default. Benchmark `reference` intentionally retains the
original unguided sampler; `guided-ground` measures the accepted app default. See the
[launch instructions](../../../docs/atacama.md#night-visibility).

## Status

Eight candidates are implemented and shader-validated. The raw-candidate image
equivalence test passed on the RX 7600 XT (RADV Mesa 26.2.2-arch1.1): 11 scenes,
two simulation times, 33×19 pixels and four samples per view, comparing all HDR
channels exactly. This is a small correctness check, not a performance result.

The Python timing runner and direct invocation with `STARGAZE_BENCH_CONFIG`
both reported `Available adapters: []` in this session. Device visibility differs
between execution contexts; `/dev/dri` was absent in the ordinary shell. The
benchmark correctly refused to use software rendering. **No new frame-time or
speedup measurements are available.** Failed attempts are under
`target/optimization-study-baseline` and `target/optimization-study-oct7`.
Use a new output directory when running again.

Ordinary Rust tests validate all variants and the new fixture inputs. Python
tests cover comparison metrics, bracketing/reversed order, speedup ranges that
include regressions, and refusing equivalence when HDR differs despite identical
display pixels. No candidate images have been accepted or discarded.

## Implemented candidates

The work reductions below describe operations, **not expected FPS percentages**.
The total benefit depends on visibility, divergent GPU execution and how much of
the scene's time is spent in each path.

| Variant | Change | Expected benefit | Compromise / acceptance check |
|---|---|---|---|
| `sky-any-hit` | Ground sky-light rays stop at the first opaque blocker instead of computing the closest hit | Less terrain/prop traversal in forest and rocky ground views; avoid unneeded hit details | Intended identical rendering; exact HDR check passed on small suite. Full images and timing still required. Ring transmission is preserved. |
| `dark-reflection` | Skip density integration and phase evaluation when sampled reflected incident light is exactly zero | Avoid four density samples plus attenuation/phase math on dark or eclipsed reflecting surfaces | Same samples and random draws; no dim-light threshold. Exact HDR check passed. Added branch may cost time in mostly lit views. |
| `raw-combined` | Both changes above | Tests whether the savings combine | Same rendering intent; exact HDR check passed. GPU compiler/register effects need measurement. |
| `view6-sun4` | Six view integration cells instead of eight; retain four light-density samples | 25% fewer atmospheric view cells and their light/shadow queries | Can miss narrow eclipse structure or shift haze/brightness. Inspect totality-edge, twilight and dense air at high spp. |
| `view8-sun2` | Two light-path density samples instead of four; retain eight view cells | Half as many density samples per light attenuation integral | Optical depth near the horizon may change; also affects ground-light extinction. Shadow-ray counts are unchanged. |
| `reflect-half` | Evaluate reflected atmospheric lighting in four of eight cells, chosen by random parity and weighted by two | Half as many reflected-light evaluations per air column | Targets the same discrete integral in expectation, but raises variance; can worsen night flicker and the fixed sky cache. Direct-star RNG is untouched. |
| `guided-ground` | Existing 50/50 cosine/reflector-directed ground continuation, now available throughout the suite | Faster convergence where large illuminated planets light the ground | Added body selection/PDF work; samples change, so image equality is not expected. Earlier single-view study measured 3.11× lower foreground error at comparable cost, not a suite-wide speedup. |
| `guided-raw` | Guided ground sampling plus both raw candidates | Tests quality improvement together with saved ray work | Same sampling caveats; compare directly with `guided-ground` as well as production. |

The guided sampler's [previous measurements](../../../docs/low-light/with-extinction/README.md)
use a noisy 1,024-sample reference; they do not establish exact convergence.

## Benchmark coverage and execution

Added optional `atacama-day` and `atacama-night` cases to the standard harness.
They use the desert initial camera at days 0.14 and 0, then advance one simulated
second per sequence frame. Existing seven default scenes and their geometry are
unchanged. The study includes all eleven scenes by default, including the hard
`eclipse-umbra-edge` and `moonlit-air` cases.

```sh
# Exact raw-candidate check (prints adapter; no timing claims):
cargo test --release gpu_raw_optimization_equivalence -- --ignored --nocapture --test-threads=1

# Two complete rounds at 640×360, 100% scale; reverse candidate order in round 2:
python3 scripts/optimization_study.py target/optimization-study-rerun

# Repeat promising candidates at the existing published benchmark resolution:
python3 scripts/optimization_study.py target/optimization-study-native \
  --width 1280 --height 720 --scale 75 --variants sky-any-hit dark-reflection raw-combined

# Review approximation bias and noise with more samples, retaining every option:
python3 scripts/optimization_study.py target/optimization-study-quality \
  --rounds 1 --image-samples 256 \
  --cases earth-twilight median-dense moonlit-air eclipse-umbra-edge atacama-day atacama-night \
  --variants view6-sun4 view8-sun2 reflect-half guided-ground guided-raw
```

Each round runs an unchanged reference before and after the candidate batch.
Reports compare every candidate against both references, retain individual
timings and all images, and expose the full speedup range across brackets/rounds.
Those ranges are not confidence intervals. Small gains within repeat variation
are inconclusive. `reference-drift` comparisons reveal same-code timing/image
variation. The HTML index links each before/after/difference gallery; raw image
equivalence is reported separately from performance.

`--plan-only` writes commands without executing them. Output directories must be
new. A failure preserves partial evidence and prevents a completed study report.
Shader changes remain isolated to test builds in `src/render_optimizations.rs`.
The old quality review keeps its original seven variants and does not require
new candidates to reproduce archived reports.

## Further opportunities, in priority order

1. **Density/transmittance lookup table.** A host-relative altitude/direction table
   could replace repeated exponential density integration for sun and planet rays.
   Keep actual finite-disc samples and eclipse visibility rays. Expected benefit:
   fewer square roots/exponentials per atmospheric cell. Costs: table generation,
   storage, interpolation and approximation error near the horizon; validate the
   sharp totality boundary. This is more targeted than caching the entire sky,
   whose illumination changes across an eclipse.
2. **Reuse the ground sky cache on camera-only rotations.** The cache describes
   directions in the local ground frame, while the current history invalidation
   rebuilds it for camera rotation too. A separate key based on observer position,
   physical lighting and atmosphere could retain it while panning. This saves
   128 atmosphere evaluations on affected frames, not during advancing simulation.
   Coordinate/rounding invariance and resize/scene-switch behavior need tests.
3. **Workgroup and register-pressure experiments.** The trace kernel mixes long
   atmosphere paths, terrain traversal and short misses in an 8×8 group. Test
   alternative layouts with a matching dispatch and inspect actual shader
   statistics. Potentially preserves every ray; gains are hardware-dependent.
   Changing only the WGSL group size would leave holes/overlaps and is not valid.
4. **Ground-only spatial/temporal denoising.** Guided sampling improves the input;
   a depth/normal-aware filter could reduce samples needed for acceptable motion.
   Costs include auxiliary buffers and passes, softened small rocks/shadows, and
   history rejection for moving eclipses to avoid ghosting. Keep a moving-image
   review separate from static RMSE and keep catalogue stars outside the filter.
5. **More selective body/ring iteration.** Compact ring indices or conservative
   per-view body bounds can cut scans in larger systems. Current small systems may
   gain little; distinguish source-selection/visibility cost from atmosphere
   integration before adding CPU lists or another acceleration structure.

Promote raw candidates only after full-size image equivalence and reproducible
timing wins. For approximation/sampling candidates, retain the images for user
review and validate dense twilight, night ground and the hard eclipse boundary.
