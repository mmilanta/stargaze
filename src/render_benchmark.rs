//! Opt-in, offscreen benchmark of production rendering and explicit quality experiments.
//! Run through scripts/benchmark.py to capture provenance and compare artifacts.
use super::*;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Instant};

#[path = "render_experiments.rs"]
mod experiments;

const CASES: &[&str] = &[
    "halo-rings",
    "earth-daylight",
    "earth-twilight",
    "median-dense",
    "vantus-eclipse",
    "vantus-airless",
    "dual-eclipse",
    "moonlit-air",
];
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    output: String,
    width: u32,
    height: u32,
    scale: u32,
    bounces: u32,
    vegetation: u32,
    warmup: u32,
    frames: u32,
    repeats: u32,
    image_samples: u32,
    adapter: String,
    cases: Vec<String>,
    #[serde(default = "reference_variant")]
    variant: String,
}
fn reference_variant() -> String {
    "reference".into()
}

impl Config {
    fn validate(&self) -> Result<()> {
        ensure!(
            experiments::VARIANTS.contains(&self.variant.as_str()),
            "unknown shader variant"
        );
        ensure!(
            (16..=3840).contains(&self.width) && (16..=2160).contains(&self.height),
            "invalid dimensions"
        );
        ensure!(
            (25..=100).contains(&self.scale) && (1..=16).contains(&self.bounces),
            "invalid quality"
        );
        ensure!(
            self.vegetation <= 100 && (1..=100).contains(&self.warmup),
            "invalid vegetation/warmup"
        );
        ensure!(
            (2..=1000).contains(&self.frames) && (1..=20).contains(&self.repeats),
            "invalid frames/repeats"
        );
        ensure!(
            (2..=4096).contains(&self.image_samples),
            "invalid image samples"
        );
        ensure!(
            !self.cases.is_empty() && self.cases.iter().all(|s| CASES.contains(&s.as_str())),
            "unknown or empty case selection"
        );
        Ok(())
    }
    fn trace_size(&self) -> (u32, u32) {
        (
            (self.width * self.scale).div_ceil(100),
            (self.height * self.scale).div_ceil(100),
        )
    }
}

// Do not read saved preferences, startup heuristics, environment overrides, or wall time.
fn state(scene: crate::sim::Scene, time: f64) -> crate::State {
    let mut observer = crate::camera::Observer::new(scene.host);
    observer.lat = scene.observer_lat_deg.to_radians();
    observer.lon = scene.observer_lon_deg.to_radians();
    observer.height_m = scene.observer_height_m;
    crate::State {
        scene,
        observer,
        sim_time: time,
        warp: crate::STOP_WARP,
        resume_warp: crate::STOP_WARP + 1,
        day_step_failed: false,
        last: Instant::now(),
        dragging: false,
        last_cursor: None,
        view_lock: None,
        press: None,
    }
}

fn sphere(center: [f32; 3], radius: f32, color: [f32; 3], emissive: f32) -> Sphere {
    Sphere {
        center,
        radius,
        color,
        emissive,
        center_low: [0.0; 4],
        ring_plane: [0.0; 4],
        ring_params: [0.0; 4],
        ring_color: [0.0; 4],
    }
}

