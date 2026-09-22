//! Ring geometry, transport and actual Saturn rendering regressions.
use super::*;

fn ringed(center: [f32; 3], radius: f32, pole: [f32; 3], tau: f32) -> Sphere {
    let mut planet = body(center, radius, [0.0; 3], false);
    planet.ring_plane = [pole[0], pole[1], pole[2], 1.25];
    planet.ring_params = [2.4, tau, 0.0, 0.0];
    planet.ring_color = [0.8, 0.8, 0.8, 0.0];
    planet
}

fn save_image(name: &str, width: u32, height: u32, pixels: &[[f32; 4]], exposure: f32) {
    let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for pixel in pixels {
        for value in &pixel[..3] {
            let x = value * exposure;
            let mapped = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
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

#[test]
fn saturn_rings_follow_equator_and_invalidate_history() {
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    let saturn = &state.scene.bodies[8];
    let rings = saturn.rings.unwrap();
    assert!((rings.inner_radius * saturn.radius * crate::sim::AU_KM - 74_658.0).abs() < 1e-6);
    assert!((rings.outer_radius * saturn.radius * crate::sim::AU_KM - 136_775.0).abs() < 1e-6);
    let positions = state.scene.positions(state.sim_time);
    let view = state
        .observer
        .frame(&state.scene, state.sim_time, &positions);
    let expected = view.world_to_view(saturn.spin_quat(state.sim_time) * glam::DVec3::Z);
    let frame = state.build_frame(64, 64);
    let pole = glam::Vec3::from_slice(&frame.bodies[8].ring_plane);
    assert!((pole.as_dvec3() - expected).length() < 1e-6);
    let mut history = History::default();
    history.update(&frame, Options::default());
    history.samples = 99;
    state.scene.bodies[8].rings.as_mut().unwrap().optical_depth *= 2.0;
    history.update(&state.build_frame(64, 64), Options::default());
    assert_eq!(history.samples, 0);
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_rings_geometry() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options::default(),
    );
    // Execute the production WGSL helpers, not a CPU reimplementation.
    let source = format!(
        "{TRACE_SHADER}\n{}",
        r#"
@compute @workgroup_size(8, 8)
fn probe(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u) { return; }
    let ray = Ray(vec3<f32>(0.0), vec3<f32>(0.0, 0.0, -1.0), MISS);
    let hit = ring_hit(ray, 0u);
    var result = vec4<f32>(0.0);
    if (hit.index != MISS) {
        result = vec4<f32>(hit.distance, length(hit.point) / bodies[0].center.w,
            ring_transmission(ray, hit), light_visibility(ray, 1u));
    }
    accumulation[0] = result;
}
"#
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ring-probe"),
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
        entry_point: Some("probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut f = frame();
    // Planet radius ~Saturn in AU, centre 10 AU away. Pole tilted to the ray;
    // the annulus hit must preserve its tiny radial coordinate.
    let r = 0.0004;
    f.bodies = vec![
        ringed([1.8 * r, 0.0, -10.0], r, [0.0, 0.8, 0.6], 0.7),
        body([0.0, 0.0, -12.0], 0.1, [1.0; 3], true),
    ];
    let p = sample(&device, &queue, &mut tracer, &f, 1)[0];
    let expected = (-0.7_f32 / 0.6).exp();
    assert!(
        (p[0] - 10.0).abs() < 1e-5 && (p[1] - 1.8).abs() < 1e-4,
        "{p:?}"
    );
    assert!(
        (p[2] - expected).abs() < 1e-5 && (p[3] - expected).abs() < 1e-5,
        "{p:?}"
    );
    // Inner hole, outside edge, and precisely edge-on must miss.
    for (offset, pole) in [
        (0.5, [0.0, 0.0, 1.0]),
        (2.6, [0.0, 0.0, 1.0]),
        (1.8, [0.0, 1.0, 0.0]),
    ] {
        f.bodies[0] = ringed([offset * r, 0.0, -10.0], r, pole, 0.7);
        assert_eq!(sample(&device, &queue, &mut tracer, &f, 1)[0], [0.0; 4]);
    }
    // The star lies BEFORE the disc: the disc must not attenuate its light.
    f.bodies[0] = ringed([1.8 * r, 0.0, -10.0], r, [0.0, 0.0, 1.0], 0.7);
    f.bodies[1].center[2] = -9.0;
    let p = sample(&device, &queue, &mut tracer, &f, 1)[0];
    assert!((p[3] - 1.0).abs() < 1e-6);
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_rings_transport() {
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
    let mut f = frame();
    // A tiny camera footprint at radius 1.8, looking through an absorbing
    // ring toward a white source. Null crossings must work even at one bounce.
    f.bodies = vec![
        ringed([1.8, 0.0, -5.0], 1.0, [0.0, 0.0, 1.0], 0.7),
        body([0.0, 0.0, -9.0], 0.5, [1.0; 3], true),
    ];
    f.bodies[0].ring_color = [0.0; 4];
    for (pole, expected) in [
        ([0.0, 0.0, 1.0], (-0.7_f32).exp()),
        ([0.0, 0.8, 0.6], (-0.7_f32 / 0.6).exp()),
    ] {
        f.bodies[0].ring_plane[..3].copy_from_slice(&pole);
        let p = sample(&device, &queue, &mut tracer, &f, 256)[0];
        assert!(
            (p[0] - expected).abs() < 0.02,
            "Beer-Lambert transmission: {p:?} vs {expected}"
        );
    }
    // Finite-light single scattering: compare GPU radiance to the analytic
    // slab value (the source is small enough to have nearly constant angles).
    f.bodies[0] = ringed([1.8, 0.0, -5.0], 1.0, [0.0, 0.0, 1.0], 0.7);
    // Same-side reflection, then opposite-side backlighting. Stars are off
    // the camera ray, so only scattered light contributes.
    for (star_z, same_side) in [(15.0, true), (-25.0, false)] {
        f.bodies[1] = body([10.0, 0.0, star_z], 0.05, [100_000.0; 3], true);
        let mu = 20.0_f32 / 500.0_f32.sqrt();
        let factor = if same_side {
            (1.0 - (-0.7 * (1.0 + 1.0 / mu)).exp()) / (1.0 + mu)
        } else {
            ((-0.7 / mu).exp() - (-0.7_f32).exp()) / (mu - 1.0)
        };
        let solid_angle = 2.0 * std::f32::consts::PI * (0.05_f32.powi(2) / 500.0)
            / (1.0 + (1.0 - 0.05_f32.powi(2) / 500.0).sqrt());
        let expected = 0.8 * factor / (4.0 * std::f32::consts::PI) * mu * 100_000.0 * solid_angle;
        let p = sample(&device, &queue, &mut tracer, &f, 32)[0];
        assert!(
            (p[0] / expected - 1.0).abs() < 0.02,
            "slab scattering: {p:?} vs {expected}"
        );
        // Block the source from the ring without blocking the camera ray.
        f.bodies
            .push(body([5.0, 0.0, (star_z - 5.0) * 0.5], 0.5, [0.0; 3], false));
        let dark = sample(&device, &queue, &mut tracer, &f, 8)[0];
        assert!(
            dark[0] < p[0] * 0.001,
            "ring must receive physical shadows: {dark:?}"
        );
        f.bodies.pop();
    }
}

fn read_exposure(device: &wgpu::Device, queue: &wgpu::Queue, tracer: &PathTracer) -> f32 {
    let size = std::mem::size_of::<Exposure>() as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("exposure-readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&tracer.exposure, 0, &buffer, 0, size);
    queue.submit(Some(encoder.finish()));
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let view = buffer.slice(..).get_mapped_range().unwrap();
    let value = f32::from_le_bytes([view[0], view[1], view[2], view[3]]);
    drop(view);
    buffer.unmap();
    value
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_auto_exposure_meters_saturn() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    state.aim_at(8);
    let positions = state.scene.positions(state.sim_time);
    let vf = state
        .observer
        .frame(&state.scene, state.sim_time, &positions);
    let angular_radius = state.scene.bodies[8].radius / (positions[8] - vf.position).length();
    state.point_at_fov(8, 5.0 * angular_radius);
    let (width, height) = (160, 90);
    let options = Options {
        samples_per_frame: 32,
        max_bounces: 1,
        auto_exposure: true,
        auto_key: 0.25,
        ..Options::default()
    };
    let key = options.auto_key;
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (width, height),
        &[],
        options,
    );
    let frame = state.build_frame(width, height);
    for _ in 0..96 {
        let mut encoder = device.create_command_encoder(&Default::default());
        tracer.encode(&queue, &mut encoder, &frame);
        queue.submit(Some(encoder.finish()));
    }
    let exposure = read_exposure(&device, &queue, &tracer);
    let pixels = read_pixels(&device, &queue, &tracer);
    // Reproduce the shader's centre-weighted percentile on the CPU.
    let mut luminances: Vec<f32> = Vec::new();
    for y in height / 4..height - height / 4 {
        for x in width / 4..width - width / 4 {
            let p = pixels[(y * width + x) as usize];
            luminances.push(0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]);
        }
    }
    luminances.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let measured = luminances[(luminances.len() as f32 * 0.9) as usize];
    let expected = key / measured;
    assert!(
        (exposure / expected - 1.0).abs() < 0.2,
        "auto exposure {exposure} vs analytic {expected} (measured {measured})"
    );
    // The observed planet must be neither black nor blown out.
    let tonemap =
        |x: f32| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    let center = pixels[((height / 2) * width + width / 2) as usize];
    let mapped = tonemap(center[0] * exposure);
    assert!(
        mapped > 0.1 && mapped < 0.98,
        "Saturn maps to {mapped} at auto exposure {exposure}"
    );
    eprintln!("auto exposure {exposure:.3} (analytic {expected:.3}, measured {measured:.6})");
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_saturn_scene() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    state.aim_at(8);
    let positions = state.scene.positions(state.sim_time);
    let vf = state
        .observer
        .frame(&state.scene, state.sim_time, &positions);
    let angular_radius = state.scene.bodies[8].radius / (positions[8] - vf.position).length();
    state.point_at_fov(8, 5.0 * angular_radius);
    eprintln!(
        "Saturn: day {}, fov {} deg",
        state.sim_time,
        state.observer.fov_y.to_degrees()
    );
    let (width, height) = (640, 360);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (width, height),
        &[],
        Options {
            samples_per_frame: 8,
            max_bounces: 6,
            auto_exposure: true,
            ..Options::default()
        },
    );
    let f = state.build_frame(width, height);
    let pixels = sample(&device, &queue, &mut tracer, &f, 16);
    assert!(pixels.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    // Save the image the way the automatic meter would present it.
    let exposure = read_exposure(&device, &queue, &tracer);
    eprintln!("Saturn auto exposure: {exposure:.2}");
    save_image("saturn-rings", width, height, &pixels, exposure);
    state.scene.bodies[8].rings = None;
    let bare = sample(
        &device,
        &queue,
        &mut tracer,
        &state.build_frame(width, height),
        16,
    );
    let changed = pixels
        .iter()
        .zip(&bare)
        .filter(|(a, b)| a[0] > b[0] + 0.0001)
        .count();
    assert!(
        changed > 1000,
        "rings should add a substantial visible silhouette: {changed}"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_rings_conserve_energy() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options::default(),
    );
    let source = format!(
        "{TRACE_SHADER}\n{}",
        r#"
@compute @workgroup_size(8, 8)
fn probe(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x != 0u || id.y != 0u) { return; }
    let optics = bodies[0].ring_color;
    let mo = settings.g.cam_forward.w;
    var energy = vec3<f32>(exp(-optics.w / mo));
    for (var i = 0u; i < 4096u; i += 1u) {
        let mi = (f32(i) + 0.5) / 4096.0;
        energy += (ring_bsdf(optics, mo, mi) + ring_bsdf(optics, mo, -mi))
            * (TAU * mi / 4096.0);
    }
    accumulation[0] = vec4<f32>(energy, 1.0);
}
"#
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ring-energy-probe"),
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
        entry_point: Some("probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut f = frame();
    f.bodies = vec![ringed([0., 0., -5.], 1., [0., 0., 1.], 0.1)];
    for albedo in [0.8, 0.88, 1.0] {
        for tau in [0.001, 0.1, 0.7, 2.0] {
            for mu in [0.05, 0.5, 1.0] {
                f.bodies[0].ring_color = [albedo, albedo, albedo, tau];
                f.globals.cam_forward[3] = mu;
                let energy = sample(&device, &queue, &mut tracer, &f, 1)[0][0];
                assert!(
                    (0.0..=1.0001).contains(&energy),
                    "ring creates energy: albedo={albedo}, tau={tau}, mu={mu}, energy={energy}"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_rings_null_crossings_preserve_mis() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (64, 64),
        &[],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            ..Options::default()
        },
    );
    let mut f = frame();
    f.globals.viewport[..2].copy_from_slice(&[64., 64.]);
    // The absorbing sheet lies between the diffuse surface and a large
    // emitter. There is no indirect source: MIS and direct-only estimates
    // must agree even when continuation rays cross the sheet.
    let mut ring = ringed([0., 4., -2.], 0.1, [0., 0., 1.], 0.4);
    ring.ring_params[0] = 100.;
    ring.ring_color = [0.; 4];
    f.bodies = vec![
        body([0., 0., -5.], 1., [0.8; 3], false),
        body([0., 4., 2.], 3.8, [1.; 3], true),
        ring,
    ];
    let mean = |pixels: Vec<[f32; 4]>| {
        pixels.iter().map(|p| p[0] as f64).sum::<f64>() / pixels.len() as f64
    };
    let direct = mean(sample(&device, &queue, &mut tracer, &f, 4));
    tracer.options.max_bounces = 2;
    let mis = mean(sample(&device, &queue, &mut tracer, &f, 4));
    assert!(
        (mis / direct - 1.).abs() < 0.01,
        "ring null MIS: {mis} vs direct-only {direct}"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_auto_exposure_small_viewport() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            auto_exposure: true,
            ..Options::default()
        },
    );
    let mut f = frame();
    f.bodies = vec![body([0., 0., -3.], 1., [10.; 3], true)];
    sample(&device, &queue, &mut tracer, &f, 64);
    let exposure = read_exposure(&device, &queue, &tracer);
    assert!(
        (exposure / 0.025 - 1.).abs() < 0.1,
        "single-pixel exposure: {exposure}"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_auto_exposure_after_convergence() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (8, 8),
        &[],
        Options::default(),
    );
    let mut f = frame();
    f.globals.viewport[..2].copy_from_slice(&[8., 8.]);
    f.bodies = vec![body([0., 0., -3.], 1., [10.; 3], true)];
    let before = sample(&device, &queue, &mut tracer, &f, 1);
    tracer.history.samples = 16_777_216;
    tracer.set_exposure_controls(true, 0.);
    let after = sample(&device, &queue, &mut tracer, &f, 64);
    let exposure = read_exposure(&device, &queue, &tracer);
    assert!(
        (exposure / 0.025 - 1.).abs() < 0.1,
        "converged exposure: {exposure}"
    );
    assert_eq!(before, after, "metering must preserve the converged image");
    assert_eq!(tracer.samples(), 16_777_216);
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_halo_observatory_shows_calyx_rings() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::halo_scene());
    let (width, height) = (480, 270);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (width, height),
        &[],
        Options {
            samples_per_frame: 8,
            max_bounces: 6,
            auto_exposure: true,
            ..Options::default()
        },
    );
    let ringed = sample(
        &device,
        &queue,
        &mut tracer,
        &state.build_frame(width, height),
        16,
    );
    assert!(ringed.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    let exposure = read_exposure(&device, &queue, &tracer);
    save_image("halo-calyx-rings", width, height, &ringed, exposure);
    state.scene.bodies[state.scene.default_target].rings = None;
    let bare = sample(
        &device,
        &queue,
        &mut tracer,
        &state.build_frame(width, height),
        16,
    );
    let visible_ring_pixels = ringed
        .iter()
        .zip(&bare)
        .filter(|(a, b)| a[0] > b[0] + 0.0001)
        .count();
    assert!(
        visible_ring_pixels > 500,
        "rings should be visible from Halo: {visible_ring_pixels}"
    );
}
