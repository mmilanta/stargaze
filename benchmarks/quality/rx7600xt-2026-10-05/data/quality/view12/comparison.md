# Benchmark comparison

Variants: **reference → view12**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 19.705 | 11.338 | 1.738× | 11.754 |
| halo-rings | advancing | 10.798 | 8.375 | 1.289× | 9.596 |
| earth-daylight | paused | 37.676 | 24.289 | 1.551× | 32.798 |
| earth-daylight | advancing | 36.629 | 22.935 | 1.597× | 23.197 |
| earth-twilight | paused | 20.863 | 18.477 | 1.129× | 19.093 |
| earth-twilight | advancing | 13.390 | 9.469 | 1.414× | 10.432 |
| median-dense | paused | 23.478 | 18.354 | 1.279× | 25.166 |
| median-dense | advancing | 23.222 | 14.418 | 1.611× | 14.646 |
| vantus-eclipse | paused | 14.584 | 10.097 | 1.444× | 10.464 |
| vantus-eclipse | advancing | 9.488 | 8.341 | 1.138× | 9.631 |
| moonlit-air | paused | 3.366 | 2.160 | 1.558× | 2.236 |
| moonlit-air | advancing | 3.039 | 2.201 | 1.381× | 2.230 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.39768 | 13.863% | 0.0113833 | 0.0104117 |
| halo-rings-spp256 | 0.11383 | 2.425% | 0.00072409 | 0.00064743 |
| halo-rings-moving-2 | 1.39778 | 13.865% | 0.0113895 | 0.0104173 |
| halo-rings-moving-3 | 1.39767 | 13.861% | 0.0113932 | 0.0104204 |
| earth-daylight-spp1 | 7.03267 | 88.298% | 0.0544372 | 0.0413409 |
| earth-daylight-spp256 | 1.08245 | 58.428% | 0.00385313 | 0.00331247 |
| earth-daylight-moving-2 | 7.03469 | 88.299% | 0.0544384 | 0.0413424 |
| earth-daylight-moving-3 | 7.03530 | 88.312% | 0.0544405 | 0.0413447 |
| earth-twilight-spp1 | 0.35098 | 6.918% | 0.00154515 | 0.00152131 |
| earth-twilight-spp256 | 0.08519 | 0.069% | 0.000123598 | 0.000122841 |
| earth-twilight-moving-2 | 0.34893 | 6.888% | 0.00153989 | 0.00151618 |
| earth-twilight-moving-3 | 0.34789 | 6.879% | 0.00153723 | 0.00151359 |
| median-dense-spp1 | 3.21102 | 93.626% | 0.0107855 | 0.00989483 |
| median-dense-spp256 | 1.02148 | 64.554% | 0.00126506 | 0.00121946 |
| median-dense-moving-2 | 3.21135 | 93.626% | 0.010787 | 0.00989637 |
| median-dense-moving-3 | 3.21141 | 93.627% | 0.0107872 | 0.00989647 |
| vantus-eclipse-spp1 | 0.34895 | 16.494% | 0.000233962 | 0.000231266 |
| vantus-eclipse-spp256 | 0.33272 | 16.085% | 0.000125697 | 0.000123924 |
| vantus-eclipse-moving-2 | 0.34849 | 16.469% | 0.000230574 | 0.000227911 |
| vantus-eclipse-moving-3 | 0.34881 | 16.547% | 0.000230447 | 0.000227788 |
| moonlit-air-spp1 | 3.21846 | 74.078% | 0.000787803 | 0.000477682 |
| moonlit-air-spp256 | 0.37606 | 1.843% | 0.000549727 | 0.000278527 |
| moonlit-air-moving-2 | 3.21869 | 74.063% | 0.000787869 | 0.000477829 |
| moonlit-air-moving-3 | 3.21844 | 74.053% | 0.000787843 | 0.000477809 |
