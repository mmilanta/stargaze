# Benchmark comparison

Variants: **reference → view12**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 3.111 | 2.811 | 1.107× | 3.199 |
| eclipse-umbra-edge | advancing | 2.988 | 1.727 | 1.730× | 1.756 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 2.63739 | 33.068% | 0.113219 | 0.0480526 |
| eclipse-umbra-edge-spp32 | 1.90749 | 31.914% | 0.0706309 | 0.0290056 |
| eclipse-umbra-edge-moving-8 | 3.36062 | 32.299% | 0.107943 | 0.0495488 |
| eclipse-umbra-edge-moving-15 | 3.76174 | 31.526% | 0.101256 | 0.0495655 |
