# Benchmark comparison

Variants: **reference → balanced**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 19.705 | 11.293 | 1.745× | 11.733 |
| halo-rings | advancing | 10.798 | 8.522 | 1.267× | 9.626 |
| earth-daylight | paused | 37.676 | 24.177 | 1.558× | 32.892 |
| earth-daylight | advancing | 36.629 | 22.641 | 1.618× | 22.765 |
| earth-twilight | paused | 20.863 | 18.413 | 1.133× | 19.010 |
| earth-twilight | advancing | 13.390 | 9.428 | 1.420× | 10.429 |
| median-dense | paused | 23.478 | 18.562 | 1.265× | 25.099 |
| median-dense | advancing | 23.222 | 14.360 | 1.617× | 14.450 |
| vantus-eclipse | paused | 14.584 | 9.982 | 1.461× | 10.392 |
| vantus-eclipse | advancing | 9.488 | 8.326 | 1.140× | 9.642 |
| moonlit-air | paused | 3.366 | 1.996 | 1.686× | 2.051 |
| moonlit-air | advancing | 3.039 | 2.030 | 1.497× | 2.040 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.40284 | 14.026% | 0.0113843 | 0.0104124 |
| halo-rings-spp256 | 0.12230 | 2.537% | 0.000728845 | 0.000651831 |
| halo-rings-moving-2 | 1.40292 | 14.027% | 0.0113905 | 0.010418 |
| halo-rings-moving-3 | 1.40278 | 14.025% | 0.0113941 | 0.0104211 |
| earth-daylight-spp1 | 7.66565 | 91.297% | 0.0550626 | 0.0418039 |
| earth-daylight-spp256 | 1.70261 | 62.620% | 0.00529015 | 0.00455491 |
| earth-daylight-moving-2 | 7.66761 | 91.288% | 0.0550636 | 0.0418051 |
| earth-daylight-moving-3 | 7.66832 | 91.299% | 0.0550656 | 0.0418073 |
| earth-twilight-spp1 | 0.34328 | 6.829% | 0.00154156 | 0.00151775 |
| earth-twilight-spp256 | 0.06855 | 0.069% | 0.000112397 | 0.000111798 |
| earth-twilight-moving-2 | 0.34096 | 6.797% | 0.00153629 | 0.00151261 |
| earth-twilight-moving-3 | 0.33992 | 6.779% | 0.00153363 | 0.00151002 |
| median-dense-spp1 | 4.00644 | 93.948% | 0.0110637 | 0.0101593 |
| median-dense-spp256 | 1.83462 | 66.921% | 0.00210562 | 0.00204112 |
| median-dense-moving-2 | 4.00674 | 93.946% | 0.0110652 | 0.0101608 |
| median-dense-moving-3 | 4.00671 | 93.945% | 0.0110653 | 0.0101609 |
| vantus-eclipse-spp1 | 0.34896 | 16.494% | 0.000233963 | 0.000231267 |
| vantus-eclipse-spp256 | 0.33273 | 16.086% | 0.000125698 | 0.000123925 |
| vantus-eclipse-moving-2 | 0.34849 | 16.470% | 0.000230574 | 0.000227912 |
| vantus-eclipse-moving-3 | 0.34881 | 16.548% | 0.000230448 | 0.000227789 |
| moonlit-air-spp1 | 3.32263 | 78.686% | 0.000796606 | 0.000483645 |
| moonlit-air-spp256 | 0.39656 | 4.698% | 0.000555028 | 0.000281505 |
| moonlit-air-moving-2 | 3.32301 | 78.702% | 0.000796676 | 0.000483794 |
| moonlit-air-moving-3 | 3.32293 | 78.704% | 0.000796651 | 0.000483773 |
