# Benchmark comparison

Variants: **reference → view12**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 35.052 | 19.262 | 1.820× | 19.369 |
| halo-rings | advancing | 35.286 | 19.409 | 1.818× | 19.467 |
| earth-daylight | paused | 125.334 | 76.547 | 1.637× | 76.884 |
| earth-daylight | advancing | 125.428 | 76.337 | 1.643× | 76.577 |
| earth-twilight | paused | 42.415 | 31.096 | 1.364× | 31.276 |
| earth-twilight | advancing | 42.835 | 31.208 | 1.373× | 31.254 |
| median-dense | paused | 78.283 | 47.579 | 1.645× | 47.870 |
| median-dense | advancing | 78.225 | 47.622 | 1.643× | 47.720 |
| vantus-eclipse | paused | 32.892 | 18.248 | 1.803× | 18.350 |
| vantus-eclipse | advancing | 33.123 | 18.417 | 1.798× | 18.455 |
| vantus-airless | paused | 2.519 | 2.506 | 1.005× | 2.528 |
| vantus-airless | advancing | 2.446 | 2.298 | 1.064× | 2.325 |
| dual-eclipse | paused | 0.874 | 1.657 | 0.527× | 1.674 |
| dual-eclipse | advancing | 0.860 | 0.993 | 0.866× | 1.210 |
| moonlit-air | paused | 8.317 | 4.730 | 1.758× | 6.720 |
| moonlit-air | advancing | 7.715 | 4.243 | 1.818× | 4.311 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.40562 | 13.859% | 0.0115106 | 0.0105121 |
| halo-rings-spp32 | 0.29417 | 9.389% | 0.00202318 | 0.00180846 |
| halo-rings-moving-8 | 1.40709 | 13.865% | 0.0115445 | 0.0105361 |
| halo-rings-moving-15 | 1.40726 | 13.866% | 0.0115541 | 0.0105429 |
| earth-daylight-spp1 | 6.99816 | 88.309% | 0.0536246 | 0.0409553 |
| earth-daylight-spp32 | 2.26090 | 81.571% | 0.00963655 | 0.0082356 |
| earth-daylight-moving-8 | 6.99915 | 88.288% | 0.0536391 | 0.0409637 |
| earth-daylight-moving-15 | 6.99884 | 88.269% | 0.053639 | 0.040964 |
| earth-twilight-spp1 | 0.35158 | 6.965% | 0.00156017 | 0.00153551 |
| earth-twilight-spp32 | 0.13895 | 0.486% | 0.000285381 | 0.000284174 |
| earth-twilight-moving-8 | 0.34322 | 6.846% | 0.00153823 | 0.00151436 |
| earth-twilight-moving-15 | 0.33663 | 6.748% | 0.00151953 | 0.00149633 |
| median-dense-spp1 | 3.19499 | 93.649% | 0.0107742 | 0.00987191 |
| median-dense-spp32 | 1.32510 | 79.969% | 0.00217285 | 0.00205508 |
| median-dense-moving-8 | 3.19530 | 93.648% | 0.0107751 | 0.00987287 |
| median-dense-moving-15 | 3.19484 | 93.645% | 0.0107736 | 0.00987145 |
| vantus-eclipse-spp1 | 0.35407 | 16.520% | 0.000265021 | 0.000262082 |
| vantus-eclipse-spp32 | 0.33510 | 16.105% | 0.000130991 | 0.000129177 |
| vantus-eclipse-moving-8 | 0.35415 | 16.606% | 0.000263282 | 0.000260374 |
| vantus-eclipse-moving-15 | 0.35463 | 16.556% | 0.000264694 | 0.000261773 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| moonlit-air-spp1 | 3.22584 | 74.183% | 0.000732218 | 0.000437805 |
| moonlit-air-spp32 | 0.62802 | 15.792% | 0.000560416 | 0.000287959 |
| moonlit-air-moving-8 | 3.22706 | 74.186% | 0.00073184 | 0.000437498 |
| moonlit-air-moving-15 | 3.22748 | 74.173% | 0.000732028 | 0.000437554 |
