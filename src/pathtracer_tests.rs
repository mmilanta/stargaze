use super::*;

#[path = "ring_tests.rs"]
mod rings;

fn frame() -> Frame {
    let mut globals = Globals::zeroed();
    globals.cam_right = [1.0, 0.0, 0.0, 0.0];
    globals.cam_up = [0.0, 1.0, 0.0, 0.0];
    globals.cam_forward = [0.0, 0.0, -1.0, 1e-7];
    globals.viewport = [1.0, 1.0, 1.0, 0.0];
    Frame {
        globals,
        bodies: vec![],
        scene_time: 0.0,
        labels: vec![],
    }
}
fn body(center: [f32; 3], radius: f32, color: [f32; 3], emissive: bool) -> Sphere {
    Sphere {
        center,
        radius,
        color,
        emissive: if emissive { 1.0 } else { 0.0 },
        center_low: [0.0; 4],
        ring_plane: [0.0; 4],
        ring_params: [0.0; 4],
        ring_color: [0.0; 4],
    }
}

#[test]
fn validate_shaders_and_buffer_layouts() {
    for source in [
        TRACE_SHADER,
        include_str!("shaders/display.wgsl"),
        include_str!("shaders/exposure.wgsl"),
    ] {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        for (_, ty) in module.types.iter() {
            let expected = match ty.name.as_deref() {
                Some("Settings") => Some(std::mem::size_of::<Settings>()),
                Some("Globals") => Some(std::mem::size_of::<Globals>()),
                Some("Body") => Some(std::mem::size_of::<Sphere>()),
                Some("CatalogueStar") => Some(std::mem::size_of::<stars::RayStar>()),
                Some("Exposure") | Some("ExposureState") => Some(std::mem::size_of::<Exposure>()),
                _ => None,
            };
            if let (Some(expected), naga::TypeInner::Struct { span, .. }) = (expected, &ty.inner) {
                assert_eq!(
                    *span as usize, expected,
                    "GPU/CPU layout mismatch: {:?}",
                    ty.name
                );
            }
        }
    }
}

#[test]
fn history_resets_for_scene_changes_but_not_overlay_changes() {
    let mut history = History::default();
    let mut frame = frame();
    let options = Options::default();
    history.update(&frame, options);
    history.samples = 100;
    frame.labels.push(crate::ui::Label {
        x: 0.0,
        y: 0.0,
        radius: 4.0,
        text: "Overlay only".into(),
        body: 0,
    });
    history.update(&frame, options);
    assert_eq!(history.samples, 100);
    frame.scene_time = 1e-12;
    history.update(&frame, options);
    assert_eq!(history.samples, 0);
    history.samples = 100;
    frame.globals.cam_forward[3] *= 2.0;
    history.update(&frame, options);
    assert_eq!(history.samples, 0);
    history.samples = 100;
    frame
        .bodies
        .push(body([0.0, 0.0, -3.0], 1.0, [0.5; 3], false));
    history.update(&frame, options);
    assert_eq!(history.samples, 0);
    // Exposure is display-only, so tuning it must not discard the image.
    history.samples = 100;
    history.update(
        &frame,
        Options {
            exposure: 8.0,
            auto_exposure: true,
            ev_bias: 1.0,
            ..options
        },
    );
    assert_eq!(history.samples, 100);
    history.samples = 100;
    history.update(
        &frame,
        Options {
            max_bounces: 4,
            ..options
        },
    );
    assert_eq!(history.samples, 0);
}

fn gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("GPU test requires a compute-capable adapter");
    eprintln!("testing on {:?}", adapter.get_info());
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap()
}

fn read_pixels(device: &wgpu::Device, queue: &wgpu::Queue, tracer: &PathTracer) -> Vec<[f32; 4]> {
    let size = tracer.dimensions.0 as u64 * tracer.dimensions.1 as u64 * 16;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&tracer.accumulation, 0, &buffer, 0, size);
    queue.submit(Some(encoder.finish()));
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let view = buffer.slice(..).get_mapped_range().unwrap();
    let pixels = bytemuck::cast_slice(&view).to_vec();
    drop(view);
    buffer.unmap();
    pixels
}

