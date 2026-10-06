# Renderer benchmark

Variant: **balanced** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 1280×720; trace: 960×540; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 20.093 | 23.077 | 20.732 | 20.883, 20.093, 19.594 |
| halo-rings | advancing | 18.681 | 19.280 | 19.231 | 18.940, 18.629, 18.447 |
| earth-daylight | paused | 73.123 | 78.826 | 75.017 | 76.236, 73.113, 72.875 |
| earth-daylight | advancing | 72.850 | 73.078 | 74.817 | 72.777, 72.936, 72.783 |
| earth-twilight | paused | 31.119 | 33.551 | 32.341 | 32.816, 31.119, 30.332 |
| earth-twilight | advancing | 29.396 | 29.983 | 29.550 | 29.588, 29.334, 29.394 |
| median-dense | paused | 45.934 | 49.647 | 46.481 | 48.261, 45.934, 45.012 |
| median-dense | advancing | 44.836 | 45.308 | 45.538 | 44.834, 44.816, 44.934 |
| vantus-eclipse | paused | 18.476 | 19.656 | 19.286 | 19.161, 18.476, 17.898 |
| vantus-eclipse | advancing | 17.340 | 17.561 | 17.525 | 17.529, 17.340, 17.153 |
| vantus-airless | paused | 2.713 | 4.886 | 2.795 | 3.975, 2.644, 2.672 |
| vantus-airless | advancing | 2.552 | 2.585 | 2.640 | 2.576, 2.552, 2.533 |
| dual-eclipse | paused | 1.669 | 1.686 | 1.757 | 1.671, 1.669, 1.624 |
| dual-eclipse | advancing | 0.991 | 1.196 | 1.062 | 1.137, 0.991, 0.895 |
| moonlit-air | paused | 4.679 | 6.744 | 4.758 | 5.066, 4.699, 4.619 |
| moonlit-air | advancing | 4.149 | 4.231 | 4.229 | 4.212, 4.149, 4.113 |
