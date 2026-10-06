# Benchmark comparison

Variants: **reference → no-planetshine**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 1.464 | 1.305 | 1.123× | 1.310 |
| eclipse-umbra-edge | advancing | 1.469 | 1.306 | 1.125× | 1.308 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-umbra-edge-spp256 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-umbra-edge-moving-2 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-umbra-edge-moving-3 | 0.00000 | 0.000% | 0 | 0 |