fn sample(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tracer: &mut PathTracer,
    frame: &Frame,
    batches: usize,
) -> Vec<[f32; 4]> {
    for _ in 0..batches {
        let mut encoder = device.create_command_encoder(&Default::default());
        tracer.encode(queue, &mut encoder, frame);
        queue.submit(Some(encoder.finish()));
    }
    read_pixels(device, queue, tracer)
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_quality_limits_rest_resume_and_preserve_valid_samples() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            sample_limit: 65,
            auto_exposure: true,
            ..Options::default()
        },
    );
    let mut frame = frame();
    frame.bodies = vec![body([0., 0., -1.], 0.1, [2., 3., 4.], true)];
    let initial = sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(tracer.samples(), 64);
    assert!(tracer.needs_redraw());
    sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(
        tracer.samples(),
        65,
        "last batch must stop exactly at the limit"
    );
    assert!(tracer.needs_redraw(), "exposure still needs to settle");
    assert_eq!(sample(&device, &queue, &mut tracer, &frame, 60), initial);
    assert_eq!(tracer.samples(), 65);
    assert!(
        !tracer.needs_redraw(),
        "a completed still view should sleep"
    );

    tracer.set_quality(8, 1, 32);
    assert_eq!(
        tracer.samples(),
        65,
        "lowering the target preserves the better image"
    );
    assert!(!tracer.needs_redraw());
    sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(
        tracer.samples(),
        65,
        "a lower target must not underflow the counter"
    );
    tracer.set_quality(8, 1, 256);
    assert!(tracer.needs_redraw());
    sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(
        tracer.samples(),
        73,
        "a higher target resumes the existing average"
    );
    tracer.set_quality(8, 2, 256);
    assert_eq!(
        tracer.samples(),
        0,
        "changing light transport invalidates the image"
    );
    sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(tracer.samples(), 8);
    tracer.set_quality(8, 2, 8);
    sample(&device, &queue, &mut tracer, &frame, 60);
    assert!(!tracer.needs_redraw());
    frame.scene_time += 1.0;
    sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(tracer.samples(), 8);
    assert!(
        tracer.needs_redraw(),
        "a changed scene starts refinement again"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_display_upscales_hdr_with_correct_orientation_and_edges() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba32Float,
        (2, 2),
        &[],
        Options::default(),
    );
    let mut scene = frame();
    scene.globals.viewport[..2].copy_from_slice(&[4., 4.]);
    sample(&device, &queue, &mut tracer, &scene, 1);
    let colors = [
        [1_f32, 0., 0., 1.],
        [0., 1., 0., 1.],
        [0., 0., 1., 1.],
        [1., 1., 1., 1.],
    ];
    tracer.accumulation = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("upscale-test-colors"),
        contents: bytemuck::cast_slice(&colors),
        usage: wgpu::BufferUsages::STORAGE,
    });
    tracer.display_group = group(
        &device,
        &tracer.display_layout,
        &[&tracer.settings, &tracer.accumulation, &tracer.exposure],
    );
    for (width, height) in [(4, 4), (2, 2), (1, 1)] {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("upscale-test"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("upscale-readback"),
            size: height as u64 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = target.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            tracer.display(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(height),
                },
            },
            target.size(),
        );
        queue.submit(Some(encoder.finish()));
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let data = readback.slice(..).get_mapped_range().unwrap();
        let aces =
            |v: f32| ((v * (2.51 * v + 0.03)) / (v * (2.43 * v + 0.59) + 0.14)).clamp(0., 1.);
        let check = |x: usize, y: usize, expected: [f32; 3]| {
            let offset = y * 256 + x * 16;
            let pixel: &[f32] = bytemuck::cast_slice(&data[offset..offset + 16]);
            for channel in 0..3 {
                assert!(
                    (pixel[channel] - aces(expected[channel])).abs() < 1e-5,
                    "size {width}x{height}, ({x}, {y}): {pixel:?}"
                );
            }
        };
        if width == 1 {
            check(0, 0, [0.5; 3]);
        } else {
            check(0, 0, [1., 0., 0.]);
            check(width as usize - 1, 0, [0., 1., 0.]);
            check(0, height as usize - 1, [0., 0., 1.]);
            check(width as usize - 1, height as usize - 1, [1.; 3]);
            if width == 4 {
                check(1, 1, [0.625, 0.25, 0.25]);
            }
        }
        drop(data);
        readback.unmap();
    }
}

