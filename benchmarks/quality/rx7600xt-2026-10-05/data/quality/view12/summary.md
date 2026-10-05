# Renderer benchmark

Variant: **view12** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 11.338 | 11.754 | 11.603 | 11.338 |
| halo-rings | advancing | 8.375 | 9.596 | 8.518 | 8.375 |
| earth-daylight | paused | 24.289 | 32.798 | 24.485 | 24.289 |
| earth-daylight | advancing | 22.935 | 23.197 | 23.104 | 22.935 |
| earth-twilight | paused | 18.477 | 19.093 | 18.742 | 18.477 |
| earth-twilight | advancing | 9.469 | 10.432 | 9.621 | 9.469 |
| median-dense | paused | 18.354 | 25.166 | 18.559 | 18.354 |
| median-dense | advancing | 14.418 | 14.646 | 14.590 | 14.418 |
| vantus-eclipse | paused | 10.097 | 10.464 | 10.369 | 10.097 |
| vantus-eclipse | advancing | 8.341 | 9.631 | 8.505 | 8.341 |
| moonlit-air | paused | 2.160 | 2.236 | 2.252 | 2.160 |
| moonlit-air | advancing | 2.201 | 2.230 | 2.294 | 2.201 |
