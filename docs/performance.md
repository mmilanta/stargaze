# Performance and image quality

The [renderer benchmark suite](../benchmarks/README.md) provides reproducible
hardware GPU timings, fixed reference images, and before/after comparison tools.
Its October 4 Radeon measurements supersede the software-renderer measurements
below for assessing performance on this machine. The older numbers remain here
as historical records of their explicitly stated workloads.

Open the observatory gear button or press F11. Settings apply to every map, save
between launches, and use a draft until Apply. Lower render resolution affects
the traced scene; menus, picking and font rasterization remain at native resolution.

| Goal | Starting point | Tradeoff |
| --- | --- | --- |
| Smooth motion | Eco, 1 sample/frame, 60 fps cap | 50% resolution and 2 bounces reduce work; frame rate still depends on GPU capacity |
| Paused refinement | Balanced, 4–8 samples/frame, 60 fps or Uncapped | More samples per displayed frame; individual frames take longer. Stop time and camera motion to accumulate |
| Maximum detail | High, 1–4 samples/frame, 4,096 still samples or Continuous | Full resolution and 8 bounces need more time and memory |

The frame cap and still-image limit are independent of the Eco/Balanced/High
presets. Presets change resolution, samples/frame, bounces and vegetation density. The default is
Balanced, capped at 30 fps with a 256-sample still limit. Uncapped remains subject
to display presentation. Continuous still refinement stops at the integrator's
precision limit, 16,777,216 samples. Changing frame rate, batch size or the still
limit preserves valid accumulation; changing resolution or bounces restarts it.

## Level-entry freeze

On October 1 at 23:48:03, 23:48:37 and 23:48:55 CEST, the kernel logged AMDGPU
watchdog timeouts attributed to Stargaze. The reported symptom was a black,
stuck screen when opening any level, including Amber Horizon (Halo). The saved
settings requested 100% resolution, 35 samples/frame and 10 bounces. These
logs confirm GPU stalls; the large initial batch is the likely trigger.

The renderer now starts new or moving views with one sample, and treats the
samples/frame setting as a maximum. Paused views increase their batch at most
2× per completed frame, based on observed work time. Slower work reduces the
batch immediately. The timing target uses 75% of the frame interval, bounded
between 3 and 50 ms. Resolution, bounce count and the final sample limit are
preserved.

Only one frame may be in flight. GPU completion is polled without waiting,
independently of the presentation cap, at 4 ms intervals while work is pending.
The event loop continues handling input rather than blocking surface acquisition
behind a queue of trace jobs. The initial menu backdrop also uses one sample.

104 ordinary tests pass in the integrated workspace. GPU checks cover completion callbacks, the reported
35-sample/10-bounce request on both Puzzle and Halo, accumulation up to the exact
limit, and resetting to one sample for a changed scene. The existing quality
regression still passes. These checks run on llvmpipe at small viewport sizes;
the original Radeon watchdog condition still needs a hardware retest.

The subsequent startup hang was a separate Wayland scheduling bug: the app
called `pre_present_notify` before attempting to render, including attempts
deferred by the GPU submission gate. Winit then waited for a frame callback
from a surface commit that never happened, suppressing every later redraw.
The notification now runs only immediately before the actual presentation,
after acquisition, encoding and submission have succeeded. Busy-GPU and
surface-acquisition retries leave Wayland free to deliver another redraw.

Plain `--game` also enters the first field study directly again, restoring its
saved progress. Explicit configurations and built-in presets retain precedence;
opening the app without options still shows the main menu.

## Measurements

Release builds measured on this machine on October 1, 2026. This session only
exposes llvmpipe (LLVM 22.1.8), Mesa 26.2.2, GL: these are software-renderer
measurements, not Radeon frame-rate estimates.

Warm 4K frame preparation costs 0.033 ms for the observatory and 0.039 ms for
the title screen. This measures frame/UI assembly alone, excluding GPU work,
vertex uploads and presentation. It does not indicate a menu CPU bottleneck in
these fixtures.

Automatic exposure used to launch a full-frame shader and immediately reject
pixels outside the central metering rectangle. The shader now dispatches only
that rectangle. At 3840×2160, this reduces launched invocations from 8,294,400 to
2,073,600, without changing the pixels read, histogram, percentile or smoothing.
Odd and tiny viewports use the exact original bounds. Eight GPU cases compare
every histogram bin against the original full-frame shader, including partial
workgroups, dark pixels, non-finite values and decorative sky radiance.