/// Actual GPU shader regression, not a CPU reimplementation. Explicit opt-in
/// keeps `cargo test` usable on machines without a GPU/display server.
#[test]
#[ignore = "requires a GPU; run cargo test gpu_transport -- --ignored --nocapture"]
fn gpu_transport() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            exposure: 1.0,
            ..Options::default()
        },
    );
    let mut frame = frame();
    let black = sample(&device, &queue, &mut tracer, &frame, 1)[0];
    assert_eq!(black, [0.0, 0.0, 0.0, 1.0]);

    // The presentation path checks the viewport on every observing frame.
    // An unchanged size must preserve both the HDR image and sample history.
    tracer.resize(&device, (1, 1));
    assert_eq!(tracer.samples(), 64);
    assert_eq!(read_pixels(&device, &queue, &tracer), vec![black]);
    sample(&device, &queue, &mut tracer, &frame, 1);
    assert_eq!(tracer.samples(), 128);

    // Emission, true silhouettes (no minimum size), and inside-sphere roots.
    frame.bodies = vec![body([0.0, 0.0, -1.0], 1e-6, [2.0, 3.0, 4.0], true)];
    assert_eq!(
        sample(&device, &queue, &mut tracer, &frame, 1)[0],
        [2.0, 3.0, 4.0, 1.0]
    );
    frame.bodies[0].center = [0.0; 3];
    frame.bodies[0].radius = 1.0;
    assert_eq!(
        sample(&device, &queue, &mut tracer, &frame, 1)[0],
        [2.0, 3.0, 4.0, 1.0]
    );

    frame.bodies = vec![
        body([0.0, 0.0, -3.0], 1.0, [0.8; 3], false),
        body([0.0, 4.0, 2.0], 0.8, [10.0; 3], true),
    ];
    let full = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    // Analytic irradiance of a uniform sphere outside the horizon:
    // Lo = albedo * Le * R^2 / d^2 * cos(theta).
    let expected = 0.8 * 10.0 * 0.8_f32.powi(2) / 32.0 * (0.5_f32).sqrt();
    assert!(
        (full - expected).abs() < expected * 0.03,
        "full={full}, expected={expected}"
    );
    assert_eq!(tracer.samples(), 2048);

    // An occluder behind the emitter cannot shadow it.
    frame
        .bodies
        .push(body([0.0, 8.0, 6.0], 1.0, [0.0; 3], false));
    let behind = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    assert!((behind - full).abs() < 1e-6);
    frame.bodies.pop();

    // Aligned occluder fully covers the stellar disc.
    frame
        .bodies
        .push(body([0.0, 2.0, 0.0], 0.5, [0.0; 3], false));
    let umbra = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    assert!(umbra < full * 0.001, "umbra={umbra}, full={full}");

    // Equal angular discs, shifted sideways: genuinely sampled partial shadow.
    frame.bodies[2].center[0] = 0.4;
    frame.bodies[2].radius = 0.4;
    let penumbra = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    assert!(
        penumbra > full * 0.2 && penumbra < full * 0.85,
        "penumbra={penumbra}, full={full}"
    );

    // Small aligned disc gives an annular eclipse, not a fully black cylinder.
    frame.bodies[2].center[0] = 0.0;
    frame.bodies[2].radius = 0.2;
    let annular = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    assert!(
        annular > full * 0.6 && annular < full * 0.85,
        "annular={annular}, full={full}"
    );

    // Multiple emitters add, rather than sharing one global shadow factor.
    frame.bodies.pop();
    frame
        .bodies
        .push(body([0.0, -4.0, 2.0], 0.8, [10.0; 3], true));
    let binary = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    assert!((binary / full - 2.0).abs() < 0.05);

    // MIS should not double-count direct emission when BSDF paths hit a light.
    tracer.options.max_bounces = 8;
    let bounced = sample(&device, &queue, &mut tracer, &frame, 128)[0][0];
    assert!(
        (bounced / binary - 1.0).abs() < 0.05,
        "MIS: {bounced} vs {binary}"
    );

    // A white nearby reflector adds indirect light to a directly shadowed point.
    frame.bodies = vec![
        body([0.0, 0.0, -3.0], 1.0, [0.8; 3], false),
        body([0.0, 4.0, 2.0], 0.8, [10.0; 3], true),
        body([0.0, 2.0, 0.0], 0.5, [0.0; 3], false),
        body([2.0, 0.0, -1.0], 1.0, [0.9; 3], false),
    ];
    let indirect = sample(&device, &queue, &mut tracer, &frame, 128)[0][0];
    assert!(indirect > 0.0001, "no reflected light: {indirect}");
    tracer.options.max_bounces = 1;
    let direct_only = sample(&device, &queue, &mut tracer, &frame, 32)[0][0];
    assert!(direct_only < 1e-6);
    eprintln!(
        "full={full:.6}, umbra={umbra:.6}, penumbra={penumbra:.6}, annular={annular:.6}, indirect={indirect:.6}"
    );

    // Resizing resets the sample history and allocates a fresh buffer.
    tracer.resize(&device, (3, 2));
    assert_eq!(tracer.samples(), 0);
    frame.globals.viewport[0] = 3.0;
    frame.globals.viewport[1] = 2.0;
    assert_eq!(sample(&device, &queue, &mut tracer, &frame, 1).len(), 6);

    // Exercise the actual presentation shader too, with an offscreen target.
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 3,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        tracer.display(&mut pass);
    }
    queue.submit(Some(encoder.finish()));
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

