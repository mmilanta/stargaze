#!/usr/bin/env python3
"""Reproducible GPU runs and timing/image comparisons. Python standard library only."""
import argparse
import array
import datetime
import gzip
import hashlib
import html
import json
import math
import os
from pathlib import Path
import platform
import statistics
import struct
import subprocess
import sys
import urllib.parse
import zlib

ROOT = Path(__file__).resolve().parents[1]
CASES = ["halo-rings", "earth-daylight", "earth-twilight", "median-dense",
         "vantus-eclipse", "vantus-airless", "dual-eclipse"]
EXTRA_CASES = ["moonlit-air"]
VARIANTS = ["reference", "sun6", "view12", "balanced", "fast", "planetshine-quarter", "no-planetshine"]
COMPATIBLE = ["width", "height", "scale", "bounces", "vegetation", "warmup", "frames", "repeats", "image_samples"]


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, allow_nan=False) + "\n")


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def percentile(values, fraction):
    values = sorted(values)
    i = (len(values) - 1) * fraction
    lo, hi = math.floor(i), math.ceil(i)
    return values[lo] + (values[hi] - values[lo]) * (i - lo)


def summary(mode):
    rows = [v for repeat in mode["repeats"] for v in repeat]
    times = [v["gpu_ms"] for v in rows]
    return {"gpu_median_ms": statistics.median(times), "gpu_p95_ms": percentile(times, .95),
            "wall_median_ms": statistics.median(v["wall_ms"] for v in rows),
            "repeat_medians_ms": [statistics.median(v["gpu_ms"] for v in r) for r in mode["repeats"]],
            "count": len(rows)}


def fresh_directory(path):
    path = path.resolve()
    if path.exists():
        raise ValueError(f"Output already exists: {path}. Choose a new directory to preserve the previous run.")
    path.mkdir(parents=True)
    return path


def page(body):
    return """<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Stargaze renderer benchmark</title><style>
body{font:16px system-ui;background:#131821;color:#e5e9f0;margin:32px auto;max-width:1500px;padding:0 20px}
a{color:#91caff}table{border-collapse:collapse}td,th{padding:8px 14px;border-bottom:1px solid #455;text-align:left}
.gallery{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px}
img{width:100%;background:black}figure{margin:0}figcaption{padding:8px 0}code{overflow-wrap:anywhere}
</style><h1>Stargaze renderer benchmark</h1>""" + body + "</html>"


def report_run(output):
    data = json.loads((output / "results.json").read_text())
    request = json.loads((output / "request.json").read_text())
    lines = ["# Renderer benchmark", "", f"Variant: **{request.get('variant', 'reference')}** (benchmark-only when not reference).", "", f"GPU: **{data['adapter']['name']}** ({data['adapter']['driver_info']}).",
             f"Display: {request['width']}×{request['height']}; trace: {data['trace_size'][0]}×{data['trace_size'][1]}; "
             f"{request['bounces']} bounces; {request['vegetation']}% vegetation; 1 spp per submission.", "",
             "GPU time includes sky-cache work when invalidated, tracing, automatic exposure, and offscreen display. "
             "Wall time includes encoding, upload/submission, timestamp resolve, and waiting. No UI, compositor, "
             "VSync, frame cap, frame construction, image readback, or PNG encoding. These are not application FPS.", "",
             "| View | Mode | GPU median ms | GPU p95 ms | Wall median ms | Repeat medians ms |",
             "|---|---|---:|---:|---:|---|"]
    rows = []
    gallery = []
    for case in data["cases"]:
        for mode in case["modes"]:
            s = summary(mode)
            rows.append({"case": case["name"], "mode": mode["mode"], **s})
            lines.append(f"| {case['name']} | {mode['mode']} | {s['gpu_median_ms']:.3f} | {s['gpu_p95_ms']:.3f} | "
                         f"{s['wall_median_ms']:.3f} | {', '.join(f'{v:.3f}' for v in s['repeat_medians_ms'])} |")
        for im in case["images"]:
            name = im["name"]
            gallery.append(f'<figure><a href="{name}.png"><img loading="lazy" src="{name}.png"></a>'
                           f'<figcaption>{name} · exposure {im["exposure"]}</figcaption></figure>')
    write_json(output / "summary.json", rows)
    (output / "summary.md").write_text("\n".join(lines) + "\n")
    body = f"<p>{html.escape(data['adapter']['name'])} · {data['trace_size']} trace pixels</p>"
    body += f"<p>Variant: <strong>{html.escape(request.get('variant', 'reference'))}</strong></p>"
    body += '<p><a href="summary.md">Timing report</a> · <a href="results.json">Raw results</a> · <a href="manifest.json">Provenance</a></p>'
    body += "<p>Fixed-exposure images: one sample, refined, and two advancing-time checkpoints. Click to inspect full resolution.</p>"
    body += '<div class="gallery">' + "".join(gallery) + "</div>"
    (output / "index.html").write_text(page(body))
    print("\n".join(lines))


