//! Actual shader checks for every procedural planet material and editor scene.
use super::*;

fn save_surface(name: &str, size: (u32, u32), pixels: &[[f32; 4]], exposure: f32) {
    let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), size.0, size.1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut data = Vec::with_capacity(pixels.len() * 3);
    for pixel in pixels {
        if pixel.iter().any(|v| !v.is_finite()) {
            data.extend_from_slice(&[255, 0, 255]);
            continue;
        }
        for channel in &pixel[..3] {
            let x = channel * exposure;
            let mapped = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
            let srgb = if mapped <= 0.0031308 {
                12.92 * mapped
            } else {
                1.055 * mapped.powf(1.0 / 2.4) - 0.055
            };
            data.push((255.0 * srgb).round() as u8);
        }
    }
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&data)
        .unwrap();
}

fn editor_frame(style: Option<crate::sim::SurfaceStyle>, size: (u32, u32)) -> Frame {
    editor_frame_at(style, size, 0, 0.0, 60.0)
}

fn editor_frame_at(
    style: Option<crate::sim::SurfaceStyle>,
    size: (u32, u32),
    seed: u32,
    rotation: f64,
    light: f64,
) -> Frame {
    let editor = crate::texture::Editor::new(style);
    let mut scene = editor.scene();
    editor.apply(&mut scene);
    let planet = crate::texture::Editor::planet_index(&scene);
    if let Some(surface) = &mut scene.bodies[planet].surface {
        surface.seed = seed;
    }
    scene.bodies[planet].spin_phase = rotation.to_radians();
    scene.bodies[scene.host].elements.m0 = (180.0 - light).to_radians();
    let mut state = crate::State::with_scene(scene);
    state.no_ground = true;
    state.point_at_fov(planet, editor.fov_deg.to_radians());
    state.build_frame(size.0, size.1)
}

#[test]
#[ignore = "requires a GPU; evaluates raw material values over the entire sphere"]
fn gpu_planet_materials_are_finite() {
    use crate::sim::SurfaceStyle;
    let (device, queue) = gpu();
    let size = (256, 128);
    let mut tracer = PathTracer::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size,
        &[],
        test_options(),
    );
    // Bypass final albedo clamping and transport: NaNs must be detected where
    // they originate, before a backend can hide them by clamping to black.
    let source = format!(
        "{TRACE_SHADER}\n{}",
        r#"
@compute @workgroup_size(8, 8)
fn material_probe(@builtin(global_invocation_id) id: vec3<u32>) {
    let dimensions = vec2<u32>(settings.g.viewport.xy);
    if (any(id.xy >= dimensions)) { return; }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(dimensions);
    let latitude = (uv.y - 0.5) * PI;
    let longitude = uv.x * TAU;
    let q = vec3<f32>(cos(latitude) * cos(longitude), cos(latitude) * sin(longitude), sin(latitude));
    let body = bodies[1];
    var color: vec3<f32>;
    switch u32(body.axis_x.w + 0.5) {
        case 1u: { color = gas_giant_albedo(body, q); }
        case 2u: { color = cratered_albedo(body, q); }
        case 3u: { color = rocky_albedo(body, q); }
        case 4u: { color = icy_albedo(body, q); }
        case 5u: { color = volcanic_albedo(body, q); }
        case 6u: { color = terran_albedo(body, q); }
        default: { color = surface_albedo(body, q); }
    }
    accumulation[id.y * dimensions.x + id.x] = vec4<f32>(color, 0.0);
}
"#
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("planet material probe"),
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
        entry_point: Some("material_probe"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut failures = Vec::new();
    for style in std::iter::once(None).chain(SurfaceStyle::ALL.into_iter().map(Some)) {
        let name = style.map_or("plain", SurfaceStyle::name);
        let frame = editor_frame(style, size);
        assert_eq!(
            frame.bodies[1].axis_x[3],
            style.map_or(0.0, SurfaceStyle::code)
        );
        let pixels = sample(&device, &queue, &mut tracer, &frame, 1);
        save_surface(&format!("material-{name}"), size, &pixels, 1.0);
        let invalid = pixels
            .iter()
            .filter(|p| p.iter().any(|v| !v.is_finite() || *v < 0.0))
            .count();
        let black = pixels
            .iter()
            .filter(|p| p[..3].iter().all(|v| *v == 0.0))
            .count();
        eprintln!("{name}: {invalid} invalid material pixels, {black} black pixels");
        if invalid != 0 || black != 0 {
            failures.push((name, invalid, black));
        }
    }
    assert!(failures.is_empty(), "broken planet materials: {failures:?}");
}

#[test]
#[ignore = "requires a GPU; renders all texture editor styles"]
fn gpu_texture_editor_surfaces() {
    use crate::sim::SurfaceStyle;
    let (device, queue) = gpu();
    let size = (320, 240);
    let mut tracer = PathTracer::new_with_guided_ground(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size,
        &[],
        Options {
            samples_per_frame: 4,
            max_bounces: 4,
            ..test_options()
        },
        true,
    );
    let mut failures = Vec::new();
    for style in std::iter::once(None).chain(SurfaceStyle::ALL.into_iter().map(Some)) {
        let name = style.map_or("plain", SurfaceStyle::name);
        let frame = editor_frame(style, size);
        let pixels = sample(&device, &queue, &mut tracer, &frame, 8);
        save_surface(&format!("planet-{name}"), size, &pixels, 1.0);
        let invalid = pixels
            .iter()
            .filter(|p| p.iter().any(|v| !v.is_finite() || *v < 0.0))
            .count();
        let lit = pixels
            .iter()
            .filter(|p| p[..3].iter().copied().sum::<f32>() > 0.01)
            .count();
        eprintln!("{name}: {invalid} invalid HDR pixels, {lit} lit pixels");
        if invalid != 0 || lit < 5000 {
            failures.push((name, invalid, lit));
        }
    }
    assert!(
        failures.is_empty(),
        "broken texture editor renders: {failures:?}"
    );
}

fn install_surface_probe(device: &wgpu::Device, tracer: &mut PathTracer, source: String) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("independent surface/lighting probe"),
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
}