#[test]
#[ignore = "requires a GPU; run cargo test gpu_distant_surface -- --ignored --nocapture"]
fn gpu_distant_surface() {
    let (device, queue) = gpu();
    let size = 256;
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (size, size),
        &[],
        Options {
            samples_per_frame: 16,
            max_bounces: 1,
            exposure: 1.0,
            ..Options::default()
        },
    );
    let mut scene = frame();
    scene.globals.viewport = [size as f32, size as f32, 1.0, 0.0];
    for radius in [1e-4, 1e-6, 1e-7] {
        scene.globals.cam_forward[3] = radius / 10.0 * 1.3;
        scene.bodies = vec![
            body([0.0, 0.0, -10.0], radius, [0.8; 3], false),
            body([5.0, 0.0, -5.0], 0.05, [10000.0; 3], true),
        ];
        let pixels = sample(&device, &queue, &mut tracer, &scene, 4);
        let mut dark = 0;
        for y in 100..156 {
            for x in 100..156 {
                if pixels[y * size as usize + x][0] < 0.05 {
                    dark += 1;
                }
            }
        }
        eprintln!("distant surface (radius {radius}): {dark} incorrectly dark central pixels");
        assert_eq!(
            dark, 0,
            "unobstructed lit surface must not have black rings"
        );
    }
}

#[test]
#[ignore = "requires a GPU; run cargo test gpu_scene_smoke -- --ignored --nocapture"]
fn gpu_scene_smoke() {
    let (device, queue) = gpu();
    let stars = stars::generate(3500, 2500, 0x5EED_1234);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (320, 180),
        &stars,
        Options {
            samples_per_frame: 4,
            max_bounces: 8,
            exposure: 1.0,
            ..Options::default()
        },
    );
    for (name, scene) in [
        ("solar", crate::sim::default_scene()),
        ("saturn-surface", crate::sim::saturn_scene()),
        ("binary", crate::sim::binary_scene()),
        ("halo", crate::sim::halo_scene()),
        (
            "puzzle",
            crate::config::parse(include_str!("../configs/puzzle.yaml")).unwrap(),
        ),
        (
            "median",
            crate::config::parse(include_str!("../configs/median-resonance.yaml")).unwrap(),
        ),
        ("horizon", crate::sim::binary_scene()),
        ("vantus", crate::sim::binary_scene()),
        ("vantus_eclipse", crate::sim::binary_scene()),
    ] {
        let mut state = crate::State::with_scene(scene);
        if name == "vantus" || name == "vantus_eclipse" {
            // Ring-artifact regression and the distant-planet eclipse preset.
            state.sim_time = if name == "vantus" { 8.3757 } else { 444.478 };
            let positions = state.scene.positions(state.sim_time);
            let observer = state
                .observer
                .frame(&state.scene, state.sim_time, &positions);
            let distance = (positions[8] - observer.position).length();
            let angular_radius = (state.scene.body(8).radius / distance).asin();
            state.point_at_fov(8, angular_radius * 2.5);
        }
        if name == "horizon" {
            // Wide-angle reproduction of the false-ground-hit sky rings.
            state.sim_time = 3.9047;
            state.point_at_fov(6, 90_f64.to_radians());
            state.observer.az -= 0.32 / state.observer.alt.cos();
            // Keep the horizon in the frame independently of the moon's
            // altitude; the old offset could aim entirely into the ground.
            state.observer.alt = 5_f64.to_radians();
        }
        let frame = state.build_frame(320, 180);
        let pixels = sample(&device, &queue, &mut tracer, &frame, 16);
        assert!(pixels.iter().flatten().all(|c| c.is_finite() && *c >= 0.0));
        let lit = pixels.iter().filter(|p| p[0] + p[1] + p[2] > 0.01).count();
        // The night-horizon scene has legitimately black ground. Its old
        // bright-pixel count included catalogue fireflies; only the visible
        // sky must contain light. Geometry is checked by gpu_surface_observer.
        let minimum_lit = if name == "horizon" { 0 } else { 100 };
        assert!(lit > minimum_lit, "{name}: unexpectedly dark frame ({lit})");
        eprintln!("{name}: {lit} lit pixels at {} spp", tracer.samples());
        if let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") {
            std::fs::create_dir_all(&dir).unwrap();
            let mut ppm = b"P6\n320 180\n255\n".to_vec();
            for pixel in pixels {
                for x in pixel[..3].iter().copied() {
                    let mapped =
                        ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
                    let srgb = if mapped <= 0.0031308 {
                        12.92 * mapped
                    } else {
                        1.055 * mapped.powf(1.0 / 2.4) - 0.055
                    };
                    ppm.push((srgb * 255.0).round() as u8);
                }
            }
            std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), ppm).unwrap();
        }
    }
}

