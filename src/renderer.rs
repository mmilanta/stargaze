//! Presentation and HUD around the progressive, all-sphere path tracer.
use std::sync::Arc;

use crate::pathtracer::{Options, PathTracer};
use crate::stars::CatalogueStar;
use crate::ui::{self, Label, UiVertex};
use anyhow::{Result, anyhow};
use bytemuck::{Pod, Zeroable};
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Globals {
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub cam_forward: [f32; 4], // w = tan(fov_y / 2)
    pub viewport: [f32; 4],    // width, height, exposure, unused
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Sphere {
    pub center: [f32; 3], // telescope space, looking along -Z
    pub radius: f32,
    pub color: [f32; 3], // reflectance or emitted radiance
    pub emissive: f32,
    pub center_low: [f32; 4], // xyz = residual centre, w = procedural albedo strength
}

pub struct Frame {
    pub globals: Globals,
    pub bodies: Vec<Sphere>,
    pub scene_time: f64,
    pub labels: Vec<Label>,
    pub ui: Vec<UiVertex>,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    tracer: PathTracer,
    ui_pipeline: wgpu::RenderPipeline,
    ui_bind_group: wgpu::BindGroup,
    ui_vbo: wgpu::Buffer,
    ui_capacity: u64,
}

impl Renderer {
    pub fn new(window: Arc<Window>, stars: &[CatalogueStar]) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;
        // Large/Retina windows need more than the default storage-buffer limit.
        let supported = adapter.limits();
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: supported.max_storage_buffer_binding_size,
            max_buffer_size: supported.max_buffer_size,
            ..Default::default()
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("stargaze-device"),
                required_limits: limits,
                ..Default::default()
            }))?;
        log::info!("GPU: {:?}", adapter.get_info());
        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .ok_or_else(|| anyhow!("no sRGB surface format available"))?;
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| anyhow!("surface is not supported by the adapter"))?;
        config.format = format;
        surface.configure(&device, &config);
        let tracer = PathTracer::new(&device, format, (width, height), stars, Options::from_env());

        let atlas_extent = wgpu::Extent3d {
            width: ui::ATLAS_W as u32,
            height: ui::ATLAS_H as u32,
            depth_or_array_layers: 1,
        };
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui-atlas"),
            size: atlas_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &ui::build_atlas(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ui::ATLAS_W as u32),
                rows_per_image: Some(ui::ATLAS_H as u32),
            },
            atlas_extent,
        );
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let ui_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui-bg"),
            layout: &ui_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui-pipeline-layout"),
            bind_group_layouts: &[Some(&ui_bgl)],
            immediate_size: 0,
        });
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });
        let ui_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        let ui_vlayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ui_attrs,
        };
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui-pipeline"),
            layout: Some(&ui_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(ui_vlayout)],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let ui_capacity = 64 * 1024;
        let ui_vbo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-vertices"),
            size: ui_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
            tracer,
            ui_pipeline,
            ui_bind_group,
            ui_vbo,
            ui_capacity,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.tracer.resize(&self.device, self.size);
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn samples(&self) -> u32 {
        self.tracer.samples()
    }

    pub fn render(&mut self, frame: &Frame) {
        let surface_tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error while acquiring frame");
                return;
            }
        };
        if !frame.ui.is_empty() {
            let bytes = bytemuck::cast_slice(&frame.ui);
            if bytes.len() as u64 > self.ui_capacity {
                self.ui_capacity = (bytes.len() as u64).next_power_of_two();
                self.ui_vbo = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ui-vertices"),
                    size: self.ui_capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue.write_buffer(&self.ui_vbo, 0, bytes);
        }
        let view = surface_tex.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        self.tracer.encode(&self.queue, &mut encoder, frame);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("display-and-hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.tracer.display(&mut pass);
            if !frame.ui.is_empty() {
                pass.set_pipeline(&self.ui_pipeline);
                pass.set_bind_group(0, &self.ui_bind_group, &[]);
                pass.set_vertex_buffer(0, self.ui_vbo.slice(..));
                pass.draw(0..frame.ui.len() as u32, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(surface_tex);
    }
}
