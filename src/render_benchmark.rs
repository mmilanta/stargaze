//! Opt-in, offscreen benchmark of production rendering and explicit quality experiments.
//! Run through scripts/benchmark.py to capture provenance and compare artifacts.
use super::*;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Instant};

#[path = "render_experiments.rs"]
mod experiments;

#[path = "lowlight_diagnostics.rs"]
mod lowlight;

const CASES: &[&str] = &[
    "halo-rings",
    "earth-daylight",
    "earth-twilight",
    "median-dense",
    "vantus-eclipse",
    "vantus-airless",
    "dual-eclipse",
    "moonlit-air",
    "eclipse-umbra-edge",
    "atacama-day",
    "atacama-night",
];
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[test]
#[ignore = "requires GPU; writes desert level day/night previews to target/atacama-preview"]
fn gpu_atacama_preview() -> Result<()> {
    let (device, queue) = super::tests::gpu();
    let mut cfg = umbra_config("target/atacama-preview".into(), "reference");
    cfg.width = 960;
    cfg.height = 540;
    std::fs::create_dir_all(&cfg.output)?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("desert-preview"),
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
        Options {
            samples_per_frame: 1,
            max_bounces: 4,
            ..Options::default()
        },
    );
    let arrival = crate::config::parse(include_str!("../configs/atacama.yaml"))?
        .initial_time_days
        .unwrap();
    for (name, time) in [("arrival", arrival), ("night", 0.0), ("daylight", 0.14)] {
        let scene = crate::config::parse(include_str!("../configs/atacama.yaml"))?;
        let mut s = state(scene, time);
        s.point_initial_view();
        let p = s.scene.positions(time);
        let vf = s.observer.frame(&s.scene, time, &p);
        for (i, b) in s.scene.bodies.iter().enumerate() {
            if i == s.scene.host {
                continue;
            }
            let d = (p[i] - vf.position).normalize();
            eprintln!(
                "{name} {}: altitude {:.2}, azimuth {:.2}",
                b.name,
                d.dot(vf.zenith).asin().to_degrees(),
                d.dot(vf.east).atan2(d.dot(vf.north)).to_degrees()
            );
        }
        let frame = s.build_frame(cfg.width, cfg.height);
        reset(
            &mut tracer,
            Options {
                samples_per_frame: 1,
                max_bounces: 4,
                auto_exposure: true,
                ..Options::default()
            },
        );
        for _ in 0..64 {
            render(&device, &queue, &mut tracer, &frame, &view, None)?;
        }
        capture(&device, &queue, &tracer, &texture, &cfg, name)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires GPU; compares distant planets to a bright star at 720p/60-degree sampling"]
fn gpu_atacama_planet_brightness() {
    let (device, queue) = super::tests::gpu();
    for name in ["Cobre", "Sal", "Hielo"] {
        let scene = crate::config::parse(include_str!("../configs/atacama.yaml")).unwrap();
        let mut s = state(scene, 0.0);
        let i = s.scene.bodies.iter().position(|b| b.name == name).unwrap();
        let fov = 2.0 * (30_f64.to_radians().tan() * 32.0 / 720.0).atan();
        s.point_at_fov(i, fov);
        let positions = s.scene.positions(s.sim_time);
        let vf = s.observer.frame(&s.scene, s.sim_time, &positions);
        let bright_star = stars::CatalogueStar {
            dir: (positions[i] - vf.position)
                .normalize()
                .as_vec3()
                .to_array(),
            color: [1.0; 3],
            bright: 1.62,
            angular_radius: 0.00136,
        };
        let opts = Options {
            samples_per_frame: 16,
            max_bounces: 4,
            milky_way: 0.0,
            ..Options::default()
        };
        let mut planet = PathTracer::new(&device, FORMAT, (32, 32), &[], opts);
        let mut reference = PathTracer::new(&device, FORMAT, (32, 32), &[bright_star], opts);
        let mut scene = s.build_frame(32, 32);
        let rendered = super::tests::sample(&device, &queue, &mut planet, &scene, 64);
        // Remove only the target so the reference dot sees the same air and
        // sky. The tiny target contributes negligibly to atmospheric lighting.
        scene.bodies[i].radius = 0.0;
        let reference_pixels = super::tests::sample(&device, &queue, &mut reference, &scene, 64);
        let sky = super::tests::sample(&device, &queue, &mut planet, &scene, 64);
        let flux = |pixels: &[[f32; 4]]| {
            let mut sum = 0.0_f64;
            for y in 13..19 {
                for x in 13..19 {
                    let j = y * 32 + x;
                    sum += f64::from(
                        (pixels[j][0] - sky[j][0]) * 0.2126
                            + (pixels[j][1] - sky[j][1]) * 0.7152
                            + (pixels[j][2] - sky[j][2]) * 0.0722,
                    );
                }
            }
            sum
        };
        let ratio = flux(&rendered) / flux(&reference_pixels);
        eprintln!("{name}: planet/star apparent linear luminance ratio {ratio:.3}");
        assert!(
            (1.1..1.6).contains(&ratio),
            "{name}: should be modestly brighter than the reference star, got {ratio}"
        );
    }
}

#[derive(Deserialize, Serialize)]
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
    if name == "eclipse-umbra-edge" {
        // Camera is 2 m above an Earth-size black host, looking 12 degrees
        // above the horizon. A nearly point-like sun is on the camera axis.
        // Its 1000 km-radius occultor is 100,000 km away, displaced 998 km
        // right: the observer is just inside totality, while air to the left
        // still sees the sun. Motion carries the shadow farther left.
        let km = (1.0 / crate::sim::AU_KM) as f32;
        let radius_m = 6_371_000.0_f32;
        let altitude = 12_f32.to_radians();
        let up = glam::Vec3::new(0.0, altitude.cos(), -altitude.sin());
        let north = glam::Vec3::new(0.0, altitude.sin(), altitude.cos());
        let center = -up * ((6371.0 + 0.002) * km);
        let air = crate::sim::Atmosphere::earthlike();
        let mut g = Globals::zeroed();
        g.cam_right = [1.0, 0.0, 0.0, 0.0];
        g.cam_up = [0.0, 1.0, 0.0, 0.0];
        g.cam_forward = [0.0, 0.0, -1.0, 30_f32.to_radians().tan()];
        g.viewport = [cfg.width as f32, cfg.height as f32, 1.0, 0.0];
        g.atmo_center = [center.x, center.y, center.z, 6371.0 * km];
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
        g.ground_east = [1.0, 0.0, 0.0, 2.0];
        g.ground_up = [up.x, up.y, up.z, radius_m];
        g.ground_north = [north.x, north.y, north.z, 1.0];
        return Frame {
            globals: g,
            bodies: vec![
                sphere(center.to_array(), 6371.0 * km, [0.0; 3], 0.0),
                sphere([0.0, 0.0, -1.0], 1.0e-5, [2.0e10; 3], 1.0),
                sphere(
                    [(998.0 - step as f32 * 0.1) * km, 0.0, -100_000.0 * km],
                    1000.0 * km,
                    [0.0; 3],
                    0.0,
                ),
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
        "atacama-day" | "atacama-night" => state(
            crate::config::parse(include_str!("../configs/atacama.yaml")).unwrap(),
            if name == "atacama-day" { 0.14 + dt } else { dt },
        ),
        _ => panic!("unknown fixture"),
    };
    match name {
        "halo-rings" | "atacama-day" | "atacama-night" => s.point_initial_view(),
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
    run_benchmark(&cfg)
}

fn run_benchmark(cfg: &Config) -> Result<()> {
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
        if name.starts_with("atacama-") {
            assert_eq!(a.globals.ground_counts[2], 1, "desert preset");
            assert!(!a.ground.is_empty());
            let sun = a.bodies.iter().find(|b| b.emissive > 0.5).unwrap();
            let up = glam::Vec3::from_array(a.globals.ground_up[..3].try_into().unwrap());
            let altitude = glam::Vec3::from(sun.center).normalize().dot(up);
            assert_eq!(altitude > 0.0, name == "atacama-day");
        }
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
        if name == "eclipse-umbra-edge" {
            use glam::DVec3;
            // Independent angular-disc geometry, not the shader's ray test.
            let clearance = |frame: &Frame, point: DVec3| {
                let sun = DVec3::from_array(frame.bodies[1].center.map(f64::from)) - point;
                let moon = DVec3::from_array(frame.bodies[2].center.map(f64::from)) - point;
                let separation = sun.normalize().cross(moon.normalize()).length().asin();
                let sun_radius = (f64::from(frame.bodies[1].radius) / sun.length()).asin();
                let moon_radius = (f64::from(frame.bodies[2].radius) / moon.length()).asin();
                (separation, sun_radius, moon_radius)
            };
            let km = 1.0 / crate::sim::AU_KM;
            for step in [0, 8, 15] {
                let f = fixture(name, step, &cfg);
                for point in [DVec3::ZERO, DVec3::new(15.0, 5.0, -30.0) * km] {
                    let (separation, sun, moon) = clearance(&f, point);
                    assert!(
                        separation + sun < moon,
                        "observer/right-hand air must be in totality"
                    );
                }
                let (separation, sun, moon) = clearance(&f, DVec3::new(-15.0, 5.0, -30.0) * km);
                assert!(
                    separation > sun + moon,
                    "left-hand air must see the entire star"
                );
            }
            let mut before = fixture(name, 0, &cfg);
            // Forty seconds earlier at the same 100 m/s transverse speed.
            before.bodies[2].center[0] += (4.0 * km) as f32;
            let (separation, sun, moon) = clearance(&before, DVec3::ZERO);
            assert!(
                separation > sun + moon,
                "the star was unobscured just before ingress"
            );
            let (_, sun, _) = clearance(&a, DVec3::ZERO);
            let pixels =
                sun.tan() * f64::from(cfg.trace_size().1) / f64::from(a.globals.cam_forward[3]);
            assert!(
                pixels < 0.02,
                "the stellar diameter must be far below a pixel"
            );
            let host = DVec3::new(
                f64::from(a.globals.atmo_center[0]),
                f64::from(a.globals.atmo_center[1]),
                f64::from(a.globals.atmo_center[2]),
            );
            let p = DVec3::new(-15.0, 5.0, -30.0) * km;
            let height = (p - host).length() - f64::from(a.globals.atmo_center[3]);
            assert!(
                (5.0 * km..20.0 * km).contains(&height),
                "sunlit sample must be in dense atmosphere"
            );
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

#[test]
#[ignore = "GPU equivalence check for benchmark-only raw optimizations; no timing claims"]
fn gpu_raw_optimization_equivalence() {
    let (device, queue) = super::tests::gpu();
    let mut cfg = umbra_config(String::new(), "reference");
    cfg.width = 33;
    cfg.height = 19;
    let stars = stars::generate(3500, 2500, 0x5EED_1234);
    let options = Options {
        samples_per_frame: 1,
        max_bounces: 4,
        auto_exposure: false,
        ..Options::default()
    };
    let frames: Vec<_> = CASES
        .iter()
        .flat_map(|name| [0, 15].map(|step| (*name, step, fixture(name, step, &cfg))))
        .collect();
    let mut reference = PathTracer::new(&device, FORMAT, (33, 19), &stars, options);
    let expected: Vec<_> = frames
        .iter()
        .map(|(_, _, frame)| {
            reset(&mut reference, options);
            super::tests::sample(&device, &queue, &mut reference, frame, 4)
        })
        .collect();
    for variant in ["sky-any-hit", "dark-reflection", "raw-combined"] {
        let mut tracer = PathTracer::new(&device, FORMAT, (33, 19), &stars, options);
        experiments::install(&device, &mut tracer, variant);
        for ((name, step, frame), expected) in frames.iter().zip(&expected) {
            reset(&mut tracer, options);
            let actual = super::tests::sample(&device, &queue, &mut tracer, frame, 4);
            for (pixel, (a, b)) in actual.iter().zip(expected).enumerate() {
                assert_eq!(a, b, "{variant}, {name}, step {step}, pixel {pixel}");
            }
        }
    }
}

fn umbra_config(output: String, variant: &str) -> Config {
    Config {
        output,
        width: 1280,
        height: 720,
        scale: 75,
        bounces: 4,
        vegetation: 70,
        warmup: 8,
        frames: 16,
        repeats: 3,
        image_samples: 32,
        adapter: String::new(),
        variant: variant.into(),
        cases: vec!["eclipse-umbra-edge".into()],
    }
}

#[test]
#[ignore = "requires a GPU; verifies lit air beside totality using production WGSL"]
fn gpu_umbra_edge_contrast() {
    let cfg = umbra_config(String::new(), "reference");
    let (device, queue) = super::tests::gpu();
    let mut tracer = PathTracer::new(
        &device,
        FORMAT,
        (96, 54),
        &[],
        Options {
            samples_per_frame: 8,
            max_bounces: 4,
            milky_way: 0.0,
            ..Options::default()
        },
    );
    for step in [0, 8, 15] {
        let scene = fixture("eclipse-umbra-edge", step, &cfg);
        let pixels = super::tests::sample(&device, &queue, &mut tracer, &scene, 4);
        let mean = |x0: usize, x1: usize, y0: usize, y1: usize| {
            let mut sum = 0.0_f64;
            for y in y0..y1 {
                for x in x0..x1 {
                    sum += pixels[y * 96 + x][..3]
                        .iter()
                        .map(|v| f64::from(*v))
                        .sum::<f64>();
                }
            }
            sum / ((x1 - x0) * (y1 - y0) * 3) as f64
        };
        let left = mean(12, 30, 14, 23);
        let right = mean(66, 84, 14, 23);
        let ground = mean(12, 84, 45, 52);
        eprintln!("umbra step {step}: left={left}, right={right}, ground={ground}");
        assert!(left > 0.001, "left-hand atmosphere must be visibly sunlit");
        assert!(
            right < left * 0.001,
            "right-hand atmosphere must remain dark"
        );
        assert!(
            ground < 1e-8,
            "the observer's ground must remain in totality"
        );
    }
}

#[test]
#[ignore = "hardware study; writes new full/quality runs for every umbra-edge variant"]
fn render_umbra_edge_study() -> Result<()> {
    ensure!(!cfg!(debug_assertions), "benchmark requires --release");
    ensure!(
        std::env::var_os("STARGAZE_GROUND").is_none(),
        "remove STARGAZE_GROUND override"
    );
    for quality in [false, true] {
        for variant in experiments::QUALITY_VARIANTS {
            let kind = if quality { "quality" } else { "full" };
            let output = format!("target/benchmarks/umbra-edge-study/{kind}/{variant}");
            ensure!(
                !Path::new(&output).exists(),
                "output already exists: {output}"
            );
            std::fs::create_dir_all(&output)?;
            let mut cfg = umbra_config(output.clone(), variant);
            if quality {
                cfg.width = 640;
                cfg.height = 360;
                cfg.image_samples = 256;
                cfg.warmup = 2;
                cfg.frames = 4;
                cfg.repeats = 1;
            }
            std::fs::write(
                Path::new(&output).join("request.json"),
                serde_json::to_vec_pretty(&cfg)?,
            )?;
            eprintln!("Umbra study: {kind}/{variant}");
            run_benchmark(&cfg)?;
        }
    }
    Ok(())
}
