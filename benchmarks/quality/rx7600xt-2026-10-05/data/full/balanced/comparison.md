# Benchmark comparison

Variants: **reference → balanced**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 35.052 | 20.093 | 1.744× | 23.077 |
| halo-rings | advancing | 35.286 | 18.681 | 1.889× | 19.280 |
| earth-daylight | paused | 125.334 | 73.123 | 1.714× | 78.826 |
| earth-daylight | advancing | 125.428 | 72.850 | 1.722× | 73.078 |
| earth-twilight | paused | 42.415 | 31.119 | 1.363× | 33.551 |
| earth-twilight | advancing | 42.835 | 29.396 | 1.457× | 29.983 |
| median-dense | paused | 78.283 | 45.934 | 1.704× | 49.647 |
| median-dense | advancing | 78.225 | 44.836 | 1.745× | 45.308 |
| vantus-eclipse | paused | 32.892 | 18.476 | 1.780× | 19.656 |
| vantus-eclipse | advancing | 33.123 | 17.340 | 1.910× | 17.561 |
| vantus-airless | paused | 2.519 | 2.713 | 0.929× | 4.886 |
| vantus-airless | advancing | 2.446 | 2.552 | 0.958× | 2.585 |
| dual-eclipse | paused | 0.874 | 1.669 | 0.524× | 1.686 |
| dual-eclipse | advancing | 0.860 | 0.991 | 0.868× | 1.196 |
| moonlit-air | paused | 8.317 | 4.679 | 1.777× | 6.744 |
| moonlit-air | advancing | 7.715 | 4.149 | 1.859× | 4.231 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 1.41097 | 14.015% | 0.0115125 | 0.0105134 |
| halo-rings-spp32 | 0.29901 | 9.410% | 0.00202496 | 0.00181007 |
| halo-rings-moving-8 | 1.41242 | 14.021% | 0.0115463 | 0.0105374 |
| halo-rings-moving-15 | 1.41253 | 14.016% | 0.011556 | 0.0105442 |
| earth-daylight-spp1 | 7.62984 | 91.219% | 0.0542528 | 0.041416 |
| earth-daylight-spp32 | 2.86864 | 84.768% | 0.0103694 | 0.0088701 |
| earth-daylight-moving-8 | 7.63052 | 91.222% | 0.0542669 | 0.041424 |
| earth-daylight-moving-15 | 7.62988 | 91.227% | 0.054266 | 0.0414238 |
| earth-twilight-spp1 | 0.34385 | 6.869% | 0.00155627 | 0.00153181 |
| earth-twilight-spp32 | 0.12264 | 0.484% | 0.000280112 | 0.000279035 |
| earth-twilight-moving-8 | 0.33582 | 6.752% | 0.00153431 | 0.00151063 |
| earth-twilight-moving-15 | 0.32952 | 6.656% | 0.00151568 | 0.00149262 |
| median-dense-spp1 | 3.99225 | 93.957% | 0.0110511 | 0.0101361 |
| median-dense-spp32 | 2.11463 | 81.019% | 0.00276538 | 0.0026425 |
| median-dense-moving-8 | 3.99254 | 93.959% | 0.011052 | 0.010137 |
| median-dense-moving-15 | 3.99207 | 93.957% | 0.0110506 | 0.0101356 |
| vantus-eclipse-spp1 | 0.35407 | 16.520% | 0.000265021 | 0.000262083 |
| vantus-eclipse-spp32 | 0.33511 | 16.106% | 0.000130992 | 0.000129178 |
| vantus-eclipse-moving-8 | 0.35415 | 16.606% | 0.000263282 | 0.000260375 |
| vantus-eclipse-moving-15 | 0.35463 | 16.556% | 0.000264695 | 0.000261774 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| moonlit-air-spp1 | 3.32574 | 78.702% | 0.000738522 | 0.00044312 |
| moonlit-air-spp32 | 0.65957 | 23.101% | 0.000565476 | 0.000290678 |
| moonlit-air-moving-8 | 3.32689 | 78.715% | 0.000738149 | 0.00044282 |
| moonlit-air-moving-15 | 3.32756 | 78.703% | 0.000738343 | 0.000442883 |
