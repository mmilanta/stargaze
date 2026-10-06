# Benchmark comparison

Variants: **reference → balanced**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 1.464 | 0.861 | 1.702× | 0.861 |
| eclipse-umbra-edge | advancing | 1.469 | 0.859 | 1.710× | 0.860 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 3.28888 | 33.486% | 0.123713 | 0.052219 |
| eclipse-umbra-edge-spp256 | 2.50078 | 32.112% | 0.0804444 | 0.0334411 |
| eclipse-umbra-edge-moving-2 | 3.46015 | 33.380% | 0.121798 | 0.0525385 |
| eclipse-umbra-edge-moving-3 | 3.55356 | 33.329% | 0.121278 | 0.0528268 |
