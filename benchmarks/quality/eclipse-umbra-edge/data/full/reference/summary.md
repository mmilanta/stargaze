# Renderer benchmark

Variant: **reference** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| eclipse-umbra-edge | paused | 3.111 | 3.143 | 3.190 | 3.104, 3.122, 3.105 |
| eclipse-umbra-edge | advancing | 2.988 | 3.041 | 3.071 | 3.029, 2.988, 2.963 |
