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
    history.update(&scene, Options::default());
    history.samples = 100;
    scene.ground[0].center[0] += 0.1;
    history.update(&scene, Options::default());
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
            ..Options::default()
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
            ..Options::default()
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
            ..Options::default()
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
