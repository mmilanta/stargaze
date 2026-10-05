# Benchmark comparison

Variants: **reference → view12**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 1.464 | 0.889 | 1.647× | 0.893 |
| eclipse-umbra-edge | advancing | 1.469 | 0.887 | 1.657× | 0.889 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 2.65212 | 33.112% | 0.114022 | 0.0483501 |
| eclipse-umbra-edge-spp256 | 1.85215 | 31.456% | 0.0690948 | 0.0282726 |
| eclipse-umbra-edge-moving-2 | 2.84106 | 32.939% | 0.112417 | 0.0487244 |
| eclipse-umbra-edge-moving-3 | 2.94386 | 32.861% | 0.111994 | 0.0490252 |
