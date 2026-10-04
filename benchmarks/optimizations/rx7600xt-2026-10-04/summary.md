# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 34.734 | 37.597 | 35.159 | 36.840, 34.734, 34.034 |
| halo-rings | advancing | 34.218 | 34.349 | 34.671 | 34.227, 34.244, 34.209 |
| earth-daylight | paused | 120.498 | 124.182 | 122.397 | 121.683, 120.319, 120.496 |
| earth-daylight | advancing | 120.813 | 121.159 | 122.729 | 120.811, 120.810, 120.824 |
| earth-twilight | paused | 41.253 | 44.893 | 41.778 | 43.076, 41.253, 40.387 |
| earth-twilight | advancing | 40.558 | 40.656 | 41.009 | 40.566, 40.566, 40.552 |
| median-dense | paused | 74.684 | 79.530 | 76.682 | 77.180, 74.684, 74.558 |
| median-dense | advancing | 74.474 | 74.685 | 76.266 | 74.392, 74.498, 74.543 |
| vantus-eclipse | paused | 32.339 | 35.041 | 33.972 | 34.227, 32.339, 31.612 |
| vantus-eclipse | advancing | 31.490 | 31.629 | 32.570 | 31.500, 31.526, 31.425 |
| vantus-airless | paused | 2.684 | 4.670 | 2.773 | 3.547, 2.635, 2.657 |
| vantus-airless | advancing | 2.549 | 2.581 | 2.632 | 2.564, 2.551, 2.518 |
| dual-eclipse | paused | 1.668 | 1.683 | 1.756 | 1.670, 1.669, 1.581 |
| dual-eclipse | advancing | 0.960 | 1.170 | 1.043 | 1.102, 0.960, 0.870 |
