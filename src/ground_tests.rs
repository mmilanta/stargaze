use super::*;

fn local_frame(radius_m: f32) -> Frame {
    let mut scene = frame();
    let metre = 1.0 / 149_597_870_700.0_f32;
    let up = glam::Vec3::new(0.0, 0.8_f32.sqrt(), 0.2_f32.sqrt());
    let north = glam::Vec3::new(0.0, -up.z, up.y);
    scene.globals.ground_east = [1.0, 0.0, 0.0, 2.0];
    scene.globals.ground_up = [up.x, up.y, up.z, radius_m];
    scene.globals.ground_north = [north.x, north.y, north.z, 1.0];
    let center = -up * ((radius_m + 2.0) * metre);
    let sun = (up * 8.0 - north * 4.0) * metre;
    scene.bodies = vec![
        body(center.into(), radius_m * metre, [0.3; 3], false),
        body(sun.into(), 0.5 * metre, [100.0; 3], true),
    ];
    scene
}

#[test]
fn local_geometry_changes_reset_accumulation() {
    let mut scene = local_frame(6_371_000.0);
    scene.ground = crate::ground::landscape(6_371_000.0);
    scene.globals.ground_counts[0] = scene.ground.len() as u32;
    assert!(scene.ground.len() <= crate::ground::CAPACITY);
    assert!(
        scene
            .ground
            .iter()
            .all(|p| p.extent[..3].iter().all(|v| *v > 0.0))
    );
    let mut history = History::default();
    history.update(&scene, test_options());
    history.samples = 100;
    scene.ground[0].center[0] += 0.1;
    history.update(&scene, test_options());
    assert_eq!(history.samples, 0);
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_local_ground_shadows() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            ..test_options()
        },
    );
    for radius in [6_371_000.0, 1_500_000.0] {
        let mut scene = local_frame(radius);
        let full = sample(&device, &queue, &mut tracer, &scene, 8)[0][0];
        assert!(full > 0.01 && full.is_finite(), "ground unlit: {full}");
        // A nearby box blocks the star without blocking the camera's ground ray.
        scene.ground.push(crate::ground::Primitive {
            center: [0.0, 4.0, -4.0, 0.0],
            extent: [0.8, 0.8, 0.8, 0.0],
            albedo: [0.0; 4],
        });
        scene.globals.ground_counts[0] = 1;
        let shadow = sample(&device, &queue, &mut tracer, &scene, 8)[0][0];
        assert!(shadow < full * 0.001, "box shadow={shadow}, lit={full}");
        scene.ground[0].center[3] = 1.0;
        let shadow = sample(&device, &queue, &mut tracer, &scene, 8)[0][0];
        assert!(
            shadow < full * 0.001,
            "ellipsoid shadow={shadow}, lit={full}"
        );
        // A blocker only two centimetres above the surface must also shadow it.
        scene.ground[0].center = [0.0, 0.02, -4.0, 0.0];
        scene.ground[0].extent = [0.2, 0.004, 0.005, 0.0];
        let small = sample(&device, &queue, &mut tracer, &scene, 8)[0][0];
        assert!(small < full * 0.01, "centimetre shadow={small}, lit={full}");
        // Rotated boxes take part in visibility too (used for the pitched roof).
        scene.ground[0].center = [0.0, 4.0, -4.0, 0.0];
        scene.ground[0].extent = [0.8, 0.8, 0.8, 0.4];
        assert!(sample(&device, &queue, &mut tracer, &scene, 8)[0][0] < full * 0.001);
    }
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_ground_preview() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    for step in 0..1000 {
        state.sim_time = step as f64 / 1000.0;
        let positions = state.scene.positions(state.sim_time);
        let vf = state
            .observer
            .frame(&state.scene, state.sim_time, &positions);
        if vf.sun_altitude > 0.4 {
            break;
        }
    }
    state.ground_view();
    eprintln!("ground preview: t = {} days", state.sim_time);
    let mut scene = state.build_frame(640, 360);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (640, 360),
        &[],
        Options {
            samples_per_frame: 8,
            max_bounces: 4,
            ..test_options()
        },
    );
    let pixels = sample(&device, &queue, &mut tracer, &scene, 8);
    assert!(pixels.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    let lit_ground = pixels[640 * 210..]
        .iter()
        .filter(|p| p[0] + p[1] + p[2] > 0.01)
        .count();
    assert!(
        lit_ground > 20_000,
        "ground unexpectedly dark: {lit_ground}"
    );
    if let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let mut ppm = b"P6\n640 360\n255\n".to_vec();
        for pixel in &pixels {
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
        std::fs::write(std::path::Path::new(&dir).join("ground.ppm"), ppm).unwrap();
    }
    // Turn the same observation into night; props and grass cannot emit light.
    scene
        .bodies
        .iter_mut()
        .filter(|b| b.emissive > 0.5)
        .for_each(|b| b.color = [0.0; 3]);
    let dark = sample(&device, &queue, &mut tracer, &scene, 1);
    assert!(dark.iter().all(|p| p[..3].iter().all(|v| *v == 0.0)));
    // Resize exercises the landscape bind group recreation.
    tracer.resize(&device, (1, 1));
    scene = local_frame(6_371_000.0);
    assert!(sample(&device, &queue, &mut tracer, &scene, 1)[0][0] > 0.01);
}

