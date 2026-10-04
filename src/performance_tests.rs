//! Repeatable metering benchmark and exact histogram regression checks.
//! GPU timing here includes submission/poll overhead; it is not application FPS.
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

const CURRENT: &str = include_str!("shaders/exposure.wgsl");
const LEGACY_MEASURE: &str = r#"
@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    let dimensions = vec2<u32>(settings.g.viewport.xy);
    if (id.x >= dimensions.x || id.y >= dimensions.y) { return; }
    let x0 = dimensions.x / 4u;
    let y0 = dimensions.y / 4u;
    if (id.x < x0 || id.x >= dimensions.x - x0 || id.y < y0 || id.y >= dimensions.y - y0) { return; }
    let hdr = accumulation[id.y * dimensions.x + id.x];
    @LUMINANCE@
    var safe = luminance;
    if (!(safe > 0.0)) { safe = exp2(LOG_MIN); }
    safe = clamp(safe, exp2(LOG_MIN), exp2(LOG_MAX));
    let t = (log2(safe) - LOG_MIN) / (LOG_MAX - LOG_MIN);
    let bin = u32(clamp(t * f32(BINS), 0.0, f32(BINS - 1u)));
    atomicAdd(&histogram[bin], 1u);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    g: crate::renderer::Globals,
    counts: [u32; 4],
    background: [f32; 4],
}

