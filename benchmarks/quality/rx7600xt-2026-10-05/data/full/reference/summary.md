# Renderer benchmark

Variant: **reference** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 35.052 | 35.240 | 70.436 | 35.069, 35.052, 35.028 |
| halo-rings | advancing | 35.286 | 35.356 | 71.904 | 35.299, 35.276, 35.286 |
| earth-daylight | paused | 125.334 | 125.706 | 165.150 | 125.425, 125.228, 125.285 |
| earth-daylight | advancing | 125.428 | 125.849 | 168.417 | 125.277, 125.511, 125.534 |
| earth-twilight | paused | 42.415 | 42.919 | 80.560 | 42.390, 42.422, 42.424 |
| earth-twilight | advancing | 42.835 | 42.965 | 77.405 | 42.866, 42.836, 42.830 |
| median-dense | paused | 78.283 | 78.459 | 115.007 | 78.218, 78.271, 78.288 |
| median-dense | advancing | 78.225 | 78.407 | 118.318 | 78.194, 78.170, 78.251 |
| vantus-eclipse | paused | 32.892 | 33.113 | 67.499 | 32.907, 32.897, 32.852 |
| vantus-eclipse | advancing | 33.123 | 33.203 | 70.135 | 33.110, 33.111, 33.188 |
| vantus-airless | paused | 2.519 | 2.533 | 40.935 | 2.519, 2.517, 2.524 |
| vantus-airless | advancing | 2.446 | 2.475 | 36.058 | 2.471, 2.441, 2.446 |
| dual-eclipse | paused | 0.874 | 0.881 | 36.338 | 0.868, 0.872, 0.877 |
| dual-eclipse | advancing | 0.860 | 0.869 | 33.299 | 0.855, 0.863, 0.863 |
| moonlit-air | paused | 8.317 | 8.386 | 39.886 | 8.332, 8.339, 8.288 |
| moonlit-air | advancing | 7.715 | 7.740 | 45.305 | 7.722, 7.723, 7.705 |
