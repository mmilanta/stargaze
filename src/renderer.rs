//! wgpu renderer: sky, procedural stars, and camera-relative sphere bodies.

use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::stars::StarInstance;
use crate::ui::{self, Label, UiVertex};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const BODY_CAPACITY: u64 = 256;
/// Maximum number of light-emitting stars.
pub const MAX_STARS: usize = 4;
/// Maximum number of bodies that can cast shadows onto others.
pub const MAX_OCCLUDERS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Globals {
    /// Column-major, reverse-Z infinite far projection.
    pub view_proj: [f32; 16],
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub cam_forward: [f32; 4], // w = tan(fov_y / 2)
    pub cam_zenith: [f32; 4],
    /// Light sources: xyz = camera-relative position, w = radius.
    pub stars: [[f32; 4]; MAX_STARS],
    /// rgb = colour, a = relative luminosity.
    pub star_color: [[f32; 4]; MAX_STARS],
    /// Shadow casters: xyz = camera-relative centre, w = radius.
    pub occluders: [[f32; 4]; MAX_OCCLUDERS],
    pub star_meta: [f32; 4],     // x = number of stars
    pub occluder_meta: [f32; 4], // x = number of occluders
    pub params: [f32; 4],        // x = aspect, y = ambient, z = night, w = sun light
    pub viewport: [f32; 4],      // x = width, y = height, z = exposure
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BodyInstance {
    pub center: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
    pub emissive: f32,
    pub intensity: f32,
}

pub struct Frame {
    pub globals: Globals,
    pub bodies: Vec<BodyInstance>,
    pub labels: Vec<Label>,
    pub ui: Vec<UiVertex>,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    depth_view: wgpu::TextureView,
    globals_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    sky_pipeline: wgpu::RenderPipeline,
    star_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    mesh_vertices: wgpu::Buffer,
    mesh_indices: wgpu::Buffer,
    mesh_index_count: u32,
    body_buf: wgpu::Buffer,
    star_buf: wgpu::Buffer,
    star_count: u32,
    ui_pipeline: wgpu::RenderPipeline,
    ui_bind_group: wgpu::BindGroup,
    ui_vbo: wgpu::Buffer,
    ui_capacity: u64,
}

fn make_sphere(stacks: u32, sectors: u32) -> (Vec<f32>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    for i in 0..=stacks {
        let phi = std::f32::consts::PI * (i as f32 / stacks as f32);
        let (sp, cp) = phi.sin_cos();
        for j in 0..=sectors {
            let theta = std::f32::consts::TAU * (j as f32 / sectors as f32);
            let (st, ct) = theta.sin_cos();
            let n = [sp * ct, cp, sp * st];
            verts.extend_from_slice(&n);
            verts.extend_from_slice(&n);
        }
    }
    let row = sectors + 1;
    for i in 0..stacks {
        for j in 0..sectors {
            let a = i * row + j;
            let b = a + row;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    (verts, idx)
}

fn create_depth(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

impl Renderer {
    pub fn new(window: Arc<Window>, stars: &[StarInstance]) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("stargaze-device"),
            ..Default::default()
        }))?;

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

        let depth_view = create_depth(&device, &config);

        // ---- shaders ----
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let star_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("star"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/star.wgsl").into()),
        });
        let body_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("body"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/body.wgsl").into()),
        });

        // ---- globals binding ----
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        // ---- vertex layouts ----
        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let mesh_layout = wgpu::VertexBufferLayout {
            array_stride: 24,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &mesh_attrs,
        };
        let body_attrs = wgpu::vertex_attr_array![
            2 => Float32x3, 3 => Float32, 4 => Float32x3, 5 => Float32, 6 => Float32
        ];
        let body_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BodyInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &body_attrs,
        };
        let star_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32, 3 => Float32
        ];
        let star_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StarInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &star_attrs,
        };

        let target = [Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];

        // ---- pipelines ----
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &sky_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &target,
            }),
            multiview_mask: None,
            cache: None,
        });

        let star_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ADDITIVE),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let star_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("star-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &star_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(star_layout)],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &star_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &star_target,
            }),
            multiview_mask: None,
            cache: None,
        });

        let body_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("body-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &body_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(mesh_layout), Some(body_layout)],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &body_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &target,
            }),
            multiview_mask: None,
            cache: None,
        });

        // ---- UI overlay ----
        let atlas_bytes = ui::build_atlas();
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
            &atlas_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ui::ATLAS_W as u32),
                rows_per_image: Some(ui::ATLAS_H as u32),
            },
            atlas_extent,
        );
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
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
        let ui_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        })];
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
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &ui_target,
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

        // ---- geometry ----
        let (verts, indices) = make_sphere(24, 48);
        let mesh_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere-vertices"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mesh_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere-indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let body_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("body-instances"),
            size: BODY_CAPACITY * std::mem::size_of::<BodyInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let star_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("star-instances"),
            contents: bytemuck::cast_slice(stars),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
            depth_view,
            globals_buf,
            bind_group,
            sky_pipeline,
            star_pipeline,
            body_pipeline,
            mesh_vertices,
            mesh_indices,
            mesh_index_count: indices.len() as u32,
            body_buf,
            star_buf,
            star_count: stars.len() as u32,
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
        self.depth_view = create_depth(&self.device, &self.config);
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn render(&mut self, frame: &Frame) {
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&frame.globals));
        if !frame.bodies.is_empty() {
            self.queue
                .write_buffer(&self.body_buf, 0, bytemuck::cast_slice(&frame.bodies));
        }
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

        let surface_tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error while acquiring frame");
                return;
            }
        };

        let view = surface_tex.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_bind_group(0, &self.bind_group, &[]);

            pass.set_pipeline(&self.sky_pipeline);
            pass.draw(0..3, 0..1);

            if self.star_count > 0 {
                pass.set_pipeline(&self.star_pipeline);
                pass.set_vertex_buffer(0, self.star_buf.slice(..));
                pass.draw(0..6, 0..self.star_count);
            }

            if !frame.bodies.is_empty() {
                pass.set_pipeline(&self.body_pipeline);
                pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
                pass.set_vertex_buffer(1, self.body_buf.slice(..));
                pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.mesh_index_count, 0, 0..frame.bodies.len() as u32);
            }

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
