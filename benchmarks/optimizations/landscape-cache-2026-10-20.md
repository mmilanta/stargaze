# CPU landscape cache — October 20, 2026

Frame construction regenerates the same forest or rocks for an unchanged host
radius, vegetation density and ground theme. Cache one generated landscape per
theme and thread, then return an independent vector to each caller. Changing the
radius or density replaces that theme's entry. Alpine and dunes still return an
empty vector directly.

The cache is adapted to the six ground themes in main `d5a71f3`. It keys the
exact radius bits and clamps density to 100, matching the existing generator.
Terrain generation, shaders and GPU dispatch are unchanged.

## CPU measurement

AMD Ryzen 7 9800X3D, Rust 1.98.1, release build. Earth radius, 70% vegetation,
32 warm-up calls per measurement, five rounds of 20,000 calls, alternating the
cached/uncached measurement order. Values below are medians of those five rounds;
they include allocation and destruction of the returned vector.

| Ground | Uncached ns/call | Cached ns/call | Ratio |
|---|---:|---:|---:|
| Forest | 8978.5 | 58.7 | 153.0× |
| Desert | 2341.2 | 34.6 | 67.6× |
| Craters | 2334.6 | 33.7 | 69.3× |
| Basalt | 2339.9 | 33.7 | 69.5× |

These are warm-cache CPU microbenchmarks, not application FPS or GPU speedups.
The absolute saving is about 9 microseconds per forest call and 2.3 microseconds
per rocky-ground call. Cold misses still generate the landscape and additionally
clone it into the cache. Alpine/dunes do no generation and are omitted from the
speedup table. Timings depend on the machine and compiler.

## Validation and reproduction

Six cache regression tests cover exact generator bytes across 180 inputs
(six themes, five radii, six densities), independent returned vectors, exact
radius keys, density normalization, replacement, per-theme retention, thread
isolation and nested generation. The release test suite passes 123 tests;
53 opt-in GPU/benchmark tests are ignored. The CPU microbenchmark passes separately.

```sh
cargo test --locked --release
cargo test --locked --release benchmark_landscape_cache -- \
  --ignored --nocapture --test-threads=1
```
