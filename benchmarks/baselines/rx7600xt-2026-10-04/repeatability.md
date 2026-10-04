# Benchmark comparison

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 71.652 | 71.597 | 1.001× | 73.153 |
| halo-rings | advancing | 73.638 | 73.597 | 1.001× | 73.721 |
| earth-daylight | paused | 130.916 | 130.823 | 1.001× | 131.231 |
| earth-daylight | advancing | 131.301 | 131.346 | 1.000× | 131.508 |
| earth-twilight | paused | 113.431 | 113.428 | 1.000× | 114.483 |
| earth-twilight | advancing | 114.843 | 114.836 | 1.000× | 115.004 |
| median-dense | paused | 103.667 | 103.604 | 1.001× | 104.214 |
| median-dense | advancing | 104.692 | 104.719 | 1.000× | 104.858 |
| vantus-eclipse | paused | 65.513 | 65.552 | 0.999× | 67.224 |
| vantus-eclipse | advancing | 67.533 | 67.642 | 0.998× | 68.590 |
| vantus-airless | paused | 2.707 | 2.696 | 1.004× | 4.610 |
| vantus-airless | advancing | 2.607 | 2.606 | 1.000× | 2.660 |
| dual-eclipse | paused | 1.480 | 1.481 | 1.000× | 1.494 |
| dual-eclipse | advancing | 0.946 | 0.937 | 1.010× | 1.170 |

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
