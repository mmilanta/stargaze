# Benchmark comparison

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 71.652 | 34.788 | 2.060× | 37.661 |
| halo-rings | advancing | 73.638 | 34.270 | 2.149× | 34.384 |
| earth-daylight | paused | 130.916 | 120.759 | 1.084× | 124.656 |
| earth-daylight | advancing | 131.301 | 121.006 | 1.085× | 121.206 |
| earth-twilight | paused | 113.431 | 41.401 | 2.740× | 45.004 |
| earth-twilight | advancing | 114.843 | 40.778 | 2.816× | 40.888 |
| median-dense | paused | 103.667 | 74.630 | 1.389× | 79.975 |
| median-dense | advancing | 104.692 | 74.394 | 1.407× | 74.582 |
| vantus-eclipse | paused | 65.513 | 32.577 | 2.011× | 35.204 |
| vantus-eclipse | advancing | 67.533 | 31.480 | 2.145× | 31.633 |
| vantus-airless | paused | 2.707 | 2.700 | 1.003× | 4.870 |
| vantus-airless | advancing | 2.607 | 2.556 | 1.020× | 2.607 |
| dual-eclipse | paused | 1.480 | 1.713 | 0.864× | 1.734 |
| dual-eclipse | advancing | 0.946 | 0.938 | 1.009× | 1.127 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.00000 | 0.000% | 0 | 0 |
| halo-rings-spp32 | 0.00000 | 0.000% | 0 | 0 |
| halo-rings-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| halo-rings-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| earth-daylight-spp1 | 0.00000 | 0.000% | 0 | 0 |
| earth-daylight-spp32 | 0.00000 | 0.000% | 0 | 0 |
| earth-daylight-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| earth-daylight-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| earth-twilight-spp1 | 0.00000 | 0.000% | 0 | 0 |
| earth-twilight-spp32 | 0.00000 | 0.000% | 0 | 0 |
| earth-twilight-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| earth-twilight-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| median-dense-spp1 | 0.00000 | 0.000% | 0 | 0 |
| median-dense-spp32 | 0.00000 | 0.000% | 0 | 0 |
| median-dense-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| median-dense-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| vantus-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