The isolated meter benchmark alternates both implementations, warms three pairs,
and reports medians of twelve submissions per implementation. Timing includes
submission and completion; it is not total application frame time.

| Meter input | Original | Cropped dispatch |
| --- | --- | --- |
| 1920×1080, uniform | 3.156 ms | 3.062 ms |
| 1920×1080, wide brightness range | 1.273 ms | 0.986 ms |
| 3840×2160, uniform | 12.337 ms | 11.949 ms |
| 3840×2160, wide brightness range | 5.598 ms | 3.437 ms |

The uniform fixture puts every pixel in one histogram bin; the wide-range fixture
spreads brightness over all 256 bins. Neither is a full path-traced game frame.
A workgroup histogram experiment improved uniform input but regressed diverse
input, so it was not retained.

Atmospheric transport uses 24 view integration steps and 12 sun integration
steps, including visibility rays, for each relevant sample. From the shader's
work count, that is a likely larger optimization target than UI assembly.
Profiling the full scene on the hardware GPU should establish its actual share
before changing atmospheric integration or image quality. No whole-game FPS
improvement is claimed from the isolated meter timings.

## Reproduce on the hardware GPU

```sh
cargo test --release profile_ -- --ignored --nocapture --test-threads=1
cargo test --release gpu_cropped_meter_matches_full_frame_histogram \
  -- --ignored --nocapture
cargo test --release gpu_auto_exposure_ -- --ignored --nocapture --test-threads=1
```

The benchmark prints the selected adapter. Compare equivalent scenes, telescope
angles, render resolution, bounces, samples/frame and exposure mode when measuring
actual frame rate. Paused convergence and moving-camera performance have different
workloads; a paused view eventually reaches its sample limit and sleeps.


## Continuous terrain (October 2)

The initial ellipsoid hills have been replaced by a continuous heightfield with
524,288 triangles. Heights, smooth derivatives and min/max bounds are generated
once and cached in about 3.68 MiB. Rays traverse the quadtree near to far, testing
exact triangles in 2×2-cell leaves. This adds no noise generation or erosion to
the rendering loop. Separate props use 61 records, including cluster bounds.
See [terrain generation](terrain.md) for the algorithm and literature.

On llvmpipe, the 384×216 daylight fixture with two bounces took 409.5 ms/sample
in a four-sample tuning run. The final 640×360, 32-sample reference took
1,173.3 ms/sample. Each measurement includes submission, completion and readback;
the shader is warmed first. The earlier ellipsoid scene measured 164.4 ms/sample
at 384×216 with eight samples. Continuous terrain is more expensive in this
software-renderer fixture; these numbers are not Radeon frame-rate estimates.
The adaptive submission cap remains in place, and reduced render resolution
remains the main available control for terrain rendering cost.

The reference image at `docs/rocky-lookout.png` is updated with the forest pass below. GPU regression checks
compare heightfield traversal against an independent exhaustive f64 triangle
scan on Earth-sized and small hosts, including rays parallel to tile boundaries.
Other checks compare grouped props with an exhaustive scan at three azimuths,
verify darkness without light sources, and exercise local shadows, open-sky
visibility and accumulation after quality changes. CPU checks verify all terrain
bounds and volume conservation during erosion.

## Forest clearing and sky lighting

The forest adds bounded conifers, grass and logs (198 records at full density).
Its 128-texel sky-light cache is generated once when scene history resets, then
reused. Local diffuse vertices sample the cache with one visibility ray, avoiding
full atmospheric integration at each surface. The additional geometry and sky
visibility work have a cost; vegetation density is available in Settings and
in the Eco/Balanced/High presets (35%/70%/100%).

A warm 384×216 preview at full vegetation density, two bounces and eight samples
measured 507.7 ms/sample on llvmpipe. This includes submission and readback and
excludes cache generation after warm-up. It is not a hardware GPU frame-rate
estimate. Moving views regenerate the cache; hardware profiling remains needed.

The final 640×360, 32-sample forest reference measured 1,395.4 ms/sample, versus
1,173.3 ms/sample for the earlier terrain-only reference at the same resolution,
bounce count and sample count (about 19% higher). These are separate warm runs
on the software renderer, not a controlled Radeon benchmark; cache regeneration
is excluded. The native Settings preview and full forest image were inspected.