fn fixture(name: &str, step: u32, cfg: &Config) -> Frame {
    // Advancing sequence: fixed one simulated second per frame, independent of GPU speed.
    let dt = f64::from(step) / 86400.0;
    if name == "dual-eclipse" {
        let mut g = Globals::zeroed();
        g.cam_right = [1.0, 0.0, 0.0, 0.0];
        g.cam_up = [0.0, 1.0, 0.0, 0.0];
        g.cam_forward = [0.0, 0.0, -1.0, (30_f32.to_radians()).tan()];
        g.viewport = [cfg.width as f32, cfg.height as f32, 1.0, 0.0];
        let mut planet = sphere([0.0, 0.0, -3.0], 1.0, [0.55, 0.65, 0.8], 0.0);
        planet.ring_plane = [0.0, 0.8, 0.6, 1.2];
        planet.ring_params = [1.9, 0.7, 1.0, 0.0];
        planet.ring_color = [0.8, 0.65, 0.4, 0.0];
        return Frame {
            globals: g,
            bodies: vec![
                planet,
                sphere([-3.0, 2.0, -1.0], 0.55, [25.0, 20.0, 12.0], 1.0),
                sphere([3.0, 2.0, -1.0], 0.45, [12.0, 18.0, 25.0], 1.0),
                sphere([-1.5 + step as f32 * 0.003, 1.0, -1.5], 0.3, [0.3; 3], 0.0),
                sphere([1.5 - step as f32 * 0.003, 1.0, -1.5], 0.25, [0.3; 3], 0.0),
            ],
            ground: vec![],
            scene_time: dt,
            labels: vec![],
        };
    }
    if name == "moonlit-air" {
        // Synthetic moonlit-air stress view based on gpu_reflected_atmosphere:
        // the sun is below the host horizon but illuminates the large moon.
        let mut g = Globals::zeroed();
        g.cam_right = [1.0, 0.0, 0.0, 0.0];
        g.cam_up = [0.0, 1.0, 0.0, 0.0];
        g.cam_forward = [0.0, 0.0, -1.0, (27.5_f32.to_radians()).tan()];
        g.viewport = [cfg.width as f32, cfg.height as f32, 1.0, 0.0];
        let air = crate::sim::Atmosphere::earthlike();
        let radius = (6371.0 / crate::sim::AU_KM) as f32;
        let center = [0.0, 0.0, radius + (0.002 / crate::sim::AU_KM) as f32];
        g.atmo_center = [center[0], center[1], center[2], radius];
        g.atmo_rayleigh = [
            air.rayleigh[0] as f32,
            air.rayleigh[1] as f32,
            air.rayleigh[2] as f32,
            air.mie as f32,
        ];
        g.atmo_params = [
            air.mie_g as f32,
            air.scale_height as f32,
            air.thickness as f32,
            1.0,
        ];
        return Frame {
            globals: g,
            bodies: vec![
                sphere(center, radius, [0.0; 3], 0.0),
                sphere(
                    [0.0004 + step as f32 * 1e-7, 0.0, -0.002],
                    0.00015,
                    [0.8; 3],
                    0.0,
                ),
                sphere([0.3, 0.0, 1.0], 0.005, [100_000.0; 3], 1.0),
            ],
            ground: vec![],
            scene_time: dt,
            labels: vec![],
        };
    }
    let mut s = match name {
        "halo-rings" => state(crate::sim::halo_scene(), dt),
        "earth-daylight" => state(crate::sim::default_scene(), 0.599 + dt),
        "earth-twilight" => state(crate::sim::default_scene(), 1.04 + dt),
        "median-dense" => state(
            crate::config::parse(include_str!("../configs/median-resonance.yaml")).unwrap(),
            0.25 + dt,
        ),
        "vantus-eclipse" => state(crate::sim::binary_scene(), 444.478 + dt),
        "vantus-airless" => state(crate::sim::binary_scene(), 444.478 + dt),
        _ => panic!("unknown fixture"),
    };
    match name {
        "halo-rings" => s.point_initial_view(),
        "earth-daylight" | "earth-twilight" => {
            s.ground_view();
            s.observer.alt = 3_f64.to_radians();
            s.observer.fov_y = 55_f64.to_radians();
        }
        "median-dense" => {
            s.observer.az = 0.0;
            s.observer.alt = 20_f64.to_radians();
            s.observer.fov_y = 65_f64.to_radians();
        }
        _ => {
            let positions = s.scene.positions(s.sim_time);
            let view = s.observer.frame(&s.scene, s.sim_time, &positions);
            let distance = (positions[8] - view.position).length();
            s.point_at_fov(8, (s.scene.body(8).radius / distance).asin() * 2.5);
            if name == "vantus-airless" {
                s.scene.atmosphere = None;
            }
        }
    }
    s.build_frame_with_vegetation(cfg.width, cfg.height, cfg.vegetation)
}

