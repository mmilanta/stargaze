use super::*;

fn galactic_frame() -> Frame {
    let mut scene = frame();
    let pole = glam::Vec3::new(0.35, 0.82, 0.45).normalize();
    let direction = pole.any_orthonormal_vector();
    scene.globals.cam_forward[..3].copy_from_slice(&direction.to_array());
    scene.globals.cam_right[..3].copy_from_slice(&direction.cross(pole).to_array());
    scene.globals.cam_up[..3].copy_from_slice(&pole.to_array());
    scene
}

#[test]
fn backdrop_changes_invalidate_history() {
    let mut history = History::default();
    let scene = frame();
    history.update(&scene, test_options());
    history.samples = 100;
    history.update(&scene, Options::default());
    assert_eq!(history.samples, 0);
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_milky_way_is_camera_only_and_does_not_drive_exposure() {
    let (device, queue) = gpu();
    let mut scene = galactic_frame();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 64,
            auto_exposure: true,
            ..Options::default()
        },
    );
    let night = sample(&device, &queue, &mut tracer, &scene, 64)[0];
    eprintln!("Milky Way night: {night:?}");
    let milky_way = tracer.options.milky_way;
    assert!(night[0] > 0.01 * milky_way && night[0] <= milky_way);
    let luminance = night[0] * 0.2126 + night[1] * 0.7152 + night[2] * 0.0722;
    assert!((luminance - night[3]).abs() < 1e-7);
    assert!(
        night[0] > night[1] * 1.2,
        "the galactic core should retain its color: {night:?}"
    );
    // An empty night sky settles at the night exposure; the Milky Way must
    // not move it (metering subtracts the backdrop).
    let with_backdrop = rings::read_exposure(&device, &queue, &tracer);
    let mut plain = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            samples_per_frame: 64,
            auto_exposure: true,
            milky_way: 0.0,
            ..Options::default()
        },
    );
    sample(&device, &queue, &mut plain, &scene, 64);
    let without = rings::read_exposure(&device, &queue, &plain);
    assert!(
        (with_backdrop - without).abs() <= 1e-4 * without,
        "decorative sky must not drive automatic exposure: {with_backdrop} vs {without}"
    );

    // Many diffuse escape rays see the entire backdrop, but receive no energy.
    scene.bodies = vec![body([0., 0., -3.], 1., [0.9; 3], false)];
    assert_eq!(sample(&device, &queue, &mut tracer, &scene, 16)[0], [0.; 4]);

    // A body in front occludes the panorama, even at the maximum telescope gain.
    scene.bodies[0].emissive = 1.0;
    scene.bodies[0].color = [1e-5; 3];
    let dim_body = sample(&device, &queue, &mut tracer, &scene, 64)[0];
    assert!(dim_body[..3].iter().all(|v| (*v - 1e-5).abs() < 1e-10));
    assert_eq!(dim_body[3], 0.0);
    // Night exposure is capped (METER_FLOOR) so noise never looks like stars.
    let capped = rings::read_exposure(&device, &queue, &tracer);
    assert!(
        (200.0..=300.0).contains(&capped),
        "night exposure cap: {capped}"
    );

    // Resize must preserve the panorama binding and world orientation.
    tracer.resize(&device, (2, 2));
    scene.bodies.clear();
    scene.globals.viewport[..2].copy_from_slice(&[2., 2.]);
    let resized = sample(&device, &queue, &mut tracer, &scene, 1);
    assert!(
        resized
            .iter()
            .all(|p| (p[0] - night[0]).abs() < night[0] * 0.01),
        "resized {resized:?} differs from {night:?}"
    );
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_bright_air_outshines_stars_and_milky_way() {
    let (device, queue) = gpu();
    let mut scene = galactic_frame();
    let a = crate::sim::Atmosphere::earthlike();
    let radius = (6_371.0 / crate::sim::AU_KM) as f32;
    let center = [0., 0., radius + (0.002 / crate::sim::AU_KM) as f32];
    scene.bodies = vec![body(center, radius, [0.; 3], false)];
    scene.globals.atmo_center = [0., 0., center[2], radius];
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
        1.,
    ];
    let star = CatalogueStar {
        dir: scene.globals.cam_forward[..3].try_into().unwrap(),
        color: [1.; 3],
        angular_radius: 0.01,
        bright: 1.0,
    };
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[star],
        Options {
            samples_per_frame: 64,
            ..Options::default()
        },
    );
    let night = sample(&device, &queue, &mut tracer, &scene, 1)[0];
    let star_radiance = crate::stars::radiance(&star)[0];
    eprintln!("night={night:?}, star radiance {star_radiance}");
    assert!(night[3] > 0.0, "the Milky Way shows in a dark sky");
    assert!(
        night[0] > 0.5 * star_radiance,
        "the star shows in a dark sky"
    );
    scene
        .bodies
        .push(body([1., 0., -1.], 0.005, [100_000.; 3], true));
    let day = sample(&device, &queue, &mut tracer, &scene, 1)[0];
    let luminance = |p: [f32; 4]| p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722;
    let sky = luminance(day) - day[3];
    eprintln!("day={day:?}, scattered sky {sky}");
    // No visibility rule: both keep their brightness (less extinction), and
    // the daylit air out-shines them by orders of magnitude.
    assert!(day[3] > 0.0 && day[3] <= night[3] * 1.0001);
    assert!(
        sky > 10.0 * star_radiance,
        "daylight must out-shine the star"
    );
    assert!(
        sky > 100.0 * night[3],
        "daylight must out-shine the Milky Way"
    );
    scene.bodies[1].color = [0.; 3];
    let dark_again = sample(&device, &queue, &mut tracer, &scene, 1)[0];
    assert!((dark_again[3] - night[3]).abs() < 1e-7);
}

