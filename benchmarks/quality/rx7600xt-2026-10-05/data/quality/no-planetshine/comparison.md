# Benchmark comparison

Variants: **reference → no-planetshine**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 19.705 | 6.052 | 3.256× | 6.221 |
| halo-rings | advancing | 10.798 | 6.239 | 1.731× | 6.242 |
| earth-daylight | paused | 37.676 | 20.328 | 1.853× | 23.795 |
| earth-daylight | advancing | 36.629 | 19.685 | 1.861× | 19.844 |
| earth-twilight | paused | 20.863 | 15.233 | 1.370× | 15.503 |
| earth-twilight | advancing | 13.390 | 8.898 | 1.505× | 9.913 |
| median-dense | paused | 23.478 | 15.451 | 1.520× | 19.907 |
| median-dense | advancing | 23.222 | 11.857 | 1.958× | 11.956 |
| vantus-eclipse | paused | 14.584 | 5.543 | 2.631× | 5.733 |
| vantus-eclipse | advancing | 9.488 | 5.774 | 1.643× | 5.782 |
| moonlit-air | paused | 3.366 | 0.896 | 3.756× | 0.908 |
| moonlit-air | advancing | 3.039 | 0.954 | 3.184× | 0.961 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.80665 | 51.487% | 0.00324303 | 0.00294687 |
| halo-rings-spp256 | 1.79036 | 51.340% | 0.00278586 | 0.00262338 |
| halo-rings-moving-2 | 1.80647 | 51.487% | 0.00324294 | 0.00294677 |
| halo-rings-moving-3 | 1.80645 | 51.484% | 0.00324292 | 0.00294676 |
| earth-daylight-spp1 | 0.00001 | 0.000% | 2.82432e-08 | 2.48022e-08 |
| earth-daylight-spp256 | 0.00001 | 0.000% | 2.07504e-08 | 1.82246e-08 |
| earth-daylight-moving-2 | 0.00000 | 0.000% | 2.82332e-08 | 2.47961e-08 |
| earth-daylight-moving-3 | 0.00000 | 0.000% | 2.82411e-08 | 2.48011e-08 |
| earth-twilight-spp1 | 0.00000 | 0.000% | 2.34259e-10 | 2.33035e-10 |
| earth-twilight-spp256 | 0.00000 | 0.000% | 6.29386e-11 | 6.13703e-11 |
| earth-twilight-moving-2 | 0.00000 | 0.000% | 2.32883e-10 | 2.31667e-10 |
| earth-twilight-moving-3 | 0.00000 | 0.000% | 2.32416e-10 | 2.31207e-10 |
| median-dense-spp1 | 0.00625 | 0.000% | 9.69351e-06 | 9.43085e-06 |
| median-dense-spp256 | 0.00630 | 0.000% | 6.67508e-06 | 6.51924e-06 |
| median-dense-moving-2 | 0.00628 | 0.000% | 9.69229e-06 | 9.42965e-06 |
| median-dense-moving-3 | 0.00624 | 0.000% | 9.69163e-06 | 9.42901e-06 |
| vantus-eclipse-spp1 | 0.00020 | 0.000% | 1.87063e-07 | 1.8642e-07 |
| vantus-eclipse-spp256 | 0.00018 | 0.000% | 1.07734e-07 | 1.07363e-07 |
| vantus-eclipse-moving-2 | 0.00017 | 0.000% | 1.87694e-07 | 1.8705e-07 |
| vantus-eclipse-moving-3 | 0.00018 | 0.000% | 1.88012e-07 | 1.87366e-07 |
| moonlit-air-spp1 | 75.00656 | 98.858% | 0.00239787 | 0.00187475 |
| moonlit-air-spp256 | 74.50113 | 98.735% | 0.00166422 | 0.0015536 |
| moonlit-air-moving-2 | 75.00287 | 98.857% | 0.00239812 | 0.00187492 |
| moonlit-air-moving-3 | 75.00078 | 98.859% | 0.00239822 | 0.00187498 |
