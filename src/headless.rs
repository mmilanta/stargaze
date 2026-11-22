//! Versioned command-line interface for deterministic offscreen renders.
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::graphics::Settings;
use crate::pathtracer::{Options, PathTracer};

const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Eco,
    Balanced,
    High,
}

impl Mode {
    fn settings(self) -> Settings {
        match self {
            Self::Eco => Settings::PRESETS[0],
            Self::Balanced => Settings::PRESETS[1],
            Self::High => Settings::PRESETS[2],
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    scene: PathBuf,
    mode: Mode,
    width: u32,
    height: u32,
    samples: u32,
    exposure: f32,
    #[serde(default)]
    adapter: String,
}

#[derive(Serialize)]
struct Capture {
    version: u32,
    mode: Mode,
    width: u32,
    height: u32,
    trace_width: u32,
    trace_height: u32,
    samples: u32,
    exposure: f32,
    render_ms: f64,
    preset: Settings,
    gpu: String,
    backend: String,
    driver: String,
    driver_info: String,
}

pub fn run(mut args: impl Iterator<Item = OsString>) -> Result<()> {
    let mut request = None;
    let mut output = None;
    while let Some(arg) = args.next() {
        let slot = match arg.to_str() {
            Some("--request") => &mut request,
            Some("--output") => &mut output,
            Some("--help" | "-h") => {
                println!("Usage: stargaze render --request FILE --output PNG");
                return Ok(());
            }
            _ => bail!("unknown render argument: {}", arg.to_string_lossy()),
        };
        ensure!(
            slot.is_none(),
            "duplicate render argument: {}",
            arg.to_string_lossy()
        );
        *slot = Some(PathBuf::from(
            args.next().context("missing render argument value")?,
        ));
    }
    let request_path = request.context("render requires --request FILE")?;
    let output = output.context("render requires --output PNG")?;
    let request: Request = serde_json::from_reader(File::open(&request_path)?)
        .with_context(|| format!("reading {}", request_path.display()))?;
    ensure!(
        request.version == 1,
        "unsupported render request version {}",
        request.version
    );
    ensure!(
        (16..=3840).contains(&request.width) && (16..=2160).contains(&request.height),
        "render dimensions must be 16–3840 wide and 16–2160 high"
    );
    ensure!(
        (1..=4096).contains(&request.samples),
        "samples must be 1–4096"
    );
    ensure!(
        request.exposure.is_finite() && request.exposure > 0.0,
        "exposure must be finite and positive"
    );
    let scene_path = request_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&request.scene);
    let scene = crate::config::load(&scene_path)?;
    ensure!(
        scene.initial_time_days.is_some(),
        "render scenes require an explicit time_days"
    );
    let state = crate::State::with_scene_settings(scene, false);
    let capture = render(request, &state, &output)?;
    println!("{}", serde_json::to_string(&capture)?);
    Ok(())
}

fn render(request: Request, state: &crate::State, output: &Path) -> Result<Capture> {
    let preset = request.mode.settings();
    let frame =
        state.build_frame_with_vegetation(request.width, request.height, preset.vegetation_percent);
    let trace_size = preset.render_size((request.width, request.height));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapter_filter = request.adapter.to_lowercase();
    let adapter = adapters
        .into_iter()
        .filter(|adapter| {
            let info = adapter.get_info();
            info.device_type != wgpu::DeviceType::Cpu
                && info.name.to_lowercase().contains(&adapter_filter)
        })
        .min_by_key(|adapter| adapter.get_info().device_type != wgpu::DeviceType::DiscreteGpu)
        .context("no matching hardware GPU")?;
    let info = adapter.get_info();
    let limits = wgpu::Limits {
        max_storage_buffer_binding_size: adapter.limits().max_storage_buffer_binding_size,
        max_buffer_size: adapter.limits().max_buffer_size,
        ..Default::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Stargaze headless render"),
        required_limits: limits,
        ..Default::default()
    }))?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless output"),
        size: wgpu::Extent3d {
            width: request.width,
            height: request.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let stars = crate::stars::generate(3500, 2500, 0x5EED_1234);
    let mut tracer = PathTracer::new(
        &device,
        OUTPUT_FORMAT,
        trace_size,
        &stars,
        Options {
            samples_per_frame: preset.samples_per_frame,
            max_bounces: preset.max_bounces,
            sample_limit: request.samples,
            exposure: request.exposure,
            ..Options::default()
        },
    );
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let start = Instant::now();
    while tracer.samples() < request.samples {
        let mut encoder = device.create_command_encoder(&Default::default());
        tracer.encode(&queue, &mut encoder, &frame);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless display"),
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
        queue.submit(Some(encoder.finish()));
        device.poll(wgpu::PollType::wait_indefinitely())?;
    }
    let render_ms = start.elapsed().as_secs_f64() * 1000.0;
    let pixels = read_display(&device, &queue, &texture)?;
    let mut png = png::Encoder::new(File::create(output)?, request.width, request.height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    png.write_header()?.write_image_data(&pixels)?;
    Ok(Capture {
        version: 1,
        mode: request.mode,
        width: request.width,
        height: request.height,
        trace_width: trace_size.0,
        trace_height: trace_size.1,
        samples: tracer.samples(),
        exposure: request.exposure,
        render_ms,
        preset,
        gpu: info.name,
        backend: format!("{:?}", info.backend),
        driver: info.driver,
        driver_info: info.driver_info,
    })
}

pub(crate) fn map(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Result<Vec<u8>> {
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

pub(crate) fn read_display(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Vec<u8>> {
    let width = texture.width();
    let pitch = (width * 4).div_ceil(256) * 256;
    let rgba = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("display-readback"),
        size: u64::from(pitch) * u64::from(texture.height()),
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
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    queue.submit(Some(encoder.finish()));
    let padded = map(device, &rgba)?;
    Ok(padded
        .chunks(pitch as usize)
        .flat_map(|r| r[..width as usize * 4].iter().copied())
        .collect())
}
