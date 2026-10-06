//! Deliberately approximate, benchmark-only shader variants. Production never
//! compiles this module. Keep each change explicit and preserve fixture inputs.
use super::*;

pub const VARIANTS: &[&str] = &[
    "reference",
    "sun6",
    "view12",
    "balanced",
    "fast",
    "planetshine-quarter",
    "no-planetshine",
];

pub fn source(variant: &str) -> String {
    assert!(VARIANTS.contains(&variant));
    if variant == "reference" {
        return TRACE_SHADER.to_string();
    }
    let mut source = TRACE_SHADER.to_string();
    // Keep the named experiments at their reviewed settings. The reference
    // follows production, which now uses the accepted fast integration counts.
    let (view, sun) = match variant {
        "sun6" => (24, 6),
        "view12" => (12, 12),
        "balanced" => (12, 6),
        "fast" => (8, 4),
        _ => (24, 12),
    };
    let mut replace = |from: &str, to: &str| {
        assert_eq!(
            source.matches(from).count(),
            1,
            "experiment hook changed: {from}"
        );
        source = source.replacen(from, to, 1);
    };
    replace(
        "const ATMO_VIEW_STEPS: u32 = 8u;",
        &format!("const ATMO_VIEW_STEPS: u32 = {view}u;"),
    );
    replace(
        "const ATMO_SUN_STEPS: u32 = 4u;",
        &format!("const ATMO_SUN_STEPS: u32 = {sun}u;"),
    );
    if variant == "no-planetshine" {
        replace(
            "let reflector = sample_reflector(origin, &reflected_rng);",
            "let reflector = ReflectorSample(MISS, 0.0);",
        );
    } else if variant == "planetshine-quarter" {
        replace(
            "let reflector = sample_reflector(origin, &reflected_rng);",
            r#"
    let reflector = sample_reflector(origin, &reflected_rng);
    // A uniformly chosen phase selects 6 of the 24 integration cells. Weight
    // those cells by four, preserving the expected discrete integral at the
    // expense of variance. The direct-light RNG is untouched.
    var phase_rng = hash(*rng ^ 0x9e3779b9u);
    let reflected_phase = u32(random(&phase_rng) * 4.0);
"#,
        );
        replace(
            "if (reflector.index != MISS) {",
            "if (reflector.index != MISS && i % 4u == reflected_phase) {",
        );
        replace(
            "* (visibility * reflector.inverse_probability / light.pdf);",
            "* (4.0 * visibility * reflector.inverse_probability / light.pdf);",
        );
    }
    source
}

pub fn install(device: &wgpu::Device, tracer: &mut PathTracer, variant: &str) {
    if variant == "reference" {
        return;
    }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(variant),
        source: wgpu::ShaderSource::Wgsl(source(variant).into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("benchmark-only experiment"),
        bind_group_layouts: &[Some(&tracer.trace_layout)],
        immediate_size: 0,
    });
    let pipeline = |entry| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(variant),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    tracer.compute = pipeline("main");
    tracer.sky_light = pipeline("cache_surface_sky");
}

#[test]
fn approximate_shaders_validate_and_reference_is_unchanged() {
    assert_eq!(source("reference"), TRACE_SHADER);
    assert_eq!(source("fast"), TRACE_SHADER);
    for variant in VARIANTS {
        let source = source(variant);
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("{variant}: {}", e.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{variant}: {}", e.emit_to_string(&source)));
    }
}
