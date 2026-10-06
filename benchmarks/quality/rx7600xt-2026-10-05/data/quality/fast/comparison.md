# Benchmark comparison

Variants: **reference → fast**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 19.705 | 8.386 | 2.350× | 8.741 |
| halo-rings | advancing | 10.798 | 8.320 | 1.298× | 8.782 |
| earth-daylight | paused | 37.676 | 22.210 | 1.696× | 31.745 |
| earth-daylight | advancing | 36.629 | 17.883 | 2.048× | 17.960 |
| earth-twilight | paused | 20.863 | 16.350 | 1.276× | 16.398 |
| earth-twilight | advancing | 13.390 | 9.106 | 1.471× | 10.041 |
| median-dense | paused | 23.478 | 19.135 | 1.227× | 22.449 |
| median-dense | advancing | 23.222 | 11.099 | 2.092× | 11.467 |
| vantus-eclipse | paused | 14.584 | 7.206 | 2.024× | 7.482 |
| vantus-eclipse | advancing | 9.488 | 7.525 | 1.261× | 7.539 |
| moonlit-air | paused | 3.366 | 1.419 | 2.372× | 1.451 |
| moonlit-air | advancing | 3.039 | 1.519 | 2.000× | 1.523 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.54352 | 17.609% | 0.0116552 | 0.0106354 |
| halo-rings-spp256 | 0.18069 | 4.307% | 0.000847205 | 0.000758283 |
| halo-rings-moving-2 | 1.54333 | 17.606% | 0.0116599 | 0.0106395 |
| halo-rings-moving-3 | 1.54338 | 17.605% | 0.0116636 | 0.0106428 |
| earth-daylight-spp1 | 8.98863 | 91.753% | 0.0561081 | 0.0428039 |
| earth-daylight-spp256 | 3.10780 | 69.349% | 0.0102517 | 0.00878787 |
| earth-daylight-moving-2 | 8.98799 | 91.741% | 0.0561049 | 0.0428007 |
| earth-daylight-moving-3 | 8.98766 | 91.754% | 0.0561043 | 0.0428002 |
| earth-twilight-spp1 | 0.40332 | 8.208% | 0.00159659 | 0.00156879 |
| earth-twilight-spp256 | 0.13464 | 0.230% | 0.000181671 | 0.000179904 |
| earth-twilight-moving-2 | 0.40104 | 8.160% | 0.00159114 | 0.00156345 |
| earth-twilight-moving-3 | 0.39992 | 8.147% | 0.00158839 | 0.00156077 |
| median-dense-spp1 | 6.27779 | 94.571% | 0.0122778 | 0.0113379 |
| median-dense-spp256 | 4.20752 | 78.594% | 0.00489598 | 0.00474866 |
| median-dense-moving-2 | 6.27753 | 94.566% | 0.0122779 | 0.011338 |
| median-dense-moving-3 | 6.27750 | 94.569% | 0.0122779 | 0.011338 |
| vantus-eclipse-spp1 | 0.89234 | 27.543% | 0.000404608 | 0.000399311 |
| vantus-eclipse-spp256 | 0.87424 | 27.570% | 0.000332283 | 0.000327528 |
| vantus-eclipse-moving-2 | 0.89288 | 27.546% | 0.00040454 | 0.000399246 |
| vantus-eclipse-moving-3 | 0.89306 | 27.551% | 0.00040443 | 0.000399139 |
| moonlit-air-spp1 | 5.55089 | 92.480% | 0.00169112 | 0.000952648 |
| moonlit-air-spp256 | 0.83740 | 55.151% | 0.00144931 | 0.000732365 |
| moonlit-air-moving-2 | 5.55091 | 92.484% | 0.00169102 | 0.000952653 |
| moonlit-air-moving-3 | 5.55103 | 92.485% | 0.00169095 | 0.000952635 |
