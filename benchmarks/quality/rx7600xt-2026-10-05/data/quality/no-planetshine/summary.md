# Renderer benchmark

Variant: **no-planetshine** (benchmark-only when not reference).

GPU: **AMD Radeon RX 7600 XT (RADV NAVI33)** (Mesa 26.2.2-arch1.1).
Display: 640×360; trace: 480×270; 4 bounces; 70% vegetation; 1 spp per submission.

GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.

| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |
|---|---|---:|---:|---:|---|
| halo-rings | paused | 6.052 | 6.221 | 6.258 | 6.052 |
| halo-rings | advancing | 6.239 | 6.242 | 6.324 | 6.239 |
| earth-daylight | paused | 20.328 | 23.795 | 20.628 | 20.328 |
| earth-daylight | advancing | 19.685 | 19.844 | 19.845 | 19.685 |
| earth-twilight | paused | 15.233 | 15.503 | 15.496 | 15.233 |
| earth-twilight | advancing | 8.898 | 9.913 | 9.044 | 8.898 |
| median-dense | paused | 15.451 | 19.907 | 15.735 | 15.451 |
| median-dense | advancing | 11.857 | 11.956 | 12.021 | 11.857 |
| vantus-eclipse | paused | 5.543 | 5.733 | 5.726 | 5.543 |
| vantus-eclipse | advancing | 5.774 | 5.782 | 5.867 | 5.774 |
| moonlit-air | paused | 0.896 | 0.908 | 0.979 | 0.896 |
| moonlit-air | advancing | 0.954 | 0.961 | 1.034 | 0.954 |
