# Renderer benchmark

Variant: **guided-ground** (benchmark-only when not reference).

GPU: **Apple M4 Pro** (1.4.2).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 1.212 | 13.193 | 13.098 | 1.205, 1.212, 1.218 |
| halo-rings | advancing | 0.040 | 0.114 | 15.053 | 0.059, 0.040, 0.029 |
| earth-daylight | paused | 3.802 | 58.225 | 58.112 | 3.911, 3.453, 15.353 |
| earth-daylight | advancing | 0.036 | 0.068 | 58.158 | 0.033, 0.037, 0.035 |
| earth-twilight | paused | 1.569 | 20.585 | 21.104 | 1.581, 1.675, 1.397 |
| earth-twilight | advancing | 0.028 | 0.181 | 21.370 | 0.026, 0.031, 0.032 |
| median-dense | paused | 1.048 | 36.790 | 37.362 | 1.016, 1.050, 1.077 |
| median-dense | advancing | 0.035 | 0.053 | 37.454 | 0.036, 0.032, 0.035 |
| vantus-eclipse | paused | 1.145 | 11.518 | 13.373 | 1.149, 1.141, 1.148 |
| vantus-eclipse | advancing | 0.034 | 0.067 | 13.536 | 0.032, 0.038, 0.035 |
| vantus-airless | paused | 0.188 | 1.610 | 2.374 | 0.186, 0.188, 0.190 |
| vantus-airless | advancing | 0.192 | 1.603 | 2.371 | 0.190, 0.195, 0.192 |
| dual-eclipse | paused | 0.080 | 0.547 | 1.271 | 0.070, 0.255, 0.076 |
| dual-eclipse | advancing | 0.198 | 0.642 | 1.282 | 0.398, 0.121, 0.131 |
| twilight-afterglow | paused | 0.742 | 1.242 | 15.990 | 0.736, 0.746, 0.738 |
| twilight-afterglow | advancing | 0.029 | 0.051 | 16.137 | 0.029, 0.031, 0.024 |
| eclipse-totality | paused | 0.131 | 1.111 | 1.705 | 0.134, 0.126, 0.134 |
| eclipse-totality | advancing | 0.136 | 1.183 | 1.718 | 0.136, 0.137, 0.136 |