def run(args):
    output = fresh_directory(args.output)
    config = {k: getattr(args, k) for k in COMPATIBLE}
    config.update(output=str(output), adapter=args.adapter, cases=args.cases or CASES, variant=args.variant)
    write_json(output / "request.json", config)
    files = sorted([*ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.wgsl"), *ROOT.glob("configs/*.yaml"), *ROOT.glob("scripts/*benchmark*.py"), ROOT / "Cargo.toml", ROOT / "Cargo.lock"])
    manifest = {"created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "git_commit": command("git", "rev-parse", "HEAD"), "git_status": command("git", "status", "--short"),
                "git_diff_stat": command("git", "diff", "--stat"), "os": platform.platform(),
                "rustc": command("rustc", "--version"), "cargo": command("cargo", "--version"),
                "cpu": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")), "unknown"),
                "source_sha256": {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
                "invocation": sys.argv, "note": "Uncapped serial offscreen renderer benchmark, not presented application FPS."}
    write_json(output / "manifest.json", manifest)
    env = {k: v for k, v in os.environ.items() if not k.startswith("STARGAZE_")}
    env["STARGAZE_BENCH_CONFIG"] = str(output / "request.json")
    cmd = ["cargo", "test", "--locked", "--release", "pathtracer::benchmark::render_benchmark", "--",
           "--exact", "--ignored", "--nocapture", "--test-threads=1"]
    with (output / "run.log").open("w") as log:
        proc = subprocess.Popen(cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        for line in proc.stdout:
            print(line, end="", flush=True)
            log.write(line)
            log.flush()
        code = proc.wait()
    if code:
        raise RuntimeError(f"Benchmark failed ({code}); partial output kept at {output}")
    for pattern in ("*.rgba", "*.f32"):
        for raw in output.glob(pattern):
            # mtime=0 makes the compressed artifacts deterministic too.
            raw.with_suffix(raw.suffix + ".gz").write_bytes(gzip.compress(raw.read_bytes(), mtime=0))
            raw.unlink()
    report_run(output)
    print(f"\nImages and report: {output / 'index.html'}")


def check_compatible(base, candidate, br, cr):
    differences = [k for k in COMPATIBLE if br[k] != cr[k]]
    # Different explicit variants are intentional, but never mislabel a run.
    for label, data, request in [("baseline", base, br), ("candidate", candidate, cr)]:
        if data.get("variant", "reference") != request.get("variant", "reference"):
            differences.append(f"{label} variant metadata")
    if base["suite_version"] != candidate["suite_version"]:
        differences.append("suite_version")
    if base["adapter"] != candidate["adapter"]:
        differences.append("GPU/backend/driver")
    if base["trace_size"] != candidate["trace_size"]:
        differences.append("trace_size")
    bc, cc = {c["name"]: c for c in base["cases"]}, {c["name"]: c for c in candidate["cases"]}
    if bc.keys() != cc.keys():
        differences.append("case selection")
    for name in bc.keys() & cc.keys():
        if bc[name]["frame_fingerprints"] != cc[name]["frame_fingerprints"]:
            differences.append(f"{name} scene/camera/geometry inputs")
        if bc[name]["images"] != cc[name]["images"]:
            differences.append(f"{name} image settings")
    if differences:
        raise ValueError("Incompatible runs: " + ", ".join(differences))


def floats(path):
    values = array.array("f")
    values.frombytes(gzip.decompress(path.read_bytes()))
    if sys.byteorder != "little":
        values.byteswap()
    if not all(math.isfinite(v) and v >= 0 for v in values):
        raise ValueError(f"Invalid HDR values: {path}")
    return values


def image_metrics(a, b, ah=None, bh=None):
    if len(a) != len(b) or not a or len(a) % 4 or ((ah is None) != (bh is None)) or (ah is not None and (len(ah) != len(bh) or not ah or len(ah) % 4)):
        raise ValueError("Image buffers have incompatible dimensions")
    diff = bytearray(len(a))
    total = square = changed = maximum = 0
    for i in range(0, len(a), 4):
        errors = [abs(a[i+c] - b[i+c]) for c in range(3)]
        total += sum(errors)
        square += sum(e*e for e in errors)
        maximum = max(maximum, *errors)
        changed += max(errors) > 1
        heat = min(255, round(sum(errors) / 3 * 8))
        diff[i:i+4] = bytes((heat, heat, heat, 255))
    count = len(a) // 4 * 3
    rmse = math.sqrt(square / count)
    error = reference = log_error = 0.0
    for i, (x, y) in enumerate(zip(ah or [], bh or [])):
        if i % 4 == 3:  # W is metering backdrop luminance, not RGB.
            continue
        error += (x-y)**2
        reference += x*x
        log_error += (math.log1p(x)-math.log1p(y))**2
    n = len(ah) // 4 * 3 if ah is not None else 0
    return {"display_mae_255": total/count, "display_rmse_255": rmse, "display_max_255": maximum,
            "display_psnr_db": 20*math.log10(255/rmse) if rmse else None,
            "pixels_changed_over_1_255_percent": changed / (len(a)//4)*100,
            "linear_rgb_rmse": math.sqrt(error/n) if n else None,
            "linear_rgb_relative_rmse": math.sqrt(error/reference) if reference else None,
            "log1p_rgb_rmse": math.sqrt(log_error/n) if n else None}, diff


def png_write(path, width, height, pixels):
    if len(pixels) != width*height*4:
        raise ValueError("Invalid PNG buffer length")
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind+data))
    scan = b"".join(b"\0" + pixels[y*width*4:(y+1)*width*4] for y in range(height))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(scan)) + chunk(b"IEND", b""))


def png_read(path):
    """Decode the non-interlaced RGBA8 PNGs emitted by this suite (no dependencies)."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("Invalid PNG signature")
    cursor, compressed, dimensions = 8, bytearray(), None
    while cursor < len(data):
        size = struct.unpack_from(">I", data, cursor)[0]
        kind = data[cursor+4:cursor+8]
        payload = data[cursor+8:cursor+8+size]
        crc = struct.unpack_from(">I", data, cursor+8+size)[0]
        if zlib.crc32(kind+payload) != crc:
            raise ValueError("PNG checksum mismatch")
        if kind == b"IHDR":
            w, h, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            if (depth, color, compression, filtering, interlace) != (8, 6, 0, 0, 0):
                raise ValueError("Expected benchmark RGBA8 PNG")
            dimensions = w, h
        elif kind == b"IDAT":
            compressed.extend(payload)
        cursor += 12+size
    if dimensions is None:
        raise ValueError("Missing PNG dimensions")
    w, h = dimensions
    scan, stride, output = zlib.decompress(compressed), w*4, bytearray()
    if len(scan) != h*(stride+1):
        raise ValueError("Unexpected PNG scanline size")
    previous = bytearray(stride)
    for y in range(h):
        start = y*(stride+1)
        filtering = scan[start]
        row = bytearray(scan[start+1:start+1+stride])
        if filtering not in range(5):
            raise ValueError("Unknown PNG filter")
        for x in range(stride):
            a, c = (row[x-4], previous[x-4]) if x >= 4 else (0, 0)
            b = previous[x]
            if filtering == 1:
                predictor = a
            elif filtering == 2:
                predictor = b
            elif filtering == 3:
                predictor = (a+b)//2
            elif filtering == 4:
                p = a+b-c
                pa, pb, pc = abs(p-a), abs(p-b), abs(p-c)
                predictor = a if pa <= pb and pa <= pc else b if pb <= pc else c
            else:
                predictor = 0
            row[x] = (row[x]+predictor) & 255
        output.extend(row)
        previous = row
    return bytes(output)


def displayed(directory, name):
    raw = directory / f"{name}.rgba.gz"
    return gzip.decompress(raw.read_bytes()) if raw.exists() else png_read(directory / f"{name}.png")


def number(value):
    return f"{value:.6g}" if value is not None else "unavailable"


def link(path, output):
    return html.escape(urllib.parse.quote(os.path.relpath(path, output)))


def compare(args):
    base_dir, candidate_dir = args.baseline.resolve(), args.candidate.resolve()
    base, candidate = [json.loads((d / "results.json").read_text()) for d in (base_dir, candidate_dir)]
    br, cr = [json.loads((d / "request.json").read_text()) for d in (base_dir, candidate_dir)]
    check_compatible(base, candidate, br, cr)
    output = fresh_directory(args.output)
    lines = ["# Benchmark comparison", "", f"Variants: **{br.get('variant', 'reference')} → {cr.get('variant', 'reference')}**.", "", "Speedup is baseline / candidate GPU median; above 1 is faster. "
             "P95 and repetition ranges help identify timing noise. Image differences are not automatically quality regressions.", "",
             "| View | Mode | Baseline ms | Candidate ms | Speedup | Candidate p95 ms |",
             "|---|---|---:|---:|---:|---:|"]
    timings, images, gallery = [], [], []
    candidates = {c["name"]: c for c in candidate["cases"]}
    for bc in base["cases"]:
        cc = candidates[bc["name"]]
        cm = {m["mode"]: m for m in cc["modes"]}
        for bm in bc["modes"]:
            bs, cs = summary(bm), summary(cm[bm["mode"]])
            speedup = bs["gpu_median_ms"] / cs["gpu_median_ms"]
            timings.append({"case": bc["name"], "mode": bm["mode"], "baseline": bs, "candidate": cs, "speedup": speedup})
            lines.append(f"| {bc['name']} | {bm['mode']} | {bs['gpu_median_ms']:.3f} | {cs['gpu_median_ms']:.3f} | {speedup:.3f}× | {cs['gpu_p95_ms']:.3f} |")
        for im in bc["images"]:
            name = im["name"]
            a, b = [displayed(d, name) for d in (base_dir, candidate_dir)]
            paths = [d / f"{name}.f32.gz" for d in (base_dir, candidate_dir)]
            ah, bh = [floats(p) for p in paths] if all(p.exists() for p in paths) else (None, None)
            if len(a) != br["width"]*br["height"]*4 or (ah is not None and len(ah) != math.prod(base["trace_size"])*4):
                raise ValueError(f"Unexpected image dimensions: {name}")
            metrics, diff = image_metrics(a, b, ah, bh)
            images.append({"name": name, **metrics})
            png_write(output / f"{name}-diff.png", br["width"], br["height"], diff)
            figures = []
            for label, path in [("Baseline", base_dir / f"{name}.png"), ("Candidate", candidate_dir / f"{name}.png"), ("Absolute difference ×8", output / f"{name}-diff.png")]:
                url = link(path, output)
                figures.append(f'<figure><a href="{url}"><img loading="lazy" src="{url}"></a><figcaption>{label}</figcaption></figure>')
            gallery.append(f'<h2>{name}</h2><p>Display MAE {metrics["display_mae_255"]:.4f}/255; '
                           f'linear RGB RMSE {number(metrics["linear_rgb_rmse"])}</p><div class="gallery">' + "".join(figures) + "</div>")
    lines += ["", "| Image | Display MAE /255 | Changed pixels >1/255 | Linear RGB RMSE | log(1+RGB) RMSE |",
              "|---|---:|---:|---:|---:|"]
    lines += [f"| {im['name']} | {im['display_mae_255']:.5f} | {im['pixels_changed_over_1_255_percent']:.3f}% | {number(im['linear_rgb_rmse'])} | {number(im['log1p_rgb_rmse'])} |" for im in images]
    write_json(output / "comparison.json", {"baseline": str(base_dir), "candidate": str(candidate_dir), "baseline_variant": br.get("variant", "reference"), "candidate_variant": cr.get("variant", "reference"), "timings": timings, "images": images})
    (output / "comparison.md").write_text("\n".join(lines) + "\n")
    (output / "index.html").write_text(page('<p><a href="comparison.md">Timing and image metrics</a></p>' + "".join(gallery)))
    print("\n".join(lines))
    print(f"\nVisual comparison: {output / 'index.html'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    p = sub.add_parser("run", help="run the release renderer on a hardware Vulkan GPU")
    p.add_argument("output", type=Path)
    for key, default in [("width", 1280), ("height", 720), ("scale", 75), ("bounces", 4), ("vegetation", 70),
                         ("warmup", 8), ("frames", 16), ("repeats", 3), ("image-samples", 32)]:
        p.add_argument("--"+key, type=int, default=default)
    p.add_argument("--adapter", default="", help="case-insensitive name substring; default: first discrete GPU")
    p.add_argument("--cases", nargs="+", choices=CASES + EXTRA_CASES)
    p.add_argument("--variant", choices=VARIANTS, default="reference", help="benchmark-only approximation; production stays unchanged")
    p.set_defaults(function=run)
    p = sub.add_parser("compare", help="compare matching runs, including HDR and displayed image differences")
    p.add_argument("baseline", type=Path)
    p.add_argument("candidate", type=Path)
    p.add_argument("output", type=Path)
    p.set_defaults(function=compare)
    args = parser.parse_args()
    try:
        args.function(args)
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
