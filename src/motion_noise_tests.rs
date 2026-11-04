//! Compare raw one-sample eclipse frames with the previous estimator.
use super::*;

const ROULETTE_MINIMUM: &str = "inverseSqrt(clamp(f32(settings.counts.y) + 1.0, 1.0, 4096.0))";
const ECLIPSE_SAMPLES: &str = "let light_samples = eclipse_light_samples(light_ray, b);";

fn legacy_source() -> String {
    assert_eq!(TRACE_SHADER.matches(ROULETTE_MINIMUM).count(), 1);
    assert_eq!(TRACE_SHADER.matches(ECLIPSE_SAMPLES).count(), 1);
    TRACE_SHADER
        .replace(ROULETTE_MINIMUM, "1.0 / 64.0")
        .replace(ECLIPSE_SAMPLES, "let light_samples = 1u;")
}

fn install(device: &wgpu::Device, tracer: &mut PathTracer, source: String) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("moving eclipse comparison"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&tracer.trace_layout)],
        immediate_size: 0,
    });
    let pipeline = |entry| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    tracer.compute = pipeline("main");
    tracer.camera_air_pipeline = pipeline("integrate_camera_air");
    tracer.sky_light = pipeline("cache_surface_sky");
}

fn eclipsed_planetshine(size: (u32, u32), step: usize) -> Frame {
    let mut scene = frame();
    let air = crate::sim::Atmosphere::earthlike();
    let km = (1.0 / crate::sim::AU_KM) as f32;
    let radius = 6371.0 * km;
    let center = [0.0, 0.0, radius + 0.002 * km];
    scene.globals.viewport = [
        size.0 as f32,
        size.1 as f32,
        1.0,
        size.0 as f32 / size.1 as f32,
    ];
    scene.globals.cam_forward[3] = 5_f32.to_radians().tan();
    scene.globals.ground_up = [0.0, 0.0, -1.0, 6371000.0];
    scene.globals.atmo_center = [0.0, 0.0, center[2], radius];
    scene.globals.atmo_rayleigh = [
        air.rayleigh[0] as f32,
        air.rayleigh[1] as f32,
        air.rayleigh[2] as f32,
        air.mie as f32,
    ];
    scene.globals.atmo_params = [
        air.mie_g as f32,
        air.scale_height as f32,
        air.thickness as f32,
        1.0,
    ];
    let d = 100_000.0 * km;
    scene.bodies = vec![
        body(center, radius, [0.0; 3], false),
        body(
            [0.0004 + step as f32 * 0.00001, 0.0, -0.002],
            0.00004,
            [0.8, 0.6, 0.4],
            false,
        ),
        body([-0.8, 0.0, -0.6], 0.005, [1e7; 3], true),
        body(
            [-0.8 * d + step as f32 * 0.5 * km, 0.0, -0.6 * d],
            1100.0 * km,
            [0.0; 3],
            false,
        ),
    ];
    scene.scene_time = step as f64;
    scene
}

fn luminance(pixel: &[f32; 4]) -> f64 {
    0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2])
}

fn mse(image: &[[f32; 4]], reference: &[[f32; 4]]) -> f64 {
    image
        .iter()
        .zip(reference)
        .map(|(p, r)| (luminance(p) - luminance(r)).powi(2))
        .sum::<f64>()
        / image.len() as f64
}

#[test]
#[ignore = "requires a GPU; measures first-frame eclipse fireflies without display filtering"]
fn gpu_moving_eclipse_planetshine_noise() {
    let (device, queue) = gpu();
    let size = (128, 72);
    let options = Options {
        max_bounces: 1,
        ..test_options()
    };
    let make = |options| {
        PathTracer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            size,
            &[],
            options,
        )
    };
    let mut candidate = make(options);
    let mut old = make(options);
    install(&device, &mut old, legacy_source());
    let mut reference = make(Options {
        samples_per_frame: 64,
        ..options
    });
    install(
        &device,
        &mut reference,
        TRACE_SHADER.replace(ROULETTE_MINIMUM, "1.0"),
    );
    let mut old_error = 0.0;
    let mut new_error = 0.0;
    for step in 0..4 {
        let scene = eclipsed_planetshine(size, step);
        let expected = sample(&device, &queue, &mut reference, &scene, 16);
        let before = sample(&device, &queue, &mut old, &scene, 1);
        let after = sample(&device, &queue, &mut candidate, &scene, 1);
        assert_eq!(candidate.samples(), 1);
        old_error += mse(&before, &expected);
        new_error += mse(&after, &expected);
        let mean =
            |image: &[[f32; 4]]| image.iter().map(luminance).sum::<f64>() / image.len() as f64;
        eprintln!(
            "planetshine step={step} MSE old={} new={} mean reference={} old={} new={}",
            mse(&before, &expected),
            mse(&after, &expected),
            mean(&expected),
            mean(&before),
            mean(&after)
        );
        surfaces::save_surface(
            &format!("eclipse-planetshine-before-{step}"),
            size,
            &before,
            20.0,
        );
        surfaces::save_surface(
            &format!("eclipse-planetshine-after-{step}"),
            size,
            &after,
            20.0,
        );
    }
    eprintln!("planetshine MSE ratio={}", new_error / old_error);
    assert!(old_error > 1e-12, "fixture must reproduce eclipse speckles");
    assert!(
        new_error < old_error * 0.2,
        "first-frame planetshine still contains roulette fireflies"
    );
}

