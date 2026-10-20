//! Exact CPU landscape reuse. A thread owns at most one radius/density entry
//! for each terrain theme; callers receive an independent cloned vector.
use crate::ground::{Primitive, Theme};
use std::cell::RefCell;

struct Entry {
    radius_bits: u64,
    density: u32,
    objects: Vec<Primitive>,
}

thread_local! {
    static CACHE: RefCell<[Option<Entry>; 6]> = const { RefCell::new([None, None, None, None, None, None]) };
}

pub fn get(
    radius: f64,
    density: u32,
    theme: Theme,
    build: impl FnOnce() -> Vec<Primitive>,
) -> Vec<Primitive> {
    let radius_bits = radius.to_bits();
    let density = density.min(100);
    let slot = theme as usize;
    if let Some(objects) = CACHE.with(|cache| {
        let cache = cache.borrow();
        cache[slot]
            .as_ref()
            .filter(|entry| entry.radius_bits == radius_bits && entry.density == density)
            .map(|entry| entry.objects.clone())
    }) {
        return objects;
    }
    // Release the lookup borrow before invoking application code. Generation
    // can itself use a different cached landscape without RefCell conflicts.
    let objects = build();
    CACHE.with(|cache| {
        cache.borrow_mut()[slot] = Some(Entry {
            radius_bits,
            density,
            objects: objects.clone(),
        });
    });
    objects
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn clear() {
        CACHE.with(|cache| *cache.borrow_mut() = [None, None, None, None, None, None]);
    }

    fn objects(seed: f32) -> Vec<Primitive> {
        vec![
            Primitive {
                center: [seed, -0.0, 2.5, 0.0],
                extent: [0.125, 2.0, 3.0, seed * 0.25],
                albedo: [0.9, 0.4, 0.125, 2.0],
            },
            Primitive {
                center: [seed + 7.0, 2.0, -3.0, 2.25],
                extent: [3.0, 0.25, 2.0, 1_234_567.0],
                albedo: [0.05, 0.15, 0.25, 2.0],
            },
        ]
    }

    fn bytes(objects: &[Primitive]) -> &[u8] {
        bytemuck::cast_slice(objects)
    }

    #[test]
    fn landscape_cache_preserves_exact_bytes_and_returned_vector_independence() {
        clear();
        let expected = objects(0.1234567);
        let calls = Cell::new(0);
        let mut first = get(6_371_000.0, 70, Theme::Forest, || {
            calls.set(calls.get() + 1);
            expected.clone()
        });
        assert_eq!(bytes(&first), bytes(&expected));
        first[0].center[0] = 99.0;
        first[1].albedo[2] = 0.0;
        first.pop();
        let mut second = get(6_371_000.0, 70, Theme::Forest, || {
            panic!("a hit must not rebuild")
        });
        assert_eq!(bytes(&second), bytes(&expected));
        second.clear();
        assert_eq!(
            bytes(&get(6_371_000.0, 70, Theme::Forest, Vec::new)),
            bytes(&expected)
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn landscape_cache_keys_exact_radius_bits_and_normalized_density() {
        clear();
        assert_eq!(
            get(1.0, 100, Theme::Forest, || objects(1.0))[0].center[0],
            1.0
        );
        for density in [101, 1_000, u32::MAX] {
            assert_eq!(
                get(1.0, density, Theme::Forest, || panic!(
                    "density normalizes to100"
                ))[0]
                    .center[0],
                1.0
            );
        }
        let adjacent_radius = f64::from_bits(1.0_f64.to_bits() + 1);
        assert_eq!(
            get(adjacent_radius, 100, Theme::Forest, || objects(2.0))[0].center[0],
            2.0
        );
        assert_eq!(
            get(1.0, 99, Theme::Forest, || objects(3.0))[0].center[0],
            3.0
        );
        assert_eq!(
            get(0.0, 99, Theme::Forest, || objects(4.0))[0].center[0],
            4.0
        );
        assert_eq!(
            get(-0.0, 99, Theme::Forest, || objects(5.0))[0].center[0],
            5.0
        );
    }

    #[test]
    fn landscape_cache_keeps_one_entry_per_theme() {
        clear();
        let themes: Vec<_> = std::iter::once(Theme::Forest)
            .chain(Theme::EXPLORE)
            .collect();
        for (i, &theme) in themes.iter().enumerate() {
            get(100.0, 70, theme, || objects(i as f32));
        }
        get(200.0, 70, Theme::Forest, || objects(99.0));
        for (i, &theme) in themes.iter().enumerate().skip(1) {
            assert_eq!(
                get(100.0, 70, theme, || panic!("another theme remains cached"))[0].center[0],
                i as f32
            );
        }
        assert_eq!(
            get(100.0, 70, Theme::Forest, || objects(88.0))[0].center[0],
            88.0
        );
        CACHE.with(|cache| assert_eq!(cache.borrow().iter().flatten().count(), themes.len()));
    }

    #[test]
    fn landscape_cache_is_thread_local_and_build_callback_can_reenter() {
        clear();
        let parent = get(100.0, 70, Theme::Forest, || {
            get(100.0, 70, Theme::Desert, || objects(7.0));
            objects(1.0)
        });
        let child = std::thread::spawn(|| get(100.0, 70, Theme::Forest, || objects(2.0)))
            .join()
            .unwrap();
        assert_eq!(parent[0].center[0], 1.0);
        assert_eq!(child[0].center[0], 2.0);
        assert_eq!(
            get(100.0, 70, Theme::Forest, || panic!(
                "child cannot replace parent cache"
            ))[0]
                .center[0],
            1.0
        );
        assert_eq!(
            get(100.0, 70, Theme::Desert, || panic!(
                "nested build remained cached"
            ))[0]
                .center[0],
            7.0
        );
    }

    #[test]
    fn actual_landscapes_match_uncached_generator_bytes_across_themes_and_keys() {
        clear();
        for theme in std::iter::once(Theme::Forest).chain(Theme::EXPLORE) {
            for radius in [5_000.0, 1_500_000.0, 3_200_000.0, 6_371_000.0, 60_000_000.0] {
                for density in [0, 1, 70, 100, 101, u32::MAX] {
                    let expected =
                        crate::ground::landscape_for_theme_uncached(radius, density, theme);
                    let mut cached = crate::ground::landscape_for_theme(radius, density, theme);
                    assert_eq!(
                        bytes(&cached),
                        bytes(&expected),
                        "{theme:?}, radius={radius}, density={density}: changed generated objects"
                    );
                    // Exercise the actual wrapper's hit path after a caller
                    // edits and destroys its returned landscape.
                    if let Some(first) = cached.first_mut() {
                        first.center[0] += 100.0;
                        first.extent[0] = 0.0;
                    }
                    cached.clear();
                    let again = crate::ground::landscape_for_theme(radius, density, theme);
                    assert_eq!(bytes(&again), bytes(&expected));
                }
            }
        }
    }

    #[test]
    fn actual_landscape_key_changes_are_not_vacuous_and_restore_previous_values() {
        clear();
        let sparse = crate::ground::landscape_for_theme(6_371_000.0, 0, Theme::Forest);
        let dense = crate::ground::landscape_for_theme(6_371_000.0, 100, Theme::Forest);
        assert_ne!(
            bytes(&sparse),
            bytes(&dense),
            "density must change forest geometry"
        );
        let smaller = crate::ground::landscape_for_theme(3_200_000.0, 100, Theme::Forest);
        assert_ne!(
            bytes(&smaller),
            bytes(&dense),
            "host radius must change geometry"
        );
        let desert = crate::ground::landscape_for_theme(6_371_000.0, 100, Theme::Desert);
        assert_ne!(bytes(&desert), bytes(&dense), "theme must change geometry");
        assert!(crate::ground::landscape_for_theme(6_371_000.0, 100, Theme::Alpine).is_empty());
        assert_eq!(
            bytes(&crate::ground::landscape_for_theme(
                6_371_000.0,
                100,
                Theme::Forest
            )),
            bytes(&dense)
        );
        assert_eq!(
            bytes(&crate::ground::landscape_for_theme(
                6_371_000.0,
                0,
                Theme::Forest
            )),
            bytes(&sparse)
        );
        assert_eq!(
            bytes(&crate::ground::landscape_for_theme(
                6_371_000.0,
                100,
                Theme::Desert
            )),
            bytes(&desert)
        );
    }

    #[test]
    #[ignore = "CPU microbenchmark; run with --release --ignored --nocapture --test-threads=1"]
    fn benchmark_landscape_cache() {
        use std::{hint::black_box, time::Instant};
        const CALLS: u32 = 20_000;
        for theme in std::iter::once(Theme::Forest).chain(Theme::EXPLORE) {
            let radius = 6_371_000.0;
            let density = 70;
            let uncached = || {
                crate::ground::landscape_for_theme_uncached(
                    black_box(radius),
                    black_box(density),
                    black_box(theme),
                )
            };
            let cached = || {
                crate::ground::landscape_for_theme(
                    black_box(radius),
                    black_box(density),
                    black_box(theme),
                )
            };
            assert_eq!(bytes(&uncached()), bytes(&cached()));
            let measure = |build: &dyn Fn() -> Vec<Primitive>| {
                for _ in 0..32 {
                    black_box(build());
                }
                let start = Instant::now();
                for _ in 0..CALLS {
                    black_box(build());
                }
                start.elapsed().as_secs_f64() * 1e9 / f64::from(CALLS)
            };
            let mut before = Vec::new();
            let mut after = Vec::new();
            for round in 0..5 {
                if round % 2 == 0 {
                    before.push(measure(&uncached));
                    after.push(measure(&cached));
                } else {
                    after.push(measure(&cached));
                    before.push(measure(&uncached));
                }
            }
            before.sort_by(f64::total_cmp);
            after.sort_by(f64::total_cmp);
            eprintln!(
                "{theme:?}: uncached {:.1} ns/call; cached {:.1} ns/call; {:.1}x (median of 5 x {CALLS} calls)",
                before[2],
                after[2],
                before[2] / after[2]
            );
        }
    }
}