#[test]
fn ground_view_and_geometry_follow_the_surface_frame() {
    let mut state = crate::State::with_scene(crate::sim::halo_scene());
    state.view_lock = Some(crate::ViewLock::Body(state.scene.default_target));
    state.toggle_star_orientation();
    let time = state.sim_time;
    state.ground_view();
    assert_eq!(state.view_lock, None);
    assert!(state.observer.star_orientation.is_none());
    assert_eq!(state.sim_time, time);
    assert_eq!(state.observer.az, 0.0);
    let first = state.build_frame(640, 360);
    // Ground host indexes account for invisible orbit anchors, without
    // depending on the host atmosphere being enabled.
    state.scene.atmosphere = None;
    state.observer.az += 0.7;
    state.observer.alt += 0.2;
    let turned = state.build_frame(640, 360);
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&first.ground),
        bytemuck::cast_slice::<_, u8>(&turned.ground)
    );
    let host = turned.globals.ground_north[3] as usize - 1;
    assert_eq!(
        turned.bodies[host].radius,
        state.scene.body(state.scene.host).radius as f32
    );
    for scene in [first, turned] {
        let vf = state
            .observer
            .frame(&state.scene, time, &state.scene.positions(time));
        // Recover the same body-fixed axes from either telescope orientation.
        let g = scene.globals;
        let east = glam::Vec3::from_array(g.ground_east[..3].try_into().unwrap());
        let up = glam::Vec3::from_array(g.ground_up[..3].try_into().unwrap());
        let north = glam::Vec3::from_array(g.ground_north[..3].try_into().unwrap());
        assert!((east.length() - 1.0).abs() < 1e-6);
        assert!((up.length() - 1.0).abs() < 1e-6);
        assert!((north.length() - 1.0).abs() < 1e-6);
        assert!(east.dot(up).abs() < 1e-6 && north.dot(up).abs() < 1e-6);
        let right = glam::Vec3::from_array(g.cam_right[..3].try_into().unwrap());
        let camera_up = glam::Vec3::from_array(g.cam_up[..3].try_into().unwrap());
        let forward = glam::Vec3::from_array(g.cam_forward[..3].try_into().unwrap());
        let world = |axis: glam::Vec3| right * axis.x + camera_up * axis.y - forward * axis.z;
        assert!((world(east) - vf.east.as_vec3()).length() < 1e-6);
        assert!((world(up) - vf.zenith.as_vec3()).length() < 1e-6);
        assert!((world(north) - vf.north.as_vec3()).length() < 1e-6);
    }
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_ground_sky_visibility() {
    let (device, queue) = gpu();
    let star = CatalogueStar {
        dir: [0.0, 0.0, -1.0],
        color: [1.0; 3],
        bright: 1.62,
        angular_radius: 0.01,
    };
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[star],
        Options {
            samples_per_frame: 64,
            max_bounces: 1,
            ..test_options()
        },
    );
    let mut scene = local_frame(6_371_000.0);
    // Look above the tree line. Missing all props must remain a true miss,
    // even though scene distances are in AU and local distances are in metres.
    scene.globals.ground_up[2] *= -1.0;
    scene.globals.ground_north[1] *= -1.0;
    scene.ground = crate::ground::landscape(6_371_000.0);
    scene.globals.ground_counts[0] = scene.ground.len() as u32;
    let sky = sample(&device, &queue, &mut tracer, &scene, 1)[0][0];
    assert!(sky > 0.1, "landscape must not hide background sky: {sky}");
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_ground_refines_again_after_scene_and_quality_changes() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 8,
            max_bounces: 1,
            sample_limit: 16,
            ..test_options()
        },
    );
    let mut scene = local_frame(6_371_000.0);
    let lit = sample(&device, &queue, &mut tracer, &scene, 2)[0][0];
    assert!(lit > 0.01);
    assert_eq!(tracer.samples(), 16);
    assert!(!tracer.needs_redraw(), "converged ground should rest");

    // A landscape change must discard the sleeping view's old lighting.
    scene.ground.push(crate::ground::Primitive {
        center: [0.0, 4.0, -4.0, 0.0],
        extent: [0.8, 0.8, 0.8, 0.0],
        albedo: [0.0; 4],
    });
    scene.globals.ground_counts[0] = 1;
    assert!(sample(&device, &queue, &mut tracer, &scene, 1)[0][0] < lit * 0.001);
    assert_eq!(tracer.samples(), 8);
    assert!(tracer.needs_redraw());
    sample(&device, &queue, &mut tracer, &scene, 1);
    assert!(!tracer.needs_redraw());

    // Main's light-detail setting and reduced render size must preserve
    // ground bindings and start a fresh, capped accumulation.
    tracer.set_quality(8, 2, 24);
    assert_eq!(tracer.samples(), 0);
    assert!(tracer.needs_redraw());
    scene.ground.clear();
    scene.globals.ground_counts[0] = 0;
    tracer.resize(&device, (2, 1));
    scene.globals.viewport[..2].copy_from_slice(&[4.0, 1.0]);
    let pixels = sample(&device, &queue, &mut tracer, &scene, 3);
    assert_eq!(pixels.len(), 2);
    assert!(pixels.iter().all(|p| p[0].is_finite() && p[0] > 0.01));
    assert_eq!(tracer.samples(), 24);
    assert!(!tracer.needs_redraw());
}

