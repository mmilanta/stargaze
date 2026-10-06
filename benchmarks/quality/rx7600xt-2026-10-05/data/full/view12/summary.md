# Renderer benchmark

Variant: **view12** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 19.262 | 19.369 | 53.794 | 19.226, 19.306, 19.251 |
| halo-rings | advancing | 19.409 | 19.467 | 54.767 | 19.337, 19.421, 19.431 |
| earth-daylight | paused | 76.547 | 76.884 | 112.716 | 76.514, 76.528, 76.554 |
| earth-daylight | advancing | 76.337 | 76.577 | 109.269 | 76.419, 76.309, 76.279 |
| earth-twilight | paused | 31.096 | 31.276 | 61.498 | 31.087, 31.109, 31.099 |
| earth-twilight | advancing | 31.208 | 31.254 | 61.791 | 31.218, 31.196, 31.213 |
| median-dense | paused | 47.579 | 47.870 | 78.391 | 47.604, 47.558, 47.579 |
| median-dense | advancing | 47.622 | 47.720 | 78.581 | 47.630, 47.624, 47.617 |
| vantus-eclipse | paused | 18.248 | 18.350 | 44.522 | 18.241, 18.258, 18.247 |
| vantus-eclipse | advancing | 18.417 | 18.455 | 44.644 | 18.441, 18.428, 18.209 |
| vantus-airless | paused | 2.506 | 2.528 | 31.249 | 2.499, 2.509, 2.509 |
| vantus-airless | advancing | 2.298 | 2.325 | 2.381 | 2.298, 2.302, 2.297 |
| dual-eclipse | paused | 1.657 | 1.674 | 1.739 | 1.661, 1.659, 1.642 |
| dual-eclipse | advancing | 0.993 | 1.210 | 1.066 | 1.135, 0.993, 0.897 |
| moonlit-air | paused | 4.730 | 6.720 | 4.858 | 5.038, 4.751, 4.678 |
| moonlit-air | advancing | 4.243 | 4.311 | 4.345 | 4.285, 4.240, 4.189 |
