# Renderer benchmark

Variant: **fast** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 14.450 | 15.409 | 14.571 | 15.172, 14.450, 14.077 |
| halo-rings | advancing | 13.380 | 13.669 | 13.848 | 13.601, 13.380, 13.255 |
| earth-daylight | paused | 56.881 | 61.638 | 57.138 | 59.604, 56.881, 56.262 |
| earth-daylight | advancing | 56.320 | 56.565 | 56.662 | 56.327, 56.348, 56.316 |
| earth-twilight | paused | 26.991 | 29.510 | 27.919 | 28.661, 26.991, 26.279 |
| earth-twilight | advancing | 25.686 | 26.040 | 26.053 | 25.741, 25.665, 25.700 |
| median-dense | paused | 36.446 | 39.962 | 36.877 | 38.548, 36.446, 35.668 |
| median-dense | advancing | 35.055 | 35.203 | 35.229 | 35.122, 35.014, 34.915 |
| vantus-eclipse | paused | 13.551 | 14.268 | 13.894 | 14.057, 13.551, 13.150 |
| vantus-eclipse | advancing | 12.617 | 12.913 | 12.856 | 12.825, 12.617, 12.519 |
| vantus-airless | paused | 2.731 | 4.259 | 2.833 | 3.523, 2.675, 2.722 |
| vantus-airless | advancing | 2.597 | 2.633 | 2.706 | 2.616, 2.594, 2.581 |
| dual-eclipse | paused | 1.667 | 1.688 | 1.753 | 1.667, 1.669, 1.628 |
| dual-eclipse | advancing | 0.980 | 1.187 | 1.063 | 1.125, 0.980, 0.880 |
| moonlit-air | paused | 3.208 | 5.575 | 3.313 | 4.694, 3.178, 3.186 |
| moonlit-air | advancing | 2.892 | 2.930 | 2.988 | 2.920, 2.891, 2.867 |
