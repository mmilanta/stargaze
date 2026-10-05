# Renderer benchmark

Variant: **no-planetshine** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 11.614 | 12.213 | 11.798 | 12.116, 11.645, 11.382 |
| halo-rings | advancing | 10.136 | 10.391 | 10.468 | 10.303, 10.079, 10.055 |
| earth-daylight | paused | 64.688 | 69.446 | 65.313 | 67.159, 64.688, 64.214 |
| earth-daylight | advancing | 63.945 | 64.015 | 64.266 | 63.945, 63.960, 63.936 |
| earth-twilight | paused | 26.300 | 28.480 | 26.918 | 27.945, 26.300, 25.485 |
| earth-twilight | advancing | 24.711 | 24.946 | 24.933 | 24.892, 24.711, 24.565 |
| median-dense | paused | 38.789 | 41.876 | 39.345 | 41.237, 38.789, 38.218 |
| median-dense | advancing | 36.980 | 37.180 | 37.297 | 36.977, 37.028, 36.972 |
| vantus-eclipse | paused | 11.591 | 12.052 | 11.729 | 11.905, 11.627, 11.406 |
| vantus-eclipse | advancing | 9.993 | 10.329 | 10.129 | 10.273, 9.974, 9.864 |
| vantus-airless | paused | 2.680 | 4.455 | 2.761 | 3.455, 2.625, 2.633 |
| vantus-airless | advancing | 2.529 | 2.581 | 2.615 | 2.565, 2.531, 2.505 |
| dual-eclipse | paused | 1.499 | 1.508 | 1.586 | 1.500, 1.500, 1.496 |
| dual-eclipse | advancing | 0.974 | 1.220 | 1.052 | 1.141, 0.974, 0.875 |
| moonlit-air | paused | 2.479 | 3.620 | 2.691 | 3.578, 2.479, 2.005 |
| moonlit-air | advancing | 1.847 | 1.890 | 1.919 | 1.879, 1.839, 1.840 |
