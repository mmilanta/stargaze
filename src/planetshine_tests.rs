//! Night-air regressions using the production texture and atmosphere passes.
use super::*;

fn moonlit_scene() -> Frame {
    let mut scene = frame();
    let air = crate::sim::Atmosphere::earthlike();
    let radius = (6371.0 / crate::sim::AU_KM) as f32;
    let center = [0.0, 0.0, radius + (0.002 / crate::sim::AU_KM) as f32];
    scene.globals.atmo_center = [center[0], center[1], center[2], radius];
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
    scene.bodies = vec![
        body(center, radius, [0.0; 3], false),
        body([0.0004, 0.0, -0.002], 0.00015, [0.8, 0.6, 0.4], false),
        body([0.3, 0.0, 1.0], 0.005, [100_000.0; 3], true),
    ];
    scene
}

fn texture(sphere: &mut Sphere, style: crate::sim::SurfaceStyle) {
    let surface = crate::config::default_surface(style, sphere.color);
    sphere.axis_x = [1.0, 0.0, 0.0, style.code()];
    sphere.axis_y = [0.0, 1.0, 0.0, 17.0];
    let gas = style == crate::sim::SurfaceStyle::GasGiant;
    let c = surface.secondary_albedo;
    sphere.surface_color = [
        c[0],
        c[1],
        c[2],
        if gas {
            surface.turbulence
        } else {
            surface.craters
        },
    ];
    sphere.surface_params = if gas {
        [surface.bands, surface.storms as f32, 0.0, 0.0]
    } else {
        [surface.ice_caps, surface.ocean, surface.clouds, 0.0]
    };
}

#[test]
#[ignore = "requires a GPU; every restored texture must illuminate night air and respect phases"]
fn gpu_textured_planets_light_night_air() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 32,
            max_bounces: 1,
            ..test_options()
        },
    );
    for style in crate::sim::SurfaceStyle::ALL {
        let mut scene = moonlit_scene();
        texture(&mut scene.bodies[1], style);
        let glow = |scene: &Frame, tracer: &mut PathTracer| {
            let lit = sample(&device, &queue, tracer, scene, 32)[0];
            let mut black = moonlit_scene();
            black.bodies[1] = scene.bodies[1];
            black.bodies[1].color = [0.0; 3];
            black.bodies[1].axis_x[3] = 0.0;
            black.bodies[2] = scene.bodies[2];
            let background = sample(&device, &queue, tracer, &black, 32)[0];
            std::array::from_fn::<_, 3, _>(|c| lit[c] - background[c])
        };
        let full = glow(&scene, &mut tracer);
        scene.bodies[2].center = [1.0, 0.0, 0.0];
        let quarter = glow(&scene, &mut tracer);
        scene.bodies[2].center = [-0.3, 0.0, -1.0];
        let new = glow(&scene, &mut tracer);
        eprintln!(
            "{}: full={full:?}, quarter={quarter:?}, new={new:?}",
            style.name()
        );
        assert!(
            full.iter().all(|v| v.is_finite() && *v > 1e-4),
            "{} lost its glow",
            style.name()
        );
        assert!(
            quarter[0] > full[0] * 0.03 && quarter[0] < full[0] * 0.8,
            "incorrect phase: {style:?}"
        );
        assert!(
            new.iter().zip(full).all(|(n, f)| n.abs() < f * 0.03),
            "unlit face glows: {style:?}"
        );
    }
}

