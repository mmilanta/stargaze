//! Offscreen visual checks use the same shader, fonts and pipeline as the app.
use crate::ui::UiVertex;
use std::path::Path;

pub struct Preview {
    device: wgpu::Device,
    queue: wgpu::Queue,
    gui: crate::gui::Gui,
    alias: bool,
}

impl Preview {
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("UI previews require a GPU");
        eprintln!("UI previews on {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let alias = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::VIEW_FORMATS);
        let gui = crate::gui::Gui::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm);
        // Also validate the compatibility shader for native GL surfaces.
        let _compat = crate::gui::Gui::new(&device, &queue, wgpu::TextureFormat::Rgba8UnormSrgb);
        Self {
            device,
            queue,
            gui,
            alias,
        }
    }
    pub fn render(
        &mut self,
        vertices: &[UiVertex],
        w: u32,
        h: u32,
        path: Option<&Path>,
    ) -> Vec<u8> {
        let extent = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui-preview"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if self.alias {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: if self.alias {
                &[wgpu::TextureFormat::Rgba8Unorm]
            } else {
                &[]
            },
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        });
        self.gui.upload(&self.device, &self.queue, vertices);
        let pitch = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-readback"),
            size: pitch as u64 * h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui-preview"),
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
            self.gui.draw(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(h),
                },
            },
            extent,
        );
        self.queue.submit(Some(encoder.finish()));
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap()
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        recv.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range().unwrap();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in mapped.chunks(pitch as usize) {
            pixels.extend_from_slice(&row[..(w * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        assert_eq!(pixels.len(), (w * h * 4) as usize);
        if let Some(path) = path {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let file = std::fs::File::create(path).unwrap();
            let mut encoder = png::Encoder::new(file, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        pixels
    }
}
