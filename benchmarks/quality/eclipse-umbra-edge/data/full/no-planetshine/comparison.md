# Benchmark comparison

Variants: **reference → no-planetshine**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 3.111 | 2.945 | 1.057× | 4.837 |
| eclipse-umbra-edge | advancing | 2.988 | 2.630 | 1.136× | 2.673 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-umbra-edge-spp32 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-umbra-edge-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-umbra-edge-moving-15 | 0.00000 | 0.000% | 0 | 0 |
