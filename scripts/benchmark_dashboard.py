#!/usr/bin/env python3
"""Build benchmarks/index.html: every renderer implementation, its overall speed and all screenshots.

Reads only checked-in benchmark evidence, so it needs no GPU. Rerun after adding results.
"""
import argparse
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT/'benchmarks'
ORIGINAL = 'baselines/rx7600xt-2026-10-04'
OPTIMIZED = 'optimizations/rx7600xt-2026-10-04'
QUALITY = 'quality/rx7600xt-2026-10-05'
UMBRA = 'quality/eclipse-umbra-edge'
QUALITY_NOTE = 'Another application shared the GPU during these timings; treat them as provisional.'
M4 = 'm4pro-2026-10-15'
M4_NOTE = 'Docker Desktop was running during these runs.'
HARDWARE = [
    {'id': 'rx7600xt', 'name': 'AMD Radeon RX 7600 XT', 'timer': 'GPU timestamps',
     'detail': 'Linux · Vulkan RADV (Mesa 26.2.2) · timings are GPU timestamps'},
    {'id': 'm4pro', 'name': 'Apple M4 Pro', 'timer': 'wall-clock time',
     'detail': 'macOS · Vulkan via MoltenVK · timings are wall-clock (MoltenVK GPU timestamps are unreliable)'},
]

QUALITY_VARIANTS = {
    'sun6': 'Keep 24 view steps; reduce light-path density integration from 12 steps to 6.',
    'view12': 'Reduce view integration from 24 steps to 12; retain 12 light-path steps.',
    'balanced': '12 view steps and 6 light-path steps.',
    'fast': '8 view steps and 4 light-path steps. Adopted as the app\'s atmosphere integration.',
    'planetshine-quarter': 'Evaluate reflected moon/planet light in 6 of the 24 cells and multiply by four.',
    'no-planetshine': 'Omit moon/planet light scattered by the atmosphere.',
}
FOLLOWUP_VARIANTS = {
    'sky-any-hit': 'Ground sky-light rays stop at the first opaque blocker instead of computing the closest hit.',
    'dark-reflection': 'Skip density integration and phase evaluation when reflected incident light is exactly zero.',
    'raw-combined': 'sky-any-hit and dark-reflection together.',
    'view6-sun4': 'Six view integration cells instead of eight; retain four light-density samples.',
    'view8-sun2': 'Two light-path density samples instead of four; retain eight view cells.',
    'reflect-half': 'Evaluate reflected atmospheric lighting in four of eight cells, weighted by two.',
    'guided-ground': '50/50 cosine/reflector-directed ground continuation. Current app default.',
    'guided-raw': 'Guided ground sampling plus both raw candidates.',
}
SHOT = re.compile(r'^(?P<case>.+)-(?P<shot>spp\d+|moving-\d+)\.png$')


def load(rel):
    return json.loads((BENCH/rel).read_text())


def geomean(values):
    values = [v for v in values if v]
    return math.exp(sum(map(math.log, values))/len(values)) if values else None


def timing_rows(timings):
    """Advancing-mode GPU medians, one row per case."""
    return [{'case': t['case'], 'base_ms': t['baseline']['gpu_median_ms'],
             'ms': t['candidate']['gpu_median_ms'], 'speedup': t['speedup']}
            for t in timings if t['mode'] == 'advancing']


def wall_rows(baselines, candidates):
    """Advancing-mode wall medians from summary.json files; cases without a baseline get no speedup."""
    advancing = lambda paths: {r['case']: r['wall_median_ms'] for p in paths for r in load(p) if r['mode'] == 'advancing'}
    base, cand = advancing(baselines), advancing(candidates)
    return [{'case': c, 'base_ms': base.get(c), 'ms': ms, 'speedup': base[c]/ms if c in base else None}
            for c, ms in cand.items()]


def image_summary(metrics):
    if not metrics:
        return None
    return {'images': len(metrics), 'identical': sum(m['display_mae_255'] == 0 for m in metrics),
            'mean_mae_255': sum(m['display_mae_255'] for m in metrics)/len(metrics),
            'max_changed_percent': max(m['pixels_changed_over_1_255_percent'] for m in metrics)}


