//! Opt-in noise diagnosis and a sampling prototype; production is unchanged.
use super::*;

fn replace(source: &mut String, from: &str, to: &str) {
    assert_eq!(
        source.matches(from).count(),
        1,
        "diagnostic hook changed: {from}"
    );
    *source = source.replacen(from, to, 1);
}

fn source(variant: &str) -> String {
    let mut s = TRACE_SHADER.to_string();
    match variant {
        "reference" => {}
        "no-surface-sky" => replace(
            &mut s,
            "radiance += throughput * albedo * surface_sky(outgoing, normal, rng);",
            "let discarded_sky = surface_sky(outgoing, normal, rng);",
        ),
        "no-ground-bounces" => replace(
            &mut s,
            "if (last_vertex) {\n            break;\n        }\n        let u = random(rng);",
            "if (last_vertex || local) {\n            break;\n        }\n        let u = random(rng);",
        ),
        "guided-ground" => {
            // Propose a 50/50 mixture of cosine directions and directions
            // toward reflective bodies. Weight by the FULL mixture density,
            // including overlapping discs; preserve the original path budget.
            s.push_str(
                r#"
fn ground_reflector_pdf(ray: Ray) -> f32 {
    let host = u32(settings.g.atmo_params.w) - 1u;
    var total = 0.0;
    var density = 0.0;
    for (var b = 0u; b < settings.counts.x; b += 1u) {
        let body = bodies[b];
        if (b == host || body.material.w > 0.5) { continue; }
        let width = cone_width(ray,b);
        let weight = width * max(body.material.r,max(body.material.g,body.material.b));
        if (weight <= 0.0) { continue; }
        total += weight;
        if (sphere_hit(ray,b).index != MISS) {
            density += weight / (TAU * width);
        }
    }
    return density / max(total,1.0e-30);
}
"#,
            );
            replace(
                &mut s,
                "let last_vertex = depth + 1u == settings.counts.w;\n        for (var light_slot",
                r#"var ground_reflector = ReflectorSample(MISS, 0.0);
        if (local && depth == 0u && settings.g.atmo_params.w > 0.5) {
            ground_reflector = sample_reflector(camera_origin(outgoing), rng);
        }
        let guided = ground_reflector.index != MISS;
        let last_vertex = depth + 1u == settings.counts.w;
        for (var light_slot"#,
            );
            replace(
                &mut s,
                "var weight = power_weight(sample.pdf, bsdf_pdf);",
                r#"var path_pdf = bsdf_pdf;
                if (guided) {
                    path_pdf = 0.5 * (bsdf_pdf + ground_reflector_pdf(outgoing));
                }
                var weight = power_weight(sample.pdf, path_pdf);"#,
            );
            replace(
                &mut s,
                "previous_pdf = cosine / PI;\n        throughput *= albedo;",
                r#"previous_pdf = cosine / PI;
        var path_weight = 1.0;
        if (guided) {
            if (random(rng) < 0.5) {
                outgoing.direction = sample_light(outgoing, ground_reflector.index, rng).direction;
            }
            let sampled_cosine = max(dot(normal, outgoing.direction), 0.0);
            if (sampled_cosine == 0.0) { break; }
            let brdf_pdf = sampled_cosine / PI;
            previous_pdf = 0.5 * (brdf_pdf + ground_reflector_pdf(outgoing));
            path_weight = brdf_pdf / previous_pdf;
        }
        throughput *= albedo * path_weight;"#,
            );
        }
        _ => panic!("unknown diagnostic variant"),
    }
    s
}

fn install(device: &wgpu::Device, tracer: &mut PathTracer, variant: &str) {
    let source = source(variant);
    let module = naga::front::wgsl::parse_str(&source).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(variant),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("low-light diagnostic"),
        bind_group_layouts: &[Some(&tracer.trace_layout)],
        immediate_size: 0,
    });
    tracer.compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(variant),
        layout: Some(&layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
}

