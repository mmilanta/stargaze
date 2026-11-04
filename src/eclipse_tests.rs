//! The eclipse-defined day uses the same authored scene and renderer as play.
use super::*;

#[test]
#[ignore = "requires a GPU; inspect ground at multiple camera directions"]
fn gpu_umbra_ground_views() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(
        crate::config::parse(include_str!("../configs/3-umbra-valley.yaml")).unwrap(),
    );
    let size = (480, 270);
    let mut tracer = PathTracer::new_with_guided_ground(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size,
        &[],
        Options {
            samples_per_frame: 1,
            max_bounces: 2,
            sample_limit: 32,
            ..test_options()
        },
        true,
    );
    for az in [0_f64, 90.0, 180.0, 270.0] {
        for alt in [-80_f64, -30.0] {
            state.observer.az = az.to_radians();
            state.observer.alt = alt.to_radians();
            let frame = state.build_frame(size.0, size.1);
            let pixels = sample(&device, &queue, &mut tracer, &frame, 32);
            let name = format!("ground-{az}-{alt}");
            super::surfaces::save_surface(&name, size, &pixels, 1.0);
            assert!(
                pixels.iter().flatten().all(|x| x.is_finite() && *x >= 0.0),
                "invalid radiance: {name}"
            );
        }
    }
}

#[test]
#[ignore = "requires a GPU; flat ground must have continuous noise and finite lighting"]
fn gpu_umbra_look_down() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(
        crate::config::parse(include_str!("../configs/3-umbra-valley.yaml")).unwrap(),
    );
    state.observer.alt = -70_f64.to_radians();
    let size = (320, 240);
    let frame = state.build_frame(size.0, size.1);
    for guided in [false, true] {
        let mut tracer = PathTracer::new_with_guided_ground(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            size,
            &[],
            Options {
                samples_per_frame: 4,
                max_bounces: 8,
                sample_limit: 64,
                ..test_options()
            },
            guided,
        );
        let pixels = sample(&device, &queue, &mut tracer, &frame, 16);
        assert!(
            pixels.iter().flatten().all(|x| x.is_finite() && *x >= 0.0),
            "invalid ground radiance with guided={guided}"
        );
        let name = if guided {
            "umbra-look-down-guided"
        } else {
            "umbra-look-down-reference"
        };
        super::surfaces::save_surface(name, size, &pixels, 1.0);

        if !guided {
            continue;
        }
        // Isolate the broad colour field. Across these few metres it should
        // change smoothly, even when hit positions straddle the y=0 plane.
        // The broken floor/fract pair exposed mesh triangles with >0.3 jumps.
        let hook = "sum += trace(Ray(vec3<f32>(0.0), direction, MISS), &rng, &backdrop);";
        let source = TRACE_SHADER.replace(
            hook,
            r#"
            let ray = Ray(vec3<f32>(0.0), direction, MISS);
            let hit = closest_hit(ray);
            let p = landscape_point(ray, hit);
            let noise = vnoise(p * 0.007 + vec3<f32>(19.0, 2.0, 4.0));
            sum += vec4<f32>(vec3<f32>(noise), 0.0);
        "#,
        );
        assert_eq!(TRACE_SHADER.matches(hook).count(), 1);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ground noise continuity"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&tracer.trace_layout)],
            immediate_size: 0,
        });
        tracer.compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ground noise continuity"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        tracer.history = History::default();
        let noise = sample(&device, &queue, &mut tracer, &frame, 1);
        assert!(
            noise
                .iter()
                .all(|p| p[0].is_finite() && (0.0..=1.0).contains(&p[0]))
        );
        let lo = noise.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let hi = noise.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (0.0..0.02).contains(&(hi - lo)),
            "discontinuous ground noise: {lo}..{hi}"
        );
    }
}

#[test]
#[ignore = "requires a GPU; daylight and totality on the locked two-moon world"]
fn gpu_umbra_day_and_eclipse() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(
        crate::config::parse(include_str!("../configs/3-umbra-valley.yaml")).unwrap(),
    );
    let size = (320, 180);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size,
        &[],
        Options {
            samples_per_frame: 4,
            max_bounces: 4,
            sample_limit: 48,
            ..test_options()
        },
    );
    let mut energies = Vec::new();
    for (name, time) in [("umbra-daylight", -0.08), ("umbra-totality", 0.0)] {
        state.sim_time = time;
        state.point_at_fov(state.scene.default_target, 25_f64.to_radians());
        let frame = state.build_frame(size.0, size.1);
        let pixels = sample(&device, &queue, &mut tracer, &frame, 12);
        assert!(pixels.iter().flatten().all(|x| x.is_finite() && *x >= 0.0));
        let energy: f64 = pixels.iter().map(|p| f64::from(p[0] + p[1] + p[2])).sum();
        energies.push(energy);
        super::surfaces::save_surface(name, size, &pixels, 1.0);
    }
    assert!(
        energies[0] > 2.0 * energies[1],
        "daylight and totality energy: {energies:?}"
    );
}
