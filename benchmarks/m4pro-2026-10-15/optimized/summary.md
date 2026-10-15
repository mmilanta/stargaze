# Renderer benchmark

GPU: **Apple M4 Pro** (1.4.2).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 3.086 | 33.022 | 33.173 | 3.135, 3.089, 3.050 |
| halo-rings | advancing | 0.044 | 0.063 | 32.921 | 0.045, 0.046, 0.044 |
| earth-daylight | paused | 8.074 | 113.483 | 113.785 | 7.952, 10.421, 8.437 |
| earth-daylight | advancing | 0.044 | 0.078 | 114.078 | 0.041, 0.046, 0.050 |
| earth-twilight | paused | 2.639 | 20.593 | 31.234 | 2.615, 2.676, 2.610 |
| earth-twilight | advancing | 0.046 | 0.067 | 31.936 | 0.038, 0.051, 0.046 |
| median-dense | paused | 2.681 | 72.550 | 74.060 | 2.689, 2.653, 2.676 |
| median-dense | advancing | 0.049 | 0.078 | 74.142 | 0.052, 0.055, 0.040 |
| vantus-eclipse | paused | 2.898 | 3.008 | 29.938 | 2.900, 2.891, 2.901 |
| vantus-eclipse | advancing | 0.041 | 0.079 | 30.862 | 0.037, 0.044, 0.044 |
| vantus-airless | paused | 0.183 | 1.074 | 2.299 | 0.186, 0.180, 0.184 |
| vantus-airless | advancing | 0.186 | 0.196 | 2.316 | 0.186, 0.186, 0.186 |
| dual-eclipse | paused | 0.180 | 0.667 | 1.240 | 0.071, 0.216, 0.141 |
| dual-eclipse | advancing | 0.436 | 0.631 | 1.213 | 0.411, 0.450, 0.441 |
