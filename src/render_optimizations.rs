//! Follow-up experiments based on the current production 8/4 atmosphere.
//! All changes are opt-in and compiled only by the benchmark/test build.
use super::super::{TRACE_SHADER, lowlight};

pub const VARIANTS: &[&str] = &[
    "sky-any-hit",
    "dark-reflection",
    "raw-combined",
    "view6-sun4",
    "view8-sun2",
    "reflect-half",
    "guided-ground",
    "guided-raw",
];

fn replace(source: &mut String, from: &str, to: &str) {
    assert_eq!(
        source.matches(from).count(),
        1,
        "optimization hook changed: {from}"
    );
    *source = source.replacen(from, to, 1);
}

pub fn source(variant: &str) -> String {
    assert!(VARIANTS.contains(&variant));
    let mut s = if matches!(variant, "guided-ground" | "guided-raw") {
        lowlight::source("guided-ground")
    } else {
        TRACE_SHADER.to_owned()
    };
    if matches!(variant, "sky-any-hit" | "raw-combined" | "guided-raw") {
        // The sky lookup needs a boolean, not the closest surface, its normal,
        // or a sorted traversal beyond the first opaque blocker. Rings remain
        // in the existing, separate transmission loop.
        s.push_str(
            r#"
fn sky_blocked(ray: Ray) -> bool {
    for (var i = 0u; i < settings.counts.x; i += 1u) {
        if (settings.g.ground_north.w == f32(i + 1u)) { continue; }
        let hit = sphere_hit(ray, i);
        if (hit.index != MISS && hit.distance < 1.0e30) { return true; }
    }
    let local = landscape_query(ray, 1.0e30, true);
    return local.index != MISS && local.distance < 1.0e30;
}
"#,
        );
        replace(
            &mut s,
            "if (closest_hit(ray).index != MISS) { return vec3<f32>(0.0); }",
            "if (sky_blocked(ray)) { return vec3<f32>(0.0); }",
        );
    }
    if matches!(variant, "dark-reflection" | "raw-combined" | "guided-raw") {
        // Keep every random draw and visibility query. Only skip deterministic
        // attenuation/phase math when this sample contributes exactly zero.
        replace(
            &mut s,
            "let light_t = exp(-beta_ext * density_integral(p, light.direction, ATMO_SUN_STEPS));",
            "if (any(incident != vec3<f32>(0.0))) {\n                let light_t = exp(-beta_ext * density_integral(p, light.direction, ATMO_SUN_STEPS));",
        );
        replace(
            &mut s,
            "inscatter += t_view * segment * beta_phase * incident * light_t;",
            "inscatter += t_view * segment * beta_phase * incident * light_t;\n                }",
        );
    }
    if variant == "view6-sun4" {
        replace(
            &mut s,
            "const ATMO_VIEW_STEPS: u32 = 8u;",
            "const ATMO_VIEW_STEPS: u32 = 6u;",
        );
    }
    if variant == "view8-sun2" {
        replace(
            &mut s,
            "const ATMO_SUN_STEPS: u32 = 4u;",
            "const ATMO_SUN_STEPS: u32 = 2u;",
        );
    }
    if variant == "reflect-half" {
        // Sample one parity of the eight cells. A separate stream leaves
        // direct-star samples unchanged; weight selected cells by two.
        replace(
            &mut s,
            "let reflector = sample_reflector(origin, &reflected_rng);",
            r#"
    let reflector = sample_reflector(origin, &reflected_rng);
    var parity_rng = hash(*rng ^ 0x9e3779b9u);
    let reflected_parity = u32(random(&parity_rng) * 2.0);
"#,
        );
        replace(
            &mut s,
            "if (reflector.index != MISS) {",
            "if (reflector.index != MISS && i % 2u == reflected_parity) {",
        );
        replace(
            &mut s,
            "* (visibility * reflector.inverse_probability / light.pdf);",
            "* (2.0 * visibility * reflector.inverse_probability / light.pdf);",
        );
    }
    s
}
