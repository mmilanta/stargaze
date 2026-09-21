//! Progressive GPU path tracer; no hardware RT extension is required.
#[cfg(test)]
#[path = "pathtracer_tests.rs"]
mod tests;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::renderer::{BodyInstance, Frame, Globals};
use crate::stars::{self, StarInstance};

const BODY_CAPACITY: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Settings {
    globals: Globals,
    counts: [u32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub samples_per_frame: u32,
    pub max_bounces: u32,
    pub exposure: f32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            samples_per_frame: 1,
            max_bounces: 8,
            exposure: 1.0,
        }
    }
}
impl Options {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        let integer = |name: &str, default| {
            std::env::var(name)
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(default)
                .clamp(1, 64)
        };
        Self {
            samples_per_frame: integer("STARGAZE_SPP", defaults.samples_per_frame),
            max_bounces: integer("STARGAZE_BOUNCES", defaults.max_bounces),
            exposure: std::env::var("STARGAZE_EXPOSURE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(defaults.exposure),
        }
    }
}

/// UI changes deliberately do not invalidate the accumulation.
#[derive(Default)]
struct History {
    key: Vec<u8>,
    samples: u32,
}
impl History {
    fn update(&mut self, frame: &Frame, options: Options) {
        let mut key = bytemuck::bytes_of(&frame.globals).to_vec();
        key.extend_from_slice(bytemuck::cast_slice(&frame.bodies));
        // Time is kept in f64: even changes smaller than the GPU's position
        // precision must invalidate history when the simulation is running.
        key.extend_from_slice(&frame.scene_time.to_le_bytes());
        key.extend_from_slice(&options.max_bounces.to_le_bytes());
        key.extend_from_slice(&options.exposure.to_le_bytes());
        if key != self.key {
            self.key = key;
            self.samples = 0;
        }
    }
}

pub struct PathTracer {
    compute: wgpu::ComputePipeline,
    display: wgpu::RenderPipeline,
    trace_layout: wgpu::BindGroupLayout,
    display_layout: wgpu::BindGroupLayout,
    trace_group: wgpu::BindGroup,
    display_group: wgpu::BindGroup,
    settings: wgpu::Buffer,
    bodies: wgpu::Buffer,
    catalogue: wgpu::Buffer,
    cells: wgpu::Buffer,
    accumulation: wgpu::Buffer,
    dimensions: (u32, u32),
    history: History,
    options: Options,
}

fn buffer_entry(
    binding: u32,
    ty: wgpu::BufferBindingType,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: &[&wgpu::Buffer],
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pathtracer-bind-group"),
        layout,
        entries: &buffers
            .iter()
            .enumerate()
            .map(|(i, buffer)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: buffer.as_entire_binding(),
            })
            .collect::<Vec<_>>(),
    })
}
fn accumulation(device: &wgpu::Device, dimensions: (u32, u32)) -> wgpu::Buffer {
    let bytes = dimensions.0 as u64 * dimensions.1 as u64 * 16;
    assert!(
        bytes <= device.limits().max_storage_buffer_binding_size,
        "window is too large for the GPU accumulation buffer"
    );
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("linear-hdr-accumulation"),
        size: bytes.max(16),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

impl PathTracer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        dimensions: (u32, u32),
        stars: &[StarInstance],
        options: Options,
    ) -> Self {
        log::info!(
            "path tracer: {} spp/frame, {} surface vertices, exposure {}",
            options.samples_per_frame,
            options.max_bounces,
            options.exposure
        );
        let trace_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pathtracer-layout"),
            entries: &[
                buffer_entry(
                    0,
                    wgpu::BufferBindingType::Uniform,
                    wgpu::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    1,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    2,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    3,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    4,
                    wgpu::BufferBindingType::Storage { read_only: false },
                    wgpu::ShaderStages::COMPUTE,
                ),
            ],
        });
        let display_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("display-layout"),
            entries: &[
                buffer_entry(
                    0,
                    wgpu::BufferBindingType::Uniform,
                    wgpu::ShaderStages::FRAGMENT,
                ),
                buffer_entry(
                    1,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::FRAGMENT,
                ),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pathtrace"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/pathtrace.wgsl").into()),
        });
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pathtrace-pipeline-layout"),
            bind_group_layouts: &[Some(&trace_layout)],
            immediate_size: 0,
        });
        let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pathtrace-pipeline"),
            layout: Some(&compute_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let display_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("display"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/display.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("display-pipeline-layout"),
            bind_group_layouts: &[Some(&display_layout)],
            immediate_size: 0,
        });
        let display = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("display-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &display_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &display_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let settings = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pathtrace-settings"),
            size: std::mem::size_of::<Settings>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bodies = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("analytic-spheres"),
            size: (BODY_CAPACITY * std::mem::size_of::<BodyInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (catalogue_data, cells_data) = stars::ray_catalogue(stars);
        let catalogue = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("angular-star-discs"),
            contents: bytemuck::cast_slice(&catalogue_data),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let cells = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("star-direction-grid"),
            contents: bytemuck::cast_slice(&cells_data),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let accumulation = accumulation(device, dimensions);
        let trace_group = group(
            device,
            &trace_layout,
            &[&settings, &bodies, &catalogue, &cells, &accumulation],
        );
        let display_group = group(device, &display_layout, &[&settings, &accumulation]);
        Self {
            compute,
            display,
            trace_layout,
            display_layout,
            trace_group,
            display_group,
            settings,
            bodies,
            catalogue,
            cells,
            accumulation,
            dimensions,
            history: History::default(),
            options,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, dimensions: (u32, u32)) {
        self.dimensions = dimensions;
        self.accumulation = accumulation(device, dimensions);
        self.trace_group = group(
            device,
            &self.trace_layout,
            &[
                &self.settings,
                &self.bodies,
                &self.catalogue,
                &self.cells,
                &self.accumulation,
            ],
        );
        self.display_group = group(
            device,
            &self.display_layout,
            &[&self.settings, &self.accumulation],
        );
        self.history = History::default();
    }
    pub fn samples(&self) -> u32 {
        self.history.samples
    }

    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Frame,
    ) {
        assert!(frame.bodies.len() <= BODY_CAPACITY, "too many scene bodies");
        self.history.update(frame, self.options);
        // Avoid counter wrap and loss of integer resolution in f32 weights.
        let count = self
            .options
            .samples_per_frame
            .min(16_777_216 - self.history.samples);
        let mut globals = frame.globals;
        globals.viewport[2] = self.options.exposure;
        let settings = Settings {
            globals,
            counts: [
                frame.bodies.len() as u32,
                self.history.samples,
                count,
                self.options.max_bounces,
            ],
        };
        queue.write_buffer(&self.settings, 0, bytemuck::bytes_of(&settings));
        if count == 0 {
            return;
        }
        if !frame.bodies.is_empty() {
            queue.write_buffer(&self.bodies, 0, bytemuck::cast_slice(&frame.bodies));
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("pathtrace"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute);
            pass.set_bind_group(0, &self.trace_group, &[]);
            pass.dispatch_workgroups(
                self.dimensions.0.div_ceil(8),
                self.dimensions.1.div_ceil(8),
                1,
            );
        }
        self.history.samples += count;
    }
    pub fn display(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.display);
        pass.set_bind_group(0, &self.display_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
