//! Reviewed guided ground sampler shared by the app and benchmark.
//! The original shader remains available for reproducible comparisons.
use super::TRACE_SHADER;

fn replace(source: &mut String, from: &str, to: &str) {
    assert_eq!(
        source.matches(from).count(),
        1,
        "guided sampler hook changed: {from}"
    );
    *source = source.replacen(from, to, 1);
}

pub(super) fn source() -> String {
    let mut s = TRACE_SHADER.to_owned();
    // Propose a 50/50 mixture of cosine directions and directions
    // toward reflective bodies. Weight by the FULL mixture density,
    // including overlapping discs; preserve the original path budget.
    s.push_str(
        r#"
fn ground_reflector_pdf(ray: Ray) -> f32 {
    let host = u32(settings.g.atmo_params.w) - 1u;
    var total = 0.0;
    var density = 0.0;
    for (var b = 0u; b < settings.counts.x; b += 1u) {
        let body = bodies[b];
        if (b == host || body.material.w > 0.5) { continue; }
        let width = cone_width(ray,b);
        let weight = width * max(body.material.r,max(body.material.g,body.material.b));
        if (weight <= 0.0) { continue; }
        total += weight;
        if (sphere_hit(ray,b).index != MISS) {
            density += weight / (TAU * width);
        }
    }
    return density / max(total,1.0e-30);
}

"#,
    );
    replace(
        &mut s,
        "let last_vertex = depth + 1u == settings.counts.w;\n        for (var light_slot",
        r#"var ground_reflector = ReflectorSample(MISS, 0.0);
        if (local && depth == 0u && settings.g.atmo_params.w > 0.5) {
            ground_reflector = sample_reflector(camera_origin(outgoing), rng);
        }
        let guided = ground_reflector.index != MISS;
        let last_vertex = depth + 1u == settings.counts.w;
        for (var light_slot"#,
    );
    replace(
        &mut s,
        "var weight = power_weight(sample.pdf, bsdf_pdf);",
        r#"var path_pdf = bsdf_pdf;
        if (guided) {
            path_pdf = 0.5 * (bsdf_pdf + ground_reflector_pdf(outgoing));
        }
        var weight = power_weight(sample.pdf, path_pdf);"#,
    );
    replace(
        &mut s,
        "previous_pdf = cosine / PI;\n        throughput *= albedo;",
        r#"previous_pdf = cosine / PI;
        var path_weight = 1.0;
        if (guided) {
            if (random(rng) < 0.5) {
                outgoing.direction = sample_light(outgoing, ground_reflector.index, rng).direction;
            }
            let sampled_cosine = max(dot(normal, outgoing.direction), 0.0);
            if (sampled_cosine == 0.0) { break; }
            let brdf_pdf = sampled_cosine / PI;
            previous_pdf = 0.5 * (brdf_pdf + ground_reflector_pdf(outgoing));
            path_weight = brdf_pdf / previous_pdf;
        }
        throughput *= albedo * path_weight;"#,
    );
    s
}
