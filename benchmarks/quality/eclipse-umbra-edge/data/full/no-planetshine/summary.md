# Renderer benchmark

Variant: **no-planetshine** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| eclipse-umbra-edge | paused | 2.945 | 4.837 | 3.026 | 4.830, 2.945, 2.671 |
| eclipse-umbra-edge | advancing | 2.630 | 2.673 | 2.711 | 2.659, 2.630, 2.612 |
