use super::*;

#[test]
#[ignore = "requires a GPU; ground noise must stay continuous across flat terrain"]
fn gpu_ground_noise_is_continuous_at_integer_planes() {
    let (device, queue) = gpu();
    let mut state = crate::State::with_scene(crate::sim::halo_scene());
    state.set_ground(crate::ground::Theme::Desert);
    state.sim_time = -0.08;
    state.observer.lat = 8_f64.to_radians();
    state.observer.lon = 140.7_f64.to_radians();
    state.observer.az = 0.0;
    state.observer.alt = -70_f64.to_radians();
    state.observer.fov_y = 70_f64.to_radians();
    let size = (320, 240);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size,
        &[],
        Options {
            samples_per_frame: 4,
            sample_limit: 4,
            ..test_options()
        },
    );
    let hook = "sum += trace(Ray(vec3<f32>(0.0), direction, MISS), &rng, &backdrop);";
    assert_eq!(TRACE_SHADER.matches(hook).count(), 1);
    let source = TRACE_SHADER.replace(
        hook,
        r#"
        let ray = Ray(vec3<f32>(0.0), direction, MISS);
        let hit = closest_hit(ray);
        let point = landscape_point(ray, hit);
        let noise = vnoise(point * 0.007 + vec3<f32>(19.0, 2.0, 4.0));
        sum += vec4<f32>(vec3<f32>(noise), 0.0);
    "#,
    );
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
    let frame = state.build_frame(size.0, size.1);
    let pixels = sample(&device, &queue, &mut tracer, &frame, 1);
    assert!(
        pixels
            .iter()
            .all(|p| p[0].is_finite() && (0.0..=1.0).contains(&p[0]))
    );
    let lo = pixels.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let hi = pixels
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    // Metre-scale views of this low-frequency field cannot contain large jumps.
    assert!(
        (0.0..0.02).contains(&(hi - lo)),
        "discontinuous noise: {lo}..{hi}"
    );
}