def shots_in(directory, metrics=None, diff_dir=None):
    shots = {}
    for png in sorted((BENCH/directory).glob('*.png')):
        match = SHOT.match(png.name)
        if not match:
            continue
        key = png.stem
        shot = {'case': match['case'], 'shot': match['shot'], 'src': f'{directory}/{png.name}'}
        if diff_dir and (BENCH/diff_dir/f'{key}-diff.png').exists():
            shot['diff'] = f'{diff_dir}/{key}-diff.png'
        if metrics and key in metrics:
            shot['metrics'] = metrics[key]
        shots[key] = shot
    return shots


def implementations():
    optimized = load(f'{OPTIMIZED}/comparison/comparison.json')
    optimized_metrics = {m['name']: m for m in optimized['images']}
    rows = timing_rows(optimized['timings'])
    impls = [
        {'id': 'original', 'name': 'Original renderer', 'hw': 'rx7600xt', 'group': 'Renderer history', 'date': '2026-10-04',
         'description': 'The published baseline before any optimization (commit e7cb173).',
         'role': 'Baseline', 'speedup': 1.0, 'versus': None, 'timings': [],
         'report': f'{ORIGINAL}/summary.md', 'shots': shots_in(ORIGINAL)},
        {'id': 'optimized', 'name': 'Optimized renderer', 'hw': 'rx7600xt', 'group': 'Renderer history', 'date': '2026-10-04',
         'description': 'First optimization pass (commit 058f4a5). Same images, less GPU work.',
         'speedup': geomean(r['speedup'] for r in rows), 'versus': 'original', 'timings': rows,
         'images': image_summary(optimized['images']), 'report': f'{OPTIMIZED}/README.md',
         'shots': shots_in(OPTIMIZED, optimized_metrics, f'{OPTIMIZED}/comparison')},
    ]
    review, umbra = load(f'{QUALITY}/review.json'), load(f'{UMBRA}/review.json')
    reference_shots = shots_in(f'{QUALITY}/images/reference')
    reference_shots.update(shots_in(f'{UMBRA}/images/reference'))
    impls.append({'id': 'reference', 'name': 'Atmosphere reference (24/12)', 'hw': 'rx7600xt', 'group': 'Atmosphere study',
                  'date': '2026-10-05', 'role': 'Baseline', 'speedup': 1.0, 'versus': None, 'timings': [],
                  'description': '24 view and 12 light-path integration steps: the baseline for the atmosphere approximations.',
                  'note': QUALITY_NOTE, 'report': f'{QUALITY}/README.md', 'shots': reference_shots})
    for variant, description in QUALITY_VARIANTS.items():
        rows = [{'case': c, 'base_ms': t['baseline']['gpu_median_ms'], 'ms': t['candidate']['gpu_median_ms'],
                 'speedup': t['speedup']} for c, t in review['timings'][variant].items()]
        metrics = {**review['metrics'][variant], **umbra['metrics'][variant]}
        shots = shots_in(f'{QUALITY}/images/{variant}', metrics, f'{QUALITY}/images/{variant}')
        shots.update(shots_in(f'{UMBRA}/images/{variant}', metrics, f'{UMBRA}/images/{variant}'))
        impls.append({'id': variant, 'name': variant, 'hw': 'rx7600xt', 'group': 'Atmosphere study', 'date': '2026-10-05',
                      'description': description, 'note': QUALITY_NOTE,
                      'speedup': geomean(r['speedup'] for r in rows), 'versus': 'reference', 'timings': rows,
                      'images': image_summary(list(metrics.values())),
                      'report': f'{QUALITY}/data/full/{variant}/comparison.md', 'shots': shots})
    m4 = [('original', 'Original renderer', 'Commit c3dad2d: the original renderer with the benchmark suite.'),
          ('optimized', 'Optimized renderer', 'Commit 058f4a5: first optimization pass.'),
          ('current', 'Current renderer', 'Current main with the app default guided-ground sampler.'),
          ('faster', 'Faster renderer', 'Planetshine sampled once per camera ray, adaptive atmosphere steps, '
           'shared sky/bounce ray and cheaper shadow tests. Compared with the current renderer.')]
    # The original renderer ran the two newest scenes separately, with the fixtures backported.
    original_runs = [f'{M4}/original', f'{M4}/original-new-scenes']
    for version, name, description in m4:
        impl = {'id': f'm4-{version}', 'hw': 'm4pro', 'name': name, 'group': 'Renderer history',
                'date': '2026-10-15', 'description': description, 'note': M4_NOTE,
                'report': f'{M4}/{version}/summary.md'}
        if version == 'original':
            shots = {}
            for run in original_runs:
                shots.update(shots_in(run))
            impl.update(role='Baseline', speedup=1.0, versus=None, shots=shots,
                        timings=wall_rows([], [f'{run}/summary.json' for run in original_runs]))
        else:
            comparisons = [d for d in [version, f'{version}-new-scenes'] if (BENCH/M4/'comparisons'/d).exists()]
            images = [m for d in comparisons for m in load(f'{M4}/comparisons/{d}/comparison.json')['images']]
            metrics = {m['name']: m for m in images}
            shots = shots_in(f'{M4}/{version}', metrics)
            for d in comparisons:
                for key, shot in shots.items():
                    diff = f'{M4}/comparisons/{d}/{key}-diff.png'
                    if (BENCH/diff).exists():
                        shot['diff'] = diff
            # Each version is compared with the original, except the newest
            # optimizations, which are compared with the renderer they improve.
            baseline = 'current' if version == 'faster' else 'original'
            baseline_runs = [f'{M4}/current'] if version == 'faster' else original_runs
            rows = wall_rows([f'{run}/summary.json' for run in baseline_runs], [f'{M4}/{version}/summary.json'])
            impl.update(speedup=geomean(r['speedup'] for r in rows), versus=f'm4-{baseline}', timings=rows,
                        images=image_summary(images), shots=shots)
        impls.append(impl)
    for variant, description in FOLLOWUP_VARIANTS.items():
        impls.append({'id': variant, 'hw': 'rx7600xt', 'name': variant, 'group': 'Follow-up candidates',
                      'date': '2026-10-07', 'description': description, 'speedup': None, 'versus': None,
                      'timings': [], 'status': 'Not measured yet',
                      'report': 'optimizations/followup-2026-10-07/README.md', 'shots': {}})
    return impls


