# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| earth-daylight | paused | 127.804 | 130.545 | 130.090 | 127.804 |
| earth-daylight | advancing | 120.450 | 121.259 | 140.835 | 120.450 |
| vantus-eclipse | paused | 34.679 | 35.549 | 55.893 | 34.679 |
| vantus-eclipse | advancing | 32.676 | 33.188 | 43.239 | 32.676 |
