# Renderer benchmark

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| earth-daylight | paused | 134.487 | 136.930 | 136.578 | 134.487 |
| earth-daylight | advancing | 126.614 | 127.123 | 128.331 | 126.614 |
| vantus-eclipse | paused | 39.178 | 39.396 | 39.665 | 39.178 |
| vantus-eclipse | advancing | 36.430 | 36.759 | 36.650 | 36.430 |
