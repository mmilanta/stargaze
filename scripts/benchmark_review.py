#!/usr/bin/env python3
"""Publish compact quality-study evidence and a local interactive image comparison."""
import argparse
import json
import html
from pathlib import Path
import shutil
import benchmark as b


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('full', type=Path)
    parser.add_argument('quality', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--timing-note', default='Another application was using the GPU during the timing runs. Treat speedups as provisional. Image comparisons remain valid.')
    args = parser.parse_args()
    out = b.fresh_directory(args.output)
    variants = b.QUALITY_VARIANTS[1:]
    cases = json.loads((args.quality/'reference/results.json').read_text())['cases']
    cases = [c['name'] for c in cases]
    data = {'variants': variants, 'cases': cases, 'timings': {}, 'metrics': {}}
    for variant in b.QUALITY_VARIANTS:
        image_dir = out/'images'/variant
        image_dir.mkdir(parents=True)
        for kind, parent in [('full', args.full), ('quality', args.quality)]:
            src = parent/variant
            dest = out/'data'/kind/variant
            dest.mkdir(parents=True)
            for name in ['request.json', 'manifest.json', 'results.json', 'summary.json', 'summary.md']:
                shutil.copyfile(src/name, dest/name)
            for case in cases:
                for samples in ([1, 32] if kind == 'full' else [256]):
                    name = f'{case}-spp{samples}.png'
                    shutil.copyfile(src/name, image_dir/name)
                if kind == 'full':
                    for moving in src.glob(f'{case}-moving-*.png'):
                        shutil.copyfile(moving, image_dir/moving.name)
            if variant != 'reference':
                src = parent/(variant+'-comparison')
                for name in ['comparison.json', 'comparison.md']:
                    shutil.copyfile(src/name, dest/name)
                comparison = json.loads((src/'comparison.json').read_text())
                if kind == 'full':
                    data['timings'][variant] = {t['case']: t for t in comparison['timings'] if t['mode'] == 'advancing'}
                data['metrics'].setdefault(variant, {}).update({
                    im['name']: im for im in comparison['images']
                    if any(im['name'] == f'{c}-spp{s}' for c in cases for s in ([1, 32] if kind == 'full' else [256]))})
                for case in cases:
                    for samples in ([1, 32] if kind == 'full' else [256]):
                        name = f'{case}-spp{samples}-diff.png'
                        shutil.copyfile(src/name, image_dir/name)
    b.write_json(out/'review.json', data)
    page = r'''<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width"><title>Stargaze quality experiments</title>
<style>
body{font:16px system-ui;background:#131821;color:#e5e9f0;max-width:1400px;margin:25px auto;padding:0 20px}a{color:#91caff}
select,button{font:inherit;background:#253244;color:inherit;border:1px solid #738299;border-radius:4px;padding:8px;margin:6px}
label{display:inline-block}p{line-height:1.5}#stats{font-variant-numeric:tabular-nums}
#view{position:relative;max-width:1280px;line-height:0;background:black;border:1px solid #64748b}
#base{width:100%}#candidate{position:absolute;inset:0;width:100%;height:100%;clip-path:inset(0 0 0 50%)}
#line{position:absolute;top:0;bottom:0;left:50%;border-left:2px solid white;pointer-events:none}
#difference{width:100%;max-width:1280px;display:none}.labels{display:flex;justify-content:space-between;max-width:1280px}
input[type=range]{width:100%;max-width:1280px;margin:15px 0}small{color:#c0cbd9}h1{font-size:28px}
</style><h1>Rendering speed versus appearance</h1>
<p>These are <strong>benchmark-only experiments</strong>. The normal renderer is unchanged.
Choose a scene and variant, then move the divider or toggle the images. Every tested option is included; no visual choice has been accepted or discarded.
<a href="README.md">Findings, tradeoffs and validation</a>.</p>
<label>Variant <select id="variant"></select></label><label>Scene <select id="scene"></select></label>
<label>Image <select id="samples"><option value="1">Interactive: 1 spp · 1280×720</option>
<option value="32">Refined: 32 spp · 1280×720</option><option value="256" selected>Longer refinement: 256 spp · 640×360</option></select></label>
<p><strong>Timing caveat:</strong> __TIMING_NOTE__</p>
<p id="description"></p><p id="stats"></p>
<div class="labels"><span>Current renderer</span><span>Experiment</span></div>
<div id="view"><img id="base" alt="Current renderer"><img id="candidate" alt="Experimental renderer"><div id="line"></div></div>
<input id="wipe" type="range" min="0" max="100" value="50" aria-label="Image divider">
<button id="toggle">Toggle full image</button><button id="diffToggle">Show difference ×8</button>
<p><a id="baseLink">Open current image</a> · <a id="candidateLink">Open experiment image</a> · <a id="report">Full measurements</a></p>
<img id="difference" alt="Absolute image difference amplified eight times">
<p><small>Timing uses 960×540 tracing, 1280×720 display, one sample per submission while simulation advances.
Image comparisons use fixed exposure. The 256-spp set uses 480×270 tracing and is for appearance, not timing claims.
It is a less noisy reference to the current renderer, not physical ground truth. More samples cannot fix integration bias or omitted light.
Pixel differences can include Monte Carlo noise; compare both interactive and refined views.</small></p>
<script>
const data = __DATA__;
const $ = id => document.getElementById(id);
const descriptions = {
 'sun6':'Keep 24 view steps; reduce light-path density integration from 12 steps to 6. May change attenuation and sky colour, especially near the horizon.',
 'view12':'Reduce view integration from 24 steps to 12; retain 12 light-path steps. May lose narrow shadow structure or change haze and twilight.',
 'balanced':'12 view steps and 6 light-path steps. Combines both integration approximations.',
 'fast':'8 view steps and 4 light-path steps. Aggressive integration reduction; inspect gradients, silhouettes and eclipse shadows.',
 'planetshine-quarter':'Evaluate reflected moon/planet light in 6 of the 24 cells and multiply by four. Intended to preserve its expected discrete integral, but increases sampling noise.',
 'no-planetshine':'Omit moon/planet light scattered by the atmosphere. Direct sunlight and visible planetary surfaces remain. This can erase moonlit sky glow.'
};
for(const v of data.variants) $('variant').add(new Option(v,v));
for(const c of data.cases) $('scene').add(new Option(c,c));
$('variant').value='balanced';$('scene').value=data.cases.includes('earth-twilight')?'earth-twilight':data.cases[0];
function wipe(){const x=$('wipe').value;$('candidate').style.clipPath=`inset(0 0 0 ${x}%)`;$('line').style.left=x+'%';}
function update(){
 const v=$('variant').value,c=$('scene').value,s=$('samples').value,n=`${c}-spp${s}`;
 const base=`images/reference/${n}.png`,candidate=`images/${v}/${n}.png`;
 $('base').src=base;$('candidate').src=candidate;$('difference').src=`images/${v}/${n}-diff.png`;
 $('baseLink').href=base;$('candidateLink').href=candidate;$('report').href=`data/full/${v}/comparison.md`;
 const t=data.timings[v][c],m=data.metrics[v][n];
 $('description').textContent=descriptions[v];
 $('stats').textContent=`GPU: ${t.baseline.gpu_median_ms.toFixed(2)} → ${t.candidate.gpu_median_ms.toFixed(2)} ms (${t.speedup.toFixed(2)}× throughput). `+
 `Image: mean absolute display difference ${m.display_mae_255.toFixed(3)}/255; ${m.pixels_changed_over_1_255_percent.toFixed(1)}% of pixels differ by more than 1/255. `+
 `HDR relative RMSE: ${m.linear_rgb_relative_rmse === null ? 'unavailable' : (100*m.linear_rgb_relative_rmse).toFixed(2)+'%'}.`;
 wipe();
}
for(const id of ['variant','scene','samples']) $(id).onchange=update;
$('wipe').oninput=wipe;$('toggle').onclick=()=>{$('wipe').value=Number($('wipe').value)>0?0:100;wipe();};
$('diffToggle').onclick=()=>{const shown=$('difference').style.display==='block';$('difference').style.display=shown?'none':'block';$('diffToggle').textContent=shown?'Show difference ×8':'Hide difference';};
update();
</script></html>'''
    (out/'index.html').write_text(page.replace('__DATA__', json.dumps(data, allow_nan=False)).replace('__TIMING_NOTE__', html.escape(args.timing_note)))
    print(out/'index.html')


if __name__ == '__main__':
    main()