#[test]
#[ignore = "requires a GPU; landscape preview and timing"]
fn gpu_rocky_lookout_preview() {
    let (device, queue) = gpu();
    let width = std::env::var("STARGAZE_TERRAIN_WIDTH")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(384)
        .clamp(64, 1920);
    let height = width * 9 / 16;
    let samples = std::env::var("STARGAZE_TERRAIN_SAMPLES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, 64);
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    state.sim_time = std::env::var("STARGAZE_TERRAIN_TIME")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(0.599);
    if std::env::var_os("STARGAZE_TERRAIN_LOW_SUN").is_some() {
        for step in 0..1000 {
            state.sim_time = step as f64 / 1000.0;
            let positions = state.scene.positions(state.sim_time);
            let view = state
                .observer
                .frame(&state.scene, state.sim_time, &positions);
            if (0.20..0.23).contains(&view.sun_altitude) {
                break;
            }
        }
    }
    eprintln!("terrain time: {} days", state.sim_time);
    state.ground_view();
    state.observer.alt = 3_f64.to_radians();
    state.observer.fov_y = 55_f64.to_radians();
    let scene = state.build_frame(width, height);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (width, height),
        &[],
        Options {
            samples_per_frame: 1,
            max_bounces: 2,
            ..test_options()
        },
    );
    // Warm the shader before timing. Complete each batch so this fixture does
    // not recreate the old large-submission watchdog problem on a hardware GPU.
    sample(&device, &queue, &mut tracer, &scene, 1);
    tracer.history = History::default();
    let start = std::time::Instant::now();
    let mut pixels = Vec::new();
    for _ in 0..samples {
        pixels = sample(&device, &queue, &mut tracer, &scene, 1);
    }
    eprintln!(
        "terrain preview: {} primitives, {}x{}, {} samples, {:.1} ms/sample",
        scene.ground.len(),
        width,
        height,
        samples,
        start.elapsed().as_secs_f64() * 1000.0 / samples as f64
    );
    assert!(pixels.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    assert!(pixels.iter().filter(|p| p[0] + p[1] + p[2] > 0.01).count() > pixels.len() / 3);
    if let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let file = std::fs::File::create(std::path::Path::new(&dir).join("lookout.png")).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|p| {
                p[..3].iter().map(|x| {
                    let x = *x;
                    let mapped =
                        ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
                    let s = if mapped <= 0.0031308 {
                        12.92 * mapped
                    } else {
                        1.055 * mapped.powf(1.0 / 2.4) - 0.055
                    };
                    (s * 255.0).round() as u8
                })
            })
            .collect();
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgb)
            .unwrap();
    }
}