fn map(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    rx.recv()??;
    let view = buffer.slice(..).get_mapped_range()?;
    let bytes = view.to_vec();
    drop(view);
    buffer.unmap();
    Ok(bytes)
}

struct Timer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period: f64,
}
impl Timer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("benchmark-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period: f64::from(queue.get_timestamp_period()),
        }
    }
}
#[derive(Serialize)]
struct Timing {
    gpu_ms: f64,
    wall_ms: f64,
    spp: u32,
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tracer: &mut PathTracer,
    frame: &Frame,
    view: &wgpu::TextureView,
    timer: Option<&Timer>,
) -> Result<Timing> {
    let start = Instant::now();
    let mut encoder = device.create_command_encoder(&Default::default());
    if let Some(t) = timer {
        encoder.write_timestamp(&t.queries, 0);
    }
    tracer.encode(queue, &mut encoder, frame);
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("benchmark-production-display"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
    if let Some(t) = timer {
        encoder.write_timestamp(&t.queries, 1);
        encoder.resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
        encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, 16);
    }
    queue.submit(Some(encoder.finish()));
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    let gpu_ms = if let Some(t) = timer {
        let data = map(device, &t.read)?;
        let a = u64::from_le_bytes(data[..8].try_into()?);
        let b = u64::from_le_bytes(data[8..16].try_into()?);
        ensure!(b > a, "invalid GPU timestamps");
        (b - a) as f64 * t.period / 1e6
    } else {
        0.0
    };
    Ok(Timing {
        gpu_ms,
        wall_ms,
        spp: tracer.samples(),
    })
}

fn reset(tracer: &mut PathTracer, options: Options) {
    tracer.history = History::default();
    tracer.options = options;
    tracer.exposure_was_enabled = false;
}

fn capture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tracer: &PathTracer,
    texture: &wgpu::Texture,
    cfg: &Config,
    name: &str,
) -> Result<serde_json::Value> {
    let pitch = (cfg.width * 4).div_ceil(256) * 256;
    let rgba = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("benchmark-image-readback"),
        size: u64::from(pitch) * u64::from(cfg.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let hdr_size = u64::from(tracer.dimensions.0) * u64::from(tracer.dimensions.1) * 16;
    let hdr = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("benchmark-linear-readback"),
        size: hdr_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &rgba,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(cfg.height),
            },
        },
        texture.size(),
    );
    encoder.copy_buffer_to_buffer(&tracer.accumulation, 0, &hdr, 0, hdr_size);
    queue.submit(Some(encoder.finish()));
    let padded = map(device, &rgba)?;
    let pixels: Vec<u8> = padded
        .chunks(pitch as usize)
        .flat_map(|r| r[..cfg.width as usize * 4].iter().copied())
        .collect();
    let linear = map(device, &hdr)?;
    ensure!(
        linear.chunks_exact(4).all(|v| {
            let f = f32::from_le_bytes(v.try_into().unwrap());
            f.is_finite() && f >= 0.0
        }),
        "nonfinite/negative HDR in {name}"
    );
    let output = Path::new(&cfg.output);
    let mut png = png::Encoder::new(
        std::fs::File::create(output.join(format!("{name}.png")))?,
        cfg.width,
        cfg.height,
    );
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&pixels)?;
    std::fs::write(output.join(format!("{name}.rgba")), pixels)?;
    std::fs::write(output.join(format!("{name}.f32")), linear)?;
    Ok(
        serde_json::json!({"name": name, "spp": tracer.samples(), "exposure": tracer.options.exposure}),
    )
}