#[test]
#[ignore = "requires a GPU; air samples must survive batches, sample limits, resize and reset"]
fn gpu_night_air_is_independent_of_sample_batch() {
    let (device, queue) = gpu();
    let options = Options {
        max_bounces: 1,
        sample_limit: 79,
        ..test_options()
    };
    let mut single = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (3, 2),
        &[],
        options,
    );
    let mut batched = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (3, 2),
        &[],
        Options {
            samples_per_frame: 64,
            ..options
        },
    );
    for size in [(3, 2), (7, 5)] {
        single.resize(&device, size);
        batched.resize(&device, size);
        for style in [
            crate::sim::SurfaceStyle::GasGiant,
            crate::sim::SurfaceStyle::Icy,
        ] {
            let mut scene = moonlit_scene();
            texture(&mut scene.bodies[1], style);
            let expected = sample(&device, &queue, &mut single, &scene, 79);
            let actual = sample(&device, &queue, &mut batched, &scene, 2);
            assert_eq!(single.samples(), 79);
            assert_eq!(batched.samples(), 79);
            assert_eq!(
                actual, expected,
                "sample indices differ: {size:?}, {style:?}"
            );
            let rested = sample(&device, &queue, &mut batched, &scene, 1);
            assert_eq!(rested, expected, "completed accumulation changed");
        }
    }
}

#[test]
#[ignore = "requires a GPU; level-1 Calyx must produce an off-disc atmospheric glow at night"]
fn gpu_calyx_textures_preserve_night_glare() {
    let (device, queue) = gpu();
    let state = crate::State::with_scene(
        crate::config::parse(include_str!("../configs/1-halo-volcano.yaml")).unwrap(),
    );
    let size = (160, 90);
    let mut tracer = PathTracer::new_with_guided_ground(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size,
        &[],
        Options {
            samples_per_frame: 4,
            max_bounces: 4,
            sample_limit: 64,
            auto_exposure: true,
            ..test_options()
        },
        true,
    );
    let scene = state.build_frame(size.0, size.1);
    let positions = state.scene.positions(state.sim_time);
    let view = state
        .observer
        .frame(&state.scene, state.sim_time, &positions);
    for star in state.scene.star_indices() {
        assert!(
            (positions[star] - view.position)
                .normalize()
                .dot(view.zenith)
                < 0.0,
            "fixture must be at night"
        );
    }
    let pixels = sample(&device, &queue, &mut tracer, &scene, 16);
    let exposure = super::surfaces::read_exposure(&device, &queue, &tracer);
    super::surfaces::save_surface("calyx-night-textured", size, &pixels, exposure);
    let hook = "sum += trace(Ray(vec3<f32>(0.0), direction, MISS), &rng, &backdrop);";
    let source = guided_ground::source().replace(
        hook,
        r#"
        let ray = Ray(vec3<f32>(0.0), direction, MISS);
        let hit = closest_hit(ray);
        let ring = closest_ring(ray, hit.distance);
        if (hit.index == MISS && ring.index == MISS) {
            sum += vec4<f32>(primary_air.radiance, 0.0);
        }
    "#,
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Calyx night-air probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&tracer.trace_layout)],
        immediate_size: 0,
    });
    tracer.compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    tracer.history = History::default();
    let glow = sample(&device, &queue, &mut tracer, &scene, 16);
    // Keep every occulting sphere and ring, but switch off Calyx's reflected
    // radiance. The difference isolates planetshine from stellar twilight.
    let calyx = state
        .scene
        .bodies
        .iter()
        .filter(|body| body.kind != crate::sim::BodyKind::Anchor)
        .position(|body| body.name == "Calyx")
        .unwrap();
    let mut dark = state.build_frame(size.0, size.1);
    dark.bodies[calyx].color = [0.0; 3];
    dark.bodies[calyx].axis_x[3] = 0.0;
    let background = sample(&device, &queue, &mut tracer, &dark, 16);
    let peak = glow
        .iter()
        .zip(&background)
        .map(|(lit, black)| (0..3).map(|c| lit[c] - black[c]).sum::<f32>())
        .fold(0.0_f32, f32::max);
    eprintln!(
        "Calyx night t={}, off-disc Calyx contribution={peak}, exposure={exposure}",
        state.sim_time
    );
    assert!(pixels.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    assert!(glow.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    assert!(peak > 1e-5, "Calyx must illuminate air outside its disc");
}
