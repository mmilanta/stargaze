# Benchmark comparison

Variants: **reference → sun6**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 35.052 | 35.027 | 1.001× | 35.230 |
| halo-rings | advancing | 35.286 | 35.319 | 0.999× | 35.383 |
| earth-daylight | paused | 125.334 | 124.021 | 1.011× | 124.514 |
| earth-daylight | advancing | 125.428 | 124.027 | 1.011× | 124.463 |
| earth-twilight | paused | 42.415 | 42.227 | 1.004× | 42.663 |
| earth-twilight | advancing | 42.835 | 42.653 | 1.004× | 42.772 |
| median-dense | paused | 78.283 | 77.329 | 1.012× | 77.591 |
| median-dense | advancing | 78.225 | 77.079 | 1.015× | 77.299 |
| vantus-eclipse | paused | 32.892 | 32.783 | 1.003× | 32.959 |
| vantus-eclipse | advancing | 33.123 | 33.051 | 1.002× | 33.093 |
| vantus-airless | paused | 2.519 | 2.539 | 0.992× | 2.555 |
| vantus-airless | advancing | 2.446 | 2.459 | 0.994× | 2.471 |
| dual-eclipse | paused | 0.874 | 0.877 | 0.996× | 0.883 |
| dual-eclipse | advancing | 0.860 | 0.860 | 1.000× | 0.868 |
| moonlit-air | paused | 8.317 | 8.134 | 1.022× | 8.193 |
| moonlit-air | advancing | 7.715 | 7.664 | 1.007× | 7.694 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.02687 | 0.002% | 4.41961e-05 | 3.97868e-05 |
| halo-rings-spp32 | 0.02553 | 0.000% | 3.53117e-05 | 3.30766e-05 |
| halo-rings-moving-8 | 0.02684 | 0.002% | 4.43574e-05 | 3.98977e-05 |
| halo-rings-moving-15 | 0.02686 | 0.002% | 4.44167e-05 | 3.99488e-05 |
| earth-daylight-spp1 | 0.76735 | 35.285% | 0.00243643 | 0.00206771 |
| earth-daylight-spp32 | 0.78919 | 34.946% | 0.00230818 | 0.00200005 |
| earth-daylight-moving-8 | 0.76684 | 35.288% | 0.00243529 | 0.0020667 |
| earth-daylight-moving-15 | 0.76626 | 35.287% | 0.00243427 | 0.00206581 |
| earth-twilight-spp1 | 0.01603 | 0.000% | 2.54437e-05 | 2.36407e-05 |
| earth-twilight-spp32 | 0.01587 | 0.000% | 2.3514e-05 | 2.24281e-05 |
| earth-twilight-moving-8 | 0.01539 | 0.000% | 2.50433e-05 | 2.33462e-05 |
| earth-twilight-moving-15 | 0.01533 | 0.000% | 2.51824e-05 | 2.34245e-05 |
| median-dense-spp1 | 0.88523 | 58.238% | 0.000970332 | 0.000939809 |
| median-dense-spp32 | 0.88953 | 58.116% | 0.000943952 | 0.000918797 |
| median-dense-moving-8 | 0.88530 | 58.238% | 0.000970336 | 0.000939813 |
| median-dense-moving-15 | 0.88524 | 58.226% | 0.000970338 | 0.000939815 |
| vantus-eclipse-spp1 | 0.00000 | 0.000% | 6.05475e-09 | 6.03455e-09 |
| vantus-eclipse-spp32 | 0.00000 | 0.000% | 2.6683e-09 | 2.65892e-09 |
| vantus-eclipse-moving-8 | 0.00000 | 0.000% | 6.20326e-09 | 6.1826e-09 |
| vantus-eclipse-moving-15 | 0.00000 | 0.000% | 6.33399e-09 | 6.31294e-09 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| moonlit-air-spp1 | 0.56330 | 2.505% | 2.98776e-05 | 2.44996e-05 |
| moonlit-air-spp32 | 0.56795 | 1.700% | 2.76705e-05 | 2.34783e-05 |
| moonlit-air-moving-8 | 0.56321 | 2.479% | 2.98965e-05 | 2.45101e-05 |
| moonlit-air-moving-15 | 0.56339 | 2.481% | 2.99133e-05 | 2.45191e-05 |
