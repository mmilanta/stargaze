# Benchmark comparison

Variants: **reference → planetshine-quarter**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 19.705 | 19.087 | 1.032× | 20.854 |
| halo-rings | advancing | 10.798 | 10.515 | 1.027× | 11.211 |
| earth-daylight | paused | 37.676 | 37.427 | 1.007× | 38.374 |
| earth-daylight | advancing | 36.629 | 36.538 | 1.002× | 37.093 |
| earth-twilight | paused | 20.863 | 19.735 | 1.057× | 21.910 |
| earth-twilight | advancing | 13.390 | 10.845 | 1.235× | 11.086 |
| median-dense | paused | 23.478 | 22.857 | 1.027× | 26.330 |
| median-dense | advancing | 23.222 | 22.775 | 1.020× | 22.890 |
| vantus-eclipse | paused | 14.584 | 14.147 | 1.031× | 17.091 |
| vantus-eclipse | advancing | 9.488 | 8.545 | 1.110× | 9.037 |
| moonlit-air | paused | 3.366 | 3.381 | 0.996× | 3.412 |
| moonlit-air | advancing | 3.039 | 3.075 | 0.988× | 3.186 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.74239 | 21.314% | 0.00218558 | 0.00198558 |
| halo-rings-spp256 | 0.05094 | 0.145% | 0.000182656 | 0.000168029 |
| halo-rings-moving-2 | 0.74230 | 21.319% | 0.00218559 | 0.00198558 |
| halo-rings-moving-3 | 0.74221 | 21.316% | 0.00218546 | 0.00198547 |
| earth-daylight-spp1 | 0.00001 | 0.000% | 2.86912e-08 | 2.51294e-08 |
| earth-daylight-spp256 | 0.00000 | 0.000% | 8.83724e-09 | 7.7105e-09 |
| earth-daylight-moving-2 | 0.00001 | 0.000% | 2.86311e-08 | 2.50994e-08 |
| earth-daylight-moving-3 | 0.00000 | 0.000% | 2.8692e-08 | 2.51299e-08 |
| earth-twilight-spp1 | 0.00000 | 0.000% | 1.50387e-10 | 1.49712e-10 |
| earth-twilight-spp256 | 0.00000 | 0.000% | 4.07713e-11 | 4.02629e-11 |
| earth-twilight-moving-2 | 0.00000 | 0.000% | 1.52092e-10 | 1.514e-10 |
| earth-twilight-moving-3 | 0.00000 | 0.000% | 1.5188e-10 | 1.51179e-10 |
| median-dense-spp1 | 0.00290 | 0.000% | 9.60196e-06 | 9.36342e-06 |
| median-dense-spp256 | 0.00027 | 0.000% | 5.96825e-07 | 5.82056e-07 |
| median-dense-moving-2 | 0.00288 | 0.000% | 9.60029e-06 | 9.36179e-06 |
| median-dense-moving-3 | 0.00289 | 0.000% | 9.59941e-06 | 9.36094e-06 |
| vantus-eclipse-spp1 | 0.00020 | 0.000% | 2.8353e-07 | 2.82594e-07 |
| vantus-eclipse-spp256 | 0.00002 | 0.000% | 1.86128e-08 | 1.85481e-08 |
| vantus-eclipse-moving-2 | 0.00020 | 0.000% | 2.84325e-07 | 2.83387e-07 |
| vantus-eclipse-moving-3 | 0.00020 | 0.000% | 2.84724e-07 | 2.83785e-07 |
| moonlit-air-spp1 | 14.71578 | 94.145% | 0.00155791 | 0.00121719 |
| moonlit-air-spp256 | 0.89745 | 27.045% | 0.000197938 | 0.00016557 |
| moonlit-air-moving-2 | 14.71544 | 94.143% | 0.00155807 | 0.0012173 |
| moonlit-air-moving-3 | 14.71474 | 94.145% | 0.00155808 | 0.00121727 |
