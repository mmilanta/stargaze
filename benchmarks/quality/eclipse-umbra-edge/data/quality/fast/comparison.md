# Benchmark comparison

Variants: **reference → fast**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 1.464 | 0.675 | 2.170× | 0.677 |
| eclipse-umbra-edge | advancing | 1.469 | 0.674 | 2.181× | 0.678 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 5.57683 | 34.029% | 0.193656 | 0.0779741 |
| eclipse-umbra-edge-spp256 | 4.88359 | 33.579% | 0.156533 | 0.0630624 |
| eclipse-umbra-edge-moving-2 | 5.78999 | 33.946% | 0.191073 | 0.0783169 |
| eclipse-umbra-edge-moving-3 | 5.90334 | 33.915% | 0.191775 | 0.0791007 |
