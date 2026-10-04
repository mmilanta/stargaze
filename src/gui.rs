//! Shared native UI pipeline, used by the window and offscreen visual checks.
use crate::typeface;
use crate::ui::{self, UiVertex};

pub struct Gui {
    atlas: wgpu::Texture,
    atlas_revision: u64,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vbo: wgpu::Buffer,
    capacity: u64,
    count: u32,
}

impl Gui {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
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
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
        let ui_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4];
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
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[(
                        "display_composition",
                        if format.is_srgb() { 0.0 } else { 1.0 },
                    )],
                    ..Default::default()
                },
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
        let mut gui = Self {
            atlas: atlas_tex,
            atlas_revision: 0,
            pipeline: ui_pipeline,
            bind_group: ui_bind_group,
            vbo: ui_vbo,
            capacity: ui_capacity,
            count: 0,
        };
        gui.sync_atlas(queue);
        gui
    }
    fn sync_atlas(&mut self, queue: &wgpu::Queue) {
        let Some(snapshot) = typeface::snapshot(self.atlas_revision) else {
            return;
        };
        queue.write_texture(
            self.atlas.as_image_copy(),
            &snapshot.bitmap,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ui::ATLAS_W as u32),
                rows_per_image: Some(ui::ATLAS_H as u32),
            },
            self.atlas.size(),
        );
        self.atlas_revision = snapshot.revision;
    }
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, vertices: &[UiVertex]) {
        self.count = vertices.len() as u32;
        if vertices.is_empty() {
            return;
        }
        self.sync_atlas(queue);
        let bytes = bytemuck::cast_slice(vertices);
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.vbo = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui-vertices"),
                size: self.capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vbo, 0, bytes);
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vbo.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}
