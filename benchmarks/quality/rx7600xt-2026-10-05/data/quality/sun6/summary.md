# Renderer benchmark

Variant: **sun6** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 19.029 | 21.248 | 19.291 | 19.029 |
| halo-rings | advancing | 10.826 | 11.023 | 10.999 | 10.826 |
| earth-daylight | paused | 37.472 | 38.212 | 37.891 | 37.472 |
| earth-daylight | advancing | 36.508 | 36.664 | 36.725 | 36.508 |
| earth-twilight | paused | 20.898 | 26.406 | 21.145 | 20.898 |
| earth-twilight | advancing | 13.271 | 13.418 | 13.435 | 13.271 |
| median-dense | paused | 23.207 | 27.352 | 23.369 | 23.207 |
| median-dense | advancing | 23.146 | 23.184 | 23.309 | 23.146 |
| vantus-eclipse | paused | 14.573 | 18.688 | 14.817 | 14.573 |
| vantus-eclipse | advancing | 9.529 | 9.704 | 9.709 | 9.529 |
| moonlit-air | paused | 3.363 | 3.402 | 3.462 | 3.363 |
| moonlit-air | advancing | 3.066 | 3.177 | 3.191 | 3.066 |