fn eclipsed_sun(size: (u32, u32), step: usize) -> Frame {
    let mut scene = eclipsed_planetshine(size, step);
    let km = (1.0 / crate::sim::AU_KM) as f32;
    let elevation = 5_f32.to_radians();
    let up = glam::Vec3::new(0.0, elevation.cos(), -elevation.sin());
    let north = glam::Vec3::new(0.0, elevation.sin(), elevation.cos());
    let center = -up * ((6371.0 + 0.002) * km);
    let sun = -north * 0.5 + up * 0.8660254;
    scene.globals.cam_forward = [0.0, 0.0, -1.0, 4_f32.to_radians().tan()];
    scene.globals.cam_up = [0.0, 1.0, 0.0, 0.0];
    scene.globals.ground_up = [up.x, up.y, up.z, 6371000.0];
    scene.globals.atmo_center = [center.x, center.y, center.z, 6371.0 * km];
    scene.bodies = vec![
        body(center.to_array(), 6371.0 * km, [0.0; 3], false),
        body(
            sun.to_array(),
            0.00465,
            [115384.68, 109615.445, 101538.516],
            true,
        ),
        body(
            (sun * (100_000.0 * km) + glam::Vec3::X * (step as f32 * 0.5 * km)).to_array(),
            513.0 * km,
            [0.0; 3],
            false,
        ),
    ];
    scene
}

#[test]
#[ignore = "requires a GPU; measures partial stellar visibility along moving eclipse sky rays"]
fn gpu_moving_eclipse_sun_noise() {
    let (device, queue) = gpu();
    let size = (192, 108);
    let options = Options {
        max_bounces: 1,
        ..test_options()
    };
    let make = |options| {
        PathTracer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            size,
            &[],
            options,
        )
    };
    let mut candidate = make(options);
    let mut old = make(options);
    let hook = "let light_samples = eclipse_light_samples(light_ray, b);";
    assert_eq!(TRACE_SHADER.matches(hook).count(), 1);
    install(
        &device,
        &mut old,
        TRACE_SHADER.replace(hook, "let light_samples = 1u;"),
    );
    let mut reference = make(Options {
        samples_per_frame: 64,
        ..options
    });
    let mut old_error = 0.0;
    let mut new_error = 0.0;
    for step in [0, 8, 15] {
        let scene = eclipsed_sun(size, step);
        let expected = sample(&device, &queue, &mut reference, &scene, 16);
        let before = sample(&device, &queue, &mut old, &scene, 1);
        let after = sample(&device, &queue, &mut candidate, &scene, 1);
        old_error += mse(&before, &expected);
        new_error += mse(&after, &expected);
        eprintln!(
            "sun step={step} MSE old={} new={}",
            mse(&before, &expected),
            mse(&after, &expected)
        );
        surfaces::save_surface(&format!("eclipse-sun-before-{step}"), size, &before, 10.0);
        surfaces::save_surface(&format!("eclipse-sun-after-{step}"), size, &after, 10.0);
        surfaces::save_surface(
            &format!("eclipse-sun-reference-{step}"),
            size,
            &expected,
            10.0,
        );
    }
    eprintln!("sun MSE ratio={}", new_error / old_error);
    assert!(old_error > 1e-12);
    assert!(new_error < old_error * 0.65);
}