fn fingerprint(frame: &Frame) -> String {
    // Stable FNV-1a over exact GPU inputs, used for detecting incompatible fixtures.
    let mut hash = 0xcbf29ce484222325_u64;
    for bytes in [
        bytemuck::bytes_of(&frame.globals),
        bytemuck::cast_slice(&frame.bodies),
        bytemuck::cast_slice(&frame.ground),
        &frame.scene_time.to_le_bytes(),
    ] {
        for &b in bytes {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

#[test]
#[ignore = "hardware benchmark; use python3 scripts/benchmark.py run OUTPUT"]
fn render_benchmark() -> Result<()> {
    ensure!(!cfg!(debug_assertions), "benchmark requires --release");
    ensure!(
        std::env::var_os("STARGAZE_GROUND").is_none(),
        "remove STARGAZE_GROUND override"
    );
    let config_path = std::env::var("STARGAZE_BENCH_CONFIG").context("run scripts/benchmark.py")?;
    let cfg: Config = serde_json::from_slice(&std::fs::read(config_path)?)?;
    cfg.validate()?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    let available: Vec<_> = adapters.iter().map(|a| a.get_info()).collect();
    eprintln!("Available adapters: {available:?}");
    let adapter = adapters
        .into_iter()
        .find(|a| {
            let i = a.get_info();
            i.device_type != wgpu::DeviceType::Cpu
                && if cfg.adapter.is_empty() {
                    i.device_type == wgpu::DeviceType::DiscreteGpu
                } else {
                    i.name.to_lowercase().contains(&cfg.adapter.to_lowercase())
                }
        })
        .context("no matching hardware GPU; software rendering is deliberately refused")?;
    let info = adapter.get_info();
    eprintln!("Benchmark GPU: {info:?}");
    let features =
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    ensure!(
        adapter.features().contains(features),
        "GPU timestamp queries are required"
    );
    let limits = wgpu::Limits {
        max_storage_buffer_binding_size: adapter.limits().max_storage_buffer_binding_size,
        max_buffer_size: adapter.limits().max_buffer_size,
        ..Default::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("stargaze-benchmark"),
        required_features: features,
        required_limits: limits,
        ..Default::default()
    }))?;
    let timer = Timer::new(&device, &queue);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("benchmark-output"),
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
    let options = Options {
        samples_per_frame: 1,
        max_bounces: cfg.bounces,
        auto_exposure: true,
        ..Options::default()
    };
    let mut tracer = PathTracer::new(&device, FORMAT, cfg.trace_size(), &stars, options);
    experiments::install(&device, &mut tracer, &cfg.variant);
    let mut cases = vec![];
    for name in &cfg.cases {
        let frames: Vec<_> = (0..cfg.frames.max(cfg.warmup))
            .map(|i| fixture(name, i, &cfg))
            .collect();
        let mut modes = vec![];
        for moving in [false, true] {
            reset(&mut tracer, options);
            for i in 0..cfg.warmup {
                render(
                    &device,
                    &queue,
                    &mut tracer,
                    &frames[if moving { i as usize } else { 0 }],
                    &view,
                    None,
                )?;
            }
            let mut repeats = vec![];
            for _ in 0..cfg.repeats {
                reset(&mut tracer, options);
                let mut measurements = vec![];
                for i in 0..cfg.frames {
                    let timing = render(
                        &device,
                        &queue,
                        &mut tracer,
                        &frames[if moving { i as usize } else { 0 }],
                        &view,
                        Some(&timer),
                    )?;
                    ensure!(
                        timing.spp == if moving { 1 } else { i + 1 },
                        "unexpected history/sample behavior"
                    );
                    measurements.push(timing);
                }
                repeats.push(measurements);
            }
            let mut times: Vec<_> = repeats.iter().flatten().map(|v| v.gpu_ms).collect();
            times.sort_by(f64::total_cmp);
            let mode = if moving { "advancing" } else { "paused" };
            eprintln!(
                "{name:20} {mode:9}: {:.3} ms GPU median",
                times[times.len() / 2]
            );
            modes.push(serde_json::json!({"mode": mode, "repeats": repeats}));
        }
        // Fixed manual exposure makes image differences independent of adaptation history.
        let exposure = match name.as_str() {
            "vantus-eclipse" | "vantus-airless" => 8.0,
            "moonlit-air" => 64.0,
            _ => 1.0,
        };
        let image_options = Options {
            auto_exposure: false,
            exposure,
            ..options
        };
        reset(&mut tracer, image_options);
        let mut images = vec![];
        for i in 1..=cfg.image_samples {
            render(&device, &queue, &mut tracer, &frames[0], &view, None)?;
            if i == 1 || i == cfg.image_samples {
                images.push(capture(
                    &device,
                    &queue,
                    &tracer,
                    &texture,
                    &cfg,
                    &format!("{name}-spp{i}"),
                )?);
            }
        }
        reset(&mut tracer, image_options);
        for i in 0..cfg.frames {
            render(
                &device,
                &queue,
                &mut tracer,
                &frames[i as usize],
                &view,
                None,
            )?;
            if i == cfg.frames / 2 || i == cfg.frames - 1 {
                images.push(capture(
                    &device,
                    &queue,
                    &tracer,
                    &texture,
                    &cfg,
                    &format!("{name}-moving-{i}"),
                )?);
            }
        }
        cases.push(serde_json::json!({"name": name, "initial_time_days": frames[0].scene_time,
            "frame_fingerprints": frames[..cfg.frames as usize].iter().map(fingerprint).collect::<Vec<_>>(),
            "modes": modes, "images": images }));
    }
    let result = serde_json::json!({"suite_version": 1, "variant": cfg.variant, "adapter": {
        "name": info.name, "vendor": info.vendor, "device": info.device,
        "device_type": format!("{:?}", info.device_type), "backend": format!("{:?}", info.backend),
        "driver": info.driver, "driver_info": info.driver_info,
    }, "timestamp_period_ns": timer.period, "trace_size": cfg.trace_size(), "cases": cases});
    std::fs::write(
        Path::new(&cfg.output).join("results.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(())
}

#[test]
fn benchmark_fixtures_are_deterministic_and_cover_requested_workloads() {
    let cfg = Config {
        output: String::new(),
        width: 1280,
        height: 720,
        scale: 75,
        bounces: 4,
        vegetation: 70,
        warmup: 3,
        frames: 8,
        repeats: 3,
        image_samples: 32,
        adapter: String::new(),
        variant: reference_variant(),
        cases: CASES.iter().map(|v| v.to_string()).collect(),
    };
    cfg.validate().unwrap();
    for &name in CASES {
        let a = fixture(name, 0, &cfg);
        assert_eq!(fingerprint(&a), fingerprint(&fixture(name, 0, &cfg)));
        assert_ne!(fingerprint(&a), fingerprint(&fixture(name, 1, &cfg)));
        assert_eq!(
            a.globals.atmo_params[3] > 0.0,
            !matches!(name, "vantus-airless" | "dual-eclipse")
        );
        if matches!(name, "earth-daylight" | "earth-twilight" | "median-dense") {
            let sun = if name == "median-dense" {
                &a.bodies[1]
            } else {
                a.bodies.iter().find(|b| b.emissive > 0.5).unwrap()
            };
            let up = glam::Vec3::from_array(a.globals.ground_up[..3].try_into().unwrap());
            let altitude = glam::Vec3::from(sun.center).normalize().dot(up);
            if name == "earth-twilight" {
                assert!((-0.05..0.0).contains(&altitude));
            } else {
                assert!(altitude > 0.2, "{name} must be daylight");
            }
            assert!(!a.ground.is_empty());
        }
        if name == "dual-eclipse" {
            assert_eq!(a.bodies.iter().filter(|b| b.emissive > 0.5).count(), 2);
            // Both occluders lie on the segment from the front of the planet to a sun.
            for (sun, blocker) in [(1, 3), (2, 4)] {
                let start = glam::Vec3::new(0.0, 0.0, -2.0);
                let midpoint = (start + glam::Vec3::from(a.bodies[sun].center)) * 0.5;
                assert!(midpoint.distance(a.bodies[blocker].center.into()) < 1e-6);
            }
        }
    }
}
