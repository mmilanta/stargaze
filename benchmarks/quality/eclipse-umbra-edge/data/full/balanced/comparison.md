# Benchmark comparison

Variants: **reference → balanced**.

Speedup is baseline / candidate GPU median; above 1 is faster. P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.

| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |
|---|---|---:|---:|---:|---:|
| eclipse-umbra-edge | paused | 3.111 | 2.818 | 1.104× | 3.081 |
| eclipse-umbra-edge | advancing | 2.988 | 1.658 | 1.802× | 1.670 |

| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |
|---|---:|---:|---:|---:|
| eclipse-umbra-edge-spp1 | 3.27157 | 33.454% | 0.122798 | 0.0518812 |
| eclipse-umbra-edge-spp32 | 2.55225 | 32.388% | 0.0818541 | 0.034089 |
| eclipse-umbra-edge-moving-8 | 3.92857 | 33.055% | 0.116619 | 0.0532618 |
| eclipse-umbra-edge-moving-15 | 4.26961 | 32.730% | 0.109137 | 0.0531384 |
