# Renderer benchmark

GPU: **Apple M4 Pro** (1.4.2).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| twilight-afterglow | paused | 7.253 | 108.127 | 101.148 | 7.103, 8.838, 51.806 |
| twilight-afterglow | advancing | 0.038 | 0.092 | 104.837 | 0.040, 0.037, 0.038 |
| eclipse-totality | paused | 0.293 | 2.340 | 3.454 | 0.288, 0.294, 0.299 |
| eclipse-totality | advancing | 0.299 | 2.339 | 3.359 | 0.297, 0.303, 0.299 |
