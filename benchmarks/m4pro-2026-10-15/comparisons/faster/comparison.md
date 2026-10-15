# Benchmark comparison

Variants: **guided-ground → guided-ground**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| halo-rings | paused | 1.212 | 0.608 | 1.992× | 5.587 |
| halo-rings | advancing | 0.040 | 0.032 | 1.253× | 0.040 |
| earth-daylight | paused | 3.802 | 1.684 | 2.258× | 6.194 |
| earth-daylight | advancing | 0.036 | 0.032 | 1.109× | 0.054 |
| earth-twilight | paused | 1.569 | 1.051 | 1.492× | 11.647 |
| earth-twilight | advancing | 0.028 | 0.032 | 0.878× | 0.054 |
| median-dense | paused | 1.048 | 0.549 | 1.908× | 6.729 |
| median-dense | advancing | 0.035 | 0.034 | 1.005× | 0.088 |
| vantus-eclipse | paused | 1.145 | 0.580 | 1.973× | 0.620 |
| vantus-eclipse | advancing | 0.034 | 0.032 | 1.038× | 0.059 |
| vantus-airless | paused | 0.188 | 0.195 | 0.965× | 1.154 |
| vantus-airless | advancing | 0.192 | 0.197 | 0.978× | 0.205 |
| dual-eclipse | paused | 0.080 | 0.425 | 0.189× | 0.572 |
| dual-eclipse | advancing | 0.198 | 0.222 | 0.895× | 0.515 |
| twilight-afterglow | paused | 0.742 | 0.549 | 1.350× | 3.133 |
| twilight-afterglow | advancing | 0.029 | 0.040 | 0.719× | 0.085 |
| eclipse-totality | paused | 0.131 | 0.133 | 0.991× | 0.519 |
| eclipse-totality | advancing | 0.136 | 0.143 | 0.953× | 1.075 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| halo-rings-spp1 | 0.33466 | 8.473% | 0.00105721 | 0.000961966 |
| halo-rings-spp32 | 0.06769 | 0.118% | 0.000222948 | 0.000201141 |
| halo-rings-moving-8 | 0.33467 | 8.471% | 0.00105868 | 0.000963135 |
| halo-rings-moving-15 | 0.33478 | 8.463% | 0.001062 | 0.000965774 |
| earth-daylight-spp1 | 9.94229 | 62.004% | 0.0715821 | 0.0529839 |
| earth-daylight-spp32 | 3.19903 | 56.632% | 0.0125897 | 0.0112007 |
| earth-daylight-moving-8 | 9.94182 | 62.003% | 0.0715859 | 0.0529898 |
| earth-daylight-moving-15 | 9.94107 | 62.010% | 0.0715867 | 0.0529922 |
| earth-twilight-spp1 | 0.36778 | 8.429% | 0.00205294 | 0.00200156 |
| earth-twilight-spp32 | 0.13037 | 0.661% | 0.00035822 | 0.000357132 |
| earth-twilight-moving-8 | 0.36167 | 8.315% | 0.00202367 | 0.00197368 |
| earth-twilight-moving-15 | 0.35630 | 8.210% | 0.00199834 | 0.00194953 |
| median-dense-spp1 | 3.76629 | 35.915% | 0.0153916 | 0.0141637 |
| median-dense-spp32 | 0.84694 | 26.957% | 0.00271794 | 0.00260932 |
| median-dense-moving-8 | 3.76625 | 35.911% | 0.0153917 | 0.0141637 |
| median-dense-moving-15 | 3.76620 | 35.911% | 0.0153918 | 0.0141639 |
| vantus-eclipse-spp1 | 0.00026 | 0.000% | 4.86142e-07 | 4.84486e-07 |
| vantus-eclipse-spp32 | 0.00007 | 0.000% | 8.62201e-08 | 8.59255e-08 |
| vantus-eclipse-moving-8 | 0.00025 | 0.000% | 4.91016e-07 | 4.89346e-07 |
| vantus-eclipse-moving-15 | 0.00024 | 0.000% | 4.95174e-07 | 4.93495e-07 |
| vantus-airless-spp1 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-spp32 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| vantus-airless-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp1 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-spp32 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| dual-eclipse-moving-15 | 0.00000 | 0.000% | 0 | 0 |
| twilight-afterglow-spp1 | 0.21969 | 5.982% | 9.08948e-05 | 9.08151e-05 |
| twilight-afterglow-spp32 | 0.09659 | 0.299% | 1.60799e-05 | 1.60786e-05 |
| twilight-afterglow-moving-8 | 0.21286 | 5.819% | 8.91049e-05 | 8.90277e-05 |
| twilight-afterglow-moving-15 | 0.20718 | 5.681% | 8.78119e-05 | 8.77361e-05 |
| eclipse-totality-spp1 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-totality-spp32 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-totality-moving-8 | 0.00000 | 0.000% | 0 | 0 |
| eclipse-totality-moving-15 | 0.00000 | 0.000% | 0 | 0 |
