# Renderer benchmark

Variant: **planetshine-quarter** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 19.087 | 20.854 | 19.807 | 19.087 |
| halo-rings | advancing | 10.515 | 11.211 | 10.679 | 10.515 |
| earth-daylight | paused | 37.427 | 38.374 | 38.146 | 37.427 |
| earth-daylight | advancing | 36.538 | 37.093 | 37.356 | 36.538 |
| earth-twilight | paused | 19.735 | 21.910 | 20.001 | 19.735 |
| earth-twilight | advancing | 10.845 | 11.086 | 10.975 | 10.845 |
| median-dense | paused | 22.857 | 26.330 | 23.016 | 22.857 |
| median-dense | advancing | 22.775 | 22.890 | 23.122 | 22.775 |
| vantus-eclipse | paused | 14.147 | 17.091 | 14.394 | 14.147 |
| vantus-eclipse | advancing | 8.545 | 9.037 | 8.655 | 8.545 |
| moonlit-air | paused | 3.381 | 3.412 | 3.456 | 3.381 |
| moonlit-air | advancing | 3.075 | 3.186 | 3.151 | 3.075 |