#[test]
#[ignore = "requires a GPU; lit pixels must equal independently evaluated albedo times incoming light"]
fn gpu_planet_lighting_preserves_materials() {
    use crate::sim::SurfaceStyle;
    let (device, queue) = gpu();
    let size = (129, 97);
    let make = || {
        PathTracer::new_with_guided_ground(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            size,
            &[],
            Options {
                samples_per_frame: 1,
                max_bounces: 1,
                ..test_options()
            },
            true,
        )
    };
    let mut rendered = make();
    let mut material = make();
    let mut illumination = make();
    let source = guided_ground::source();
    let material_hook = "        shine_body = MISS;\n        if (!local) {";
    assert_eq!(source.matches(material_hook).count(), 1);
    install_surface_probe(&device, &mut material, source.replace(material_hook,
        "        return vec4<f32>(albedo, 0.0);\n        shine_body = MISS;\n        if (!local) {"));
    let light_hook = "albedo = surface_albedo(body, normal);";
    assert_eq!(source.matches(light_hook).count(), 1);
    install_surface_probe(
        &device,
        &mut illumination,
        source.replace(light_hook, "albedo = vec3<f32>(1.0);"),
    );
    for style in std::iter::once(None).chain(SurfaceStyle::ALL.into_iter().map(Some)) {
        let name = style.map_or("plain", SurfaceStyle::name);
        for (seed, rotation, light) in [(0, 0.0, 60.0), (17, 73.0, 0.0), (99, 193.0, 120.0)] {
            let scene = editor_frame_at(style, size, seed, rotation, light);
            let actual = sample(&device, &queue, &mut rendered, &scene, 1);
            let albedo = sample(&device, &queue, &mut material, &scene, 1);
            let incoming = sample(&device, &queue, &mut illumination, &scene, 1);
            let mut bad = 0;
            let mut worst = 0.0_f32;
            let mut lit = 0;
            for ((pixel, a), i) in actual.iter().zip(&albedo).zip(&incoming) {
                if i[0] > 0.01 {
                    lit += 1;
                }
                for c in 0..3 {
                    let expected = a[c] * i[c];
                    let error = (pixel[c] - expected).abs();
                    worst = worst.max(error);
                    // Allow normal f32 reassociation, never tile-sized color
                    // corruption or missing chunks of an illuminated planet.
                    if !pixel[c].is_finite()
                        || !expected.is_finite()
                        || error > 2e-4 * (1.0 + expected.abs())
                    {
                        bad += 1;
                    }
                }
            }
            eprintln!(
                "{name}, seed={seed}, rotation={rotation}, light={light}: {bad} bad channels, max error={worst}"
            );
            assert!(lit > 100, "fixture must exercise lit surface pixels");
            assert_eq!(
                bad, 0,
                "material changed during lighting: {name}, seed={seed}, max error={worst}"
            );
        }
    }
}
