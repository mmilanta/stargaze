#!/usr/bin/env python3
"""Run bracketed optimization experiments and retain every image comparison."""
import argparse
import html
import json
from pathlib import Path
import statistics

import benchmark as b

RAW = {"sky-any-hit", "dark-reflection", "raw-combined"}


def plan(variants, rounds):
    """Alternate candidate order, bracket each round with unchanged references."""
    result = []
    for i in range(rounds):
        order = variants if i % 2 == 0 else list(reversed(variants))
        runs = [(f"round-{i+1}/reference-before", "reference")]
        runs += [(f"round-{i+1}/{v}", v) for v in order]
        runs += [(f"round-{i+1}/reference-after", "reference")]
        result.append(runs)
    return result


def summarize(comparisons):
    groups = {}
    for variant, round_number, side, comparison in comparisons:
        for timing in comparison["timings"]:
            key = variant, timing["case"], timing["mode"]
            groups.setdefault(key, []).append({"round": round_number, "reference": side,
                "speedup": timing["speedup"],
                "candidate_ms": timing["candidate"]["gpu_median_ms"],
                "reference_ms": timing["baseline"]["gpu_median_ms"]})
    return [{"variant": key[0], "case": key[1], "mode": key[2],
             "speedup_min": min(r["speedup"] for r in rows),
             "speedup_max": max(r["speedup"] for r in rows),
             "candidate_median_ms": statistics.median(r["candidate_ms"] for r in rows),
             "comparisons": rows} for key, rows in groups.items()]


def publish(out, comparisons, round_count):
    rows = summarize(comparisons)
    b.write_json(out / "study.json", rows)
    lines = ["# Optimization study", "", "GPU times are offscreen renderer measurements, not application FPS.",
             "Speedup ranges include both bracketing references in every round; they are not confidence intervals.",
             "A range spanning 1.0 is inconclusive. Inspect repeat variation and individual images before accepting a change.", "",
             "| Variant | View | Mode | Candidate median ms | Speedup range |",
             "|---|---|---|---:|---:|"]
    for row in rows:
        lines.append(f"| {row['variant']} | {row['case']} | {row['mode']} | "
                     f"{row['candidate_median_ms']:.3f} | {row['speedup_min']:.3f}–{row['speedup_max']:.3f}× |")
    lines += ["", "## Raw candidate image checks", "",
              "These require identical linear RGB and displayed captures. A mismatch prevents an equivalence claim.", ""]
    for variant in sorted(RAW & {c[0] for c in comparisons}):
        images = [im for v, _, _, c in comparisons if v == variant for im in c['images']]
        identical = all(im['linear_rgb_rmse'] == 0 and im['display_mae_255'] == 0 for im in images)
        lines.append(f"- {variant}: {'identical captures' if identical else 'DIFFERENCES — investigate before promotion'}.")
    (out / "README.md").write_text("\n".join(lines) + "\n")
    links = []
    for variant, number, side, _ in comparisons:
        path = f"round-{number}/{variant}-vs-{side}/index.html"
        links.append(f'<li><a href="{html.escape(path)}">Round {number}: {html.escape(variant)} vs {side}</a></li>')
    for number in range(1, round_count+1):
        links.append(f'<li><a href="round-{number}/reference-drift/index.html">Round {number}: reference repeatability</a></li>')
    (out / "index.html").write_text(b.page(
        '<p><a href="README.md">Timing ranges and equivalence checks</a> · '
        '<a href="study.json">Raw aggregate data</a></p><p>Every candidate is retained. '
        'No rendering choice has been accepted or discarded automatically.</p><ul>' + ''.join(links) + '</ul>'))


def execute(args, out, schedule, variants):
    comparisons = []
    for number, runs in enumerate(schedule, 1):
        for relative, variant in runs:
            config = argparse.Namespace(**{key: getattr(args, key) for key in b.COMPATIBLE},
                output=out/relative, variant=variant, adapter=args.adapter, cases=args.cases)
            b.run(config)
        directory = out/f'round-{number}'
        b.compare(argparse.Namespace(baseline=directory/'reference-before', candidate=directory/'reference-after',
                                    output=directory/'reference-drift'))
        for variant in variants:
            for side in ['before', 'after']:
                dest = directory/f'{variant}-vs-{side}'
                b.compare(argparse.Namespace(baseline=directory/f'reference-{side}', candidate=directory/variant, output=dest))
                comparisons.append((variant, number, side, json.loads((dest/'comparison.json').read_text())))
    publish(out, comparisons, args.rounds)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--variants', nargs='+', choices=b.OPTIMIZATION_VARIANTS, default=b.OPTIMIZATION_VARIANTS)
    parser.add_argument('--cases', nargs='+', choices=b.CASES+b.EXTRA_CASES, default=b.CASES+b.EXTRA_CASES)
    parser.add_argument('--rounds', type=int, choices=range(1, 6), default=2)
    parser.add_argument('--plan-only', action='store_true', help='write commands without requiring a GPU')
    parser.add_argument('--adapter', default='')
    for key, default in [('width', 640), ('height', 360), ('scale', 100), ('bounces', 4),
                         ('vegetation', 70), ('warmup', 8), ('frames', 16), ('repeats', 3), ('image-samples', 32)]:
        parser.add_argument('--'+key, type=int, default=default)
    args = parser.parse_args()
    variants = list(dict.fromkeys(args.variants))
    out = b.fresh_directory(args.output)
    schedule = plan(variants, args.rounds)
    commands = []
    for runs in schedule:
        for relative, variant in runs:
            command = ['python3', 'scripts/benchmark.py', 'run', str(out/relative), '--variant', variant]
            for key in b.COMPATIBLE:
                command += ['--'+key.replace('_','-'), str(getattr(args, key))]
            command += ['--cases', *args.cases]
            if args.adapter:
                command += ['--adapter', args.adapter]
            commands.append(command)
    b.write_json(out/'plan.json', {'status': 'planned', 'commands': commands})
    if args.plan_only:
        print(f'Plan only; no measurements: {out / "plan.json"}')
        return
    try:
        execute(args, out, schedule, variants)
    except (ValueError, RuntimeError, OSError) as error:
        b.write_json(out/'plan.json', {'status': 'failed', 'error': str(error), 'commands': commands})
        parser.exit(1, f'error: {error}\nPartial artifacts retained at {out}\n')
    b.write_json(out/'plan.json', {'status': 'complete', 'commands': commands})
    print(f'All comparisons: {out / "index.html"}')


if __name__ == '__main__':
    main()
