//! Presentation and HUD around the progressive analytic path tracer.
use std::sync::Arc;
use std::time::Instant;

use crate::graphics;
use crate::pathtracer::{Options, PathTracer};
use crate::stars::CatalogueStar;
use crate::ui::{Label, UiVertex};
use anyhow::{Result, anyhow};
use bytemuck::{Pod, Zeroable};
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Globals {
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub cam_forward: [f32; 4], // w = tan(fov_y / 2)
    pub viewport: [f32; 4],    // trace width, height, exposure, display aspect
    // Host-atmosphere shell, in telescope space.
    pub atmo_center: [f32; 4],   // xyz = host centre, w = host radius
    pub atmo_rayleigh: [f32; 4], // rgb = Rayleigh coefficients (1/AU), w = Mie
    // x = Mie g, y = scale height (AU), z = top altitude (AU),
    // w = GPU host body index + 1 (0 disables atmosphere)
    pub atmo_params: [f32; 4],
    // Local landscape frame in telescope space; lengths in metres.
    pub ground_east: [f32; 4],   // w = observer height
    pub ground_up: [f32; 4],     // w = host radius
    pub ground_north: [f32; 4],  // w = host GPU index + 1 (0 disables)
    pub ground_counts: [u32; 4], // x = local primitive count
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Sphere {
    pub center: [f32; 3], // telescope space, looking along -Z
    pub radius: f32,
    pub color: [f32; 3], // reflectance or emitted radiance
    pub emissive: f32,
    pub center_low: [f32; 4], // xyz = residual centre, w = procedural albedo strength
    pub ring_plane: [f32; 4], // xyz = unit pole in telescope space, w = inner radius / body radius
    pub ring_params: [f32; 4], // outer radius / body radius (0 = absent), optical depth, banded, unused
    pub ring_color: [f32; 4],  // rgb = single-scattering albedo
}

pub struct Frame {
    pub globals: Globals,
    pub bodies: Vec<Sphere>,
    pub ground: Vec<crate::ground::Primitive>,
    pub scene_time: f64,
    pub labels: Vec<Label>,
}

pub enum RenderOutcome {
    Presented,
    Retry,
    Unavailable,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    tracer: PathTracer,
    graphics: graphics::Settings,
    gui: crate::gui::Gui,
    work: crate::render_work::Work,
}

impl Renderer {
    pub fn new(
        window: Arc<Window>,
        stars: &[CatalogueStar],
        graphics: graphics::Settings,
    ) -> Result<Self> {
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
        // The world is linear light; Blueprint's CSS colors and translucent
        // overlays compose in display space. Both views share the same image.
        if adapter.get_downlevel_capabilities().flags.contains(
            wgpu::DownlevelFlags::VIEW_FORMATS | wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS,
        ) {
            config.view_formats = vec![format.remove_srgb_suffix()];
        }
        surface.configure(&device, &config);
        let mut options = Options::from_env();
        options.samples_per_frame = graphics.samples_per_frame;
        options.max_bounces = graphics.max_bounces;
        options.sample_limit = graphics.sample_limit();
        let tracer = PathTracer::new(
            &device,
            format,
            graphics.render_size((width, height)),
            stars,
            options,
        );

        let ui_format = config.view_formats.first().copied().unwrap_or(format);
        let gui = crate::gui::Gui::new(&device, &queue, ui_format);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
            tracer,
            graphics,
            gui,
            work: crate::render_work::Work::default(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        if width == 0 || height == 0 {
            return;
        }
        if (self.config.width, self.config.height) == self.size {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn samples(&self) -> u32 {
        self.tracer.samples()
    }
    pub fn needs_redraw(&self) -> bool {
        self.graphics.render_size(self.size) != self.tracer.size() || self.tracer.needs_redraw()
    }
    pub fn poll_work(&mut self) -> bool {
        self.work.ready(&self.device)
    }
    pub fn set_graphics(&mut self, settings: graphics::Settings) {
        self.graphics = settings.normalized();
        self.work.reset_budget();
        self.tracer.set_quality(
            settings.samples_per_frame,
            settings.max_bounces,
            settings.sample_limit(),
        );
    }
    /// Switch between automatic metering and the manual exposure, and set the
    /// exposure-compensation bias in stops.
    pub fn set_exposure_controls(&mut self, auto: bool, bias: f32) {
        self.tracer.set_exposure_controls(auto, bias);
    }
    pub fn auto_exposure(&self) -> bool {
        self.tracer.auto_exposure()
    }
    pub fn ev_bias(&self) -> f32 {
        self.tracer.ev_bias()
    }

    /// Trace a small initial backdrop once; menus reuse it while the GPU rests.
    pub fn prepare_background(&mut self, frame: &Frame) {
        self.tracer
            .set_quality(1, self.graphics.max_bounces, self.graphics.sample_limit());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("menu-backdrop"),
            });
        self.tracer.encode(&self.queue, &mut encoder, frame);
        let started = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        self.work.submitted(&self.queue, 1, started);
        self.tracer.set_quality(
            self.graphics.samples_per_frame,
            self.graphics.max_bounces,
            self.graphics.sample_limit(),
        );
    }

    /// An opaque menu or diagram needs only UI. Leave the world accumulation
    /// and exposure untouched until observation resumes.
    pub fn render(
        &mut self,
        window: &Window,
        frame: Option<&Frame>,
        ui: &[UiVertex],
    ) -> RenderOutcome {
        if self.size.0 == 0 || self.size.1 == 0 {
            return RenderOutcome::Unavailable;
        }
        if !self.poll_work() {
            // Let the event loop handle input instead of blocking acquisition
            // behind queued trace jobs. Retry after the current frame finishes.
            return RenderOutcome::Retry;
        }
        let surface_tex = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return RenderOutcome::Retry;
            }
            wgpu::CurrentSurfaceTexture::Timeout => return RenderOutcome::Retry,
            wgpu::CurrentSurfaceTexture::Occluded => return RenderOutcome::Unavailable,
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error while acquiring frame");
                return RenderOutcome::Unavailable;
            }
        };
        self.gui.upload(&self.device, &self.queue, ui);
        let view = surface_tex.texture.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let mut traced_samples = 0;
        if let Some(frame) = frame {
            // Resizing a menu must not repeatedly allocate a full HDR buffer.
            self.tracer
                .resize(&self.device, self.graphics.render_size(self.size));
            if self.tracer.prepare_frame(frame) {
                self.work.reset_budget();
            }
            self.tracer.set_quality(
                self.work
                    .samples(self.graphics.samples_per_frame, self.graphics.max_fps),
                self.graphics.max_bounces,
                self.graphics.sample_limit(),
            );
            let before = self.tracer.samples();
            self.tracer.encode(&self.queue, &mut encoder, frame);
            traced_samples = self.tracer.samples() - before;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("display"),
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
            if frame.is_some() || self.tracer.samples() > 0 {
                self.tracer.display(&mut pass);
            }
        }
        let ui_view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(
                    self.config
                        .view_formats
                        .first()
                        .copied()
                        .unwrap_or(self.config.format),
                ),
                ..Default::default()
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blueprint-ui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ui_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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
        let started = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        self.work.submitted(&self.queue, traced_samples, started);
        // Wayland suppresses redraws after this notification until the frame
        // callback arrives. Never notify on a retry: without a surface commit,
        // that callback cannot arrive and even the first window stays blank.
        window.pre_present_notify();
        self.queue.present(surface_tex);
        RenderOutcome::Presented
    }
}
