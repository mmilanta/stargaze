# Benchmark comparison

Variants: **reference → fast**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 35.052 | 14.450 | 2.426× | 15.409 |
| halo-rings | advancing | 35.286 | 13.380 | 2.637× | 13.669 |
| earth-daylight | paused | 125.334 | 56.881 | 2.203× | 61.638 |
| earth-daylight | advancing | 125.428 | 56.320 | 2.227× | 56.565 |
| earth-twilight | paused | 42.415 | 26.991 | 1.571× | 29.510 |
| earth-twilight | advancing | 42.835 | 25.686 | 1.668× | 26.040 |
| median-dense | paused | 78.283 | 36.446 | 2.148× | 39.962 |
| median-dense | advancing | 78.225 | 35.055 | 2.231× | 35.203 |
| vantus-eclipse | paused | 32.892 | 13.551 | 2.427× | 14.268 |
| vantus-eclipse | advancing | 33.123 | 12.617 | 2.625× | 12.913 |
| vantus-airless | paused | 2.519 | 2.731 | 0.922× | 4.259 |
| vantus-airless | advancing | 2.446 | 2.597 | 0.942× | 2.633 |
| dual-eclipse | paused | 0.874 | 1.667 | 0.524× | 1.688 |
| dual-eclipse | advancing | 0.860 | 0.980 | 0.877× | 1.187 |
| moonlit-air | paused | 8.317 | 3.208 | 2.593× | 5.575 |
| moonlit-air | advancing | 7.715 | 2.892 | 2.667× | 2.930 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.54370 | 17.578% | 0.0116277 | 0.0106247 |
| halo-rings-spp32 | 0.34106 | 9.983% | 0.00209173 | 0.00186922 |
| halo-rings-moving-8 | 1.54512 | 17.577% | 0.011648 | 0.0106397 |
| halo-rings-moving-15 | 1.54520 | 17.585% | 0.0116461 | 0.0106379 |
| earth-daylight-spp1 | 8.99425 | 91.775% | 0.055541 | 0.0425806 |
| earth-daylight-spp32 | 4.21167 | 86.894% | 0.0137179 | 0.0117301 |
| earth-daylight-moving-8 | 8.99195 | 91.783% | 0.0555507 | 0.0425867 |
| earth-daylight-moving-15 | 8.99034 | 91.793% | 0.0555531 | 0.0425889 |
| earth-twilight-spp1 | 0.40173 | 8.274% | 0.00158551 | 0.00155928 |
| earth-twilight-spp32 | 0.18883 | 0.704% | 0.000318465 | 0.000316297 |
| earth-twilight-moving-8 | 0.39243 | 8.112% | 0.00156231 | 0.00153702 |
| earth-twilight-moving-15 | 0.38424 | 7.966% | 0.00154334 | 0.00151857 |
| median-dense-spp1 | 6.26022 | 94.466% | 0.0122495 | 0.0113055 |
| median-dense-spp32 | 4.41429 | 84.961% | 0.00524037 | 0.00506015 |
| median-dense-moving-8 | 6.26026 | 94.465% | 0.0122499 | 0.0113059 |
| median-dense-moving-15 | 6.26028 | 94.466% | 0.0122491 | 0.0113052 |
| vantus-eclipse-spp1 | 0.89393 | 27.559% | 0.000411058 | 0.000405717 |
| vantus-eclipse-spp32 | 0.87568 | 27.581% | 0.000334491 | 0.000329718 |
| vantus-eclipse-moving-8 | 0.89334 | 27.558% | 0.000406141 | 0.000400845 |
| vantus-eclipse-moving-15 | 0.89466 | 27.561% | 0.000407279 | 0.000401982 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| moonlit-air-spp1 | 5.55888 | 92.524% | 0.00161428 | 0.000888308 |
| moonlit-air-spp32 | 1.19528 | 69.737% | 0.00146307 | 0.000743321 |
| moonlit-air-moving-8 | 5.56059 | 92.527% | 0.0016144 | 0.000888443 |
| moonlit-air-moving-15 | 5.56177 | 92.524% | 0.00161455 | 0.000888521 |