#[test]
#[ignore = "requires a GPU; run cargo test gpu_solar_planets -- --ignored --nocapture"]
fn gpu_solar_planets() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (64, 64),
        &[],
        Options {
            samples_per_frame: 16,
            max_bounces: 2,
            exposure: 1.0,
            ..Options::default()
        },
    );
    for target in [3, 5, 6, 7, 8, 9, 10] {
        let mut scene = crate::sim::default_scene();
        // Test the planet itself, not a potentially bright twilight foreground.
        scene.atmosphere = None;
        let mut state = crate::State::with_scene(scene);
        state.aim_at(target);
        let positions = state.scene.positions(state.sim_time);
        let vf = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        assert!(state.observer.alt > 0.0, "planet must be above the horizon");
        let radius = state.scene.body(target).radius;
        let angular_radius = (radius / (positions[target] - vf.position).length()).asin();
        state.point_at_fov(target, angular_radius * 2.5);
        let frame = state.build_frame(64, 64);
        let pixels = sample(&device, &queue, &mut tracer, &frame, 4);
        assert!(pixels.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
        let lit = pixels.iter().filter(|p| p[0] + p[1] + p[2] > 1e-5).count();
        assert!(
            lit > 100 && lit < pixels.len() * 9 / 10,
            "{} has an incorrect lit silhouette",
            state.scene.body(target).name
        );
        eprintln!("{}: {lit} lit pixels", state.scene.body(target).name);
    }
}

#[test]
#[ignore = "requires a GPU; run cargo test gpu_surface_observer -- --ignored --nocapture"]
fn gpu_surface_observer() {
    let (device, queue) = gpu();
    let (width, height) = (320, 180);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (width, height),
        &[],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            exposure: 1.0,
            ..Options::default()
        },
    );
    let mut state = crate::State::with_scene(crate::sim::binary_scene());
    state.sim_time = 3.9047;
    state.observer.fov_y = 90_f64.to_radians();
    state.observer.alt = 35_f64.to_radians();
    let mut scene = state.build_frame(width, height);
    let radius = state.scene.body(state.scene.host).radius as f32;
    let mut host = *scene.bodies.iter().find(|b| b.radius == radius).unwrap();
    // An emissive host makes every false primary hit unmistakable. No
    // atmosphere, catalogue, textures or indirect lighting can hide the bug.
    host.color = [1.0; 3];
    host.emissive = 1.0;
    scene.bodies = vec![host];
    scene.globals.atmo_params[3] = 0.0;
    let pixels = sample(&device, &queue, &mut tracer, &scene, 1);
    let zenith = -glam::DVec3::from_array(host.center.map(f64::from)).normalize();
    let mut false_sky_hits = 0;
    let mut ground_misses = 0;
    for y in 0..height {
        for x in 0..width {
            let dir = glam::DVec3::new(
                (2.0 * (x as f64 + 0.5) / width as f64 - 1.0) * width as f64 / height as f64,
                1.0 - 2.0 * (y as f64 + 0.5) / height as f64,
                -1.0,
            )
            .normalize();
            let altitude = dir.dot(zenith);
            let value = pixels[(y * width + x) as usize][0];
            // Exclude the horizon's antialiased silhouette.
            if altitude > 0.02 && value != 0.0 {
                false_sky_hits += 1;
            }
            if altitude < -0.02 && value != 1.0 {
                ground_misses += 1;
            }
        }
    }
    assert_eq!(
        false_sky_hits, 0,
        "outward rays must never hit the ground behind the camera"
    );
    assert_eq!(ground_misses, 0, "inward rays must hit the nearby ground");
}

