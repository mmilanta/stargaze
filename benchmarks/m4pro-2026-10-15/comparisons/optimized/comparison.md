# Benchmark comparison

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 9.835 | 3.086 | 3.187× | 33.022 |
| halo-rings | advancing | 0.048 | 0.044 | 1.075× | 0.063 |
| earth-daylight | paused | 11.548 | 8.074 | 1.430× | 113.483 |
| earth-daylight | advancing | 0.041 | 0.044 | 0.925× | 0.078 |
| earth-twilight | paused | 119.441 | 2.639 | 45.256× | 20.593 |
| earth-twilight | advancing | 0.041 | 0.046 | 0.892× | 0.067 |
| median-dense | paused | 13.465 | 2.681 | 5.023× | 72.550 |
| median-dense | advancing | 0.042 | 0.049 | 0.851× | 0.078 |
| vantus-eclipse | paused | 10.477 | 2.898 | 3.616× | 3.008 |
| vantus-eclipse | advancing | 0.048 | 0.041 | 1.173× | 0.079 |
| vantus-airless | paused | 0.266 | 0.183 | 1.449× | 1.074 |
| vantus-airless | advancing | 0.265 | 0.186 | 1.426× | 0.196 |
| dual-eclipse | paused | 0.187 | 0.180 | 1.041× | 0.667 |
| dual-eclipse | advancing | 0.207 | 0.436 | 0.476× | 0.631 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.00000 | 0.000% | 2.83942e-09 | 2.83188e-09 |
| halo-rings-spp32 | 0.00000 | 0.000% | 9.48462e-10 | 9.47869e-10 |
| halo-rings-moving-8 | 0.00000 | 0.000% | 2.83739e-09 | 2.82802e-09 |
| halo-rings-moving-15 | 0.00000 | 0.000% | 2.85556e-09 | 2.84604e-09 |
| earth-daylight-spp1 | 0.00003 | 0.000% | 3.63574e-05 | 3.2806e-05 |
| earth-daylight-spp32 | 0.00003 | 0.000% | 7.38794e-06 | 6.28013e-06 |
| earth-daylight-moving-8 | 0.00004 | 0.001% | 3.76116e-05 | 3.39319e-05 |
| earth-daylight-moving-15 | 0.00001 | 0.000% | 7.14443e-07 | 6.46649e-07 |
| earth-twilight-spp1 | 0.00000 | 0.000% | 5.02708e-09 | 5.01046e-09 |
| earth-twilight-spp32 | 0.00000 | 0.000% | 9.97304e-08 | 9.90465e-08 |
| earth-twilight-moving-8 | 0.00000 | 0.000% | 4.97481e-09 | 4.96113e-09 |
| earth-twilight-moving-15 | 0.00000 | 0.000% | 3.27924e-07 | 3.26513e-07 |
| median-dense-spp1 | 0.00005 | 0.001% | 1.08164e-05 | 1.05818e-05 |
| median-dense-spp32 | 0.00002 | 0.000% | 1.40035e-06 | 1.3666e-06 |
| median-dense-moving-8 | 0.00003 | 0.001% | 6.53978e-06 | 6.3919e-06 |
| median-dense-moving-15 | 0.00004 | 0.001% | 1.58377e-05 | 1.55375e-05 |
| vantus-eclipse-spp1 | 0.00000 | 0.000% | 1.06029e-12 | 1.04822e-12 |
| vantus-eclipse-spp32 | 0.00000 | 0.000% | 4.78198e-15 | 4.78191e-15 |
| vantus-eclipse-moving-8 | 0.00000 | 0.000% | 1.49363e-12 | 1.46657e-12 |
| vantus-eclipse-moving-15 | 0.00000 | 0.000% | 1.8687e-13 | 1.86307e-13 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
