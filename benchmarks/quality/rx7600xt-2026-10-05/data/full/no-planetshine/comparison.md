# Benchmark comparison

Variants: **reference → no-planetshine**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 35.052 | 11.614 | 3.018× | 12.213 |
| halo-rings | advancing | 35.286 | 10.136 | 3.481× | 10.391 |
| earth-daylight | paused | 125.334 | 64.688 | 1.937× | 69.446 |
| earth-daylight | advancing | 125.428 | 63.945 | 1.962× | 64.015 |
| earth-twilight | paused | 42.415 | 26.300 | 1.613× | 28.480 |
| earth-twilight | advancing | 42.835 | 24.711 | 1.733× | 24.946 |
| median-dense | paused | 78.283 | 38.789 | 2.018× | 41.876 |
| median-dense | advancing | 78.225 | 36.980 | 2.115× | 37.180 |
| vantus-eclipse | paused | 32.892 | 11.591 | 2.838× | 12.052 |
| vantus-eclipse | advancing | 33.123 | 9.993 | 3.315× | 10.329 |
| vantus-airless | paused | 2.519 | 2.680 | 0.940× | 4.455 |
| vantus-airless | advancing | 2.446 | 2.529 | 0.967× | 2.581 |
| dual-eclipse | paused | 0.874 | 1.499 | 0.583× | 1.508 |
| dual-eclipse | advancing | 0.860 | 0.974 | 0.883× | 1.220 |
| moonlit-air | paused | 8.317 | 2.479 | 3.355× | 3.620 |
| moonlit-air | advancing | 7.715 | 1.847 | 4.178× | 1.890 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.79741 | 51.463% | 0.00330229 | 0.00300759 |
| halo-rings-spp32 | 1.78788 | 51.253% | 0.00283044 | 0.00265235 |
| halo-rings-moving-8 | 1.79764 | 51.461% | 0.00330637 | 0.00301044 |
| halo-rings-moving-15 | 1.79764 | 51.459% | 0.00330691 | 0.00301094 |
| earth-daylight-spp1 | 0.00001 | 0.000% | 2.82271e-08 | 2.47988e-08 |
| earth-daylight-spp32 | 0.00001 | 0.000% | 2.08492e-08 | 1.83003e-08 |
| earth-daylight-moving-8 | 0.00001 | 0.000% | 2.82685e-08 | 2.4818e-08 |
| earth-daylight-moving-15 | 0.00000 | 0.000% | 2.82616e-08 | 2.4812e-08 |
| earth-twilight-spp1 | 0.00000 | 0.000% | 2.16699e-10 | 2.15544e-10 |
| earth-twilight-spp32 | 0.00000 | 0.000% | 7.20524e-11 | 6.99477e-11 |
| earth-twilight-moving-8 | 0.00000 | 0.000% | 2.16577e-10 | 2.15441e-10 |
| earth-twilight-moving-15 | 0.00000 | 0.000% | 2.17158e-10 | 2.16044e-10 |
| median-dense-spp1 | 0.00624 | 0.000% | 9.67701e-06 | 9.41532e-06 |
| median-dense-spp32 | 0.00630 | 0.000% | 6.78146e-06 | 6.62167e-06 |
| median-dense-moving-8 | 0.00616 | 0.000% | 9.67232e-06 | 9.41071e-06 |
| median-dense-moving-15 | 0.00621 | 0.000% | 9.66743e-06 | 9.40592e-06 |
| vantus-eclipse-spp1 | 0.00017 | 0.000% | 1.87236e-07 | 1.8659e-07 |
| vantus-eclipse-spp32 | 0.00018 | 0.000% | 1.16536e-07 | 1.16156e-07 |
| vantus-eclipse-moving-8 | 0.00018 | 0.000% | 1.89769e-07 | 1.89116e-07 |
| vantus-eclipse-moving-15 | 0.00018 | 0.000% | 1.92013e-07 | 1.91353e-07 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| moonlit-air-spp1 | 75.10205 | 98.917% | 0.00189159 | 0.00164273 |
| moonlit-air-spp32 | 75.14925 | 98.875% | 0.00179811 | 0.00159569 |
| moonlit-air-moving-8 | 75.08524 | 98.918% | 0.00189243 | 0.00164312 |
| moonlit-air-moving-15 | 75.06847 | 98.917% | 0.0018932 | 0.00164345 |
