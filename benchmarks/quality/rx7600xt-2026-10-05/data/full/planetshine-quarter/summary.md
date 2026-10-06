# Renderer benchmark

Variant: **planetshine-quarter** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 33.603 | 36.148 | 33.908 | 35.371, 33.530, 32.923 |
| halo-rings | advancing | 32.189 | 32.672 | 32.455 | 32.478, 32.166, 32.145 |
| earth-daylight | paused | 119.570 | 123.341 | 122.182 | 120.434, 119.557, 119.457 |
| earth-daylight | advancing | 119.804 | 119.962 | 122.514 | 119.850, 119.739, 119.735 |
| earth-twilight | paused | 35.376 | 38.258 | 35.797 | 37.151, 35.354, 34.921 |
| earth-twilight | advancing | 34.301 | 34.538 | 34.735 | 34.297, 34.269, 34.331 |
| median-dense | paused | 72.867 | 77.482 | 74.042 | 75.166, 72.852, 72.530 |
| median-dense | advancing | 72.709 | 73.152 | 73.072 | 72.634, 72.747, 72.863 |
| vantus-eclipse | paused | 29.010 | 31.513 | 29.345 | 30.842, 28.957, 28.445 |
| vantus-eclipse | advancing | 27.846 | 28.221 | 27.992 | 28.117, 27.841, 27.840 |
| vantus-airless | paused | 2.714 | 3.982 | 2.803 | 3.282, 2.669, 2.702 |
| vantus-airless | advancing | 2.597 | 2.637 | 2.689 | 2.627, 2.590, 2.593 |
| dual-eclipse | paused | 1.660 | 1.669 | 1.751 | 1.663, 1.662, 1.653 |
| dual-eclipse | advancing | 1.011 | 1.235 | 1.081 | 1.161, 1.011, 0.909 |
| moonlit-air | paused | 8.498 | 8.848 | 8.598 | 8.746, 8.507, 8.366 |
| moonlit-air | advancing | 7.648 | 7.791 | 7.764 | 7.751, 7.648, 7.530 |