#[test]
#[ignore = "requires a GPU; compares acceleration against all visible geometry"]
fn gpu_terrain_bounds_preserve_images_and_night_stays_dark() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    state.sim_time = 0.599;
    state.ground_view();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (48, 27),
        &[],
        Options {
            samples_per_frame: 4,
            max_bounces: 2,
            ..test_options()
        },
    );
    for azimuth in [0.0, 1.7, 4.0] {
        state.observer.az = azimuth;
        let mut scene = state.build_frame(48, 27);
        let bounded = sample(&device, &queue, &mut tracer, &scene, 1);
        scene.ground.retain(|p| p.center[3] >= 0.0);
        scene.globals.ground_counts[0] = scene.ground.len() as u32;
        let flat = sample(&device, &queue, &mut tracer, &scene, 1);
        for (a, b) in bounded.iter().zip(&flat) {
            for c in 0..3 {
                assert!(
                    (a[c] - b[c]).abs() < 1e-5,
                    "bound changed visibility: {a:?} vs {b:?}"
                );
            }
        }
        scene
            .bodies
            .iter_mut()
            .filter(|b| b.emissive > 0.5)
            .for_each(|b| b.color = [0.0; 3]);
        let dark = sample(&device, &queue, &mut tracer, &scene, 1);
        assert!(
            dark.iter()
                .all(|p| p[..3].iter().all(|v| v.is_finite() && v.abs() < 1e-12))
        );
    }
}

#[test]
#[ignore = "requires a GPU; distant terrain and convex rock intersections"]
fn gpu_distant_hills_and_rocks_occlude_the_sky() {
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 1,
            max_bounces: 1,
            ..test_options()
        },
    );
    let metre = 1.0 / 149_597_870_700.0_f32;
    let altitude = 3_f32.to_radians();
    let up = glam::Vec3::new(0.0, altitude.cos(), -altitude.sin());
    let north = glam::Vec3::new(0.0, -altitude.sin(), -altitude.cos());
    for radius in [1_500_000.0, 6_371_000.0] {
        let mut scene = frame();
        scene.globals.ground_east = [1.0, 0.0, 0.0, 2.0];
        scene.globals.ground_up = [up.x, up.y, up.z, radius];
        scene.globals.ground_north = [north.x, north.y, north.z, 1.0];
        scene.bodies = vec![
            body(
                (-up * (radius + 2.0) * metre).into(),
                radius * metre,
                [0.0; 3],
                false,
            ),
            body([0.0, 0.0, -1.0], 0.1, [1.0; 3], true),
        ];
        assert!(sample(&device, &queue, &mut tracer, &scene, 1)[0][0] > 0.99);
        for kind in [1.0, 2.0, 3.0, 5.0] {
            scene.ground = vec![crate::ground::Primitive {
                center: [0.0, 25.0, 450.0, kind],
                extent: [35.0, 30.0, 60.0, 0.2],
                albedo: [0.0; 4],
            }];
            scene.globals.ground_counts[0] = 1;
            assert!(
                sample(&device, &queue, &mut tracer, &scene, 1)[0][0] < 1e-6,
                "distant geometry must block the sky beyond the former 190 m bounds"
            );
            scene.ground[0].center[0] = 100.0;
            assert!(sample(&device, &queue, &mut tracer, &scene, 1)[0][0] > 0.99);
        }
    }
}

