# Incremental experiments

These short runs guided implementation; they are **screening measurements**, not
independently established speedups. Each addition builds on the preceding row.
They used identical inputs: 1280×720 display, 960×540 tracing, four bounces,
70% vegetation, four warmups, one repetition of four frames, and two-sample
reference images. GPU clock/cache state was not controlled. The longer, repeated
seven-view results in the parent directory are the basis of the performance claim.

| Cumulative implementation | Daylight advancing GPU ms | Vantus advancing GPU ms | Cost / compromise |
|---|---:|---:|---|
| Any-hit shadow queries | 135.458 | 37.773 | Shadows use a finite segment and stop at the first blocker; more shader code, unchanged transport |
| + Compact terrain traversal stack | 126.614 | 36.430 | 256 rather than 640 bytes of stack per ray; extra integer decoding per visited node |
| + Cached terrain bounds, vertices and normals | 123.331 | 36.158 | 8.02 MiB extra GPU memory; one compute pass on first terrain use or host-radius change |
| + Compact emissive-body indices | 120.450 | 32.676 | 16 uniform bytes and a CPU body scan per submission; up to three sources cached, larger systems retain the original scan |
| Separate atmosphere pass (rejected) | 119.764 | 30.612 | Extra full-resolution buffer and passes; measured wall medians rose to 323.888 / 231.521 ms |

The any-hit change alone did not improve the daylight case relative to the
long-run original baseline. Later changes recovered that loss. The final bundle
is validated together; these short runs should not be used to promise additive
or universal gains from each component.

All eight captured HDR RGBA and displayed RGBA images match byte-for-byte between
each successive retained experiment (24 image pairs): see
[image-exactness.json](image-exactness.json). The final bundle additionally has
28 full-resolution comparisons against the original renderer, including 32-spp
images and advancing checkpoints. No approximate atmospheric integration,
reduced bounce count, reduced resolution or reduced vegetation was retained.

Each experiment directory preserves request, raw timings, summary and source
hashes. Experimental sources were uncommitted; hashes identify what was tested,
but these are not separately checkoutable implementations. The retained source
is commit `058f4a5`; reproduction and final validation are documented above.
