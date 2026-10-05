# Renderer benchmark

Variant: **sun6** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 35.027 | 35.230 | 70.203 | 35.017, 35.035, 35.036 |
| halo-rings | advancing | 35.319 | 35.383 | 69.928 | 35.325, 35.314, 35.322 |
| earth-daylight | paused | 124.021 | 124.514 | 157.717 | 124.150, 124.083, 123.846 |
| earth-daylight | advancing | 124.027 | 124.463 | 159.587 | 124.151, 124.041, 123.951 |
| earth-twilight | paused | 42.227 | 42.663 | 78.380 | 42.255, 42.238, 42.197 |
| earth-twilight | advancing | 42.653 | 42.772 | 74.313 | 42.714, 42.634, 42.626 |
| median-dense | paused | 77.329 | 77.591 | 113.739 | 77.326, 77.340, 77.323 |
| median-dense | advancing | 77.079 | 77.299 | 111.759 | 77.149, 77.048, 77.080 |
| vantus-eclipse | paused | 32.783 | 32.959 | 65.581 | 32.783, 32.805, 32.750 |
| vantus-eclipse | advancing | 33.051 | 33.093 | 65.858 | 33.053, 33.041, 33.051 |
| vantus-airless | paused | 2.539 | 2.555 | 26.162 | 2.548, 2.533, 2.536 |
| vantus-airless | advancing | 2.459 | 2.471 | 37.820 | 2.460, 2.462, 2.444 |
| dual-eclipse | paused | 0.877 | 0.883 | 31.735 | 0.876, 0.877, 0.878 |
| dual-eclipse | advancing | 0.860 | 0.868 | 33.048 | 0.862, 0.863, 0.855 |
| moonlit-air | paused | 8.134 | 8.193 | 39.332 | 8.154, 8.123, 8.137 |
| moonlit-air | advancing | 7.664 | 7.694 | 40.702 | 7.654, 7.673, 7.670 |
