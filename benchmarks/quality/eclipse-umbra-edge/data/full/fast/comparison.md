# Benchmark comparison

Variants: **reference → fast**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 3.111 | 2.365 | 1.315× | 2.373 |
| eclipse-umbra-edge | advancing | 2.988 | 1.269 | 2.356× | 1.443 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 5.57418 | 34.008% | 0.192203 | 0.077709 |
| eclipse-umbra-edge-spp32 | 4.92154 | 33.718% | 0.157738 | 0.063555 |
| eclipse-umbra-edge-moving-8 | 6.37302 | 33.713% | 0.185463 | 0.079599 |
| eclipse-umbra-edge-moving-15 | 6.86621 | 33.447% | 0.176114 | 0.0797555 |
