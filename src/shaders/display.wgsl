// Display only: linear HDR accumulation -> ACES -> sRGB swapchain.
struct Globals {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>,
    viewport: vec4<f32>,
};
struct Settings {
    g: Globals,
    counts: vec4<u32>,
};
@group(0) @binding(0) var<uniform> settings: Settings;
@group(0) @binding(1) var<storage, read> accumulation: array<vec4<f32>>;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let index = u32(position.y) * u32(settings.g.viewport.x) + u32(position.x);
    let x = accumulation[index].rgb * settings.g.viewport.z;
    let mapped = clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(mapped, 1.0);
}
