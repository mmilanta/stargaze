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
    // sky-any-hit (boolean sky visibility) is now part of the production
    // shader as well; those variant names match their siblings.
    // dark-reflection (skip planetshine work for an unlit reflector sample)
    // is now part of the production shader; the variant names remain so
    // earlier study commands still run, and match their non-dark siblings.
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
            "if (planetshine) {",
            "if (planetshine && i % 2u == reflected_parity) {",
        );
        replace(
            &mut s,
            "* reflected * (visibility * light_t);",
            "* reflected * (2.0 * visibility * light_t);",
        );
    }
    s
}
