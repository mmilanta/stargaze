//! Image and exposure readback helpers shared by GPU ground regressions.
use super::*;

pub(super) fn save_surface(name: &str, size: (u32, u32), pixels: &[[f32; 4]], exposure: f32) {
    let Ok(dir) = std::env::var("STARGAZE_TEST_IMAGE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let path = std::path::Path::new(&dir).join(format!("{name}.png"));
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), size.0, size.1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut data = Vec::with_capacity(pixels.len() * 3);
    for pixel in pixels {
        if pixel.iter().any(|v| !v.is_finite()) {
            data.extend_from_slice(&[255, 0, 255]);
            continue;
        }
        for channel in &pixel[..3] {
            let x = channel * exposure;
            let mapped = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
            let srgb = if mapped <= 0.0031308 {
                12.92 * mapped
            } else {
                1.055 * mapped.powf(1.0 / 2.4) - 0.055
            };
            data.push((255.0 * srgb).round() as u8);
        }
    }
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&data)
        .unwrap();
}

pub(super) fn read_exposure(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tracer: &PathTracer,
) -> f32 {
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("preview-exposure"),
        size: 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&tracer.exposure, 0, &readback, 0, 4);
    queue.submit(Some(encoder.finish()));
    let (send, receive) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let data = readback.slice(..).get_mapped_range().unwrap();
    bytemuck::cast_slice::<u8, f32>(&data)[0]
}
