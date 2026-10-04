# RX 7600 XT baseline — October 4, 2026

Measured on an AMD Radeon RX 7600 XT using Vulkan, RADV, Mesa 26.2.2-arch1.1,
with an AMD Ryzen 7 9800X3D. Production renderer revision:
`e7cb17370747b665320cd39135066fc5a1c95554`, plus the test-only benchmark harness.
The renderer and shaders were not optimized for this baseline. Both runs used
identical renderer/fixture source hashes; manifests record the working-tree
state rather than pretending these were clean committed harness builds.

- [Baseline timings](summary.md), [raw samples](results.json), [provenance](manifest.json).
- [Reference image gallery](index.html): open locally; individual PNGs also render on GitHub.
- [Same-code rerun comparison](repeatability.md), [comparison metrics](repeatability.json).
- `repeat-results.json` retains every timing from the second run.

Both complete runs used the suite defaults: 1280×720 display, 960×540 tracing,
4 bounces, 70% vegetation, 8 warm-ups per mode, 3 repetitions of 16 one-sample
frames. Each hardware run took about 84 seconds for timing and image rendering,
plus artifact compression/report generation. No software-renderer results are
included here.

All **28 displayed images and all 28 linear HDR RGB images were exactly identical**
between the two runs. Across atmosphere-enabled views, the largest difference
between whole-run GPU medians was **0.162%**. Across all views/modes it was
**1.004%**, in the sub-millisecond advancing synthetic eclipse case. These are
observed differences from two runs, not confidence bounds or guaranteed future
repeatability. Within-run drift is larger in some short workloads; inspect the
individual repetition medians and p95 before claiming small improvements.

The controlled Vantus pair took **65.513 ms paused / 67.533 ms advancing** with
atmosphere and **2.707 ms / 2.607 ms** without it. Geometry, camera, epoch, and
render settings match. This strongly motivates optimizing atmospheric work;
it does not promise that an atmosphere-preserving optimization can achieve
the airless time. The daylight terrain case took **130.916 / 131.301 ms**.
These are offscreen renderer times, not presented game FPS.

Only PNGs and small reports are stored in the repository (about 16 MiB).
The full baseline at `target/benchmarks/rx7600xt-v1` and its repeat at
`target/benchmarks/rx7600xt-v1-repeat` also contain compressed linear HDR and
RGBA readbacks, about 175 MiB per run. Preserve or archive these locally for
HDR comparisons. Comparisons against this compact repository baseline use PNGs
and explicitly report HDR metrics as unavailable. Do not mistake missing HDR
metrics for zero image error.

See the [suite instructions](../../README.md) for running future comparisons,
changing the workload, and interpreting timing/image differences.
