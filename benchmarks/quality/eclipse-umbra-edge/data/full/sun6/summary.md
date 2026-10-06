# Renderer benchmark

Variant: **sun6** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| eclipse-umbra-edge | paused | 3.007 | 5.289 | 3.090 | 5.277, 3.007, 2.878 |
| eclipse-umbra-edge | advancing | 2.831 | 2.883 | 2.913 | 2.873, 2.831, 2.802 |