#[test]
#[ignore = "GPU low-light diagnosis; writes target/lowlight-study; no production changes"]
fn gpu_lowlight_study() -> Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN))
        .into_iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::DiscreteGpu)
        .context("low-light study requires the hardware GPU, not software rendering")?;
    let info = adapter.get_info();
    eprintln!("Low-light GPU: {info:?}");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
        ..Default::default()
    }))?;
    let mut cfg = umbra_config("target/lowlight-study".into(), "reference");
    cfg.width = 480;
    cfg.height = 270;
    std::fs::create_dir_all(&cfg.output)?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("low-light comparison"),
        size: wgpu::Extent3d {
            width: cfg.width,
            height: cfg.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let stars = stars::generate(3500, 2500, 0x5EED_1234);
    let mut tracer = PathTracer::new(
        &device,
        FORMAT,
        (cfg.width, cfg.height),
        &stars,
        Options::default(),
    );
    let mut s = state(
        crate::config::parse(include_str!("../configs/atacama.yaml"))?,
        0.0,
    );
    s.point_initial_view();
    let frame = s.build_frame(cfg.width, cfg.height);
    let mut timing = Vec::new();
    for variant in [
        "reference",
        "no-surface-sky",
        "no-ground-bounces",
        "guided-ground",
    ] {
        install(&device, &mut tracer, variant);
        let options = Options {
            samples_per_frame: 1,
            max_bounces: 4,
            auto_exposure: false,
            exposure: 1.0,
            ..Options::default()
        };
        reset(&mut tracer, options);
        for _ in 0..8 {
            render(&device, &queue, &mut tracer, &frame, &view, None)?;
        }
        reset(&mut tracer, options);
        let limit = if matches!(variant, "reference" | "guided-ground") {
            1024
        } else {
            64
        };
        let mut measured = Vec::new();
        for spp in 1..=limit {
            let t = render(&device, &queue, &mut tracer, &frame, &view, None)?;
            if spp <= 64 {
                measured.push(t.wall_ms);
            }
            if [1, 16, 64, 1024].contains(&spp) {
                capture(
                    &device,
                    &queue,
                    &tracer,
                    &texture,
                    &cfg,
                    &format!("{variant}-{spp}"),
                )?;
            }
        }
        measured.sort_by(f64::total_cmp);
        let median = (measured[31] + measured[32]) * 0.5;
        eprintln!("{variant}: wall median {median:.3} ms/submission");
        timing.push(serde_json::json!({"variant":variant,"wall_median_ms":median,"samples":limit}));
    }
    std::fs::write(
        Path::new(&cfg.output).join("timings.json"),
        serde_json::to_vec_pretty(&timing)?,
    )?;
    // Alternating short blocks reduce clock/order bias. Measure advancing
    // simulation as well: no image history is reused by either variant.
    let timer = Timer::new(&device, &queue);
    let frames: Vec<_> = (1..=32)
        .map(|i| {
            s.sim_time = f64::from(i) / 86400.0;
            s.build_frame(cfg.width, cfg.height)
        })
        .collect();
    let mut advancing = Vec::new();
    for repeat in 0..3 {
        for variant in if repeat % 2 == 0 {
            ["reference", "guided-ground"]
        } else {
            ["guided-ground", "reference"]
        } {
            install(&device, &mut tracer, variant);
            reset(
                &mut tracer,
                Options {
                    samples_per_frame: 1,
                    max_bounces: 4,
                    auto_exposure: false,
                    exposure: 1.0,
                    ..Options::default()
                },
            );
            for frame in &frames[..12] {
                render(&device, &queue, &mut tracer, frame, &view, None)?;
            }
            let mut times = Vec::new();
            for frame in &frames {
                let t = render(&device, &queue, &mut tracer, frame, &view, Some(&timer))?;
                assert_eq!(tracer.samples(), 1);
                times.push(t.gpu_ms);
            }
            let mut sorted = times.clone();
            sorted.sort_by(f64::total_cmp);
            let median = (sorted[15] + sorted[16]) * 0.5;
            eprintln!("advancing {repeat} {variant}: {median:.3} ms GPU");
            advancing.push(serde_json::json!({"variant":variant,"repeat":repeat,"gpu_median_ms":median,"gpu_ms":times}));
        }
    }
    std::fs::write(
        Path::new(&cfg.output).join("advancing.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "adapter":info.name,"driver":info.driver_info,"width":cfg.width,"height":cfg.height,
            "bounces":4,"fixed_exposure":1,"runs":advancing,
        }))?,
    )?;
    Ok(())
}
