# Renderer benchmark

Variant: **fast** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 8.386 | 8.741 | 8.569 | 8.386 |
| halo-rings | advancing | 8.320 | 8.782 | 8.498 | 8.320 |
| earth-daylight | paused | 22.210 | 31.745 | 22.735 | 22.210 |
| earth-daylight | advancing | 17.883 | 17.960 | 18.049 | 17.883 |
| earth-twilight | paused | 16.350 | 16.398 | 16.619 | 16.350 |
| earth-twilight | advancing | 9.106 | 10.041 | 9.289 | 9.106 |
| median-dense | paused | 19.135 | 22.449 | 19.461 | 19.135 |
| median-dense | advancing | 11.099 | 11.467 | 11.269 | 11.099 |
| vantus-eclipse | paused | 7.206 | 7.482 | 7.394 | 7.206 |
| vantus-eclipse | advancing | 7.525 | 7.539 | 7.715 | 7.525 |
| moonlit-air | paused | 1.419 | 1.451 | 1.505 | 1.419 |
| moonlit-air | advancing | 1.519 | 1.523 | 1.609 | 1.519 |