struct Meter {
    group: wgpu::BindGroup,
    histogram: wgpu::Buffer,
    current: wgpu::ComputePipeline,
    legacy: wgpu::ComputePipeline,
    size: (u32, u32),
}
impl Meter {
    fn new(device: &wgpu::Device, size: (u32, u32), hdr: &[[f32; 4]]) -> Self {
        let mut uniforms = Uniforms::zeroed();
        uniforms.g.viewport = [size.0 as f32, size.1 as f32, 1.0, 1.0];
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("meter-settings"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let pixels = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("meter-hdr"),
            contents: bytemuck::cast_slice(hdr),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let histogram = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("meter-histogram"),
            size: 256 * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let exposure = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("meter-exposure"),
            size: 32,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("meter-layout"),
            entries: &(0..4)
                .map(|binding| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: if binding == 0 {
                            wgpu::BufferBindingType::Uniform
                        } else {
                            wgpu::BufferBindingType::Storage {
                                read_only: binding == 1,
                            }
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                })
                .collect::<Vec<_>>(),
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("meter-group"),
            layout: &layout,
            entries: &[&uniform, &pixels, &histogram, &exposure]
                .iter()
                .enumerate()
                .map(|(binding, buffer)| wgpu::BindGroupEntry {
                    binding: binding as u32,
                    resource: buffer.as_entire_binding(),
                })
                .collect::<Vec<_>>(),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("meter-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |source: &str| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("meter-shader"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("meter-pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("measure"),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        // Keep the original full-frame dispatch as an independent GPU oracle.
        let header = CURRENT.split("@compute").next().unwrap();
        let luminance = CURRENT
            .lines()
            .find(|line| line.contains("let luminance ="))
            .unwrap();
        let legacy_source = format!(
            "{header}\n{}",
            LEGACY_MEASURE.replace("@LUMINANCE@", luminance)
        );
        Self {
            group,
            histogram,
            current: make(CURRENT),
            legacy: make(&legacy_source),
            size,
        }
    }

    fn dispatch(&self, device: &wgpu::Device, queue: &wgpu::Queue, legacy: bool) {
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&self.histogram, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(if legacy { &self.legacy } else { &self.current });
            pass.set_bind_group(0, &self.group, &[]);
            let size = if legacy {
                self.size
            } else {
                crate::pathtracer::exposure_meter_size(self.size)
            };
            pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }

    fn histogram(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u32> {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("meter-readback"),
            size: 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.histogram, 0, &buffer, 0, 256 * 4);
        queue.submit(Some(encoder.finish()));
        let (send, receive) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range().unwrap();
        let values = bytemuck::cast_slice::<_, u32>(&mapped).to_vec();
        drop(mapped);
        buffer.unmap();
        values
    }
}

fn gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    eprintln!("Meter benchmark on {:?}", adapter.get_info());
    pollster::block_on(adapter.request_device(&Default::default())).unwrap()
}

#[test]
fn central_meter_dimensions_match_every_pixel_of_the_original_region() {
    for w in 1..130 {
        for h in 1..80 {
            let actual = crate::pathtracer::exposure_meter_size((w, h));
            let expected = (
                (0..w).filter(|&x| x >= w / 4 && x < w - w / 4).count() as u32,
                (0..h).filter(|&y| y >= h / 4 && y < h - h / 4).count() as u32,
            );
            assert_eq!(actual, expected);
        }
    }
    assert_eq!(
        crate::pathtracer::exposure_meter_size((3840, 2160)),
        (1920, 1080)
    );
}

#[test]
#[ignore = "requires a GPU; compares the optimized histogram with the original shader"]
fn gpu_cropped_meter_matches_full_frame_histogram() {
    let (device, queue) = gpu();
    for size in [
        (1, 1),
        (2, 3),
        (5, 7),
        (16, 16),
        (17, 19),
        (31, 33),
        (129, 71),
        (513, 287),
    ] {
        let hdr = (0..size.0 * size.1)
            .map(|i| match i % 17 {
                0 => [0.0; 4],
                1 => [f32::NAN, 0.0, 0.0, 0.0],
                2 => [f32::INFINITY, 0.0, 0.0, 0.0],
                3 => [-1.0, -1.0, -1.0, 0.0],
                4 => [8.0, 8.0, 8.0, 8.0],
                _ => {
                    let x = 2.0f32.powf((i % 80) as f32 * 0.4 - 16.1);
                    [x, x * 0.7, x * 0.3, 0.0]
                }
            })
            .collect::<Vec<_>>();
        let meter = Meter::new(&device, size, &hdr);
        meter.dispatch(&device, &queue, true);
        let reference = meter.histogram(&device, &queue);
        meter.dispatch(&device, &queue, false);
        let actual = meter.histogram(&device, &queue);
        assert_eq!(actual, reference, "meter changed for {size:?}");
        let extent = crate::pathtracer::exposure_meter_size(size);
        assert_eq!(actual.iter().sum::<u32>(), extent.0 * extent.1);
    }
}

#[test]
#[ignore = "benchmark: use --release --ignored --nocapture --test-threads=1"]
fn profile_exposure_meter() {
    let (device, queue) = gpu();
    for size in [(1920, 1080), (3840, 2160)] {
        for pattern in ["uniform", "wide-range"] {
            let hdr = (0..size.0 * size.1)
                .map(|i| {
                    let luminance = if pattern == "uniform" {
                        0.25
                    } else {
                        2.0f32.powf((i % 256) as f32 * 48.0 / 256.0 - 24.0)
                    };
                    [luminance, luminance, luminance, 0.0]
                })
                .collect::<Vec<_>>();
            let meter = Meter::new(&device, size, &hdr);
            for _ in 0..3 {
                meter.dispatch(&device, &queue, true);
                meter.dispatch(&device, &queue, false);
            }
            let mut legacy = Vec::new();
            let mut current = Vec::new();
            for i in 0..12 {
                for old in if i % 2 == 0 {
                    [true, false]
                } else {
                    [false, true]
                } {
                    let start = std::time::Instant::now();
                    meter.dispatch(&device, &queue, old);
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    if old {
                        legacy.push(ms);
                    } else {
                        current.push(ms);
                    }
                }
            }
            legacy.sort_by(f64::total_cmp);
            current.sort_by(f64::total_cmp);
            let original = (legacy[5] + legacy[6]) * 0.5;
            let cropped = (current[5] + current[6]) * 0.5;
            eprintln!(
                "Meter {size:?} {pattern}: original {:.3}ms, cropped {:.3}ms, speedup {:.2}x (median submission+completion)",
                original,
                cropped,
                original / cropped
            );
        }
    }
}