fn presented_pixels(device: &wgpu::Device, queue: &wgpu::Queue, tracer: &PathTracer) -> Vec<u8> {
    let (width, height) = tracer.dimensions;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("milky-way-display-check"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let stride = (width * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("milky-way-display-readback"),
        size: (stride * height) as u64,
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
                bytes_per_row: Some(stride),
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
    let view = readback.slice(..).get_mapped_range().unwrap();
    let pixels = view
        .chunks_exact(stride as usize)
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect();
    drop(view);
    readback.unmap();
    pixels
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_milky_way_exposure_cap_and_preview() {
    let (device, queue) = gpu();
    let mut scene = galactic_frame();
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        (1, 1),
        &[],
        Options {
            exposure: 1e8,
            ..Options::default()
        },
    );
    sample(&device, &queue, &mut tracer, &scene, 1);
    let bright = presented_pixels(&device, &queue, &tracer);
    assert!(
        bright[0] > 0 && bright[0] < 50,
        "haze must stay subdued at telescope gain: {bright:?}"
    );
    assert_eq!(bright[3], 255, "presentation remains opaque");
    tracer.options.exposure = 1.0;
    sample(&device, &queue, &mut tracer, &scene, 1);
    let normal = presented_pixels(&device, &queue, &tracer);
    assert!(normal[0] < bright[0], "lower exposure presents dimmer haze");

    // Longitude wraps across the panorama seam without a sudden discontinuity.
    let pole = glam::Vec3::new(0.35, 0.82, 0.45).normalize();
    let u = pole.any_orthonormal_vector();
    let v = pole.cross(u);
    let mut seam = Vec::new();
    for side in [-1., 1.] {
        scene.globals.cam_forward[..3]
            .copy_from_slice(&(-u + v * side * 1e-6).normalize().to_array());
        seam.push(sample(&device, &queue, &mut tracer, &scene, 1)[0][3]);
    }
    assert!((seam[0] - seam[1]).abs() < 1e-5, "longitude seam: {seam:?}");

    if let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") {
        let stars = stars::generate(3500, 2500, 0x5EED_1234);
        let mut preview = PathTracer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            (640, 360),
            &stars,
            Options {
                exposure: 8.0,
                samples_per_frame: 64,
                ..Options::default()
            },
        );
        let mut scene = galactic_frame();
        scene.globals.cam_forward[3] = (75_f32.to_radians() * 0.5).tan();
        scene.globals.viewport[..2].copy_from_slice(&[640., 360.]);
        sample(&device, &queue, &mut preview, &scene, 4);
        let pixels = presented_pixels(&device, &queue, &preview);
        let mut ppm = b"P6\n640 360\n255\n".to_vec();
        for pixel in pixels.as_chunks::<4>().0 {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("milky-way.ppm"), ppm).unwrap();
    }
}
