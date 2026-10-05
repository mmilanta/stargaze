# Benchmark comparison

Variants: **reference → sun6**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 3.111 | 3.007 | 1.035× | 5.289 |
| eclipse-umbra-edge | advancing | 2.988 | 2.831 | 1.056× | 2.883 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 0.71146 | 28.814% | 0.0119352 | 0.00624122 |
| eclipse-umbra-edge-spp32 | 0.71632 | 28.985% | 0.0115755 | 0.00620928 |
| eclipse-umbra-edge-moving-8 | 0.66070 | 27.389% | 0.00987729 | 0.00555552 |
| eclipse-umbra-edge-moving-15 | 0.62407 | 26.136% | 0.00855039 | 0.00505562 |
