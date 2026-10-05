# Benchmark comparison

Variants: **reference → sun6**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 1.464 | 1.430 | 1.024× | 1.436 |
| eclipse-umbra-edge | advancing | 1.469 | 1.430 | 1.027× | 1.430 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 0.71258 | 28.783% | 0.0118383 | 0.00621612 |
| eclipse-umbra-edge-spp256 | 0.71696 | 28.997% | 0.0115697 | 0.00620959 |
| eclipse-umbra-edge-moving-2 | 0.69893 | 28.403% | 0.0112961 | 0.00604186 |
| eclipse-umbra-edge-moving-3 | 0.69230 | 28.237% | 0.0110279 | 0.00595419 |
