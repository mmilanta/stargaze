// Display-only automatic exposure. A luminance histogram over the central
// region is reduced to one percentile; the result feeds an eased exposure.
//
// Metering the whole frame would fail: background stars are far brighter per
// pixel than a distant planet, so a global percentile would expose for the
// starfield and leave Saturn black. The telescope keeps the aimed body near
// the centre, so a centre-weighted region sees the object being observed.
struct Globals {
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_forward: vec4<f32>,
    viewport: vec4<f32>, // trace width, height, manual exposure, display aspect
    atmo_center: vec4<f32>,
    atmo_rayleigh: vec4<f32>,
    atmo_params: vec4<f32>,
    ground_east: vec4<f32>,
    ground_up: vec4<f32>,
    ground_north: vec4<f32>,
    ground_counts: vec4<u32>,
};
struct Settings {
    g: Globals,
    counts: vec4<u32>,
    background: vec4<f32>,
    lights: vec4<u32>, // first three emissive indices, total count; fallback for >3
};
struct Exposure {
    value: f32, // smoothed automatic exposure (updated on the GPU)
    key: f32, // target luminance for the metered percentile
    percentile: f32, // which luminance percentile to meter (0..1)
    enabled: f32, // 1 when automatic exposure is active
    bias: f32, // user exposure compensation, in stops
};
@group(0) @binding(0) var<uniform> settings: Settings;
@group(0) @binding(1) var<storage, read> accumulation: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> exposure: Exposure;

const BINS: u32 = 256u;
const LOG_MIN: f32 = -24.0;
const LOG_MAX: f32 = 24.0;
// Below this the metered region is essentially empty sky; keep the current
// exposure instead of chasing a meaningless dark frame towards infinity.
const METER_FLOOR: f32 = 1.0e-6;
const MIN_EXPOSURE: f32 = 1.0e-8;
const MAX_EXPOSURE: f32 = 1.0e8;

@compute @workgroup_size(8, 8)
fn measure(@builtin(global_invocation_id) id: vec3<u32>) {
    let dimensions = vec2<u32>(settings.g.viewport.xy);
    // Dispatch only the metered rectangle. At 4K this avoids launching six
    // million invocations which would immediately return outside the centre.
    let origin = dimensions / 4u;
    let meter_dimensions = dimensions - 2u * origin;
    if (id.x >= meter_dimensions.x || id.y >= meter_dimensions.y) {
        return;
    }
    let pixel = id.xy + origin;
    let hdr = accumulation[pixel.y * dimensions.x + pixel.x];
    // The decorative Milky Way must not change metering of a faint planet or
    // drive automatic exposure when the centre contains otherwise empty sky.
    let luminance = max(0.0, dot(hdr.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)) - hdr.w);
    // Zero and any non-finite junk fall into the darkest bin.
    var safe = luminance;
    if (!(safe > 0.0)) {
        safe = exp2(LOG_MIN);
    }
    safe = clamp(safe, exp2(LOG_MIN), exp2(LOG_MAX));
    let t = (log2(safe) - LOG_MIN) / (LOG_MAX - LOG_MIN);
    let bin = u32(clamp(t * f32(BINS), 0.0, f32(BINS - 1u)));
    atomicAdd(&histogram[bin], 1u);
}

@compute @workgroup_size(1)
fn resolve() {
    if (exposure.enabled < 0.5) {
        return;
    }
    var total = 0u;
    for (var i = 0u; i < BINS; i += 1u) {
        total += atomicLoad(&histogram[i]);
    }
    if (total == 0u) {
        return;
    }
    // A percentile selects an observation with a one-based rank, even for
    // a one-pixel viewport or a percentile smaller than 1 / total.
    let cutoff = max(1u, u32(ceil(f32(total) * clamp(exposure.percentile, 0.0, 1.0))));
    var cumulative = 0u;
    var bin = 0u;
    for (var i = 0u; i < BINS; i += 1u) {
        cumulative += atomicLoad(&histogram[i]);
        if (cumulative >= cutoff) {
            bin = i;
            break;
        }
    }
    let t = (f32(bin) + 0.5) / f32(BINS);
    let measured = exp2(LOG_MIN + t * (LOG_MAX - LOG_MIN));
    if (measured < METER_FLOOR) {
        return;
    }
    let desired = clamp(exposure.key / measured, MIN_EXPOSURE, MAX_EXPOSURE);
    // Ease toward the new exposure so a scene or time change does not flicker.
    let rate = 0.15;
    exposure.value = clamp(exposure.value + (desired - exposure.value) * rate,
        MIN_EXPOSURE, MAX_EXPOSURE);
}
