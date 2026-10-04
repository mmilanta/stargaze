# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 71.652 | 73.180 | 72.050 | 71.670, 71.638, 71.658 |
| halo-rings | advancing | 73.638 | 73.752 | 74.033 | 73.638, 73.659, 73.624 |
| earth-daylight | paused | 130.916 | 131.234 | 131.354 | 131.004, 130.934, 130.871 |
| earth-daylight | advancing | 131.301 | 131.447 | 131.689 | 131.234, 131.318, 131.346 |
| earth-twilight | paused | 113.431 | 114.475 | 113.803 | 113.403, 113.433, 113.432 |
| earth-twilight | advancing | 114.843 | 115.014 | 115.192 | 114.835, 114.825, 114.878 |
| median-dense | paused | 103.667 | 104.395 | 104.021 | 103.643, 103.664, 103.676 |
| median-dense | advancing | 104.692 | 104.827 | 105.067 | 104.663, 104.684, 104.738 |
| vantus-eclipse | paused | 65.513 | 67.171 | 65.862 | 65.512, 65.507, 65.518 |
| vantus-eclipse | advancing | 67.533 | 68.526 | 67.990 | 67.557, 67.519, 67.531 |
| vantus-airless | paused | 2.707 | 4.750 | 2.831 | 3.595, 2.671, 2.671 |
| vantus-airless | advancing | 2.607 | 2.664 | 2.695 | 2.646, 2.605, 2.590 |
| dual-eclipse | paused | 1.480 | 1.493 | 1.563 | 1.479, 1.481, 1.480 |
| dual-eclipse | advancing | 0.946 | 1.177 | 1.034 | 1.100, 0.946, 0.846 |
