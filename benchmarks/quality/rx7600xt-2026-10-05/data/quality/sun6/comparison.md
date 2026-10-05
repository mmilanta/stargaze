# Benchmark comparison

Variants: **reference → sun6**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 19.705 | 19.029 | 1.036× | 21.248 |
| halo-rings | advancing | 10.798 | 10.826 | 0.997× | 11.023 |
| earth-daylight | paused | 37.676 | 37.472 | 1.005× | 38.212 |
| earth-daylight | advancing | 36.629 | 36.508 | 1.003× | 36.664 |
| earth-twilight | paused | 20.863 | 20.898 | 0.998× | 26.406 |
| earth-twilight | advancing | 13.390 | 13.271 | 1.009× | 13.418 |
| median-dense | paused | 23.478 | 23.207 | 1.012× | 27.352 |
| median-dense | advancing | 23.222 | 23.146 | 1.003× | 23.184 |
| vantus-eclipse | paused | 14.584 | 14.573 | 1.001× | 18.688 |
| vantus-eclipse | advancing | 9.488 | 9.529 | 0.996× | 9.704 |
| moonlit-air | paused | 3.366 | 3.363 | 1.001× | 3.402 |
| moonlit-air | advancing | 3.039 | 3.066 | 0.991× | 3.177 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.02658 | 0.000% | 4.3813e-05 | 3.94237e-05 |
| halo-rings-spp256 | 0.02564 | 0.000% | 3.34965e-05 | 3.19773e-05 |
| halo-rings-moving-2 | 0.02663 | 0.000% | 4.38121e-05 | 3.94228e-05 |
| halo-rings-moving-3 | 0.02662 | 0.000% | 4.38075e-05 | 3.94187e-05 |
| earth-daylight-spp1 | 0.76741 | 35.428% | 0.00244252 | 0.0020684 |
| earth-daylight-spp256 | 0.79233 | 35.106% | 0.00230077 | 0.00199447 |
| earth-daylight-moving-2 | 0.76737 | 35.432% | 0.00244225 | 0.00206816 |
| earth-daylight-moving-3 | 0.76751 | 35.435% | 0.00244207 | 0.00206802 |
| earth-twilight-spp1 | 0.01565 | 0.000% | 2.69819e-05 | 2.4996e-05 |
| earth-twilight-spp256 | 0.01584 | 0.000% | 2.17733e-05 | 2.13507e-05 |
| earth-twilight-moving-2 | 0.01560 | 0.000% | 2.6987e-05 | 2.49967e-05 |
| earth-twilight-moving-3 | 0.01564 | 0.001% | 2.69628e-05 | 2.4977e-05 |
| median-dense-spp1 | 0.88570 | 58.226% | 0.000970513 | 0.000940141 |
| median-dense-spp256 | 0.89015 | 58.114% | 0.000942644 | 0.00091762 |
| median-dense-moving-2 | 0.88569 | 58.226% | 0.000970515 | 0.000940143 |
| median-dense-moving-3 | 0.88572 | 58.228% | 0.000970517 | 0.000940144 |
| vantus-eclipse-spp1 | 0.00000 | 0.000% | 6.03887e-09 | 6.01819e-09 |
| vantus-eclipse-spp256 | 0.00000 | 0.000% | 2.22741e-09 | 2.21781e-09 |
| vantus-eclipse-moving-2 | 0.00000 | 0.000% | 6.07577e-09 | 6.05498e-09 |
| vantus-eclipse-moving-3 | 0.00000 | 0.000% | 6.09432e-09 | 6.07345e-09 |
| moonlit-air-spp1 | 0.56293 | 2.481% | 3.67035e-05 | 2.89507e-05 |
| moonlit-air-spp256 | 0.56344 | 1.647% | 2.3727e-05 | 2.19976e-05 |
| moonlit-air-moving-2 | 0.56306 | 2.497% | 3.67092e-05 | 2.89543e-05 |
| moonlit-air-moving-3 | 0.56303 | 2.493% | 3.67132e-05 | 2.89565e-05 |
