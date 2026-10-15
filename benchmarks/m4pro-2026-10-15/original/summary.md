# Renderer benchmark

GPU: **Apple M4 Pro** (1.4.2).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 9.835 | 101.242 | 100.735 | 9.406, 94.330, 99.824 |
| halo-rings | advancing | 0.048 | 0.080 | 101.572 | 0.044, 0.051, 0.051 |
| earth-daylight | paused | 11.548 | 141.962 | 141.816 | 11.464, 11.929, 11.297 |
| earth-daylight | advancing | 0.041 | 0.086 | 144.368 | 0.041, 0.041, 0.049 |
| earth-twilight | paused | 119.441 | 129.754 | 128.743 | 125.600, 63.066, 63.453 |
| earth-twilight | advancing | 0.041 | 0.070 | 129.084 | 0.036, 0.048, 0.042 |
| median-dense | paused | 13.465 | 132.627 | 132.877 | 10.078, 33.488, 40.540 |
| median-dense | advancing | 0.042 | 0.072 | 134.297 | 0.041, 0.043, 0.040 |
| vantus-eclipse | paused | 10.477 | 94.194 | 93.855 | 10.071, 10.539, 45.396 |
| vantus-eclipse | advancing | 0.048 | 0.079 | 95.325 | 0.048, 0.052, 0.038 |
| vantus-airless | paused | 0.266 | 1.772 | 2.794 | 0.267, 0.267, 0.263 |
| vantus-airless | advancing | 0.265 | 1.818 | 2.876 | 0.267, 0.263, 0.265 |
| dual-eclipse | paused | 0.187 | 0.433 | 1.207 | 0.166, 0.191, 0.159 |
| dual-eclipse | advancing | 0.207 | 0.445 | 1.219 | 0.199, 0.225, 0.201 |
