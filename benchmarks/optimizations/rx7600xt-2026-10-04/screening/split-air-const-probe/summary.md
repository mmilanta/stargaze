# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| earth-daylight | paused | 119.593 | 119.874 | 321.725 | 119.593 |
| earth-daylight | advancing | 119.764 | 119.914 | 323.888 | 119.764 |
| vantus-eclipse | paused | 30.374 | 30.566 | 230.808 | 30.374 |
| vantus-eclipse | advancing | 30.612 | 30.639 | 231.521 | 30.612 |
