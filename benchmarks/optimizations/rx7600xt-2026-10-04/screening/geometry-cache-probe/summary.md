# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| earth-daylight | paused | 128.210 | 130.829 | 130.336 | 128.210 |
| earth-daylight | advancing | 123.331 | 124.169 | 125.400 | 123.331 |
| vantus-eclipse | paused | 38.038 | 38.648 | 38.862 | 38.038 |
| vantus-eclipse | advancing | 36.158 | 36.386 | 36.376 | 36.158 |