#[test]
#[ignore = "requires a GPU; compare the authored Umbra eclipse with the previous one-sample estimator"]
fn gpu_authored_umbra_eclipse_is_preserved() {
    let (device, queue) = gpu();
    let size = (160, 90);
    let mut state = crate::State::with_scene(
        crate::config::parse(include_str!("../configs/3-umbra-valley.yaml")).unwrap(),
    );
    let options = Options {
        max_bounces: 4,
        ..test_options()
    };
    let make = |options| {
        PathTracer::new_with_guided_ground(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            size,
            &[],
            options,
            true,
        )
    };
    let mut candidate = make(options);
    let mut old = make(options);
    let legacy = guided_ground::source()
        .replace(ROULETTE_MINIMUM, "1.0 / 64.0")
        .replace(ECLIPSE_SAMPLES, "let light_samples = 1u;");
    install(&device, &mut old, legacy);
    let mut reference = make(Options {
        samples_per_frame: 64,
        ..options
    });
    install(
        &device,
        &mut reference,
        guided_ground::source().replace(ROULETTE_MINIMUM, "1.0"),
    );
    let mut old_error = 0.0;
    let mut new_error = 0.0;
    for time in [-0.001, 0.0, 0.001] {
        state.sim_time = time;
        state.point_at_fov(state.scene.default_target, 70_f64.to_radians());
        let scene = state.build_frame(size.0, size.1);
        let expected = sample(&device, &queue, &mut reference, &scene, 8);
        let before = sample(&device, &queue, &mut old, &scene, 1);
        let after = sample(&device, &queue, &mut candidate, &scene, 1);
        old_error += mse(&before, &expected);
        new_error += mse(&after, &expected);
        eprintln!(
            "Umbra time={time} MSE old={} new={}",
            mse(&before, &expected),
            mse(&after, &expected)
        );
        for (name, pixels) in [
            ("before", &before),
            ("after", &after),
            ("reference", &expected),
        ] {
            surfaces::save_surface(&format!("umbra-{time}-{name}"), size, pixels, 1000.0);
        }
    }
    eprintln!("Umbra MSE ratio={}", new_error / old_error);
    assert!(new_error <= old_error * 1.01, "authored eclipse regressed");
}

#[test]
#[ignore = "requires a GPU; first-sample quality must not change energy or sample batching"]
fn gpu_eclipse_energy_and_sample_batching() {
    let (device, queue) = gpu();
    let size = (32, 18);
    let options = Options {
        max_bounces: 1,
        sample_limit: 79,
        ..test_options()
    };
    let make = |options| {
        PathTracer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            size,
            &[],
            options,
        )
    };
    let mut single = make(options);
    let mut batched = make(Options {
        samples_per_frame: 64,
        ..options
    });
    for scene in [
        eclipsed_planetshine(size, 0),
        eclipsed_sun(size, 0),
        eclipsed_planetshine(size, 1),
    ] {
        let expected = sample(&device, &queue, &mut single, &scene, 79);
        let actual = sample(&device, &queue, &mut batched, &scene, 2);
        assert_eq!(single.samples(), 79);
        assert_eq!(batched.samples(), 79);
        assert_eq!(
            actual, expected,
            "first-frame estimator depended on batch size"
        );
        assert_eq!(sample(&device, &queue, &mut batched, &scene, 1), expected);
    }
    let options = Options {
        max_bounces: 1,
        samples_per_frame: 64,
        sample_limit: 4096,
        ..test_options()
    };
    let mut production = make(options);
    let mut old = make(options);
    install(&device, &mut old, legacy_source());
    let mut reference = make(options);
    install(
        &device,
        &mut reference,
        TRACE_SHADER.replace(ROULETTE_MINIMUM, "1.0"),
    );
    for (name, scene) in [
        ("planetshine", eclipsed_planetshine(size, 0)),
        ("sun", eclipsed_sun(size, 0)),
    ] {
        let expected = sample(&device, &queue, &mut reference, &scene, 64);
        let before = sample(&device, &queue, &mut old, &scene, 64);
        let after = sample(&device, &queue, &mut production, &scene, 64);
        let mean =
            |image: &[[f32; 4]]| image.iter().map(luminance).sum::<f64>() / image.len() as f64;
        let energy = mean(&expected);
        assert!(energy > 1e-6);
        eprintln!(
            "{name} 4096 spp mean reference={energy} old={} new={}",
            mean(&before),
            mean(&after)
        );
        assert!(
            (mean(&after) / energy - 1.0).abs() < 0.03,
            "production changed expected energy: {name}"
        );
        assert!(
            (mean(&before) / energy - 1.0).abs() < 0.03,
            "comparison reference is not converged: {name}"
        );
    }
}
