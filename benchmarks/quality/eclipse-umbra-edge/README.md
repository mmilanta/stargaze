# Eclipse at the edge of totality

[Open the interactive comparison](index.html) or [view the current renderer's image](images/reference/eclipse-umbra-edge-spp32.png).
All six experiments are retained for your review, at 1, 32 and 256 samples per pixel.
No visual option has been accepted or discarded, and application rendering defaults are unchanged.
Open the HTML locally; GitHub displays the PNGs but does not execute the viewer.

## Scene

`eclipse-umbra-edge` places the observer 2 metres above an Earth-sized world,
looking 12 degrees above the horizon, just inside the shadow of a 1,000 km-radius
occultor 100,000 km away. A tiny star at 1 AU has an apparent diameter of about
0.0094 tracing pixels at the full benchmark resolution. Its brightness is chosen
to keep the lit atmosphere visible despite that tiny solid angle.

The observer and atmosphere on the right are fully eclipsed. Dense air on the
left still sees the entire star, creating a sharp boundary within the scattering
volume. The ground and occultor are black to isolate direct atmospheric sunlight
from reflected light. The benchmark retains the background starfield; the
lighting regression test disables it to measure darkness independently.

The shadow advances at 100 m/s. Forty seconds before the initial scene, the
star was completely unobscured at the observer. At the start and both captured
advancing checkpoints, the observer remains in totality while air on the left
remains sunlit. This stresses preservation of narrow illuminated regions as
atmospheric integration steps are reduced. It is a difficult appearance test,
not necessarily the slowest workload: much of the image is eclipsed.

Inspect the transition position, width, brightness and gradients using the wipe,
whole-image toggle and amplified difference. Extra Monte Carlo samples help
separate noise from persistent integration changes, but do not provide physical
ground truth. The 256-spp images use a smaller resolution; compare each variant
against the reference at the same setting.

## Measurements

RX 7600 XT, RADV / Mesa 26.2.2-arch1.1. Display 1280×720, tracing 960×540,
four bounces, eight warmups, three repeats of sixteen frames. Advancing frames
reset accumulation and render one sample. The reference median was **2.988 ms**.
These are offscreen GPU renderer times, not application FPS.

Single-session measurements; GPU clocks and background activity were not
controlled. Small differences are inconclusive. Several paused runs show large
variation between repetitions; retain the raw measurements and rerun before
making performance decisions. The table uses advancing frames only.

| Variant | Advancing GPU median | Observed speedup |
|---|---:|---:|
| `sun6` | 2.831 ms | 1.06× |
| `view12` | 1.727 ms | 1.73× |
| `balanced` | 1.658 ms | 1.80× |
| `fast` | 1.269 ms | 2.36× |
| `planetshine-quarter` | 2.949 ms | 1.01× |
| `no-planetshine` | 2.630 ms | 1.14× |

Each candidate's full measurements are linked from the viewer. The black bodies
make this case primarily a direct-light test; use `moonlit-air` in the
[earlier review](../rx7600xt-2026-10-05/index.html) to judge reflected-light approximations.

## Moving checkpoints

These are separate 1-spp snapshots, not a temporal-flicker metric.

| Renderer | After 8 seconds | After 15 seconds |
|---|---|---|
| `reference` | [Image](images/reference/eclipse-umbra-edge-moving-8.png) | [Image](images/reference/eclipse-umbra-edge-moving-15.png) |
| `sun6` | [Image](images/sun6/eclipse-umbra-edge-moving-8.png) | [Image](images/sun6/eclipse-umbra-edge-moving-15.png) |
| `view12` | [Image](images/view12/eclipse-umbra-edge-moving-8.png) | [Image](images/view12/eclipse-umbra-edge-moving-15.png) |
| `balanced` | [Image](images/balanced/eclipse-umbra-edge-moving-8.png) | [Image](images/balanced/eclipse-umbra-edge-moving-15.png) |
| `fast` | [Image](images/fast/eclipse-umbra-edge-moving-8.png) | [Image](images/fast/eclipse-umbra-edge-moving-15.png) |
| `planetshine-quarter` | [Image](images/planetshine-quarter/eclipse-umbra-edge-moving-8.png) | [Image](images/planetshine-quarter/eclipse-umbra-edge-moving-15.png) |
| `no-planetshine` | [Image](images/no-planetshine/eclipse-umbra-edge-moving-8.png) | [Image](images/no-planetshine/eclipse-umbra-edge-moving-15.png) |

## Validation and reproduction

Independent finite-disc geometry checks verify the star's apparent size,
pre-ingress visibility, observer/right-hand totality and fully lit neighbouring
air at the three checkpoints. The production-shader GPU regression test passed:
mean linear RGB in the left atmospheric region was 0.306, 0.269 and 0.251;
matching right-hand air and ground regions were zero with the background disabled.
All seven shader variants completed hardware captures. The Rust suite passed
106 tests (46 hardware/benchmark tests ignored), and four Python tests passed.

```sh
python3 scripts/benchmark.py run target/benchmarks/umbra-reference --cases eclipse-umbra-edge
python3 scripts/benchmark.py run target/benchmarks/umbra-fast --cases eclipse-umbra-edge --variant fast
python3 scripts/benchmark.py compare target/benchmarks/umbra-reference target/benchmarks/umbra-fast target/benchmarks/umbra-comparison
cargo test --release gpu_umbra_edge_contrast -- --ignored --nocapture --test-threads=1
```

For 256-spp images add `--width 640 --height 360 --warmup 2 --frames 4
--repeats 1 --image-samples 256` to both runs. The original seven default cases
remain unchanged for historical comparisons; explicitly select this new case or
append it to your case list.

This capture used `render_umbra_edge_study`, the same benchmark implementation
called directly for all seven variants at both settings. Its raw outputs remain
under `target/benchmarks/umbra-edge-study/`; Python postprocessing produced the
summaries and image comparisons. The bundle retains request parameters, source
hashes, raw frame timings, PNGs and numeric HDR comparisons. Compressed HDR
buffers remain in the local output directory. The provenance snapshot precedes
capture; subsequent edits add documentation and a CPU pre-ingress assertion.

The interactive controls, image paths, statistics and report links were checked
across all 18 variant/sample combinations. Visual acceptance remains yours.
