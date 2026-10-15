# Benchmark comparison

Variants: **reference → guided-ground**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 9.835 | 1.212 | 8.116× | 13.193 |
| halo-rings | advancing | 0.048 | 0.040 | 1.205× | 0.114 |
| earth-daylight | paused | 11.548 | 3.802 | 3.037× | 58.225 |
| earth-daylight | advancing | 0.041 | 0.036 | 1.145× | 0.068 |
| earth-twilight | paused | 119.441 | 1.569 | 76.144× | 20.585 |
| earth-twilight | advancing | 0.041 | 0.028 | 1.452× | 0.181 |
| median-dense | paused | 13.465 | 1.048 | 12.848× | 36.790 |
| median-dense | advancing | 0.042 | 0.035 | 1.213× | 0.053 |
| vantus-eclipse | paused | 10.477 | 1.145 | 9.150× | 11.518 |
| vantus-eclipse | advancing | 0.048 | 0.034 | 1.417× | 0.067 |
| vantus-airless | paused | 0.266 | 0.188 | 1.411× | 1.610 |
| vantus-airless | advancing | 0.265 | 0.192 | 1.376× | 1.603 |
| dual-eclipse | paused | 0.187 | 0.080 | 2.327× | 0.547 |
| dual-eclipse | advancing | 0.207 | 0.198 | 1.046× | 0.642 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.52859 | 18.173% | 0.0116252 | 0.0106226 |
| halo-rings-spp32 | 0.33444 | 10.092% | 0.00209117 | 0.00186871 |
| halo-rings-moving-8 | 1.53000 | 18.171% | 0.0116452 | 0.0106374 |
| halo-rings-moving-15 | 1.53002 | 18.172% | 0.0116443 | 0.0106364 |
| earth-daylight-spp1 | 20.17868 | 99.632% | 0.0649011 | 0.0524123 |
| earth-daylight-spp32 | 17.37328 | 99.386% | 0.0351745 | 0.0308669 |
| earth-daylight-moving-8 | 20.16928 | 99.632% | 0.0649091 | 0.0524174 |
| earth-daylight-moving-15 | 20.16140 | 99.632% | 0.0649137 | 0.0524198 |
| earth-twilight-spp1 | 0.43440 | 9.418% | 0.00159489 | 0.00156807 |
| earth-twilight-spp32 | 0.20181 | 1.002% | 0.000319093 | 0.000316922 |
| earth-twilight-moving-8 | 0.42333 | 9.255% | 0.00157157 | 0.00154571 |
| earth-twilight-moving-15 | 0.41364 | 9.103% | 0.0015525 | 0.00152716 |
| median-dense-spp1 | 11.13110 | 99.671% | 0.0169209 | 0.01598 |
| median-dense-spp32 | 10.69698 | 99.593% | 0.0127414 | 0.0121953 |
| median-dense-moving-8 | 11.13096 | 99.671% | 0.0169209 | 0.01598 |
| median-dense-moving-15 | 11.13105 | 99.671% | 0.0169212 | 0.0159803 |
| vantus-eclipse-spp1 | 0.89525 | 27.602% | 0.000411857 | 0.000406513 |
| vantus-eclipse-spp32 | 0.87701 | 27.644% | 0.000334747 | 0.000329971 |
| vantus-eclipse-moving-8 | 0.89449 | 27.612% | 0.000405809 | 0.000400514 |
| vantus-eclipse-moving-15 | 0.89546 | 27.615% | 0.000407418 | 0.000402118 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 1.04035e-09 | 9.97777e-10 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 8.75632e-10 | 8.39987e-10 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 1.04266e-09 | 9.99983e-10 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 1.04376e-09 | 1.00102e-09 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 2.75838e-09 | 2.18559e-09 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 2.3187e-09 | 1.9285e-09 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 2.77992e-09 | 2.20349e-09 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 2.8032e-09 | 2.22271e-09 |
