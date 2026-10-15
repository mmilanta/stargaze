# Benchmark comparison

Variants: **reference → guided-ground**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| twilight-afterglow | paused | 7.253 | 0.742 | 9.780× | 1.242 |
| twilight-afterglow | advancing | 0.038 | 0.029 | 1.323× | 0.051 |
| eclipse-totality | paused | 0.293 | 0.131 | 2.232× | 1.111 |
| eclipse-totality | advancing | 0.299 | 0.136 | 2.192× | 1.183 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| twilight-afterglow-spp1 | 1.00064 | 32.102% | 0.000345571 | 0.000304364 |
| twilight-afterglow-spp32 | 0.55537 | 14.301% | 0.000168554 | 0.000145649 |
| twilight-afterglow-moving-8 | 0.98251 | 31.570% | 0.000345487 | 0.000300181 |
| twilight-afterglow-moving-15 | 0.96701 | 31.105% | 0.000333006 | 0.000292182 |
| eclipse-totality-spp1 | 19.22872 | 40.852% | 0.00586659 | 0.00539112 |
| eclipse-totality-spp32 | 6.91622 | 43.282% | 0.00107403 | 0.00106211 |
| eclipse-totality-moving-8 | 19.25353 | 40.847% | 0.00577566 | 0.00531342 |
| eclipse-totality-moving-15 | 19.42531 | 40.885% | 0.00602486 | 0.00553171 |
