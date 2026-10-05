# Benchmark comparison

Variants: **reference → planetshine-quarter**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 35.052 | 33.603 | 1.043× | 36.148 |
| halo-rings | advancing | 35.286 | 32.189 | 1.096× | 32.672 |
| earth-daylight | paused | 125.334 | 119.570 | 1.048× | 123.341 |
| earth-daylight | advancing | 125.428 | 119.804 | 1.047× | 119.962 |
| earth-twilight | paused | 42.415 | 35.376 | 1.199× | 38.258 |
| earth-twilight | advancing | 42.835 | 34.301 | 1.249× | 34.538 |
| median-dense | paused | 78.283 | 72.867 | 1.074× | 77.482 |
| median-dense | advancing | 78.225 | 72.709 | 1.076× | 73.152 |
| vantus-eclipse | paused | 32.892 | 29.010 | 1.134× | 31.513 |
| vantus-eclipse | advancing | 33.123 | 27.846 | 1.190× | 28.221 |
| vantus-airless | paused | 2.519 | 2.714 | 0.928× | 3.982 |
| vantus-airless | advancing | 2.446 | 2.597 | 0.942× | 2.637 |
| dual-eclipse | paused | 0.874 | 1.660 | 0.526× | 1.669 |
| dual-eclipse | advancing | 0.860 | 1.011 | 0.850× | 1.235 |
| moonlit-air | paused | 8.317 | 8.498 | 0.979× | 8.848 |
| moonlit-air | advancing | 7.715 | 7.648 | 1.009× | 7.791 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.74002 | 21.304% | 0.00217161 | 0.00198202 |
| halo-rings-spp32 | 0.13313 | 0.848% | 0.000448151 | 0.000402446 |
| halo-rings-moving-8 | 0.74017 | 21.308% | 0.00217275 | 0.00198392 |
| halo-rings-moving-15 | 0.74036 | 21.310% | 0.00217319 | 0.00198485 |
| earth-daylight-spp1 | 0.00000 | 0.000% | 2.814e-08 | 2.46608e-08 |
| earth-daylight-spp32 | 0.00000 | 0.000% | 9.25289e-09 | 8.06772e-09 |
| earth-daylight-moving-8 | 0.00000 | 0.000% | 2.81113e-08 | 2.46475e-08 |
| earth-daylight-moving-15 | 0.00001 | 0.000% | 2.80803e-08 | 2.46318e-08 |
| earth-twilight-spp1 | 0.00000 | 0.000% | 1.37207e-10 | 1.36514e-10 |
| earth-twilight-spp32 | 0.00000 | 0.000% | 5.21902e-11 | 4.94721e-11 |
| earth-twilight-moving-8 | 0.00000 | 0.000% | 1.38451e-10 | 1.3775e-10 |
| earth-twilight-moving-15 | 0.00000 | 0.000% | 1.38033e-10 | 1.37341e-10 |
| median-dense-spp1 | 0.00291 | 0.000% | 9.58619e-06 | 9.34796e-06 |
| median-dense-spp32 | 0.00078 | 0.000% | 1.68718e-06 | 1.64558e-06 |
| median-dense-moving-8 | 0.00291 | 0.000% | 9.57948e-06 | 9.34143e-06 |
| median-dense-moving-15 | 0.00295 | 0.000% | 9.57336e-06 | 9.33547e-06 |
| vantus-eclipse-spp1 | 0.00018 | 0.000% | 2.82459e-07 | 2.81519e-07 |
| vantus-eclipse-spp32 | 0.00004 | 0.000% | 5.31157e-08 | 5.2953e-08 |
| vantus-eclipse-moving-8 | 0.00019 | 0.000% | 2.85676e-07 | 2.84728e-07 |
| vantus-eclipse-moving-15 | 0.00019 | 0.000% | 2.88484e-07 | 2.87528e-07 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| moonlit-air-spp1 | 14.71367 | 94.202% | 0.0014298 | 0.00117709 |
| moonlit-air-spp32 | 2.55112 | 67.159% | 0.000438668 | 0.000323777 |
| moonlit-air-moving-8 | 14.71243 | 94.204% | 0.00143026 | 0.00117746 |
| moonlit-air-moving-15 | 14.71092 | 94.200% | 0.00143067 | 0.00117781 |
