//! Progressive GPU path tracer; no hardware RT extension is required.
#[cfg(test)]
#[path = "pathtracer_tests.rs"]
mod tests;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::renderer::{Frame, Globals, Sphere};
use crate::stars::{self, CatalogueStar};

const BODY_CAPACITY: usize = 256;
const TRACE_SHADER: &str = concat!(
    include_str!("shaders/pathtrace.wgsl"),
    "\n",
    include_str!("shaders/rings.wgsl"),
);
const HISTOGRAM_BINS: usize = 256;
const HISTOGRAM_ZEROS: [u8; HISTOGRAM_BINS * 4] = [0; HISTOGRAM_BINS * 4];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Settings {
    globals: Globals,
    counts: [u32; 4],
}

/// Display-only exposure state, shared with the shaders. `value` is eased on
/// the GPU; the rest is written from [`Options`] each frame.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Exposure {
    pub value: f32,
    pub key: f32,
    pub percentile: f32,
    pub enabled: f32,
    pub bias: f32,
}

/// The fields after `value`, so the GPU's smoothed `value` survives a write.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ExposureTail {
    key: f32,
    percentile: f32,
    enabled: f32,
    bias: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub samples_per_frame: u32,
    pub max_bounces: u32,
    /// Manual exposure, used when automatic metering is disabled.
    pub exposure: f32,
    pub auto_exposure: bool,
    /// User exposure compensation, in stops, applied to either mode.
    pub ev_bias: f32,
    /// Luminance the metered percentile is mapped to.
    pub auto_key: f32,
    /// Which centre-weighted luminance percentile to meter (0..1).
    pub auto_percentile: f32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            samples_per_frame: 1,
            max_bounces: 8,
            exposure: 1.0,
            auto_exposure: false,
            ev_bias: 0.0,
            auto_key: 0.25,
            auto_percentile: 0.9,
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
        let positive = |name: &str, default: f32| {
            std::env::var(name)
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(default)
        };
        Self {
            samples_per_frame: integer("STARGAZE_SPP", defaults.samples_per_frame),
            max_bounces: integer("STARGAZE_BOUNCES", defaults.max_bounces),
            exposure: positive("STARGAZE_EXPOSURE", defaults.exposure),
            // The application meters automatically unless explicitly disabled.
            auto_exposure: std::env::var("STARGAZE_AUTO_EXPOSURE")
                .ok()
                .map(|v| {
                    !matches!(
                        v.trim().to_ascii_lowercase().as_str(),
                        "0" | "false" | "off"
                    )
                })
                .unwrap_or(true),
            ev_bias: std::env::var("STARGAZE_EV_BIAS")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(defaults.ev_bias),
            auto_key: positive("STARGAZE_AUTO_KEY", defaults.auto_key),
            auto_percentile: positive("STARGAZE_AUTO_PERCENTILE", defaults.auto_percentile)
                .clamp(0.01, 0.999),
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
        // Exposure is display-only and deliberately does not invalidate.
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
    measure: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    exposure_layout: wgpu::BindGroupLayout,
    exposure_group: wgpu::BindGroup,
    histogram: wgpu::Buffer,
    exposure: wgpu::Buffer,
    /// Whether the GPU's eased value should be preserved between frames.
    exposure_was_enabled: bool,
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
        stars: &[CatalogueStar],
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
                buffer_entry(
                    2,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::FRAGMENT,
                ),
            ],
        });
        let exposure_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("exposure-layout"),
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
                    wgpu::BufferBindingType::Storage { read_only: false },
                    wgpu::ShaderStages::COMPUTE,
                ),
                buffer_entry(
                    3,
                    wgpu::BufferBindingType::Storage { read_only: false },
                    wgpu::ShaderStages::COMPUTE,
                ),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pathtrace"),
            source: wgpu::ShaderSource::Wgsl(TRACE_SHADER.into()),
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
        let exposure_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("exposure"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/exposure.wgsl").into()),
        });
        let exposure_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("exposure-pipeline-layout"),
                bind_group_layouts: &[Some(&exposure_layout)],
                immediate_size: 0,
            });
        let measure = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("exposure-measure-pipeline"),
            layout: Some(&exposure_pipeline_layout),
            module: &exposure_shader,
            entry_point: Some("measure"),
            compilation_options: Default::default(),
            cache: None,
        });
        let resolve = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("exposure-resolve-pipeline"),
            layout: Some(&exposure_pipeline_layout),
            module: &exposure_shader,
            entry_point: Some("resolve"),
            compilation_options: Default::default(),
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
            size: (BODY_CAPACITY * std::mem::size_of::<Sphere>()) as u64,
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
        let histogram = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("luminance-histogram"),
            size: HISTOGRAM_ZEROS.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let exposure = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("exposure-state"),
            size: std::mem::size_of::<Exposure>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let accumulation = accumulation(device, dimensions);
        let trace_group = group(
            device,
            &trace_layout,
            &[&settings, &bodies, &catalogue, &cells, &accumulation],
        );
        let display_group = group(
            device,
            &display_layout,
            &[&settings, &accumulation, &exposure],
        );
        let exposure_group = group(
            device,
            &exposure_layout,
            &[&settings, &accumulation, &histogram, &exposure],
        );
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
            measure,
            resolve,
            exposure_layout,
            exposure_group,
            histogram,
            exposure,
            exposure_was_enabled: false,
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
            &[&self.settings, &self.accumulation, &self.exposure],
        );
        self.exposure_group = group(
            device,
            &self.exposure_layout,
            &[
                &self.settings,
                &self.accumulation,
                &self.histogram,
                &self.exposure,
            ],
        );
        self.history = History::default();
    }
    pub fn samples(&self) -> u32 {
        self.history.samples
    }

    /// Switch between automatic metering and the manual exposure, and set the
    /// exposure-compensation bias in stops. Display-only: never invalidates.
    pub fn set_exposure_controls(&mut self, auto: bool, bias: f32) {
        self.options.auto_exposure = auto;
        self.options.ev_bias = bias;
    }
    pub fn auto_exposure(&self) -> bool {
        self.options.auto_exposure
    }
    pub fn ev_bias(&self) -> f32 {
        self.options.ev_bias
    }
    pub fn manual_exposure(&self) -> f32 {
        self.options.exposure
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
        // Publish the exposure controls. When automatic metering has been on,
        // overwrite only the tail so the GPU's eased `value` is preserved.
        let tail = ExposureTail {
            key: self.options.auto_key,
            percentile: self.options.auto_percentile,
            enabled: if self.options.auto_exposure { 1.0 } else { 0.0 },
            bias: self.options.ev_bias,
        };
        if self.options.auto_exposure && self.exposure_was_enabled {
            queue.write_buffer(&self.exposure, 4, bytemuck::bytes_of(&tail));
        } else {
            queue.write_buffer(
                &self.exposure,
                0,
                bytemuck::bytes_of(&Exposure {
                    value: self.options.exposure,
                    key: tail.key,
                    percentile: tail.percentile,
                    enabled: tail.enabled,
                    bias: tail.bias,
                }),
            );
        }
        self.exposure_was_enabled = self.options.auto_exposure;
        if count > 0 {
            if !frame.bodies.is_empty() {
                queue.write_buffer(&self.bodies, 0, bytemuck::cast_slice(&frame.bodies));
            }
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
        if self.options.auto_exposure {
            // Clear, accumulate the centre-weighted histogram, then resolve it
            // to an eased exposure, even after accumulation reaches its limit.
            queue.write_buffer(&self.histogram, 0, &HISTOGRAM_ZEROS);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("exposure-measure"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.measure);
            pass.set_bind_group(0, &self.exposure_group, &[]);
            pass.dispatch_workgroups(
                self.dimensions.0.div_ceil(8),
                self.dimensions.1.div_ceil(8),
                1,
            );
            drop(pass);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("exposure-resolve"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.resolve);
            pass.set_bind_group(0, &self.exposure_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        self.history.samples += count;
    }
    pub fn display(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.display);
        pass.set_bind_group(0, &self.display_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