#[test]
#[ignore = "requires a GPU; run cargo test gpu_atmosphere -- --ignored --nocapture"]
fn gpu_atmosphere() {
    let (device, queue) = gpu();
    let background = CatalogueStar {
        dir: [0.0, 0.0, -1.0],
        color: [1.0; 3],
        bright: 1.0,
        angular_radius: 0.01,
    };
    let options = Options {
        samples_per_frame: 64,
        max_bounces: 1,
        exposure: 1.0,
        ..Options::default()
    };
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[background],
        options,
    );
    let mut scene = frame();
    let a = crate::sim::Atmosphere::earthlike();
    let radius = (6_371.0 / crate::sim::AU_KM) as f32;
    // Camera looks straight up, two metres above the host surface.
    let center = [0.0, 0.0, radius + (0.002 / crate::sim::AU_KM) as f32];
    scene.bodies = vec![body(center, radius, [0.0; 3], false)];
    scene.globals.atmo_center = [center[0], center[1], center[2], radius];
    scene.globals.atmo_rayleigh = [
        a.rayleigh[0] as f32,
        a.rayleigh[1] as f32,
        a.rayleigh[2] as f32,
        a.mie as f32,
    ];
    scene.globals.atmo_params = [
        a.mie_g as f32,
        a.scale_height as f32,
        a.thickness as f32,
        1.0,
    ];
    let attenuated = sample(&device, &queue, &mut tracer, &scene, 1)[0];
    for (channel, actual) in attenuated[..3].iter().enumerate() {
        let column = a.scale_height
            * ((-0.002 / crate::sim::AU_KM / a.scale_height).exp()
                - (-a.thickness / a.scale_height).exp());
        let expected = (-(a.rayleigh[channel] + a.mie) * column).exp() as f32;
        assert!(
            (actual - expected).abs() < 0.01,
            "extinction: {actual} vs {expected}"
        );
    }
    scene.globals.atmo_params[3] = 0.0;
    assert_eq!(sample(&device, &queue, &mut tracer, &scene, 1)[0], [1.0; 4]);
    scene.globals.atmo_params[3] = 1.0;

    // No catalogue: any light in this zenith ray must come from scattering.
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        options,
    );
    scene
        .bodies
        .push(body([1.0, 0.0, -1.0], 0.005, [100_000.0; 3], true));
    let day = sample(&device, &queue, &mut tracer, &scene, 4)[0];
    assert!(
        day[..3].iter().all(|v| v.is_finite() && *v > 0.01),
        "day: {day:?}"
    );
    assert!(day[2] > day[0], "daytime sky should be blue: {day:?}");
    scene
        .bodies
        .push(body([0.5, 0.0, -0.5], 0.05, [0.0; 3], false));
    let eclipsed = sample(&device, &queue, &mut tracer, &scene, 4)[0];
    assert_eq!(
        eclipsed,
        [0.0, 0.0, 0.0, 1.0],
        "occluder must shadow the air"
    );
    scene.bodies.pop();

    scene.bodies[1].center = [1.0, 0.0, 0.03];
    let twilight = sample(&device, &queue, &mut tracer, &scene, 4)[0];
    assert!(
        twilight[0] > 0.0001,
        "elevated air still sees a set sun: {twilight:?}"
    );
    scene.bodies[1].center = [0.0, 0.0, 1.0];
    assert_eq!(
        sample(&device, &queue, &mut tracer, &scene, 1)[0],
        [0.0, 0.0, 0.0, 1.0]
    );

    // Dense haze must hide even the brightest catalogue dots during the day,
    // while keeping them visible at night and when an eclipse shadows the air.
    let dense = crate::sim::Atmosphere::dense();
    scene.globals.atmo_rayleigh = [
        dense.rayleigh[0] as f32,
        dense.rayleigh[1] as f32,
        dense.rayleigh[2] as f32,
        dense.mie as f32,
    ];
    let mut with_stars = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[CatalogueStar {
            bright: 1.62,
            ..background
        }],
        options,
    );
    let night = sample(&device, &queue, &mut with_stars, &scene, 4)[0];
    assert!(night[0] > 0.1, "night stars must remain visible: {night:?}");
    // Similar irradiance to Lumen at Median: luminosity 8 at about 6 AU.
    scene.bodies[1] = body([1.0, 0.0, -1.0], 0.005, [44_444.0; 3], true);
    let sky = sample(&device, &queue, &mut tracer, &scene, 4)[0];
    let day_stars = sample(&device, &queue, &mut with_stars, &scene, 4)[0];
    eprintln!("dense atmosphere: night {night:?}, day sky {sky:?}, day stars {day_stars:?}");
    for c in 0..3 {
        assert!(
            (day_stars[c] - sky[c]).abs() < 0.001,
            "daytime catalogue should disappear: {day_stars:?} vs {sky:?}"
        );
    }
    scene
        .bodies
        .push(body([0.5, 0.0, -0.5], 0.05, [0.0; 3], false));
    let eclipse = sample(&device, &queue, &mut with_stars, &scene, 4)[0];
    for c in 0..3 {
        assert!(
            (eclipse[c] - night[c]).abs() < 0.001,
            "eclipse should restore star visibility: {eclipse:?} vs {night:?}"
        );
    }
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_reflected_atmosphere() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            ..Options::default()
        },
    );
    let mut scene = frame();
    let a = crate::sim::Atmosphere::earthlike();
    let radius = (6_371.0 / crate::sim::AU_KM) as f32;
    let center = [0.0, 0.0, radius + (0.002 / crate::sim::AU_KM) as f32];
    scene.globals.atmo_center = [center[0], center[1], center[2], radius];
    scene.globals.atmo_rayleigh = [
        a.rayleigh[0] as f32,
        a.rayleigh[1] as f32,
        a.rayleigh[2] as f32,
        a.mie as f32,
    ];
    scene.globals.atmo_params = [
        a.mie_g as f32,
        a.scale_height as f32,
        a.thickness as f32,
        1.0,
    ];
    // A large moon lies just outside the zenith ray. The sun is below the
    // host's horizon, but illuminates the moon without a lunar eclipse.
    scene.bodies = vec![
        body(center, radius, [0.0; 3], false),
        body([0.0004, 0.0, -0.002], 0.00015, [0.8; 3], false),
        body([0.3, 0.0, 1.0], 0.005, [100_000.0; 3], true),
    ];
    let glow = |scene: &mut Frame, tracer: &mut PathTracer| {
        let color = scene.bodies[1].color;
        scene.bodies[1].color = [0.0; 3];
        let background = sample(&device, &queue, tracer, scene, 32)[0];
        scene.bodies[1].color = color;
        let lit = sample(&device, &queue, tracer, scene, 32)[0];
        std::array::from_fn::<_, 3, _>(|c| lit[c] - background[c])
    };
    let full = glow(&mut scene, &mut tracer);
    eprintln!("reflected atmosphere full: {full:?}");
    assert!(
        full.iter().all(|v| v.is_finite() && *v > 1e-4),
        "moon must illuminate nearby air"
    );

    scene.bodies[1].color = [0.4, 0.2, 0.1];
    let tinted = glow(&mut scene, &mut tracer);
    for c in 0..3 {
        let expected = full[c] * [0.5, 0.25, 0.125][c];
        assert!(
            (tinted[c] - expected).abs() < expected * 0.02,
            "albedo and tint: {tinted:?}"
        );
    }
    scene.bodies[1].color = [0.8; 3];
    scene.bodies[1].radius *= 0.5;
    let small = glow(&mut scene, &mut tracer);
    assert!(
        small[0] > full[0] * 0.15 && small[0] < full[0] * 0.35,
        "smaller apparent disc must give less glow: {small:?} vs {full:?}"
    );
    scene.bodies[1].radius *= 2.0;

    scene.bodies[2].center = [1.0, 0.0, 0.0];
    let quarter = glow(&mut scene, &mut tracer);
    scene.bodies[2].center = [-0.3, 0.0, -1.0];
    let new = glow(&mut scene, &mut tracer);
    eprintln!("quarter: {quarter:?}, new: {new:?}");
    assert!(quarter[0] > full[0] * 0.1 && quarter[0] < full[0] * 0.6);
    assert!(new[0].abs() < full[0] * 0.02, "unlit face must not glow");

    scene.bodies[2].center = [0.3, 0.0, 1.0];
    // Block starlight on its way to the moon, leaving the moon visible.
    scene
        .bodies
        .push(body([0.1502, 0.0, 0.499], 0.01, [0.0; 3], false));
    let eclipsed = glow(&mut scene, &mut tracer);
    assert!(
        eclipsed.iter().all(|v| v.abs() < 1e-6),
        "eclipsed moon: {eclipsed:?}"
    );
    scene.bodies.pop();
    // Block moonlight on its way to the air, leaving the moon sunlit.
    scene
        .bodies
        .push(body([0.0002, 0.0, -0.001], 0.00012, [0.0; 3], false));
    let hidden = glow(&mut scene, &mut tracer);
    assert!(
        hidden.iter().all(|v| v.abs() < 1e-6),
        "occluded moon: {hidden:?}"
    );
    scene.bodies.pop();
    scene.globals.atmo_params[3] = 0.0;
    let airless = glow(&mut scene, &mut tracer);
    assert!(
        airless.iter().all(|v| v.abs() < 1e-6),
        "airless view must have no aura"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_dense_level_daylight() {
    let (device, queue) = gpu();
    let scene = crate::config::parse(include_str!("../configs/median-resonance.yaml")).unwrap();
    let mut state = crate::State::with_scene(scene);
    let lumen = state
        .scene
        .bodies
        .iter()
        .position(|b| b.name == "Lumen")
        .unwrap();
    // Find local noon for the bright sun over one Cadence rotation.
    let noon = (0..120)
        .map(|step| {
            let t = step as f64 * 0.01;
            let positions = state.scene.positions(t);
            let view = state.observer.frame(&state.scene, t, &positions);
            (
                t,
                view.zenith
                    .dot((positions[lumen] - view.position).normalize()),
            )
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
        .0;
    let options = Options {
        samples_per_frame: 64,
        max_bounces: 1,
        ..Options::default()
    };
    let mut sky_tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        options,
    );
    for (time, altitude, azimuth) in [
        (noon, 90.0_f64, 0.0_f64),
        (noon, 45.0, 0.0),
        (noon, 45.0, 90.0),
        (noon, 45.0, 180.0),
        (noon, 45.0, 270.0),
        (noon + 0.6, 90.0, 0.0),
    ] {
        state.sim_time = time;
        state.observer.alt = altitude.to_radians();
        state.observer.az = azimuth.to_radians();
        state.observer.fov_y = 1e-7;
        let frame = state.build_frame(1, 1);
        let star = CatalogueStar {
            dir: frame.globals.cam_forward[..3].try_into().unwrap(),
            color: [1.0; 3],
            bright: 1.62,
            angular_radius: 0.01,
        };
        let mut star_tracer = PathTracer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            (1, 1),
            &[star],
            options,
        );
        let sky = sample(&device, &queue, &mut sky_tracer, &frame, 4)[0];
        let stars = sample(&device, &queue, &mut star_tracer, &frame, 4)[0];
        let contrast = (stars[0] - sky[0]).abs();
        eprintln!(
            "Median t={time:.2} alt={altitude} az={azimuth}: sky={sky:?}, star contrast={contrast}"
        );
        if time == noon {
            assert!(
                contrast < 0.001,
                "daytime stars should disappear across the sky"
            );
        } else {
            assert!(contrast > 0.1, "stars should return on the faint-sun side");
        }
    }
}

#[test]
#[ignore = "requires a GPU; run cargo test gpu_star_dots -- --ignored --nocapture"]
fn gpu_star_dots() {
    let (device, queue) = gpu();
    let options = Options {
        samples_per_frame: 64,
        max_bounces: 2,
        exposure: 1.0,
        ..Options::default()
    };
    let star = CatalogueStar {
        dir: glam::Vec3::new(0.8, 0.0, -1.0).normalize().to_array(),
        color: [1.0; 3],
        bright: 1.0,
        angular_radius: 0.1,
    };
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (128, 128),
        &[star],
        options,
    );
    let mut scene = frame();
    scene.globals.cam_forward[3] = 1.0;
    scene.globals.viewport = [128.0, 128.0, 1.0, 0.0];
    let pixels = sample(&device, &queue, &mut tracer, &scene, 1);
    let mut lit = 0;
    for (i, pixel) in pixels.iter().enumerate() {
        if pixel[0] > 0.0 {
            let dx = (i % 128) as f32 + 0.5 - 115.2;
            let dy = (i / 128) as f32 + 0.5 - 64.0;
            assert!(
                dx.hypot(dy) < 3.0 + std::f32::consts::FRAC_1_SQRT_2,
                "off-axis star exceeded the three-pixel cap"
            );
            lit += 1;
        }
    }
    assert!(lit > 20, "star should still be visible");

    // A decorative catalogue star behind the camera must not create bright
    // bounce samples on an otherwise unlit surface, at either zoom level.
    let star = CatalogueStar {
        dir: [0.0, 0.0, 1.0],
        bright: 100.0,
        ..star
    };
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[star],
        options,
    );
    let mut scene = frame();
    scene.bodies = vec![body([0.0, 0.0, -3.0], 1.0, [0.8; 3], false)];
    let before = sample(&device, &queue, &mut tracer, &scene, 128)[0][0];
    scene.globals.cam_forward[3] *= 0.01;
    let after = sample(&device, &queue, &mut tracer, &scene, 128)[0][0];
    assert_eq!(before, 0.0, "background stars must not light dark ground");
    assert_eq!(after, 0.0, "zoom must not introduce ground illumination");
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_display_extreme_exposure() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba32Float,
        (1, 1),
        &[],
        Options::default(),
    );
    let mut f = frame();
    f.bodies = vec![body([0., 0., -3.], 1., [10.; 3], true)];
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("exposure-test-target"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    for (base, bias, expected) in [
        (1., 80., 1.),
        (1., -1000., 0.),
        (1., 1000., 1.),
        (f32::MAX, 0., 1.),
    ] {
        tracer.options.exposure = base;
        tracer.set_exposure_controls(false, bias);
        let mut encoder = device.create_command_encoder(&Default::default());
        tracer.encode(&queue, &mut encoder, &f);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            tracer.display(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            target.size(),
        );
        queue.submit(Some(encoder.finish()));
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let data = readback.slice(..).get_mapped_range().unwrap();
        let pixel: &[f32] = bytemuck::cast_slice(&data[..16]);
        assert!(
            pixel[..3]
                .iter()
                .all(|v| v.is_finite() && (v - expected).abs() < 1e-5),
            "base {base}, bias {bias}: {pixel:?}"
        );
        drop(data);
        readback.unmap();
    }
    assert_eq!(
        tracer.samples(),
        4,
        "exposure changes must preserve accumulation"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_night_ground_has_no_catalogue_fireflies() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    state.sim_time = 1.1065;
    state.observer.alt = -90_f64.to_radians();
    state.observer.fov_y = 45_f64.to_radians();
    let f = state.build_frame(128, 128);
    let catalogue = stars::generate(3500, 2500, 0x5EED_1234);
    for stars in [&catalogue[..], &[][..]] {
        let mut tracer = PathTracer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            (128, 128),
            stars,
            Options {
                samples_per_frame: 64,
                ..Options::default()
            },
        );
        let pixels = sample(&device, &queue, &mut tracer, &f, 1);
        let max = pixels.iter().map(|p| p[0]).fold(0.0_f32, f32::max);
        let bright = pixels.iter().filter(|p| p[0] > 0.0001).count();
        let mean = pixels.iter().map(|p| p[0]).sum::<f32>() / pixels.len() as f32;
        eprintln!(
            "night ground with {} background stars: max {max}, mean {mean}, bright pixels {bright}",
            stars.len()
        );
        assert_eq!(
            bright, 0,
            "the unlit ground must not contain bright speckles"
        );
        assert_eq!(max, 0.0, "catalogue light must not leak into ground paths");
    }
}
