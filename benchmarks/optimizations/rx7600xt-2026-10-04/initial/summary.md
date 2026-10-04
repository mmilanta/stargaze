# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 34.788 | 37.661 | 35.260 | 36.721, 34.788, 34.067 |
| halo-rings | advancing | 34.270 | 34.384 | 34.723 | 34.281, 34.223, 34.274 |
| earth-daylight | paused | 120.759 | 124.656 | 122.608 | 122.150, 120.572, 120.781 |
| earth-daylight | advancing | 121.006 | 121.206 | 122.884 | 120.986, 121.014, 120.993 |
| earth-twilight | paused | 41.401 | 45.004 | 41.911 | 43.304, 41.401, 40.554 |
| earth-twilight | advancing | 40.778 | 40.888 | 41.226 | 40.800, 40.742, 40.767 |
| median-dense | paused | 74.630 | 79.975 | 76.590 | 77.294, 74.567, 74.443 |
| median-dense | advancing | 74.394 | 74.582 | 76.243 | 74.337, 74.407, 74.397 |
| vantus-eclipse | paused | 32.577 | 35.204 | 33.215 | 34.422, 32.577, 31.794 |
| vantus-eclipse | advancing | 31.480 | 31.633 | 31.984 | 31.505, 31.478, 31.452 |
| vantus-airless | paused | 2.700 | 4.870 | 2.826 | 3.655, 2.700, 2.656 |
| vantus-airless | advancing | 2.556 | 2.607 | 2.637 | 2.580, 2.552, 2.544 |
| dual-eclipse | paused | 1.713 | 1.734 | 1.794 | 1.717, 1.716, 1.498 |
| dual-eclipse | advancing | 0.938 | 1.127 | 1.020 | 1.072, 0.938, 0.868 |