PAGE = r'''<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width"><title>Stargaze benchmarks</title>
<style>
*{box-sizing:border-box}body{font:15px system-ui;background:#10141b;color:#e5e9f0;margin:0}
main{max-width:1320px;margin:0 auto;padding:24px}a{color:#91caff}h1{font-size:24px;margin:0 0 4px}
h2{font-size:13px;letter-spacing:.08em;text-transform:uppercase;color:#93a1b5;margin:28px 0 8px}
.muted{color:#93a1b5}.small{font-size:13px}button{font:inherit;color:inherit;cursor:pointer}
.crumbs{display:flex;gap:8px;align-items:center;margin-bottom:18px;color:#93a1b5}
.crumbs button{background:none;border:0;padding:0;color:#91caff}
.list{display:grid;gap:6px}
.impl{display:grid;grid-template-columns:1fr 150px 230px;gap:16px;align-items:center;text-align:left;
 background:#18202b;border:1px solid #263243;border-radius:8px;padding:12px 16px}
.impl:hover,.impl:focus{border-color:#5d7fa8;background:#1c2633;outline:none}
.impl .name{font-weight:600}.impl .desc{font-size:13px;color:#93a1b5;margin-top:2px}
.num{font-size:26px;font-variant-numeric:tabular-nums;font-weight:600;text-align:right}
.num small{display:block;font-size:12px;font-weight:400;color:#93a1b5}
.good{color:#7ee2a8}.bad{color:#ff9b8f}.none{color:#5b6779;font-size:16px}
.tag{font-size:11px;border:1px solid #4b5a70;border-radius:10px;padding:1px 7px;margin-left:6px;color:#b9c4d4;font-weight:400}
table{border-collapse:collapse;font-variant-numeric:tabular-nums;font-size:13px;margin-top:8px}
td,th{padding:4px 14px 4px 0;text-align:right}td:first-child,th:first-child{text-align:left}th{color:#93a1b5;font-weight:400}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:10px}
.thumb{background:#18202b;border:1px solid #263243;border-radius:6px;padding:0;overflow:hidden;text-align:left}
.thumb:hover,.thumb:focus{border-color:#5d7fa8;outline:none}.thumb img{width:100%;display:block;aspect-ratio:16/9;object-fit:cover;background:#000}
.thumb div{padding:6px 8px;font-size:12px}.thumb b{font-weight:600}
.runs{display:flex;flex-wrap:wrap;gap:6px;margin:10px 0}
.runs button{background:#18202b;border:1px solid #2f3d51;border-radius:6px;padding:6px 10px;font-size:13px}
.runs button.on{background:#2b4566;border-color:#7aa7dc}.runs button span{color:#93a1b5;margin-left:6px}
.tools{display:flex;gap:6px;flex-wrap:wrap;margin:8px 0}.tools button{background:#253244;border:1px solid #4b5a70;border-radius:5px;padding:5px 10px;font-size:13px}
.tools button.on{background:#2b4566;border-color:#7aa7dc}
#view{position:relative;line-height:0;background:#000;border:1px solid #263243}#view img{width:100%}
#over{position:absolute;inset:0;height:100%;clip-path:inset(0 0 0 50%)}#line{position:absolute;top:0;bottom:0;left:50%;border-left:2px solid #fff}
input[type=range]{width:100%}
</style><main id="app"></main>
<script>
const {hardware, impls: data} = __DATA__;
const byId = Object.fromEntries(data.map(i => [i.id, i]));
const hwById = Object.fromEntries(hardware.map(h => [h.id, h]));
const app = document.getElementById('app');
const esc = s => String(s).replace(/[&<>"]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const SHOT_ORDER = ['spp1','spp32','spp256','moving-8','moving-15'];
const shotLabel = s => s.startsWith('spp') ? `${s.slice(3)} spp` : `moving frame ${s.slice(7)}`;
const shotRank = s => SHOT_ORDER.indexOf(s.shot);
const caseOrder = [...new Set(data.flatMap(i => Object.values(i.shots).map(s => s.case)))];
const sortShots = shots => shots.sort((a,b) => caseOrder.indexOf(a.case)-caseOrder.indexOf(b.case) || shotRank(a)-shotRank(b));
const go = hash => `location.hash='${hash}'`;
function speed(i, big=true){
 if (i.speedup == null) return `<span class="none">${esc(i.status || 'n/a')}</span>`;
 const cls = i.versus ? (i.speedup > 1.02 ? 'good' : i.speedup < 0.98 ? 'bad' : '') : '';
 const scenes = i.timings.filter(t => t.speedup != null).length;
 const vs = i.versus ? `vs ${esc(byId[i.versus].name)} · ${scenes} scenes` : 'baseline';
 return `<span class="${cls}">${i.speedup.toFixed(2)}×</span>${big ? `<small>${vs}</small>` : ''}`;
}
function images(i){
 const m = i.images; if (!m) return i.role ? 'Reference images' : '';
 return m.identical === m.images ? `All ${m.images} images identical`
  : `${m.images - m.identical}/${m.images} images differ · mean ${m.mean_mae_255.toFixed(2)}/255`;
}
const crumbs = parts => `<nav class="crumbs">${parts.map(([label, hash]) => hash ? `<button onclick="${go(hash)}">${esc(label)}</button>` : `<span>${esc(label)}</span>`).join('<span>›</span>')}</nav>`;

function home(){
 let out = `<h1>Stargaze renderer benchmarks</h1><p class="muted small">Pick the hardware. Every comparison stays on that hardware.</p><div class="list">`;
 for (const h of hardware){
  const list = data.filter(i => i.hw === h.id), best = list.filter(i => i.versus && i.speedup).sort((a,b) => b.speedup - a.speedup)[0];
  out += `<button class="impl" onclick="${go(`#/${h.id}`)}"><div><div class="name">${esc(h.name)}</div><div class="desc">${esc(h.detail)}</div></div>
   <div class="num">${best ? `<span class="good">${best.speedup.toFixed(2)}×</span><small>best: ${esc(best.name)}</small>` : ''}</div><div class="small muted">${list.length} implementations</div></button>`;
 }
 app.innerHTML = out + '</div>';
}

function menu(hw){
 const h = hwById[hw]; if (!h) return home();
 let out = crumbs([['Hardware','#/'],[h.name]]);
 out += `<h1>${esc(h.name)}</h1><p class="muted small">${esc(h.detail)}. Overall number = geometric mean of throughput speedups across scenes (advancing simulation), relative to the baseline named under it. Higher is faster.</p>`;
 const list = data.filter(i => i.hw === hw);
 for (const group of [...new Set(list.map(i => i.group))]){
  out += `<h2>${esc(group)}</h2><div class="list">`;
  for (const i of list.filter(i => i.group === group)){
   const n = Object.keys(i.shots).length;
   out += `<button class="impl" onclick="${go(`#/${hw}/impl/${i.id}`)}"><div><div class="name">${esc(i.name)}${i.role ? `<span class="tag">${esc(i.role)}</span>` : ''}</div><div class="desc">${esc(i.description)}</div></div>
    <div class="num">${speed(i)}</div><div class="small muted">${n ? `${n} screenshots` : 'No screenshots'}<br>${esc(images(i))}</div></button>`;
  }
  out += '</div>';
 }
 app.innerHTML = out;
}

function impl(hw, id){
 const i = byId[id]; if (!i || i.hw !== hw) return menu(hw);
 const h = hwById[hw];
 let out = crumbs([['Hardware','#/'],[h.name,`#/${hw}`],[i.name]]);
 out += `<h1>${esc(i.name)}</h1><p class="muted">${esc(i.description)}</p><div class="num" style="text-align:left">${speed(i)}</div>`;
 out += `<p class="small muted">${esc(h.name)} · ${esc(h.timer)}${i.note ? ' · ' + esc(i.note) : ''}</p>`;
 if (i.timings.length) out += `<table><tr><th>Scene</th><th>Baseline ms</th><th>This ms</th><th>Speedup</th></tr>${i.timings.map(t => `<tr><td>${esc(t.case)}</td><td>${t.base_ms == null ? '' : t.base_ms.toFixed(2)}</td><td>${t.ms.toFixed(2)}</td><td>${t.speedup == null ? '' : t.speedup.toFixed(2)+'×'}</td></tr>`).join('')}</table>`;
 out += `<p class="small"><a href="${esc(i.report)}">Full report</a> · ${esc(images(i))}</p>`;
 const shots = sortShots(Object.entries(i.shots).map(([key, s]) => ({key, ...s})));
 out += shots.length ? `<h2>Screenshots</h2><div class="grid">${shots.map(s => `<button class="thumb" onclick="${go(`#/${hw}/shot/${s.key}/${i.id}`)}"><img loading="lazy" src="${esc(s.src)}" alt=""><div><b>${esc(s.case)}</b> · ${shotLabel(s.shot)}${s.metrics ? `<br><span class="muted">${s.metrics.display_mae_255 === 0 ? 'identical' : `diff ${s.metrics.display_mae_255.toFixed(2)}/255`}</span>` : ''}</div></button>`).join('')}</div>`
  : `<p class="muted">No runs yet. Commands are in the <a href="${esc(i.report)}">report</a>.</p>`;
 app.innerHTML = out;
}

let mode = 'single';
function shot(hw, key, id){
 const h = hwById[hw]; if (!h) return home();
 const runs = data.filter(i => i.hw === hw && i.shots[key]);
 const i = runs.find(r => r.id === id) || runs[0]; if (!i) return menu(hw);
 const s = i.shots[key], base = i.versus && byId[i.versus].shots[key];
 if ((mode === 'compare' && !base) || (mode === 'diff' && !s.diff)) mode = 'single';
 let out = crumbs([['Hardware','#/'],[h.name,`#/${hw}`],[i.name,`#/${hw}/impl/${i.id}`],[`${s.case} · ${shotLabel(s.shot)}`]]);
 out += `<h1>${esc(s.case)} · ${shotLabel(s.shot)}</h1><p class="small muted">Every ${esc(h.name)} run that captured this screenshot. ← → switch run · Esc back.</p>`;
 out += `<div class="runs">${runs.map(r => `<button class="${r === i ? 'on' : ''}" onclick="${go(`#/${hw}/shot/${key}/${r.id}`)}">${esc(r.name)}<span>${r.speedup == null ? '' : r.speedup.toFixed(2)+'×'}</span></button>`).join('')}</div>`;
 const m = s.metrics;
 out += `<div class="tools"><button data-m="single">This run</button>${base ? `<button data-m="compare">Wipe vs ${esc(byId[i.versus].name)}</button>` : ''}${s.diff ? '<button data-m="diff">Difference ×8</button>' : ''}</div>`;
 if (m) out += `<p class="small muted">vs ${esc(byId[i.versus].name)}: mean ${m.display_mae_255.toFixed(3)}/255 · ${m.pixels_changed_over_1_255_percent.toFixed(2)}% pixels changed &gt;1/255 · HDR relative RMSE ${m.linear_rgb_relative_rmse == null ? 'n/a' : (100*m.linear_rgb_relative_rmse).toFixed(2)+'%'}</p>`;
 out += `<div id="view"></div><div id="wiper"></div><p class="small"><a href="${esc(s.src)}">Open image</a></p>`;
 app.innerHTML = out;
 const view = document.getElementById('view'), wiper = document.getElementById('wiper');
 app.querySelectorAll('.tools button').forEach(b => { b.classList.toggle('on', b.dataset.m === mode); b.onclick = () => { mode = b.dataset.m; shot(hw, key, i.id); }; });
 if (mode === 'compare'){
  view.innerHTML = `<img src="${esc(base.src)}" alt="baseline"><img id="over" src="${esc(s.src)}" alt="this run"><div id="line"></div>`;
  wiper.innerHTML = `<div class="small muted" style="display:flex;justify-content:space-between"><span>${esc(byId[i.versus].name)}</span><span>${esc(i.name)}</span></div><input type="range" value="50" aria-label="Divider">`;
  wiper.querySelector('input').oninput = e => { document.getElementById('over').style.clipPath = `inset(0 0 0 ${e.target.value}%)`; document.getElementById('line').style.left = e.target.value+'%'; };
 } else view.innerHTML = `<img src="${esc(mode === 'diff' ? s.diff : s.src)}" alt="">`;
 shot.current = {hw, key, runs, i};
}

document.addEventListener('keydown', e => {
 const [, hw, page] = location.hash.split('/');
 if (e.key === 'Escape') {
  if (page === 'shot') location.hash = `#/${hw}/impl/${shot.current.i.id}`;
  else if (page === 'impl') location.hash = `#/${hw}`;
  else if (hw) location.hash = '#/';
 }
 if (page === 'shot' && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')){
  const {key, runs, i} = shot.current, n = runs.indexOf(i) + (e.key === 'ArrowRight' ? 1 : -1);
  location.hash = `#/${hw}/shot/${key}/${runs[(n + runs.length) % runs.length].id}`;
 }
});
function route(){
 // Links from before the hardware level (#/impl/<id>, #/shot/<key>/<id>): infer the hardware.
 const [, legacy, x, y] = location.hash.split('/');
 const legacyImpl = legacy === 'impl' ? byId[x] : legacy === 'shot' ? byId[y] : null;
 if (legacyImpl) return location.replace(`#/${legacyImpl.hw}/${legacy}/${x}${legacy === 'shot' ? '/' + y : ''}`);
 const [, hw, page, a, b] = location.hash.split('/');
 !hw ? home() : page === 'impl' ? impl(hw, a) : page === 'shot' ? shot(hw, a, b) : menu(hw);
 if (page !== 'shot') scrollTo(0, 0);
}
addEventListener('hashchange', route); route();
</script></html>'''


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    impls = implementations()
    for i in impls:
        for s in i['shots'].values():
            for path in [s['src'], s.get('diff')]:
                if path and not (BENCH/path).exists():
                    raise FileNotFoundError(path)
    output = BENCH/'index.html'
    page_data = {'hardware': HARDWARE, 'impls': impls}
    output.write_text(PAGE.replace('__DATA__', json.dumps(page_data, allow_nan=False)))
    print(output)


if __name__ == '__main__':
    main()
