# Renderer benchmark

Variant: **reference** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 19.705 | 21.336 | 20.135 | 19.705 |
| halo-rings | advancing | 10.798 | 11.354 | 11.002 | 10.798 |
| earth-daylight | paused | 37.676 | 38.471 | 38.371 | 37.676 |
| earth-daylight | advancing | 36.629 | 36.896 | 37.184 | 36.629 |
| earth-twilight | paused | 20.863 | 26.517 | 21.102 | 20.863 |
| earth-twilight | advancing | 13.390 | 13.538 | 13.528 | 13.390 |
| median-dense | paused | 23.478 | 25.950 | 23.637 | 23.478 |
| median-dense | advancing | 23.222 | 23.277 | 23.381 | 23.222 |
| vantus-eclipse | paused | 14.584 | 18.720 | 14.825 | 14.584 |
| vantus-eclipse | advancing | 9.488 | 9.697 | 9.639 | 9.488 |
| moonlit-air | paused | 3.366 | 3.383 | 3.446 | 3.366 |
| moonlit-air | advancing | 3.039 | 3.139 | 3.151 | 3.039 |
