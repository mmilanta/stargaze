# Low-light study after integrating PR #2

[Compare images side by side](index.html). These captures combine the Atacama
level and guided-sampling prototype with PR #2's atmospheric extinction fix.
The [original reviewed results](../README.md) remain available.

| Measurement | Production reference | Guided prototype |
|---|---:|---:|
| Foreground luminance RMSE at 64 samples | 0.0066028 | 0.0021206 |
| Mean foreground luminance at 1,024 samples | 0.00527075 | 0.00528500 |
| Median GPU time, advancing simulation | 6.580 ms | 6.639 ms |

Guided sampling retains a **3.11× lower measured foreground error** at 64 samples,
with a **0.27%** difference in mean brightness at 1,024 samples and comparable
GPU cost. Atmospheric extinction makes the ground darker in both versions.
Guided sampling remains benchmark-only; normal gameplay uses the production
reference, including the extinction fix.

This uses the same RX 7600 XT, 480×270 resolution, four bounces, fixed exposure 1,
foreground region and timing protocol as the original study. The 1,024-sample
reference is still noisy, so these are diagnostic measurements from one scene,
not ground-truth accuracy or application FPS. The original report's limitations
and follow-up work still apply. See [summary.json](summary.json),
[advancing.json](advancing.json) and [manifest.json](manifest.json) for raw
metrics, GPU timings and source provenance. Provenance was recorded after capture.

```sh
cargo test --release gpu_lowlight_study -- --ignored --nocapture --test-threads=1
python3 scripts/lowlight_report.py target/lowlight-study docs/low-light/with-extinction
```