#[test]
#[ignore = "requires a GPU; exact heightfield traversal versus brute-force triangles"]
fn gpu_heightfield_matches_exhaustive_triangles() {
    use glam::DVec3;
    let (device, queue) = gpu();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terrain traversal oracle"),
        source: wgpu::ShaderSource::Wgsl(
            format!(
                "{TRACE_SHADER}\n{}",
                r#"
            @group(0) @binding(8) var<storage, read_write> probes: array<vec4<f32>>;
            @compute @workgroup_size(1)
            fn terrain_probe(@builtin(global_invocation_id) id: vec3<u32>) {
                let i = id.x * 3u;
                probes[i + 2u] = terrain_hit(probes[i].xyz, probes[i + 1u].xyz, 1.0e29);
            }
        "#
            )
            .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("terrain oracle"),
        layout: None,
        module: &shader,
        entry_point: Some("terrain_probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let terrain = crate::terrain::cached();
    let mut landscape = vec![0.0_f32; crate::ground::CAPACITY * 12 + crate::ground::SKY_TEXELS * 4];
    landscape.extend_from_slice(&terrain.data);
    let ground = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&landscape),
        usage: wgpu::BufferUsages::STORAGE,
    });
    // Independent f64 exhaustive intersection: no quadtree, bounds, derivative
    // interpolation or shader traversal logic is used for the reference.
    let triangle = |o: DVec3, d: DVec3, a: DVec3, b: DVec3, c: DVec3| {
        let ab = b - a;
        let ac = c - a;
        let p = d.cross(ac);
        let determinant = ab.dot(p);
        if determinant.abs() < 1e-15 {
            return f64::INFINITY;
        }
        let relative = o - a;
        let u = relative.dot(p) / determinant;
        let q = relative.cross(ab);
        let v = d.dot(q) / determinant;
        let t = ac.dot(q) / determinant;
        if u < -1e-9 || v < -1e-9 || u + v > 1.0 + 1e-9 || t <= 0.001 {
            f64::INFINITY
        } else {
            t
        }
    };
    for radius in [6_371_000.0_f32, 5_000.0] {
        let scale = crate::terrain::scale(radius as f64);
        let mut rays = Vec::new();
        for (o, d) in [
            ([0.0, 300.0, 0.0], [0.0, -1.0, 0.0]),
            ([0.0, 2.0, 0.0], [0.0, 0.03, 1.0]),
            ([0.0, 2.0, 0.0], [1.0, 0.08, 0.25]),
            ([0.0, 2.0, 0.0], [-1.0, 0.1, -0.4]),
            ([0.0, 2.0, 0.0], [0.0, 1.0, 0.0]),
            ([1_000.0, 2_000.0, -3_000.0], [0.0, -1.0, 0.0]),
            ([-4_000.0, 1_500.0, 2_000.0], [1.0, -0.2, 0.1]),
            ([16_000.0, 100.0, 0.0], [0.0, -1.0, 0.0]),
            ([16_100.0, 100.0, 0.0], [0.0, -1.0, 0.0]),
            ([0.0, 0.1, 0.0], [0.5, -0.01, -1.0]),
        ] {
            let o = glam::Vec3::from_array(o) * scale;
            let d = glam::Vec3::from_array(d).normalize();
            rays.extend_from_slice(&[o.extend(0.0).to_array(), d.extend(0.0).to_array(), [0.0; 4]]);
        }
        let mut settings = Settings::zeroed();
        settings.globals.ground_up[3] = radius;
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&settings),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let probes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&rays),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: probes.size(),
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: ground.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: probes.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((rays.len() / 3) as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&probes, 0, &readback, 0, probes.size());
        queue.submit(Some(encoder.finish()));
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let mapping = readback.slice(..).get_mapped_range().unwrap();
        let results: &[[f32; 4]] = bytemuck::cast_slice(&mapping);
        let width = crate::terrain::WIDTH;
        let vertices: Vec<DVec3> = (0..width * width)
            .map(|i| {
                let x = (crate::terrain::coordinate(i % width) * scale) as f64;
                let z = (crate::terrain::coordinate(i / width) * scale) as f64;
                let r = radius as f64;
                let curvature = -(x * x + z * z) / (r + (r * r - x * x - z * z).sqrt());
                DVec3::new(x, terrain.data[i] as f64 * scale as f64 + curvature, z)
            })
            .collect();
        for (i, ray) in rays.as_chunks::<3>().0.iter().enumerate() {
            let o = DVec3::new(ray[0][0] as f64, ray[0][1] as f64, ray[0][2] as f64);
            let d = DVec3::new(ray[1][0] as f64, ray[1][1] as f64, ray[1][2] as f64);
            let mut expected = f64::INFINITY;
            for z in 0..width - 1 {
                for x in 0..width - 1 {
                    let a = vertices[z * width + x];
                    let b = vertices[z * width + x + 1];
                    let c = vertices[(z + 1) * width + x];
                    let e = vertices[(z + 1) * width + x + 1];
                    expected = expected
                        .min(triangle(o, d, a, c, b))
                        .min(triangle(o, d, e, b, c));
                }
            }
            let result = results[i * 3 + 2];
            let actual = result[3] as f64;
            if expected.is_infinite() {
                assert!(
                    actual > 1e29,
                    "radius={radius}, ray={i}: expected miss, {result:?}"
                );
            } else {
                assert!(
                    (actual - expected).abs() < 0.005 + expected * 1e-5,
                    "radius={radius}, ray={i}: GPU {actual}, exhaustive {expected}"
                );
                let n = glam::Vec3::new(result[0], result[1], result[2]);
                assert!(n.is_finite() && (n.length() - 1.0).abs() < 1e-4 && n.y > 0.0);
            }
        }
    }
}

