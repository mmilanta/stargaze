# Renderer benchmark

Variant: **guided-ground** (benchmark-only when not reference).

GPU: **Apple M4 Pro** (1.4.2).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 0.608 | 5.587 | 7.004 | 0.608, 0.609, 0.607 |
| halo-rings | advancing | 0.032 | 0.040 | 7.106 | 0.037, 0.033, 0.027 |
| earth-daylight | paused | 1.684 | 6.194 | 20.188 | 1.548, 1.713, 1.692 |
| earth-daylight | advancing | 0.032 | 0.054 | 20.281 | 0.032, 0.030, 0.037 |
| earth-twilight | paused | 1.051 | 11.647 | 11.719 | 1.049, 1.060, 1.055 |
| earth-twilight | advancing | 0.032 | 0.054 | 11.589 | 0.031, 0.033, 0.032 |
| median-dense | paused | 0.549 | 6.729 | 16.437 | 0.546, 0.550, 0.549 |
| median-dense | advancing | 0.034 | 0.088 | 16.514 | 0.035, 0.032, 0.040 |
| vantus-eclipse | paused | 0.580 | 0.620 | 6.452 | 0.576, 0.582, 0.584 |
| vantus-eclipse | advancing | 0.032 | 0.059 | 6.526 | 0.035, 0.031, 0.033 |
| vantus-airless | paused | 0.195 | 1.154 | 2.438 | 0.194, 0.196, 0.196 |
| vantus-airless | advancing | 0.197 | 0.205 | 2.440 | 0.196, 0.196, 0.198 |
| dual-eclipse | paused | 0.425 | 0.572 | 1.281 | 0.448, 0.469, 0.060 |
| dual-eclipse | advancing | 0.222 | 0.515 | 1.294 | 0.340, 0.169, 0.078 |
| twilight-afterglow | paused | 0.549 | 3.133 | 9.332 | 0.552, 0.542, 0.549 |
| twilight-afterglow | advancing | 0.040 | 0.085 | 9.313 | 0.037, 0.042, 0.041 |
| eclipse-totality | paused | 0.133 | 0.519 | 1.560 | 0.135, 0.130, 0.131 |
| eclipse-totality | advancing | 0.143 | 1.075 | 1.669 | 0.142, 0.529, 0.148 |
