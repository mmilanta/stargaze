# Benchmark comparison

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 71.652 | 34.734 | 2.063× | 37.597 |
| halo-rings | advancing | 73.638 | 34.218 | 2.152× | 34.349 |
| earth-daylight | paused | 130.916 | 120.498 | 1.086× | 124.182 |
| earth-daylight | advancing | 131.301 | 120.813 | 1.087× | 121.159 |
| earth-twilight | paused | 113.431 | 41.253 | 2.750× | 44.893 |
| earth-twilight | advancing | 114.843 | 40.558 | 2.832× | 40.656 |
| median-dense | paused | 103.667 | 74.684 | 1.388× | 79.530 |
| median-dense | advancing | 104.692 | 74.474 | 1.406× | 74.685 |
| vantus-eclipse | paused | 65.513 | 32.339 | 2.026× | 35.041 |
| vantus-eclipse | advancing | 67.533 | 31.490 | 2.145× | 31.629 |
| vantus-airless | paused | 2.707 | 2.684 | 1.009× | 4.670 |
| vantus-airless | advancing | 2.607 | 2.549 | 1.023× | 2.581 |
| dual-eclipse | paused | 1.480 | 1.668 | 0.887× | 1.683 |
| dual-eclipse | advancing | 0.946 | 0.960 | 0.986× | 1.170 |

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