#[test]
#[ignore = "requires a GPU; atmospheric surface lighting, occlusion and cache invalidation"]
fn gpu_forest_skylight_is_occluded_and_has_no_unlit_glow() {
    fn read(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &wgpu::Buffer,
        offset: u64,
        size: u64,
    ) -> Vec<[f32; 4]> {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(source, offset, &buffer, 0, size);
        queue.submit(Some(encoder.finish()));
        let (send, receive) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let mapping = buffer.slice(..).get_mapped_range().unwrap();
        bytemuck::cast_slice(&mapping).to_vec()
    }
    let (device, queue) = gpu();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 1,
            max_bounces: 1,
            ..test_options()
        },
    );
    let mut state = crate::State::with_scene(crate::sim::default_scene());
    state.sim_time = 0.599;
    state.ground_view();
    let day = state.build_frame(1, 1);
    sample(&device, &queue, &mut tracer, &day, 1);
    let offset = (crate::ground::CAPACITY * 48) as u64;
    let size = (crate::ground::SKY_TEXELS * 16) as u64;
    let cache = read(&device, &queue, &tracer.ground, offset, size);
    assert!(cache.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    assert!(cache.iter().map(|p| p[0] + p[1] + p[2]).sum::<f32>() > 0.01);
    // A continued accumulation must not change its cached lighting.
    sample(&device, &queue, &mut tracer, &day, 1);
    assert_eq!(cache, read(&device, &queue, &tracer.ground, offset, size));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None, source: wgpu::ShaderSource::Wgsl(format!("{TRACE_SHADER}\n{}",r#"
            @group(0) @binding(8) var<storage, read_write> sky_probe_results: array<vec4<f32>>;
            @compute @workgroup_size(64)
            fn sky_probe(@builtin(global_invocation_id) id: vec3<u32>) {
                var rng = hash(id.x+817u);
                let normal = settings.g.ground_up.xyz;
                sky_probe_results[id.x] = vec4<f32>(surface_sky(Ray(vec3<f32>(0.0),normal,MISS),normal,&rng),0.0);
            }
        "#).into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("sky_probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let results = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64 * 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: tracer.settings.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: tracer.bodies.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: tracer.ground.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: results.as_entire_binding(),
            },
        ],
    });
    let probe = || {
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        queue.submit(Some(encoder.finish()));
        read(&device, &queue, &results, 0, 64 * 16)
            .iter()
            .map(|p| p[0] + p[1] + p[2])
            .sum::<f32>()
    };
    let open = probe();
    assert!(open > 0.001);
    // Enclose the observer under an opaque canopy while retaining the same
    // bright sky cache. Visibility, rather than an ambient gain, must darken it.
    let canopy = crate::ground::Primitive {
        center: [0.0, 5.0, 0.0, 0.0],
        extent: [10_000.0, 0.5, 10_000.0, 0.0],
        albedo: [0.0; 4],
    };
    queue.write_buffer(&tracer.ground, 0, bytemuck::bytes_of(&canopy));
    assert!(probe() < open * 0.001);
    for airless in [false, true] {
        let mut scene = state.build_frame(1, 1);
        if airless {
            scene.globals.atmo_params[3] = 0.0;
        } else {
            for b in &mut scene.bodies {
                if b.emissive > 0.5 {
                    b.color = [0.0; 3];
                }
            }
        }
        sample(&device, &queue, &mut tracer, &scene, 1);
        let dark = read(&device, &queue, &tracer.ground, offset, size);
        assert!(
            dark.iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() < 1e-12)
        );
    }
}
