// Display only: linear HDR accumulation -> ACES -> sRGB swapchain.
struct Globals {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>,
    viewport: vec4<f32>,
    atmo_center: vec4<f32>,
    atmo_rayleigh: vec4<f32>,
    atmo_params: vec4<f32>,
};
struct Settings {
    g: Globals,
    counts: vec4<u32>,
};
// Display-only exposure state. `value` is the eased automatic exposure;
// `viewport.z` remains the manual exposure. `bias` is user compensation in stops.
struct ExposureState {
    value: f32,
    key: f32,
    percentile: f32,
    enabled: f32,
    bias: f32,
};
@group(0) @binding(0) var<uniform> settings: Settings;
@group(0) @binding(1) var<storage, read> accumulation: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> exposure: ExposureState;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex
fn vs(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, 1.0 - y);
    return out;
}

fn hdr_at(pixel: vec2<i32>) -> vec3<f32> {
    let dimensions = vec2<i32>(settings.g.viewport.xy);
    let p = clamp(pixel, vec2<i32>(0), dimensions - vec2<i32>(1));
    return accumulation[u32(p.y * dimensions.x + p.x)].rgb;
}

@fragment
fn fs(input: VertexOut) -> @location(0) vec4<f32> {
    // Upscale only the world; HUD geometry still uses the native viewport.
    let pixel = input.uv * settings.g.viewport.xy - vec2<f32>(0.5);
    let lo = vec2<i32>(floor(pixel));
    let weight = fract(pixel);
    let hdr = mix(
        mix(hdr_at(lo), hdr_at(lo + vec2<i32>(1, 0)), weight.x),
        mix(hdr_at(lo + vec2<i32>(0, 1)), hdr_at(lo + vec2<i32>(1, 1)), weight.x),
        weight.y
    );
    let automatic = exposure.enabled > 0.5;
    let base = select(settings.g.viewport.z, exposure.value, automatic);
    // Combine exposure in stops so valid large controls cannot overflow.
    // Values above 1e6 already map to white; cap BEFORE multiplying to keep
    // both the exposed radiance and the quadratic tone curve finite.
    let stops = clamp(log2(clamp(base, 1.0e-30, 1.0e30))
        + clamp(exposure.bias, -100.0, 100.0), -100.0, 100.0);
    let multiplier = exp2(stops);
    let x = min(hdr, vec3<f32>(1.0e6 / multiplier)) * multiplier;
    let mapped = clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(mapped, 1.0);
}
