# Renderer benchmark

Variant: **balanced** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 11.293 | 11.733 | 11.562 | 11.293 |
| halo-rings | advancing | 8.522 | 9.626 | 8.676 | 8.522 |
| earth-daylight | paused | 24.177 | 32.892 | 24.693 | 24.177 |
| earth-daylight | advancing | 22.641 | 22.765 | 23.157 | 22.641 |
| earth-twilight | paused | 18.413 | 19.010 | 18.676 | 18.413 |
| earth-twilight | advancing | 9.428 | 10.429 | 9.592 | 9.428 |
| median-dense | paused | 18.562 | 25.099 | 18.826 | 18.562 |
| median-dense | advancing | 14.360 | 14.450 | 14.528 | 14.360 |
| vantus-eclipse | paused | 9.982 | 10.392 | 10.245 | 9.982 |
| vantus-eclipse | advancing | 8.326 | 9.642 | 8.493 | 8.326 |
| moonlit-air | paused | 1.996 | 2.051 | 2.077 | 1.996 |
| moonlit-air | advancing | 2.030 | 2.040 | 2.112 | 2.030 |
