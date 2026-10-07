#!/usr/bin/env python3
"""Summarize the fixed Atacama low-light GPU study and publish its comparisons."""
import argparse
from array import array
import json
import math
from pathlib import Path
import shutil
import statistics


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    src, out = args.input, args.output
    timing = json.loads((src/'advancing.json').read_text())
    width, height = timing['width'], timing['height']
    # Flat foreground: avoid the horizon, prominent rocks and bright sky.
    x0, y0, x1, y1 = width//8, height*211//270, width*7//8, height*260//270
    region = [y*width+x for y in range(y0,y1) for x in range(x0,x1)]

    def read(name):
        values = array('f')
        values.frombytes((src/(name+'.f32')).read_bytes())
        assert len(values) == width*height*4
        return [sum(values[4*i+c]*k for c,k in enumerate((0.2126,0.7152,0.0722))) for i in region]

    reference = read('reference-1024')
    measurements = {}
    for variant in ['reference','guided-ground','no-surface-sky','no-ground-bounces']:
        for spp in ([1,16,64,1024] if variant in ['reference','guided-ground'] else [64]):
            name = f'{variant}-{spp}'
            pixels = read(name)
            measurements[name] = {
                'mean_luminance': statistics.mean(pixels),
                'rmse_to_reference1024': math.sqrt(statistics.mean((p-r)**2 for p,r in zip(pixels,reference))),
            }
    gpu = {}
    for variant in ['reference','guided-ground']:
        runs = [r for r in timing['runs'] if r['variant']==variant]
        gpu[variant] = {'median_ms': statistics.median(t for r in runs for t in r['gpu_ms']),
                        'repeat_medians_ms': [r['gpu_median_ms'] for r in runs]}
    summary = {'region_xyxy': [x0,y0,x1,y1], 'measurements': measurements, 'gpu': gpu}
    out.mkdir(parents=True,exist_ok=True)
    (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    for name in measurements:
        shutil.copyfile(src/(name+'.png'), out/(name+'.png'))
    shutil.copyfile(src/'advancing.json',out/'advancing.json')
    if (src/'manifest.json').exists():
        shutil.copyfile(src/'manifest.json',out/'manifest.json')
    page = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Stargaze low-light sampling comparison</title><style>
body{background:#141820;color:#e9e8e4;font:16px system-ui;max-width:1100px;margin:30px auto;padding:0 16px}
a{color:#8fcbff}.pair{display:grid;grid-template-columns:1fr 1fr;gap:12px}img{width:100%}figure{margin:0}p{line-height:1.5}
@media(max-width:650px){.pair{grid-template-columns:1fr}}
</style><h1>Low-light sampling</h1><p>Left: current renderer. Right: guided ground bounces.
Both use the same exposure, resolution and number of samples. The prototype is benchmark-only.
<a href="README.md">Measurements and limitations</a>.</p>'''
    for spp in [1,16,64,1024]:
        page += f'<h2>{spp} samples per pixel</h2><div class="pair">'
        for variant,caption in [('reference','Current renderer'),('guided-ground','Guided ground')]:
            name = f'{variant}-{spp}.png'
            page += f'<figure><a href="{name}"><img src="{name}" alt="{caption}, {spp} samples"></a><figcaption>{caption}</figcaption></figure>'
        page += '</div>'
    page += '''<h2>Diagnostic omissions — not fixes</h2><p>These remove real light paths solely to locate the noise source.</p><div class="pair">'''
    for name, caption in [('no-surface-sky-64','Ground sky-light contribution disabled'),('no-ground-bounces-64','Ground bounce continuation disabled')]:
        page += f'<figure><img src="{name}.png" alt="{caption}"><figcaption>{caption}</figcaption></figure>'
    (out/'index.html').write_text(page+'</div></html>')
    print(json.dumps(summary,indent=2))


if __name__ == '__main__':
    main()
