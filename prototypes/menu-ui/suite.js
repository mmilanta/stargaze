'use strict';
/* Stargaze · Celestial Blueprint — every in-game menu page as one interactive mockup.
   Illustrative browser state only; nothing here talks to the Rust renderer. */

const $ = (s, r = document) => r.querySelector(s);
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const num = (n, d = 1) => n.toLocaleString('en-US', { minimumFractionDigits: d, maximumFractionDigits: d });
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const rgba = (c, a) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;

const ICON = {
  logo: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(-35 12 12)"/></svg>',
  arrow: '<svg viewBox="0 0 24 24"><path d="M4 12h15M13 6l6 6-6 6"/></svg>',
  search: '<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="6"/><path d="m20 20-4.5-4.5"/></svg>',
  star: '<svg viewBox="0 0 24 24"><path d="m12 3 2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z"/></svg>',
  lock: '<svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>',
  unlock: '<svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/></svg>',
  play: '<svg viewBox="0 0 24 24" style="fill:currentColor"><path d="M8 5v14l11-7z"/></svg>',
  pause: '<svg viewBox="0 0 24 24" style="fill:currentColor;stroke:none"><rect x="6" y="5" width="4" height="14"/><rect x="14" y="5" width="4" height="14"/></svg>',
  left: '<svg viewBox="0 0 24 24"><path d="m15 6-6 6 6 6"/></svg>',
  right: '<svg viewBox="0 0 24 24"><path d="m9 6 6 6-6 6"/></svg>',
  sun: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>',
  eye: '<svg viewBox="0 0 24 24"><path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></svg>',
  menu: '<svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h16"/></svg>',
  target: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M12 2v4M12 18v4M2 12h4M18 12h4"/></svg>',
};

/* ---------------------------------------------------------------- data */
// Mirrors configs/*.yaml, sorted by path as menu::discover does.
const SYSTEMS = [
  { id: 'halo', file: 'halo.yaml', name: 'Calyx and Halo', short: 'Calyx & Halo', puzzle: 'Amber horizon', host: 'Halo', lat: '35° N', fov: 80, bodies: 12, stars: 2, atmo: 'Earth-like', day: 1440, tags: ['Binary stars', 'Rings', 'Tidal lock'],
    desc: 'The default observatory. Stand on Halo, the tidally locked inner moon of Calyx. Calyx and its rings stay fixed overhead while two suns wheel past.',
    targets: ['Calyx', 'Aur', 'Igni', 'Umbra', 'Nerid', 'Vantus'], eclipse: 'Iri shadow on Vantus · day 444.478', transit: 'Umbra crosses Calyx · day 12.806' },
  { id: 'median-resonance', file: 'median-resonance.yaml', name: 'Median and the three resonant moons', short: 'Median', puzzle: 'Clockwork sky', host: 'Cadence', lat: '30° N', fov: 50, bodies: 6, stars: 2, atmo: 'Dense', day: 1440, tags: ['Binary stars', 'Resonance', 'Dense air'],
    desc: 'Three moons in resonance around Median. The dense haze over Cadence hides the background stars in daylight. Wait for night, or for an eclipse.',
    targets: ['Median', 'Pulse', 'Refrain', 'Lumen', 'Umber'], eclipse: 'Pulse eclipses Umber · day 31.250', transit: 'Refrain crosses Median · day 4.118' },
  { id: 'puzzle', file: 'puzzle.yaml', name: 'Reconstruction - first field study', short: 'First field study', puzzle: 'First field study', host: 'Lookout', lat: '15° N', fov: 35, bodies: 5, stars: 1, atmo: 'Earth-like', day: 1440, tags: ['Tutorial', 'Gas giant'],
    desc: 'One giant and its moons, seen from the outermost one. The introductory reconstruction puzzle.',
    targets: ['Giant', 'Sun', 'Inner', 'Middle'], eclipse: 'Inner eclipses the Sun · day 2.604', transit: 'Middle crosses Giant · day 1.375' },
  { id: 'solar-system', file: 'solar-system.yaml', name: 'Solar System', short: 'Solar System', puzzle: 'Distant lights', host: 'Earth', lat: '35° N', fov: 1.1, bodies: 19, stars: 1, atmo: 'Earth-like', day: 1440, tags: ['Real data', 'Rings'],
    desc: 'Start close to home. Watch the Moon from Earth, then explore the familiar dance of eight planets and their moons.',
    targets: ['Moon', 'Sun', 'Mercury', 'Venus', 'Mars', 'Jupiter', 'Saturn', 'Uranus', 'Neptune'], eclipse: 'Moon eclipses the Sun · day 245.920', transit: 'Phobos crosses Mars · day 88.412' },
  { id: 'vesper', file: 'vesper.yaml', name: 'Vesper and the bright wanderers', short: 'Vesper', puzzle: 'Bright wanderers', host: 'Vesper', lat: '20° N', fov: 60, bodies: 10, stars: 1, atmo: 'Earth-like', day: 1444.63, tags: ['Resonance', 'Rings', 'Open sky'],
    desc: 'Two moons in a 2:1 resonance and two bright wandering planets. The view opens on empty northern sky, so the planets must be found by observing.',
    targets: ['Aureole', 'Cinder', 'Chime', 'Echo'], eclipse: 'Chime eclipses Solis · day 17.502', transit: 'Mica crosses Aureole · day 6.930' },
];
const BROKEN = { id: 'three-suns', file: 'three-suns.yaml', name: 'three-suns', short: 'three-suns', puzzle: 'Uncharted map 06', broken: true, tags: [],
  error: "Invalid config configs/three-suns.yaml\nbodies[4] (id: wisp): parent 'sol' does not exist.\nKnown ids: solis, cinder, vesper, aureole.\n  at line 48, column 13" };

// Hidden solutions for scoring (the same tree shapes as the configs). Never shown to the player.
const TRUTH = {
  puzzle: { star: 1, kids: [{ kids: [{}, {}, { viewer: 1 }] }] },
  vesper: { star: 1, kids: [{ kids: [{}] }, { viewer: 1, kids: [{}, {}] }, { rings: 1, kids: [{}, {}, {}] }] },
  'median-resonance': { star: 1, kids: [{ star: 1 }, { kids: [{}, { viewer: 1 }, {}] }] },
  halo: { kids: [{ star: 1 }, { star: 1 }, { kids: [{}] }, { rings: 1, kids: [{ viewer: 1 }, {}] }, { kids: [{}, {}, {}] }] },
  'solar-system': { star: 1, kids: [{}, {}, { viewer: 1, kids: [{}] }, { kids: [{}, {}] }, { kids: [{}, {}, {}, {}] }, { rings: 1, kids: [{}] }, { kids: [{}] }, { kids: [{}] }] },
};

// Drawn skies. Positions are normalized to the view (x across, y down), radii to its height.
const SKY = {
  halo: { top: [1, 3, 12], mid: [6, 16, 44], glow: [40, 60, 110], starA: 1, light: [0.85, -0.35], preview: { cx: .5, cy: .38, zoom: 1.25 }, bodies: [
    { n: 'Calyx', k: 'planet', x: .5, y: .36, r: .19, c: [201, 180, 142], bands: 1, fixed: 1, rings: { a: 1.3, b: 2.15, rot: -.32, flat: .2, c: [232, 220, 192] } },
    { n: 'Umbra', k: 'moon', x: .78, y: .2, r: .013, c: [160, 165, 175] },
    { n: 'Aur', k: 'star', x: .93, y: .56, r: .008, c: [255, 236, 190] },
    { n: 'Igni', k: 'star', x: .07, y: .62, r: .005, c: [255, 190, 150] },
    { n: 'Nerid', k: 'planet', x: .19, y: .17, r: .003, c: [160, 200, 240] },
    { n: 'Vantus', k: 'planet', x: .31, y: .09, r: .0025, c: [214, 185, 140] }] },
  'median-resonance': { top: [10, 16, 48], mid: [42, 47, 90], glow: [120, 96, 110], starA: .3, light: [-.7, -.5], preview: { cx: .45, cy: .42, zoom: 1.5 }, bodies: [
    { n: 'Median', k: 'planet', x: .45, y: .42, r: .12, c: [120, 150, 190], bands: 1, vx: .004 },
    { n: 'Pulse', k: 'moon', x: .7, y: .28, r: .016, c: [190, 180, 170], vx: .02 },
    { n: 'Refrain', k: 'moon', x: .24, y: .26, r: .01, c: [170, 170, 190], vx: -.01 },
    { n: 'Lumen', k: 'star', x: .86, y: .7, r: .006, c: [255, 210, 170] }] },
  puzzle: { top: [2, 4, 14], mid: [8, 20, 52], glow: [60, 80, 130], starA: 1, light: [.9, .2], preview: { cx: .52, cy: .42, zoom: 1.4 }, bodies: [
    { n: 'Giant', k: 'planet', x: .52, y: .42, r: .16, c: [196, 160, 120], bands: 1, vx: .002 },
    { n: 'Inner', k: 'moon', x: .3, y: .3, r: .014, c: [200, 196, 188], vx: .03 },
    { n: 'Middle', k: 'moon', x: .74, y: .25, r: .018, c: [168, 180, 196], vx: -.015 },
    { n: 'Sun', k: 'star', x: .96, y: .76, r: .006, c: [255, 240, 210] }] },
  'solar-system': { top: [1, 2, 8], mid: [4, 10, 30], glow: [30, 45, 90], starA: .8, light: [-1, .15], preview: { cx: .5, cy: .45, zoom: 1.1 }, bodies: [
    { n: 'Moon', k: 'moon', x: .5, y: .45, r: .24, c: [205, 203, 196], maria: 1, vx: .01 }] },
  vesper: { top: [1, 3, 12], mid: [5, 14, 40], glow: [36, 54, 100], starA: 1, light: [.6, .7], preview: { cx: .5, cy: .38, zoom: 1.6 }, bodies: [
    { n: 'Aureole', k: 'planet', x: .7, y: .32, r: .006, c: [226, 214, 180], rings: { a: 1.4, b: 2.2, rot: .4, flat: .35, c: [230, 222, 200] } },
    { n: 'Cinder', k: 'planet', x: .3, y: .44, r: .004, c: [226, 128, 92] },
    { n: 'Chime', k: 'moon', x: .56, y: .2, r: .022, c: [214, 214, 222], vx: .04 },
    { n: 'Echo', k: 'moon', x: .15, y: .26, r: .014, c: [190, 190, 176], vx: .02 }] },
};

const SPEEDS = [-1000, -100, -10, -1, 0, 1, 10, 100, 1000]; // min/s: decade steps (idea.md feat1)
const STOP = 4;

/* ---------------------------------------------------------------- state */
const S = {
  screen: 'title', back: 'title', rail: true, notes: false,
  variant: { systems: 'normal', observe: 'normal', maps: 'normal' },
  titleSel: 0, sel: 0, mapSel: 0, favs: new Set(['halo']), search: '', filter: 'all',
  loaded: 'vesper', map: 'puzzle',
  best: { puzzle: 72.4, halo: null, 'median-resonance': 41.7, 'solar-system': null, vesper: 100 },
  obs: null, pobs: null,
  theory: null, dialog: null, pendingMap: null, ctx: null,
  settingsTab: 0,
  settings: { spp: 1, bounces: 8, auto: true, key: .25, pct: .9, ev: 0, hud: 100, labels: true, orient: 'horizon', steps: 'decade', grid: true },
  toast: null, theories: {},
};

function freshObs(sysId, puzzle) {
  const sky = SKY[sysId];
  return { sys: sysId, puzzle, labels: !puzzle && S.settings.labels, hud: true, speed: puzzle ? STOP : 5, prevSpeed: 5, t: 0,
    cx: .5, cy: sysId === 'vesper' ? .45 : .46, zoom: 1, lock: null, orient: S.settings.orient, capture: 0,
    ev: 0, auto: true, spp: 0, targets: !puzzle, objective: true, dayFail: false, sky };
}
function freshTheory(map, demo) {
  const t = { map, nodes: [{ id: 0, parent: null, x: 0, y: 0, star: false, rings: false }], next: 1, sel: 0, viewer: null,
    undo: [], redo: [], score: null, attempts: [], edits: 0,
    msg: 'Select a parent, then right-click empty space to add. [N] also adds at the cursor.' };
  if (demo) {
    t.nodes = [
      { id: 0, parent: null, x: 0, y: 0, star: true, rings: false },
      { id: 1, parent: 0, x: 250, y: -40, star: false, rings: true },
      { id: 2, parent: 1, x: 318, y: 40, star: false, rings: false },
      { id: 3, parent: 1, x: 250, y: 115, star: false, rings: false },
      { id: 4, parent: 1, x: 125, y: -120, star: false, rings: false }];
    t.next = 5; t.sel = 4; t.viewer = 4; t.edits = 7; t.attempts = [{ total: 58.0 }, { total: 72.4 }];
    t.msg = 'Viewer placed on the selected object.';
  }
  return t;
}
S.obs = freshObs('vesper', false);
S.pobs = freshObs('puzzle', true);
S.theory = freshTheory('puzzle', true);
S.theories.puzzle = S.theory;

const sysById = id => SYSTEMS.find(s => s.id === id) || BROKEN;
const listSystems = () => S.variant.systems === 'empty' ? [] : S.variant.systems === 'invalid' ? [...SYSTEMS, BROKEN] : SYSTEMS;
const listMaps = () => [...SYSTEMS].sort((a, b) => (a.id === 'puzzle' ? -1 : b.id === 'puzzle' ? 1 : 0));

/* ---------------------------------------------------------------- screen registry */
const GROUPS = [
  ['Start', [['title', 'Main menu', 'new']]],
  ['Explore', [['systems', 'Explore list', 'game', [['invalid', 'Invalid config'], ['empty', 'No systems found']]],
    ['observe', 'Observation HUD', 'game', [['hidden', 'HUD hidden [H]'], ['search', 'Eclipse search [E]'], ['noday', 'No day found']]]]],
  ['Play · levels', [['maps', 'Levels', 'game', [['failed', 'Map failed to load']]], ['pobserve', 'Level HUD', 'game'],
    ['theory', 'Your theory', 'game'], ['clear', 'Clear theory?', 'game'], ['result', 'Check results', 'game'], ['discard', 'Discard progress?', 'new']]],
  ['System', [['settings', 'Settings', 'new'], ['controls', 'Controls', 'new']]],
];
const ORDER = GROUPS.flatMap(g => g[1].map(i => i[0]));

const NOTES = {
  title: { t: 'Main menu', now: ['Not in the game yet: it opens straight into halo.yaml (or puzzle.yaml with --game).'],
    add: ['Just the wordmark and four entries: Play, Explore, Settings, Quit.', 'Play opens the levels (today launched with --game).', 'The live sky, dimmed, sits behind every menu; in the game this can be the traced view.', 'Keyboard cursor, mouse hover moves it too; console-style key prompts along the bottom.'], src: 'src/main.rs · args / game_requested()' },
  systems: { t: 'Explore list', now: ['"STARGAZE / Choose a solar system" and Resume [Esc].', 'Rows [1]–[9] with the system name and filename; PgUp/PgDn paging.', 'Refreshes configs/ every time it opens; invalid YAML stays listed and shows its error.'],
    add: ['One column, one yellow Explore button per system, nothing else.', 'Invalid configs stay in the list with their error and a disabled button.'], src: 'src/menu.rs · build(), discover()' },
  observe: { t: 'Observation HUD', now: ['Bottom bar: [M]enu, [L]abels, Time ← speed →, Stop [Spc], -1d [PgDn], +1d [PgUp], minute clock.', 'Lock indicator: "Locked [U]" + target, or "Unlocked [U] / click sky".', '[S]tars orientation, EV slider [,][.], Auto [A].', 'Right-click locks a body or a fixed sky direction; left-click releases it.', 'H hides the HUD; Menu and the lock stay visible.', 'E/T search eclipses/transits; F1–F9 aim; R resets; scroll zooms.'],
    add: ['Decade speed steps: 1, 10, 100, 1000 min/s, with the minute clock next to the speed (idea.md feat1).', 'Every control shows its key (feat2).', 'Star orientation as a Horizon / Stars choice next to the lock (feat5). With Stars held, the horizon turns instead.', 'Targets panel [G] lists the F-keys and event searches.', 'Samples-per-pixel meter; grain clears as the image converges while time is stopped.'], src: 'src/ui.rs · layout_mode(), build_bar(), build_lock()' },
  maps: { t: 'Levels', now: ['"MAPS / Choose a map to start a new puzzle", spoiler-free titles, "Start a fresh theory".', 'Choosing a map clears the one theory you have.'],
    add: ['Levels in a column: title, progress bar, status, best score and objects drawn.', 'Yellow Play / Resume per level; progress is kept per level.', 'Discard progress [D] on the side, behind a confirmation.'], src: 'src/menu.rs · Entry::puzzle_title()' },
  pobserve: { t: 'Level HUD', now: ['Top-left status: "STOPPED | +0.0 min/s | View locked".', 'The Labels button becomes Draw [Tab].', 'The lock shows only Star, Planet / moon or Background sky.', 'L, E, T and F1–F9 are disabled; R returns to the initial view.'],
    add: ['Objective card with the next steps.', 'Theory [Tab] keeps its place in the bottom bar (feat3).'], src: 'src/main.rs · lock_label(), key handling' },
  theory: { t: 'Your theory', now: ['"YOUR THEORY": select a parent, right-click to add [N], drag to order orbits. 1 = innermost.', 'Buttons: [S]tar, Has [R]ings, Redo [Y], [V]iewer here, Delete [Del], Undo [Z], Check [Enter], Clear all [X], [M]enu, Observe [Tab].', 'Gold rays mark stars; an oval marks rings. YOU marks the viewer.', 'Score = 60% ordered structure + 20% star/ring traits + 20% viewer.', 'Limit: 64 objects. 100 undo steps.'],
    add: ['Full-page diagram with every action in one bottom bar (idea.md feat6).', 'Inspector for the selected object; ghost line preview before right-click.', 'No inward / outward buttons: dragging sets the orbit order.'], src: 'src/game.rs · build(), score()' },
  clear: { t: 'Clear the entire theory?', now: ['Exact copy: "Removes all objects and the viewer. Undo can restore everything."', 'Cancel [Esc] · Clear all [Enter].'], add: ['Shows how many objects will be removed.'], src: 'src/game.rs · confirm_clear' },
  result: { t: 'Check results', now: ['A single line: "Score: 72.4/100  Orbits: 80.0%  Traits: 90.0%  Viewer: matches".', 'Without a viewer: "Select an object and choose Viewer here [V] before checking."'],
    add: ['A score dial and a weighted breakdown.', 'Attempt history. The hidden solution is never revealed.'], src: 'src/game.rs · Score::total()' },
  discard: { t: 'Discard progress?', now: ['No per-level progress today; picking a map silently clears the theory.'], add: ['Discarding removes the theory, checks and best score for one level.', 'Cancel [Esc] · Discard [Enter].'], src: 'idea.md · feat4' },
  settings: { t: 'Settings', now: ['Only environment variables today: STARGAZE_SPP, _BOUNCES, _AUTO_EXPOSURE, _EXPOSURE, _EV_BIAS, _AUTO_KEY, _AUTO_PERCENTILE, _FOV.'], add: ['The same options in the game, each labelled with its variable.'], src: 'README.md · Quality settings' },
  controls: { t: 'Controls', now: ['Keys are listed in the README and the window title.'], add: ['A complete reference in the game. Keys marked "new" come from this mockup.'], src: 'README.md · Controls' },
};

/* ---------------------------------------------------------------- shell */
function renderRail() {
  const cur = S.screen, v = S.variant[cur];
  $('#rail').innerHTML = `
    <div class="rail-head"><a href="index.html">← All concepts</a> · <a href="buttons.html">Button motion ↗</a><h1>Blueprint · all screens</h1><p>Every menu page in Stargaze, in the Celestial Blueprint style.</p></div>
    ${GROUPS.map(([g, items]) => `<div class="rail-group"><h2>${g}</h2>${items.map(([id, label, tag, subs]) => `
      <button class="rail-item ${cur === id && (!v || v === 'normal') ? 'active' : ''}" data-go="${id}"><span class="n">${String(ORDER.indexOf(id)).padStart(2, '0')}</span>${label}<span class="tag ${tag === 'new' ? 'new' : ''}">${tag === 'new' ? 'NEW' : 'IN GAME'}</span></button>
      ${(subs || []).map(([sv, sl]) => `<button class="rail-item sub ${cur === id && v === sv ? 'active' : ''}" data-go="${id}" data-variant="${sv}">${sl}</button>`).join('')}`).join('')}</div>`).join('')}
    <div class="rail-legend"><kbd>\`</kbd>screen index<br><kbd>?</kbd>screen notes<br><kbd>[</kbd><kbd>]</kbd>previous / next screen<br>IN GAME = the game has this screen today.<br>NEW = a proposed screen.</div>`;
}
function renderNotes() {
  const n = NOTES[S.screen], el = $('#notes');
  el.hidden = !S.notes;
  if (!S.notes || !n) return;
  el.innerHTML = `<div class="eyebrow">Screen ${String(ORDER.indexOf(S.screen)).padStart(2, '0')} / notes</div><h3>${n.t}</h3>
    <div class="faint" style="font-size:9px;letter-spacing:1.2px">IN THE GAME TODAY</div><ul>${n.now.map(x => `<li>${esc(x)}</li>`).join('')}</ul>
    <div style="font-size:9px;letter-spacing:1.2px;color:var(--accent)">IN THIS MOCKUP</div><ul>${n.add.map(x => `<li>${esc(x)}</li>`).join('')}</ul>
    <div class="src">Source · ${n.src}</div>`;
}
function go(screen, variant) {
  if (['settings', 'controls'].includes(screen) && !['settings', 'controls'].includes(S.screen)) S.back = S.screen;
  S.ctx = null;
  const dialogs = { clear: 'clear', result: 'result', discard: 'discard' };
  if (dialogs[screen]) {
    S.screen = screen === 'discard' ? 'maps' : 'theory';
    if (screen === 'result') runCheck(true);
    S.dialog = screen;
  } else { S.screen = screen; S.dialog = null; }
  if (variant !== undefined && S.variant[S.screen] !== undefined) S.variant[S.screen] = variant;
  else if (S.variant[S.screen] !== undefined && variant === undefined) S.variant[S.screen] = 'normal';
  if (S.screen === 'observe') {
    const o = S.obs, vv = S.variant.observe;
    o.hud = vv !== 'hidden'; o.dayFail = vv === 'noday';
    if (vv === 'search') setTimeout(() => runSearch('E'), 60);
  }
  if (S.screen === 'systems') S.sel = S.variant.systems === 'invalid' ? SYSTEMS.length : clamp(S.sel, 0, Math.max(0, listSystems().length - 1));
  if (S.dialog === 'result' && !T().score) { S.dialog = null; toast('Place the viewer [V] before checking.'); }
  if (S.dialog === 'discard' && !S.pendingMap) S.pendingMap = 'puzzle';
  render();
  S.railPick = screen;
}
function render() {
  document.body.classList.toggle('rail-closed', !S.rail);
  renderRail(); renderNotes();
  const st = $('#screen');
  const fn = { title: vTitle, systems: vSystems, observe: vObserve, pobserve: vObserve, maps: vMaps, theory: vTheory, settings: vSettings, controls: vControls }[S.screen];
  st.innerHTML = fn();
  st.className = 'screen' + (['observe', 'pobserve', 'theory'].includes(S.screen) ? ' full' : '');
  mount();
  renderDialog();
}
function toast(text, opts = {}) {
  S.toast = { text, done: opts.done !== false, id: Math.random() };
  const id = S.toast.id;
  paintToast();
  clearTimeout(toast.timer);
  if (opts.sticky) return;
  toast.timer = setTimeout(() => { if (S.toast && S.toast.id === id) { S.toast = null; paintToast(); } }, opts.ms || 2600);
}
function paintToast() {
  const el = $('#toast');
  el.hidden = !S.toast;
  if (S.toast) { el.className = 'toast' + (S.toast.done ? ' done' : ''); el.innerHTML = `<span class="spin"></span>${S.toast.text}`; }
}

/* ---------------------------------------------------------------- shared pieces */
const brand = (sub = 'CELESTIAL ATLAS') => `<button class="brand" data-act="title" title="Title screen [T]">${ICON.logo}stargaze<span class="dot">.</span><small>/ ${sub}</small></button>`;
function chartSvg({ ticks = true, labels = true } = {}) {
  let t = '';
  if (ticks) for (let i = 0; i < 72; i++) {
    const a = i * Math.PI / 36, r1 = i % 6 ? 47.2 : 46, r2 = 48;
    t += `<line x1="${50 + r1 * Math.cos(a)}" y1="${50 + r1 * Math.sin(a)}" x2="${50 + r2 * Math.cos(a)}" y2="${50 + r2 * Math.sin(a)}" stroke="#edf3ff70" stroke-width=".25"/>`;
  }
  return `<svg viewBox="0 0 100 100" aria-hidden="true">
    <circle cx="50" cy="50" r="48" stroke="#edf3ff55" stroke-width=".3" stroke-dasharray=".8 .8"/>
    <circle cx="50" cy="50" r="42" stroke="#edf3ff90" stroke-width=".35"/>
    <circle cx="50" cy="50" r="28" stroke="#edf3ff30" stroke-width=".25" stroke-dasharray="1 1.2"/>
    <line x1="2" y1="50" x2="98" y2="50" stroke="#eef3ff40" stroke-width=".2"/><line x1="50" y1="2" x2="50" y2="98" stroke="#eef3ff40" stroke-width=".2"/>
    <ellipse cx="50" cy="50" rx="46" ry="15" transform="rotate(-24 50 50)" stroke="#f0e76a55" stroke-width=".25" stroke-dasharray="2 1"/>
    ${t}</svg>${labels ? `<span class="cap" style="top:-2px;left:50%;transform:translateX(-50%)">N / 00°</span><span class="cap" style="bottom:-4px;left:50%;transform:translateX(-50%)">S / 180°</span><span class="cap" style="left:-44px;top:50%">W / 270°</span><span class="cap" style="right:-40px;top:50%">E / 90°</span>` : ''}`;
}
const kb = k => `<kbd>${k}</kbd>`;
const foot = keys => `<div class="foot"><span>■ ${esc(keys[0])}</span><div class="keys">${keys.slice(1).map(([k, l]) => `<span>${k.split(' ').map(kb).join('')} ${l}</span>`).join('')}</div></div>`;

/* ---------------------------------------------------------------- game-screen shell */
// Menus are full-screen game screens: the live sky dimmed behind, one centred column, a keyboard
// cursor and console-style prompts. Only rects, lines and text, so they port to src/ui.rs.
const pad = n => String(n).padStart(2, '0');
const prompts = list => `<div class="prompts">${list.map(([k, l]) => `<span>${k.split(' ').map(kb).join('')}${l}</span>`).join('')}</div>`;
const skyBg = id => `<canvas class="menu-sky" data-sky="${id}" data-wide="1"></canvas><div class="menu-veil"></div>`;
function gameScreen({ title, sub, body, keys, back = 'back-title' }) {
  return `<div class="gscreen">${skyBg(S.obs.sys)}
    <header class="ghead"><div><div class="eyebrow">${sub}</div><h1>${title}</h1></div><button class="gback" data-act="${back}">${kb('Esc')}Back</button></header>
    <div class="gbody">${body}</div>${prompts(keys)}</div>`;
}
// Moving the cursor only moves the highlight; re-rendering would restart the sky and entrance animations.
function setSel(key, i) { S[key] = i; document.querySelectorAll('[data-row]').forEach(r => r.classList.toggle('sel', +r.dataset.row === i)); }
function moveSel(key, n, dir) { setSel(key, (S[key] + dir + n) % n); }

/* ---------------------------------------------------------------- 00 title */
const TITLE_ITEMS = [['P', 'Play', 'Your levels'], ['E', 'Explore', 'Any system, freely'], ['O', 'Settings', 'Quality, exposure, interface'], ['Q', 'Quit', '']];
function vTitle() {
  return `<div class="gscreen title">${skyBg(S.obs.sys)}
    <h1 class="wordmark">${ICON.logo}stargaze<span>.</span></h1>
    <nav class="tmenu">${TITLE_ITEMS.map(([k, l, d], i) => `<button class="titem ${S.titleSel === i ? 'sel' : ''}" data-title="${i}" data-row="${i}" style="--i:${i}"><span class="caret">▸</span><b>${l}</b>${d ? `<small>${d}</small>` : ''}${kb(k)}</button>`).join('')}</nav>
    ${prompts([['↑ ↓', 'Select'], ['Enter', 'Confirm']])}</div>`;
}
function titleAction(i) {
  S.titleSel = i;
  const k = TITLE_ITEMS[i][0];
  if (k === 'P') go('maps');
  else if (k === 'E') go('systems');
  else if (k === 'O') go('settings');
  else toast('In the game, Quit closes the window.');
}

/* ---------------------------------------------------------------- 01 explore */
function vSystems() {
  const list = listSystems();
  const body = !list.length
    ? `<div class="gempty"><b>No systems found</b>Add a .yaml file to configs/, then reopen this menu.</div>`
    : `<div class="list">${list.map((x, i) => `<div class="lrow ${i === S.sel ? 'sel' : ''} ${x.broken ? 'bad' : ''}" data-row="${i}" style="--i:${i}">
        <span class="lnum">${pad(i + 1)}</span>
        <div class="linfo"><b>${esc(x.short)}</b>${x.broken ? `<div class="err">${esc(x.error.split('\n').slice(1, 2).join(''))}</div>` : `<div class="lsub">${esc(x.file)}</div>`}</div>
        <button class="gbtn play" data-explore="${i}" ${x.broken ? 'disabled' : ''}>${x.broken ? 'Invalid' : 'Explore'}${kb('Enter')}</button></div>`).join('')}</div>`;
  return gameScreen({ title: 'Explore', sub: 'Any system · labels and targets on', body, keys: [['↑ ↓', 'Select'], ['Enter', 'Explore'], ['Esc', 'Back']] });
}
function openSystem(i = S.sel) {
  const s = listSystems()[i];
  if (!s || s.broken) { toast(s ? 'This config has errors and cannot be loaded.' : 'Nothing to open.'); return; }
  S.sel = i; S.loaded = s.id; S.obs = freshObs(s.id, false); go('observe');
  toast(`Loaded ${s.name} · camera on ${s.host}, ${s.lat}`);
}

/* ---------------------------------------------------------------- 03 field studies */
function progressOf(id) {
  const t = S.theories[id], objs = t ? t.nodes.length - 1 : 0, best = S.best[id];
  const solved = best != null && best >= 99.9, started = objs > 0 || best != null;
  return { objs, best, solved, started, attempts: t ? t.attempts.length : 0, status: solved ? 'Solved' : started ? 'In progress' : 'Not started' };
}
function vMaps() {
  const rows = listMaps().map((m, i) => {
    const p = progressOf(m.id);
    return `<div class="lrow ${i === S.mapSel ? 'sel' : ''} ${p.solved ? 'solved' : ''}" data-row="${i}" style="--i:${i}">
      <span class="lnum">${pad(i + 1)}</span>
      <div class="linfo"><b>${esc(m.puzzle)}</b><div class="lprog"><span class="pbar"><i style="width:${p.best || 0}%"></i></span><span class="status">${p.status}</span><span>${p.best != null ? `best ${num(p.best)}` : ''}${p.objs ? ` · ${p.objs} objects drawn` : ''}</span></div></div>
      <button class="gbtn play" data-play="${i}">${p.started ? 'Resume' : 'Play'}${kb('Enter')}</button>
      <button class="gbtn discard" data-discard="${i}" ${p.started ? '' : 'disabled'} title="Discard progress">✕ Discard${kb('D')}</button></div>`;
  }).join('');
  const err = S.variant.maps === 'failed' ? `<div class="gerror">Could not load this map. Your progress is unchanged.</div>` : '';
  return gameScreen({ title: 'Levels', sub: 'Play · reconstruct unknown skies', body: `${err}<div class="list">${rows}</div>`,
    keys: [['↑ ↓', 'Select'], ['Enter', 'Play'], ['D', 'Discard progress'], ['Esc', 'Back']] });
}
function startMap(i = S.mapSel) {
  const s = listMaps()[i];
  if (S.variant.maps === 'failed') { toast('Could not load this map. Your progress is unchanged.'); return; }
  const resumed = progressOf(s.id).started;
  S.mapSel = i; S.map = s.id; S.theory = S.theories[s.id] ||= freshTheory(s.id); S.pobs = freshObs(s.id, true);
  go('pobserve'); toast(`${resumed ? 'Resumed' : 'Started'} · ${s.puzzle}`);
}
function askDiscard(i = S.mapSel) {
  const s = listMaps()[i];
  if (!progressOf(s.id).started) return;
  S.mapSel = i; S.pendingMap = s.id; S.dialog = 'discard'; render();
}

/* ---------------------------------------------------------------- 02/04 observation */
function obs() { return S.screen === 'pobserve' ? S.pobs : S.obs; }
function vObserve() {
  const o = obs(), s = sysById(o.sys);
  return `<div class="observe" id="obs"><canvas id="sky"></canvas><div class="sky-labels" id="labels"></div><div class="brackets" id="brackets" hidden><i></i></div><div class="reticle"></div>
    ${o.puzzle ? `<div class="readout"><span id="pstatus"></span></div>
      ${o.objective && o.hud ? `<div class="objective" data-stop><div class="eyebrow">Level · ${esc(s.puzzle)}</div><h4>What is out there?</h4>Observe, then draw the system in your theory.
        <ul class="steps"><li class="${S.theory.nodes.length > 1 ? 'done' : ''}">Find every moving light</li><li class="${S.theory.nodes.some(n => n.star) ? 'done' : ''}">Decide which are stars</li><li class="${S.theory.viewer != null ? 'done' : ''}">Mark where you are standing</li><li>Check your theory ${kb('Enter')}</li></ul>
        <div style="margin-top:10px;display:flex;justify-content:space-between;align-items:center"><button class="btn" data-act="theory" style="min-height:30px">Open theory ${kb('Tab')}</button><button class="faint" data-act="hide-obj" style="font-size:9px">hide</button></div></div>` : ''}`
      : `<div class="readout" ${o.hud ? '' : 'hidden'}><span class="eyebrow">${esc(s.name)}</span><b>${s.host} · ${s.lat} · FOV <span id="fov"></span></b><span id="daytxt"></span></div>`}
    <div class="conv" ${o.hud ? '' : 'hidden'}><span id="spp"></span><span class="meter"><i id="sppbar"></i></span></div>
    ${!o.puzzle && o.targets && o.hud ? vTargets(s) : ''}
    <div id="hud"></div></div>`;
}
function vTargets(s) {
  return `<div class="targets" data-stop><h5><span>TARGETS</span><button data-act="targets" class="faint">${kb('G')}</button></h5>
    ${s.targets.map((t, i) => `<button data-aim="${i}" class="${obs().lock?.name === t ? 'on' : ''}"><span>${t}</span>${kb('F' + (i + 1))}</button>`).join('')}
    <div class="events"><button data-act="eclipse">Eclipse ${kb('E')}</button><button data-act="transit">Transit ${kb('T')}</button><button data-act="reset">Reset ${kb('R')}</button><button data-act="zoomin">Zoom ${kb('=')}${kb('-')}</button></div></div>`;
}
function lockText(o) {
  if (!o.lock) return ['Unlocked', '[U] / right-click sky'];
  if (o.lock.type === 'sky') return ['Locked', 'Background sky'];
  const b = o.sky.bodies[o.lock.i];
  const named = !o.puzzle && o.labels;
  return ['Locked', named ? b.n : b.k === 'star' ? 'Star' : 'Planet / moon'];
}
function renderHud() {
  const el = $('#hud'); if (!el) return;
  const o = obs(), sp = SPEEDS[o.speed], [lt, ld] = lockText(o);
  const lockBtn = `<button class="lock ${o.lock ? 'on' : ''}" data-act="unlock" title="Release lock [U]">${o.lock ? ICON.lock : ICON.unlock}<span><b>${lt} ${kb('U')}</b><small>${esc(ld)}</small></span></button>`;
  if (!o.hud) {
    el.innerHTML = `<div class="hud-mini" data-stop><button class="hb" data-act="menu">${ICON.menu}Menu ${kb('M')}</button>${lockBtn}</div><div class="hud-hint">HUD hidden · ${kb('H')} show</div>`;
    return;
  }
  const steps = SPEEDS.map((v, i) => `<i class="${i === o.speed ? 'on' : ''}" style="height:${4 + Math.abs(i - STOP) * 2.5}px"></i>`).join('');
  el.innerHTML = `<div class="hud" data-stop>
    <div class="grp"><button class="hb" data-act="menu">${ICON.menu}${o.puzzle ? 'Maps' : 'Menu'} ${kb('M')}</button>
      ${o.puzzle ? `<button class="hb" data-act="theory">Theory ${kb('Tab')}</button>` : `<button class="hb ${o.labels ? 'on' : ''}" data-act="labels">Labels ${kb('L')}</button>`}
      <button class="hb" data-act="hud" title="Hide HUD [H]">${ICON.eye}${kb('H')}</button></div>
    <div class="grp"><span class="label">TIME</span><button class="hb icon" data-act="slower" title="Slower [←]">${ICON.left}</button>
      <div class="speed"><span id="speedtxt">${sp === 0 ? '0' : (sp > 0 ? '+' : '−') + Math.abs(sp).toLocaleString()} min/s</span><small><span class="steps-ind">${steps}</span></small></div>
      <button class="hb icon" data-act="faster" title="Faster [→]">${ICON.right}</button>
      <button class="hb play" data-act="stop" title="${sp ? 'Stop' : 'Resume'} [Space]">${sp ? ICON.pause : ICON.play}${kb('Spc')}</button>
      <button class="hb" data-act="prevday" title="−1 local day">−1d ${kb('PgDn')}</button><div class="clock ${o.dayFail ? 'fail' : ''}" id="clock"></div><button class="hb" data-act="nextday" title="+1 local day">+1d ${kb('PgUp')}</button></div>
    <div class="grp">${lockBtn}<div class="orient-wrap"><div class="orient" title="Star orientation [S]"><button data-orient="horizon" class="${o.orient === 'horizon' ? 'on' : ''}">Horizon<small>keep ground</small></button><button data-orient="sky" class="${o.orient === 'sky' ? 'on' : ''}">Stars<small>keep sky</small></button></div>${kb('S')}</div></div>
    <div class="grp grow"><div class="ev ${o.auto ? 'auto' : ''}"><div class="cap"><span>${ICON.sun.replace('<svg', '<svg style="width:11px;height:11px;vertical-align:-2px"')} Exposure ${kb(',')}${kb('.')}</span><span id="evtxt">${o.ev >= 0 ? '+' : ''}${o.ev.toFixed(2)} EV</span></div><input type="range" id="ev" min="-8" max="8" step=".25" value="${o.ev}"></div>
      <button class="check ${o.auto ? 'on' : ''}" data-act="auto"><i></i>Auto ${kb('A')}</button>
      ${o.puzzle ? '' : `<button class="hb ${o.targets ? 'on' : ''}" data-act="targets" title="Targets panel">${ICON.target}${kb('G')}</button>`}</div></div>`;
  paintClock();
}
function paintClock() {
  const o = obs(), c = $('#clock'); if (!c) return;
  const day = sysById(o.sys).day || 1440, d = Math.floor(o.t / day), m = ((o.t % day) + day) % day;
  c.innerHTML = o.dayFail ? `No day found<small>host is locked to its star</small>` : `T+ ${num(o.t)} min<small>day ${d} · ${String(Math.floor(m / 60)).padStart(2, '0')}:${String(Math.floor(m % 60)).padStart(2, '0')} local</small>`;
  const ps = $('#pstatus');
  if (ps) ps.textContent = `${SPEEDS[o.speed] === 0 ? 'STOPPED' : 'PLAYING'} | ${SPEEDS[o.speed] >= 0 ? '+' : ''}${num(SPEEDS[o.speed])} min/s${o.lock ? ' | View locked' : ''}`;
  const f = $('#fov'); if (f) { const v = sysById(o.sys).fov / o.zoom; f.textContent = (v < 1 ? v.toFixed(3) : v.toFixed(1)) + '°'; }
  const dt = $('#daytxt'); if (dt) dt.textContent = `t = ${(o.t / 1440).toFixed(3)} d · ${o.orient === 'sky' ? 'stars held' : 'horizon up'}`;
}
function changeSpeed(dir) { const o = obs(); o.speed = clamp(o.speed + dir, 0, SPEEDS.length - 1); if (SPEEDS[o.speed]) o.prevSpeed = o.speed; o.spp = 0; renderHud(); }
function toggleStop() { const o = obs(); if (SPEEDS[o.speed]) { o.prevSpeed = o.speed; o.speed = STOP; } else o.speed = o.prevSpeed || 5; renderHud(); }
function stepDay(dir) {
  const o = obs();
  if (o.dayFail) { toast('No day found · time unchanged'); return; }
  o.t += dir * (sysById(o.sys).day || 1440); o.spp = 0; paintClock();
  toast(`${dir > 0 ? '+' : '−'}1 local solar day · ${num(sysById(o.sys).day || 1440, 2)} min`, { ms: 1400 });
}
function setOrient(v) { const o = obs(); if (o.orient === v) return; o.orient = v; o.capture = skyRot(o); renderHud(); }
function runSearch(kind) {
  const o = obs(), s = sysById(o.sys);
  if (o.puzzle) { toast('Event search is disabled during a level.'); return; }
  toast(kind === 'E' ? 'Searching up to ten model years for a visible eclipse…' : 'Searching for a moon transit…', { done: false, sticky: true });
  setTimeout(() => {
    const txt = kind === 'E' ? s.eclipse : s.transit, day = parseFloat(txt.split('day ')[1]);
    const bi = o.sky.bodies.findIndex(x => txt.startsWith(x.n)); o.t = day * 1440; o.speed = STOP; o.spp = 0; o.lock = { type: 'body', i: Math.max(0, bi), name: o.sky.bodies[Math.max(0, bi)].n }; renderHud();
    toast(`${kind === 'E' ? 'Eclipse' : 'Transit'} found · ${txt} · time stopped`, { ms: 3600 });
  }, 1300);
}
function aim(i) {
  const o = obs();
  if (o.puzzle) { toast('Named targets are disabled during a level.'); return; }
  const name = sysById(o.sys).targets[i]; if (!name) return;
  const bi = o.sky.bodies.findIndex(b => b.n === name);
  if (bi < 0) { toast(`${name} is below the horizon right now.`); return; }
  const p = bodyPos(o, o.sky.bodies[bi]); o.cx = p[0]; o.cy = p[1]; o.lock = null; o.spp = 0; renderHud();
  toast(`Aimed at ${name} · lock released`, { ms: 1400 });
}
function resetView() { const o = obs(), f = freshObs(o.sys, o.puzzle); Object.assign(o, { cx: f.cx, cy: f.cy, zoom: 1, lock: null, spp: 0 }); renderHud(); toast('View reset', { ms: 1200 }); }

/* ---- sky rendering */
const W_POLE = [.5, -.9];
function skyRot(o) { return o.t / (sysById(o.sys).day || 1440) * Math.PI * 2 * .25; }
function rotN(x, y, a) { // rotate a normalized point about the pole, in a 16:9 pixel frame
  const px = (x - W_POLE[0]) * 16, py = (y - W_POLE[1]) * 9, c = Math.cos(a), s = Math.sin(a);
  return [W_POLE[0] + (px * c - py * s) / 16, W_POLE[1] + (px * s + py * c) / 9];
}
function bodyPos(o, b) {
  const x = b.x + (b.vx || 0) * o.t / 1440 % 2;
  return b.fixed ? [x, b.y] : rotN(x, b.y, skyRot(o));
}
let STARS = null;
function stars() {
  if (STARS) return STARS;
  let seed = 7; const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  STARS = Array.from({ length: 1800 }, () => ({ x: rnd() * 3 - 1, y: rnd() * 2.6 - 1.6, m: Math.pow(rnd(), 3), c: rnd() }));
  return STARS;
}
let NOISE = null;
function noise() {
  if (NOISE) return NOISE;
  NOISE = document.createElement('canvas'); NOISE.width = NOISE.height = 160;
  const g = NOISE.getContext('2d'), d = g.createImageData(160, 160);
  for (let i = 0; i < d.data.length; i += 4) { const v = Math.random() * 255; d.data[i] = d.data[i + 1] = v; d.data[i + 2] = v * 1.1; d.data[i + 3] = Math.random() < .5 ? 90 : 0; }
  g.putImageData(d, 0, 0); return NOISE;
}
function drawSky(cv, o, opts = {}) {
  const dpr = Math.min(2, devicePixelRatio || 1), W = cv.clientWidth, H = cv.clientHeight;
  if (!W || !H) return [];
  if (cv.width !== Math.round(W * dpr) || cv.height !== Math.round(H * dpr)) { cv.width = Math.round(W * dpr); cv.height = Math.round(H * dpr); }
  const ctx = cv.getContext('2d'), sky = o.sky, z = o.zoom, rot = skyRot(o);
  const roll = o.orient === 'sky' ? -(rot - o.capture) : 0;
  const X = x => (x - o.cx) * W * z, Y = y => (y - o.cy) * H * z;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = '#01030a'; ctx.fillRect(0, 0, W, H);
  ctx.translate(W / 2, H / 2); ctx.rotate(roll);
  const hy = Y(.86), big = Math.max(W, H) * 3;
  const g = ctx.createLinearGradient(0, hy - H * .9 * Math.min(z, 3), 0, hy);
  g.addColorStop(0, rgba(sky.top, 1)); g.addColorStop(.7, rgba(sky.mid, 1)); g.addColorStop(1, rgba(sky.glow, 1));
  ctx.fillStyle = g; ctx.fillRect(-big, -big, big * 2, big * 2);
  // Background stars: fixed angular dots, capped in size, rotating with the sky.
  const sz = Math.min(1.6, .8 + Math.log10(z + 1) * .5);
  for (const st of stars()) {
    const [x, y] = rotN(st.x, st.y, rot), sx = X(x), sy = Y(y);
    if (Math.abs(sx) > W || Math.abs(sy) > H || y > .9) continue;
    const a = sky.starA * (.25 + st.m * .75);
    ctx.fillStyle = st.c > .85 ? `rgba(255,220,190,${a})` : st.c < .15 ? `rgba(190,210,255,${a})` : `rgba(235,240,255,${a})`;
    const r = (.45 + st.m * 1.1) * sz;
    ctx.fillRect(sx - r / 2, sy - r / 2, r, r);
  }
  const out = [];
  sky.bodies.forEach((b, i) => {
    const [x, y] = bodyPos(o, b), sx = X(x), sy = Y(y), sr = b.r * H * z;
    if (y > .9) return;
    drawBody(ctx, b, sx, sy, sr, sky.light);
    const c = Math.cos(roll), s = Math.sin(roll);
    out.push({ i, b, x: W / 2 + sx * c - sy * s, y: H / 2 + sx * s + sy * c, r: Math.max(sr, 2) });
  });
  // Horizon glow and ground: the observer's planet is real geometry, two metres below.
  const glow = ctx.createLinearGradient(0, hy - H * .22 * z, 0, hy);
  glow.addColorStop(0, rgba(sky.glow, 0)); glow.addColorStop(1, rgba(sky.glow, .45));
  ctx.fillStyle = glow; ctx.fillRect(-big, hy - H * .22 * z, big * 2, H * .22 * z);
  ctx.beginPath(); ctx.moveTo(X(-2), Y(4));
  for (let k = 0; k <= 120; k++) { const x = -2 + k * 5 / 120; ctx.lineTo(X(x), Y(.86 + .016 * Math.sin(x * 9) + .009 * Math.sin(x * 23 + 1) - .02 * Math.exp(-Math.pow((x - .12) * 30, 2)))); }
  ctx.lineTo(X(3), Y(4)); ctx.closePath(); ctx.fillStyle = '#020308'; ctx.fill();
  ctx.strokeStyle = rgba(sky.glow, .35); ctx.lineWidth = 1; ctx.stroke();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  if (opts.grain) { // Monte Carlo noise fades as samples accumulate.
    const a = Math.min(.22, .5 / Math.sqrt(o.spp + 1));
    if (a > .02) { ctx.globalAlpha = a; ctx.globalCompositeOperation = 'lighter'; ctx.fillStyle = ctx.createPattern(noise(), 'repeat'); ctx.translate(Math.random() * 160, Math.random() * 160); ctx.fillRect(-160, -160, W + 160, H + 160); ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.globalAlpha = 1; ctx.globalCompositeOperation = 'source-over'; }
  }
  return out;
}
function drawBody(ctx, b, x, y, sr, light) {
  if (b.k === 'star') {
    const R = Math.max(sr * 5, 10), g = ctx.createRadialGradient(x, y, 0, x, y, R);
    g.addColorStop(0, 'rgba(255,255,255,1)'); g.addColorStop(.12, rgba(b.c, .9)); g.addColorStop(.4, rgba(b.c, .18)); g.addColorStop(1, rgba(b.c, 0));
    ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, R, 0, 7); ctx.fill();
    ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(x, y, Math.max(sr, 1.6), 0, 7); ctx.fill(); return;
  }
  if (sr < 2.4) {
    const g = ctx.createRadialGradient(x, y, 0, x, y, 5);
    g.addColorStop(0, rgba(b.c, 1)); g.addColorStop(.35, rgba(b.c, .5)); g.addColorStop(1, rgba(b.c, 0));
    ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, 5, 0, 7); ctx.fill(); return;
  }
  const rings = half => {
    if (!b.rings) return;
    const { a, b: bb, rot, flat, c } = b.rings;
    for (let k = 0; k < 16; k++) {
      const f = k / 15; if (f > .58 && f < .66) continue; // Cassini-like gap
      ctx.strokeStyle = rgba(c, (.25 + .45 * Math.sin(f * 9) ** 2) * (half ? .95 : .7));
      ctx.lineWidth = Math.max(.6, (bb - a) * sr / 16 * 1.05);
      ctx.beginPath(); const rr = (a + (bb - a) * f) * sr;
      ctx.ellipse(x, y, rr, rr * flat, rot, half ? 0 : Math.PI, half ? Math.PI : Math.PI * 2); ctx.stroke();
    }
  };
  rings(false);
  ctx.save(); ctx.beginPath(); ctx.arc(x, y, sr, 0, 7); ctx.clip();
  ctx.fillStyle = rgba(b.c, 1); ctx.fillRect(x - sr, y - sr, sr * 2, sr * 2);
  if (b.bands) for (let k = -6; k <= 6; k++) { ctx.fillStyle = k % 2 ? 'rgba(90,60,30,.10)' : 'rgba(255,245,225,.07)'; ctx.fillRect(x - sr, y + k * sr / 6.5 - sr / 14, sr * 2, sr / 7 * (1 + (k * k) % 3 * .4)); }
  if (b.maria) [[-.3, -.2, .28], [.15, -.35, .2], [.25, .15, .3], [-.1, .35, .18], [-.45, .25, .12]].forEach(([dx, dy, r]) => { ctx.fillStyle = 'rgba(70,72,80,.28)'; ctx.beginPath(); ctx.ellipse(x + dx * sr, y + dy * sr, r * sr, r * sr * .8, .4, 0, 7); ctx.fill(); });
  const [lx, ly] = light, sh = ctx.createRadialGradient(x + lx * sr * .5, y + ly * sr * .5, sr * .05, x + lx * sr * .2, y + ly * sr * .2, sr * 1.7);
  sh.addColorStop(0, 'rgba(255,255,255,.12)'); sh.addColorStop(.35, 'rgba(0,0,0,0)'); sh.addColorStop(.62, 'rgba(1,2,8,.45)'); sh.addColorStop(.82, 'rgba(1,2,8,.9)'); sh.addColorStop(1, 'rgba(1,2,8,.98)');
  ctx.fillStyle = sh; ctx.fillRect(x - sr, y - sr, sr * 2, sr * 2);
  ctx.restore();
  rings(true);
}
let FRAME = [];
function tickObserve(dt) {
  const o = obs(), cv = $('#sky'); if (!cv) return;
  const sp = SPEEDS[o.speed];
  o.t += sp * dt;
  if (o.lock) {
    if (o.lock.type === 'body') { const p = bodyPos(o, o.sky.bodies[o.lock.i]); o.cx = p[0]; o.cy = p[1]; }
    else { const p = rotN(o.lock.x, o.lock.y, skyRot(o)); o.cx = p[0]; o.cy = p[1]; }
  }
  if (sp || drag.moved) o.spp = 0; else o.spp += S.settings.spp;
  cv.style.filter = `brightness(${Math.pow(2, (o.auto ? 0 : 0) + o.ev * .3)})`;
  FRAME = drawSky(cv, o, { grain: true });
  // Labels and lock brackets follow the traced bodies.
  const lab = $('#labels'), br = $('#brackets');
  const showLabels = !o.puzzle && o.labels;
  let html = '';
  for (const f of FRAME) {
    const locked = o.lock?.type === 'body' && o.lock.i === f.i;
    if (showLabels && f.x > -50 && f.x < cv.clientWidth + 50 && f.y > -20 && f.y < cv.clientHeight) html += `<span class="sky-label ${locked ? 'locked' : ''}" style="left:${f.x + f.r + 8}px;top:${f.y}px">${f.b.n}</span>`;
    if (locked) { const R = Math.max(f.r + 8, 12); Object.assign(br.style, { left: f.x - R + 'px', top: f.y - R + 'px', width: 2 * R + 'px', height: 2 * R + 'px' }); }
  }
  lab.innerHTML = html;
  br.hidden = !(o.lock?.type === 'body' && FRAME.some(f => f.i === o.lock.i));
  tickObserve.acc = (tickObserve.acc || 0) + dt;
  if (tickObserve.acc > .1) {
    tickObserve.acc = 0; paintClock();
    const spp = $('#spp'), bar = $('#sppbar');
    if (spp) { spp.textContent = sp ? 'spp 1 · time running, stop to converge' : `${o.spp.toLocaleString()} spp · ${o.spp < 256 ? 'converging' : 'converged'}`; bar.style.width = Math.min(100, Math.log2(o.spp + 1) / 10 * 100) + '%'; }
  }
}
function tickPreviews(dt) {
  tickPreviews.t = (tickPreviews.t || 0) + dt * 40;
  document.querySelectorAll('canvas[data-sky]').forEach(cv => {
    const id = cv.dataset.sky, p = cv.dataset.wide ? { cx: .5, cy: .5, zoom: 1 } : SKY[id].preview;
    drawSky(cv, { sys: id, sky: SKY[id], t: tickPreviews.t, cx: p.cx, cy: p.cy, zoom: p.zoom, orient: 'horizon', capture: 0, spp: 999 });
  });
}
const drag = { down: false, moved: false, x: 0, y: 0 };
function mountObserve() {
  renderHud();
  const el = $('#obs'), o = obs();
  el.addEventListener('contextmenu', e => {
    e.preventDefault(); if (e.target.closest('[data-stop]')) return;
    const r = el.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    const hit = FRAME.filter(f => Math.hypot(f.x - mx, f.y - my) <= Math.max(f.r, 12)).pop();
    if (hit) o.lock = { type: 'body', i: hit.i, name: hit.b.n };
    else { // Hold this direction fixed against the background stars.
      const W = el.clientWidth, H = el.clientHeight, p = [o.cx + (mx - W / 2) / (W * o.zoom), o.cy + (my - H / 2) / (H * o.zoom)];
      o.cx = p[0]; o.cy = p[1]; o.lock = { type: 'sky', ...Object.fromEntries(rotN(p[0], p[1], -skyRot(o)).map((v, k) => [k ? 'y' : 'x', v])) };
    }
    o.spp = 0; renderHud();
  });
  el.addEventListener('mousedown', e => { if (e.button || e.target.closest('[data-stop]')) return; Object.assign(drag, { down: true, moved: false, x: e.clientX, y: e.clientY }); });
  el.addEventListener('wheel', e => { if (e.target.closest('[data-stop]')) return; e.preventDefault(); o.zoom = clamp(o.zoom * (e.deltaY < 0 ? 1.15 : 1 / 1.15), .6, 5000); o.spp = 0; paintClock(); }, { passive: false });
}
addEventListener('mousemove', e => {
  if (!drag.down) return;
  const o = obs(), el = $('#obs'); if (!el) return;
  const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
  if (!drag.moved && Math.hypot(dx, dy) < 4) return;
  if (!drag.moved && o.lock) { o.lock = null; renderHud(); }
  drag.moved = true; el.classList.add('dragging');
  o.cx -= dx / (el.clientWidth * o.zoom); o.cy -= dy / (el.clientHeight * o.zoom); o.spp = 0;
  drag.x = e.clientX; drag.y = e.clientY;
});
addEventListener('mouseup', () => {
  if (!drag.down) return;
  const o = obs(), el = $('#obs');
  if (!drag.moved && o.lock) { o.lock = null; renderHud(); } // left-click releases the lock
  drag.down = false; drag.moved = false; el?.classList.remove('dragging');
});

/* ---------------------------------------------------------------- 05 theory */
const T = () => S.theory;
function snapshot() { const t = T(); return JSON.stringify({ nodes: t.nodes, viewer: t.viewer, sel: t.sel, next: t.next }); }
function save() { const t = T(); t.redo = []; t.undo.push(snapshot()); if (t.undo.length > 100) t.undo.shift(); t.score = null; t.edits++; }
function restore(json) { Object.assign(T(), JSON.parse(json)); }
const nodeById = id => T().nodes.find(n => n.id === id);
const dist = n => { const p = nodeById(n.parent); return Math.hypot(n.x - p.x, n.y - p.y); };
function rank(n) { if (n.parent == null) return 0; return T().nodes.filter(m => m.parent === n.parent).sort((a, b) => dist(a) - dist(b)).indexOf(n); }
function topology(nodes, viewerId) {
  const idx = new Map(nodes.map((n, i) => [n.id, i])), ch = nodes.map(() => []);
  nodes.forEach((n, i) => { if (n.parent != null) ch[idx.get(n.parent)].push(i); });
  ch.forEach(c => c.sort((a, b) => dist(nodes[a]) - dist(nodes[b])));
  return { root: nodes.findIndex(n => n.parent == null), ch, tr: nodes.map(n => ({ star: !!n.star, rings: !!n.rings })), host: viewerId == null ? -1 : idx.get(viewerId) };
}
function truthTopology(tree) {
  const ch = [], tr = []; let host = -1;
  const walk = n => { const i = tr.length; tr.push({ star: !!n.star, rings: !!n.rings }); ch.push([]); if (n.viewer) host = i; (n.kids || []).forEach(k => ch[i].push(walk(k))); return i; };
  return { root: walk(tree), ch, tr, host };
}
function score(g, t) { // port of game::score
  const m = (i, j) => {
    let count = 1, traits = (g.tr[i].star === t.tr[j].star) + (g.tr[i].rings === t.tr[j].rings), v = i === g.host && j === t.host;
    for (let k = 0; k < Math.min(g.ch[i].length, t.ch[j].length); k++) { const r = m(g.ch[i][k], t.ch[j][k]); count += r[0]; traits += r[1]; v = v || r[2]; }
    return [count, traits, v];
  };
  const [c, tr, v] = m(g.root, t.root), n = Math.max(g.ch.length, t.ch.length);
  const s = { topology: 100 * c / n, traits: 100 * tr / (2 * n), viewer: v };
  s.total = s.topology * .6 + s.traits * .2 + (v ? 20 : 0);
  return s;
}
function runCheck(silent) {
  const t = T();
  if (t.viewer == null) { t.msg = 'Select an object and choose Viewer here [V] before checking.'; if (!silent) { S.dialog = null; renderTheory(); } return false; }
  t.score = score(topology(t.nodes, t.viewer), truthTopology(TRUTH[t.map]));
  if (!silent || t.attempts[t.attempts.length - 1]?.total !== t.score.total) t.attempts.push({ total: t.score.total });
  S.best[t.map] = Math.max(S.best[t.map] || 0, t.score.total);
  t.msg = 'Edit your theory and check again, or return to observing.';
  return true;
}
function vTheory() {
  return `<div class="theory"><div class="theory-top"><h1>Your theory</h1><span class="eyebrow">${esc(sysById(T().map).puzzle)}</span>
    <span class="hint">Select a parent, right-click to add ${kb('N')}. Drag to order orbits. 1 = innermost.</span><span class="spacer"></span>
    <span class="status"><i></i>${T().nodes.length} / 64 objects · ${T().undo.length} undo steps</span></div>
    <div class="canvas-wrap" id="cw"><svg id="tsvg"></svg>
      <div class="legend"><b>C</b> centre of the system<br><b>1, 2, 3…</b> orbit order, innermost first<br><span style="color:var(--accent)">✺</span> star · <b>⬭</b> rings · <span style="color:var(--ok)">YOU</span> viewer</div>
      <div class="inspector" id="insp"></div>
      <div class="msg"><span id="tmsg"></span><span class="formula" id="tformula"></span></div></div>
    <div id="thud"></div></div>`;
}
function renderTheory() {
  const t = T(), svg = $('#tsvg'); if (!svg) return;
  const W = svg.clientWidth, H = svg.clientHeight, cx = W / 2 - 60, cy = H / 2;
  const P = n => [cx + n.x, cy + n.y];
  let h = '';
  t.nodes.forEach(n => { if (n.parent == null) return; const [px, py] = P(nodeById(n.parent)); h += `<circle cx="${px}" cy="${py}" r="${dist(n)}" stroke="${n.id === t.sel ? '#8debd2aa' : '#aebfff40'}" stroke-dasharray="5 6" stroke-width="1"/>`; });
  t.nodes.forEach(n => { if (n.parent == null) return; const [px, py] = P(nodeById(n.parent)), [x, y] = P(n); h += `<line x1="${px}" y1="${py}" x2="${x}" y2="${y}" stroke="#aebfff80" stroke-width="1.3"/>`; });
  if (S.ghost && nodeById(t.sel)) { const [px, py] = P(nodeById(t.sel)); h += `<line x1="${px}" y1="${py}" x2="${S.ghost[0]}" y2="${S.ghost[1]}" stroke="#f0e76a80" stroke-dasharray="3 4"/><circle cx="${S.ghost[0]}" cy="${S.ghost[1]}" r="14" stroke="#f0e76a80" stroke-dasharray="3 3"/><text x="${S.ghost[0] + 20}" y="${S.ghost[1] + 4}" style="fill:#f0e76a;font-size:10px">right-click to add</text>`; }
  t.nodes.forEach(n => {
    const [x, y] = P(n), sel = n.id === t.sel, lbl = n.parent == null ? 'C' : rank(n) + 1;
    h += `<g data-node="${n.id}" style="cursor:pointer">`;
    if (n.rings) h += `<ellipse cx="${x}" cy="${y}" rx="31" ry="10" stroke="#f0f4ff" stroke-width="1.4"/>`;
    h += `<circle cx="${x}" cy="${y}" r="18" fill="#0d2a84" stroke="${n.star ? '#f0e76a' : '#c0cef5'}" stroke-width="1.6"/>`;
    if (n.rings) h += `<path d="M${x - 31} ${y} A31 10 0 0 0 ${x + 31} ${y}" stroke="#f0f4ff" stroke-width="1.4"/>`;
    if (n.star) for (let k = 0; k < 8; k++) { const a = k * Math.PI / 4; h += `<line x1="${x + Math.cos(a) * 24}" y1="${y + Math.sin(a) * 24}" x2="${x + Math.cos(a) * 30}" y2="${y + Math.sin(a) * 30}" stroke="#f0e76a" stroke-width="2"/>`; }
    if (sel) h += `<circle cx="${x}" cy="${y}" r="23" stroke="#8debd2" stroke-width="1.6"/>`;
    h += `<text x="${x}" y="${y + 4}" text-anchor="middle" style="font-size:12px">${lbl}</text>`;
    if (t.viewer === n.id) h += `<rect x="${x - 17}" y="${y + 33}" width="34" height="15" fill="#0d2a84" stroke="#8debd2"/><text x="${x}" y="${y + 44}" text-anchor="middle" style="fill:#8debd2;font-size:10px">YOU</text>`;
    h += `</g>`;
  });
  svg.innerHTML = h;
  const n = nodeById(t.sel);
  $('#insp').innerHTML = n ? `<h4><span>SELECTED OBJECT</span><span>${n.parent == null ? 'C' : '#' + (rank(n) + 1)}</span></h4><div class="body">
    <div class="kv"><span>Role</span><span>${n.parent == null ? 'Centre' : n.star ? 'Star' : 'Orbiting body'}</span></div>
    <div class="kv"><span>Orbits</span><span>${n.parent == null ? '—' : nodeById(n.parent).parent == null ? 'C' : '#' + (rank(nodeById(n.parent)) + 1)}</span></div>
    <div class="kv"><span>Orbit order</span><span>${n.parent == null ? '—' : `${rank(n) + 1} of ${t.nodes.filter(m => m.parent === n.parent).length}`}</span></div>
    <div class="kv"><span>Satellites</span><span>${t.nodes.filter(m => m.parent === n.id).length}</span></div>
    <div class="kv"><span>Traits</span><span>${[n.star && 'star', n.rings && 'rings'].filter(Boolean).join(' · ') || 'none'}</span></div>
    <div class="kv"><span>Viewer</span><span style="color:${t.viewer === n.id ? 'var(--ok)' : 'inherit'}">${t.viewer === n.id ? 'you are here' : 'no'}</span></div></div>` : '<h4>NOTHING SELECTED</h4>';
  $('#tmsg').textContent = t.msg;
  $('#tformula').innerHTML = t.score ? `<span style="color:var(--ok);font-size:12px">Score: ${num(t.score.total)}/100 · Orbits: ${num(t.score.topology)}% · Traits: ${num(t.score.traits)}% · Viewer: ${t.score.viewer ? 'matches' : 'does not match'}</span>`
    : '60% ordered structure + 20% star/ring traits + 20% viewer. Orbit 1 = innermost.';
  const root = n && n.parent == null;
  $('#thud').innerHTML = `<div class="hud">
    <div class="grp"><button class="hb" data-act="menu">${ICON.menu}Maps ${kb('M')}</button><button class="hb" data-act="observe-p">${ICON.eye}Observe ${kb('Tab')}</button></div>
    <div class="grp"><span class="label">SELECTED</span><button class="hb ${n?.star ? 'on' : ''}" data-act="t-star">Is star ${kb('S')}</button><button class="hb ${n?.rings ? 'on' : ''}" data-act="t-rings">Has rings ${kb('R')}</button><button class="hb ${t.viewer === t.sel ? 'on' : ''}" data-act="t-viewer">Viewer here ${kb('V')}</button><button class="hb" data-act="t-add">Add orbit ${kb('N')}</button></div>
    <div class="grp"><button class="hb" data-act="t-undo" ${t.undo.length ? '' : 'disabled style="opacity:.35"'}>Undo ${kb('Z')}</button><button class="hb" data-act="t-redo" ${t.redo.length ? '' : 'disabled style="opacity:.35"'}>Redo ${kb('Y')}</button><button class="hb" data-act="t-del" ${root ? 'style="opacity:.35"' : ''}>Delete ${kb('Del')}</button></div>
    <div class="grp grow"><button class="hb" data-act="t-clear">Clear all ${kb('X')}</button><button class="btn primary" data-act="t-check" style="min-height:36px;gap:24px">Check theory ${kb('Enter')}</button></div></div>`;
}
function tAct(a, pt) {
  const t = T(), n = nodeById(t.sel);
  if (a === 't-star' || a === 't-rings') { save(); n[a === 't-star' ? 'star' : 'rings'] ^= 1; n.star = !!n.star; n.rings = !!n.rings; t.msg = 'Object updated. Gold rays mark stars; an oval marks rings.'; }
  else if (a === 't-viewer') { save(); t.viewer = t.sel; t.msg = 'Viewer placed on the selected object.'; }
  else if (a === 't-del') {
    if (n.parent == null) t.msg = 'The centre stays. Select an orbiting object to delete its branch.';
    else { save(); const gone = new Set([n.id]); let grew = true; while (grew) { grew = false; t.nodes.forEach(m => { if (m.parent != null && gone.has(m.parent) && !gone.has(m.id)) { gone.add(m.id); grew = true; } }); }
      t.nodes = t.nodes.filter(m => !gone.has(m.id)); if (gone.has(t.viewer)) t.viewer = null; t.sel = n.parent; t.msg = 'Branch deleted. Undo restores it.'; }
  } else if (a === 't-undo' || a === 't-redo') {
    const from = a === 't-undo' ? t.undo : t.redo, to = a === 't-undo' ? t.redo : t.undo;
    if (!from.length) return; to.push(snapshot()); restore(from.pop()); t.score = null; t.msg = a === 't-undo' ? 'Edit undone.' : 'Edit redone.';
  } else if (a === 't-add') {
    if (t.nodes.length >= 64) { t.msg = 'Diagram limit: 64 objects. Delete a branch to make room.'; }
    else { const p = nodeById(t.sel), svg = $('#tsvg'), W = svg.clientWidth, H = svg.clientHeight;
      const q = pt || S.cursor || [W / 2 - 60 + p.x + 90, H / 2 + p.y + 40];
      save(); const id = t.next++; t.nodes.push({ id, parent: t.sel, x: q[0] - (W / 2 - 60), y: q[1] - H / 2, star: false, rings: false }); t.sel = id;
      t.msg = 'Added. Distance from its parent sets the orbit order.'; }
  } else if (a === 't-clear') { S.dialog = 'clear'; renderDialog(); return; }
  else if (a === 't-check') { if (runCheck()) { S.dialog = 'result'; renderDialog(); } }
  renderTheory();
}
const tdrag = { id: null, saved: false };
addEventListener('mouseup', () => { tdrag.id = null; });
function mountTheory() {
  renderTheory();
  const svg = $('#tsvg'), cw = $('#cw');
  const pt = e => { const r = svg.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
  svg.addEventListener('mousedown', e => {
    const g = e.target.closest('[data-node]'); if (!g || e.button) return;
    T().sel = +g.dataset.node; tdrag.id = T().sel; tdrag.saved = false; S.ghost = null; renderTheory();
  });
  svg.addEventListener('mousemove', e => {
    const p = pt(e); S.cursor = p;
    if (tdrag.id != null) {
      if (!tdrag.saved) { save(); tdrag.saved = true; }
      const n = nodeById(tdrag.id), W = svg.clientWidth, H = svg.clientHeight;
      n.x = clamp(p[0], 30, W - 30) - (W / 2 - 60); n.y = clamp(p[1], 30, H - 30) - H / 2;
      T().msg = 'Orbit order follows distance from each parent. Undo restores this drag.'; renderTheory(); return;
    }
    const over = e.target.closest('[data-node]');
    S.ghost = over ? null : p; renderTheory();
  });
  svg.addEventListener('mouseleave', () => { S.ghost = null; renderTheory(); });
  cw.addEventListener('contextmenu', e => {
    e.preventDefault(); if (!e.target.closest('svg')) return;
    const g = e.target.closest('[data-node]');
    if (g) { T().sel = +g.dataset.node; renderTheory(); return; }
    tAct('t-add', pt(e)); S.ghost = null; renderTheory();
  });
}

/* ---------------------------------------------------------------- dialogs */
function renderDialog() {
  const el = $('#dialog');
  if (!S.dialog) { el.hidden = true; el.innerHTML = ''; return; }
  el.hidden = false;
  const t = T();
  if (S.dialog === 'clear') el.innerHTML = `<div class="dialog"><div class="eyebrow">Your theory</div><h2>Clear the entire theory?</h2>
    <p>Removes all objects and the viewer. ${t.nodes.length - 1} orbiting object${t.nodes.length === 2 ? '' : 's'} will be removed; the centre stays.</p><p>Undo can restore everything.</p>
    <div class="actions"><button class="btn" data-act="d-cancel">Cancel ${kb('Esc')}</button><button class="btn primary danger" data-act="d-clear" style="gap:24px">Clear all ${kb('Enter')}</button></div></div>`;
  if (S.dialog === 'discard') {
    const m = sysById(S.pendingMap), p = progressOf(m.id);
    el.innerHTML = `<div class="dialog"><div class="eyebrow">Discard progress</div><h2>Discard ${esc(m.puzzle)}?</h2>
      <p>Your theory${p.objs ? ` (${p.objs} objects)` : ''}, ${p.attempts} check${p.attempts === 1 ? '' : 's'} and best score ${p.best != null ? num(p.best) : '—'} will be removed. The level starts fresh.</p><p>This cannot be undone.</p>
      <div class="actions"><button class="btn" data-act="d-cancel">Cancel ${kb('Esc')}</button><button class="btn primary danger" data-act="d-discard" style="gap:24px">Discard ${kb('Enter')}</button></div></div>`;
  }
  if (S.dialog === 'result') {
    const s = t.score, C = 2 * Math.PI * 88;
    const rows = [['Ordered structure', s.topology, '60%', s.topology * .6], ['Star / ring traits', s.traits, '20%', s.traits * .2], ['Viewer position', s.viewer ? 100 : 0, '20%', s.viewer ? 20 : 0]];
    el.innerHTML = `<div class="dialog wide"><div class="eyebrow">Theory check · ${esc(sysById(t.map).puzzle)} · attempt ${t.attempts.length}</div>
      <h2>${s.total >= 99.9 ? 'Solved. The sky makes sense.' : s.total >= 70 ? 'Close. Something is still out of place.' : 'Keep observing.'}</h2>
      <div class="score-hero"><div class="ring"><svg viewBox="0 0 200 200"><circle cx="100" cy="100" r="88" stroke="#ffffff20" stroke-width="8"/><circle cx="100" cy="100" r="88" stroke="#f0e76a" stroke-width="8" stroke-dasharray="${C * s.total / 100} ${C}"/></svg><div class="v"><div>${num(s.total)}<small>/ 100</small></div></div></div>
        <div class="breakdown">${rows.map(([l, v, w, pts]) => `<div class="bd-row ${v < 50 ? 'no' : ''}"><div class="top"><span>${l} · ${w}</span><span>${l.startsWith('Viewer') ? (s.viewer ? 'matches' : 'does not match') : num(v) + '%'} → ${num(pts)} pts</span></div><div class="track"><i style="width:${v}%"></i></div></div>`).join('')}
          <div class="faint" style="font-size:9.5px">Orbits are compared innermost-first under each parent. The hidden solution is never shown.</div></div></div>
      <div class="compare"><div class="panel"><h5>ATTEMPTS</h5>${t.attempts.map((a, i) => `<div style="display:flex;justify-content:space-between"><span>#${i + 1}</span><span>${num(a.total)}</span></div>`).join('')}</div>
        <div class="panel"><h5>HINT</h5>${s.topology < 100 ? 'Some objects orbit the wrong parent, or in the wrong order. Watch which lights move together.' : s.traits < 100 ? 'The shape is right. Check which objects shine on their own, and which have rings.' : !s.viewer ? 'Everything is placed. Now: which object are you standing on?' : 'Perfect reconstruction.'}</div></div>
      <div class="actions"><button class="btn" data-act="d-cancel">Keep editing ${kb('Esc')}</button><button class="btn" data-act="d-observe">Observe again ${kb('Tab')}</button><button class="btn primary" data-act="d-maps" style="gap:24px">Choose another map ${kb('M')}</button></div></div>`;
  }
}
function dialogAct(a) {
  const d = S.dialog; S.dialog = null;
  if (a === 'd-clear') { save(); const t = T(); t.nodes = [{ ...t.nodes.find(n => n.parent == null), star: false, rings: false }]; t.viewer = null; t.sel = t.nodes[0].id; t.msg = 'Theory cleared. Undo [Z] restores everything.'; }
  if (a === 'd-discard') { const id = S.pendingMap; S.theories[id] = freshTheory(id); S.best[id] = null; if (S.map === id) S.theory = S.theories[id]; S.pendingMap = null; render(); toast(`Progress discarded · ${sysById(id).puzzle}`); return; }
  if (a === 'd-observe') { go('pobserve'); return; }
  if (a === 'd-maps') { go('maps'); return; }
  renderDialog();
  if (d && S.screen === 'theory') renderTheory();
}

/* ---------------------------------------------------------------- settings / controls */
const SET_TABS = ['Rendering', 'Exposure', 'Time & view', 'Interface'];
function vSettings() {
  const st = S.settings, tab = S.settingsTab;
  const range = (k, min, max, step, fmtv, label, desc, env) => `<div class="setting"><div><b>${label}</b><small>${desc}</small><code>${env}</code></div><div class="ctl"><input type="range" data-set="${k}" min="${min}" max="${max}" step="${step}" value="${st[k]}"><span class="val">${fmtv(st[k])}</span></div></div>`;
  const toggle = (k, label, desc, env) => `<div class="setting"><div><b>${label}</b><small>${desc}</small><code>${env}</code></div><div class="ctl"><button class="check ${st[k] ? 'on' : ''}" data-toggle="${k}"><i></i>${st[k] ? 'On' : 'Off'}</button></div></div>`;
  const seg = (k, opts, label, desc, env) => `<div class="setting"><div><b>${label}</b><small>${desc}</small><code>${env}</code></div><div class="ctl"><div class="seg">${opts.map(([v, l]) => `<button data-seg="${k}" data-v="${v}" class="${st[k] === v ? 'on' : ''}">${l}</button>`).join('')}</div></div></div>`;
  const body = [
    () => `<h3>Rendering</h3><p>Every frame adds fresh Monte Carlo samples to a running average. Stop time and stop moving to converge.</p>
      ${range('spp', 1, 64, 1, v => v + ' spp', 'Samples per frame', 'More samples per frame makes each frame slower, not more accurate.', 'STARGAZE_SPP · 1–64')}
      ${range('bounces', 1, 64, 1, v => v, 'Maximum bounces', 'Surface vertices per path. 1 gives direct light only.', 'STARGAZE_BOUNCES · 1–64')}`,
    () => `<h3>Exposure</h3><p>Exposure is applied when presenting the image, so changing it never discards samples.</p>
      ${toggle('auto', 'Automatic metering', 'Meters the centre of the frame, not the bright starfield.', 'STARGAZE_AUTO_EXPOSURE · [A]')}
      ${range('ev', -8, 8, .25, v => (v >= 0 ? '+' : '') + (+v).toFixed(2) + ' EV', 'Exposure compensation', 'Shifts either mode, in quarter stops.', 'STARGAZE_EV_BIAS · [,] [.]')}
      ${range('key', .05, 1, .05, v => (+v).toFixed(2), 'Metering key', 'The linear luminance the metered percentile maps to.', 'STARGAZE_AUTO_KEY')}
      ${range('pct', .5, .999, .001, v => (+v).toFixed(3), 'Metering percentile', 'Which luminance percentile of the centre region to meter.', 'STARGAZE_AUTO_PERCENTILE')}`,
    () => `<h3>Time &amp; view</h3><p>Playback speed is signed. ±1 day follows the local solar day, not a fixed 1,440 minutes.</p>
      ${seg('steps', [['decade', '1 · 10 · 100 · 1000'], ['legacy', '1m · 1h · 1d']], 'Speed steps', 'Decade steps in minutes per second (idea.md feat1).', '← → in the HUD')}
      ${seg('orient', [['horizon', 'Keep horizon'], ['sky', 'Keep stars']], 'Default orientation', 'Keep the horizon level, or hold the stars still while the ground turns.', '[S]')}`,
    () => `<h3>Interface</h3><p>The HUD and labels never reset the image.</p>
      ${range('hud', 75, 200, 25, v => v + '%', 'HUD scale', 'Bar and menu scale on top of the display scale factor.', 'window scale factor')}
      ${toggle('labels', 'Labels in Explore', 'Name bodies in the sky. Levels always hide them.', '[L]')}
      ${toggle('grid', 'Chart grid', 'Drafting grid behind menus.', '—')}`,
  ][tab]();
  return gameScreen({ title: 'Settings', sub: 'Calibrate the telescope', back: 'back',
    body: `<div class="gsettings"><div class="tabs">${SET_TABS.map((l, i) => `<button class="${i === tab ? 'on' : ''}" data-tab="${i}">${l}${kb(i + 1)}</button>`).join('')}</div><div class="setting-sec">${body}</div></div>`,
    keys: [['1–4', 'Section'], ['Esc', 'Back']] });
}
const KEYS = [
  ['Menus', [['M Esc', 'open / resume'], ['1–9', 'choose a system or map'], ['PgUp PgDn', 'change page'], ['Enter', 'open selected'], ['/', 'search', 1], ['F', 'favorite', 1], ['P', 'Explore ↔ Levels', 1], ['T', 'title screen', 1]]],
  ['Observation', [['drag', 'look around'], ['right-click', 'lock body / sky direction'], ['left-click U', 'release lock'], ['scroll = -', 'zoom (to 0.001°)'], ['← →', 'signed speed'], ['Space', 'stop / resume'], ['PgDn PgUp', '−1 / +1 local day'], ['S', 'star orientation'], [', .', 'exposure ±0.25 EV'], ['A', 'auto exposure'], ['L', 'labels'], ['H', 'hide HUD'], ['F1–F9', 'aim at targets'], ['E', 'find eclipse'], ['T', 'find transit'], ['R', 'reset view'], ['G', 'targets panel', 1]]],
  ['Theory editor', [['Tab', 'observe ↔ theory'], ['right-click N', 'add orbiting object'], ['drag', 'reorder orbits'], ['S', 'is star'], ['R', 'has rings'], ['V', 'viewer here'], ['Del', 'delete branch'], ['Z Y', 'undo / redo'], ['X', 'clear all'], ['Enter', 'check theory'], ['M', 'maps']]],
];
function vControls() {
  return gameScreen({ title: 'Controls', sub: 'Every key, one page', back: 'back',
    body: `<div class="keys-grid">${KEYS.map(([h, rows]) => `<div class="keys-col"><h3>${h.toUpperCase()}</h3>${rows.map(([k, l, n]) => `<div class="k ${n ? 'new' : ''}"><span>${k.split(' ').map(kb).join('')}</span><span>${l}</span></div>`).join('')}</div>`).join('')}</div>`,
    keys: [['Esc', 'Back']] });
}

/* ---------------------------------------------------------------- mount + events */
function mount() {
  const key = { title: 'titleSel', systems: 'sel', maps: 'mapSel' }[S.screen];
  if (key) document.querySelectorAll('[data-row]').forEach(r => r.addEventListener('mouseenter', () => {
    setSel(key, +r.dataset.row);
  }));
  if (S.screen === 'observe' || S.screen === 'pobserve') mountObserve();
  if (S.screen === 'theory') mountTheory();
  const q = $('#q');
  if (q) q.addEventListener('input', () => { S.search = q.value; const pos = q.selectionStart; render(); const n = $('#q'); n.focus(); n.setSelectionRange(pos, pos); });
  document.querySelectorAll('[data-set]').forEach(r => { r.addEventListener('input', () => { S.settings[r.dataset.set] = +r.value; r.nextElementSibling.textContent = r.value; }); r.addEventListener('change', render); });
  const ev = $('#ev');
  if (ev) ev.addEventListener('input', () => { const o = obs(); o.ev = +ev.value; o.auto = false; $('#evtxt').textContent = (o.ev >= 0 ? '+' : '') + o.ev.toFixed(2) + ' EV'; ev.closest('.ev').classList.remove('auto'); $('.check[data-act=auto]')?.classList.remove('on'); });
}
document.addEventListener('click', e => {
  const b = e.target.closest('button,[data-fav]'); if (!b) { if (S.rail && !e.target.closest('#rail') && !e.target.closest('.rail-toggle') && innerWidth < 1500) { S.rail = false; render(); } return; }
  const d = b.dataset;
  if (d.go) { go(d.go, d.variant); return; }
  if (d.fav !== undefined) { e.stopPropagation(); const id = listSystems()[+d.fav].id; S.favs.has(id) ? S.favs.delete(id) : S.favs.add(id); render(); return; }
  if (d.title !== undefined) { titleAction(+d.title); return; }
  if (d.explore !== undefined) { openSystem(+d.explore); return; }
  if (d.play !== undefined) { startMap(+d.play); return; }
  if (d.discard !== undefined) { askDiscard(+d.discard); return; }
  if (d.sys !== undefined) { if (S.sel === +d.sys && e.detail > 1) openSystem(); S.sel = +d.sys; render(); return; }
  if (d.map !== undefined) { S.mapSel = +d.map; render(); return; }
  if (d.filter) { S.filter = d.filter; render(); return; }
  if (d.tab !== undefined) { S.settingsTab = +d.tab; render(); return; }
  if (d.toggle) { S.settings[d.toggle] = !S.settings[d.toggle]; render(); return; }
  if (d.seg) { S.settings[d.seg] = d.v; render(); return; }
  if (d.orient) { setOrient(d.orient); return; }
  if (d.aim !== undefined) { aim(+d.aim); return; }
  if (d.act) act(d.act);
});
function act(a) {
  const o = obs();
  const map = {
    title: () => go('title'), 'back-title': () => go('title'), resume: () => go(S.screen === 'maps' ? 'pobserve' : 'observe'), open: openSystem, 'to-maps': () => go('maps'), 'to-systems': () => go('systems'),
    'start-map': () => startMap(), back: () => go(S.back || 'title'),
    menu: () => go(S.screen === 'observe' ? 'systems' : 'maps'), labels: () => { o.labels = !o.labels; renderHud(); }, hud: () => { o.hud = !o.hud; render(); },
    theory: () => go('theory'), 'observe-p': () => go('pobserve'), 'hide-obj': () => { o.objective = false; render(); },
    slower: () => changeSpeed(-1), faster: () => changeSpeed(1), stop: toggleStop, prevday: () => stepDay(-1), nextday: () => stepDay(1),
    unlock: () => { if (o.lock) { o.lock = null; renderHud(); } else toast('Right-click a body or the sky to lock the view.', { ms: 1800 }); },
    auto: () => { o.auto = !o.auto; if (o.auto) o.ev = 0; renderHud(); }, targets: () => { o.targets = !o.targets; render(); },
    eclipse: () => runSearch('E'), transit: () => runSearch('T'), reset: resetView, zoomin: () => { o.zoom = clamp(o.zoom * 1.5, .6, 5000); o.spp = 0; paintClock(); },
  };
  if (map[a]) return map[a]();
  if (a.startsWith('t-')) return tAct(a);
  if (a.startsWith('d-')) return dialogAct(a);
}
document.addEventListener('keydown', e => {
  const k = e.key, inInput = e.target.tagName === 'INPUT' && e.target.type !== 'range';
  if (k === '`') { S.rail = !S.rail; render(); e.preventDefault(); return; }
  if (k === '?' && !inInput) { S.notes = !S.notes; renderNotes(); return; }
  if ((k === '[' || k === ']') && !inInput) { const i = ORDER.indexOf(S.railPick || S.screen); go(ORDER[(i + (k === ']' ? 1 : -1) + ORDER.length) % ORDER.length]); return; }
  if (S.dialog) {
    const m = { Escape: 'd-cancel', Enter: { clear: 'd-clear', discard: 'd-discard', result: 'd-maps' }[S.dialog], Tab: S.dialog === 'result' && 'd-observe', m: S.dialog === 'result' && 'd-maps' }[k];
    if (m) { e.preventDefault(); dialogAct(m); }
    return;
  }
  if (inInput) { if (k === 'Escape') { e.target.blur(); } if (k === 'Enter') openSystem(); return; }
  const scr = S.screen, lower = k.length === 1 ? k.toLowerCase() : k;
  const digit = /^[1-9]$/.test(k) ? +k - 1 : -1;
  if (scr === 'title') {
    if (k === 'ArrowDown' || k === 'ArrowUp') { e.preventDefault(); moveSel('titleSel', TITLE_ITEMS.length, k === 'ArrowDown' ? 1 : -1); }
    else if (k === 'Enter') titleAction(S.titleSel);
    else { const i = TITLE_ITEMS.findIndex(t => t[0].toLowerCase() === lower); if (i >= 0) titleAction(i); }
  } else if (scr === 'systems') {
    const n = listSystems().length;
    if ((k === 'ArrowDown' || k === 'ArrowUp') && n) { e.preventDefault(); moveSel('sel', n, k === 'ArrowDown' ? 1 : -1); }
    else if (digit >= 0 && digit < n) setSel('sel', digit);
    else if (k === 'Enter') openSystem();
    else if (k === 'Escape') go('title');
  } else if (scr === 'maps') {
    const n = listMaps().length;
    if (k === 'ArrowDown' || k === 'ArrowUp') { e.preventDefault(); moveSel('mapSel', n, k === 'ArrowDown' ? 1 : -1); }
    else if (digit >= 0 && digit < n) setSel('mapSel', digit);
    else if (k === 'Enter') startMap();
    else if (lower === 'd' || k === 'Delete') askDiscard();
    else if (k === 'Escape') go('title');
  } else if (scr === 'observe' || scr === 'pobserve') {
    const o = obs(), puzzle = o.puzzle;
    const F = /^F([1-9])$/.exec(k);
    if (F) { e.preventDefault(); aim(+F[1] - 1); return; }
    const m = {
      Escape: () => act('menu'), m: () => act('menu'), ArrowLeft: () => changeSpeed(-1), ArrowRight: () => changeSpeed(1), ' ': toggleStop,
      PageDown: () => stepDay(-1), PageUp: () => stepDay(1), u: () => { o.lock = null; renderHud(); }, s: () => setOrient(o.orient === 'sky' ? 'horizon' : 'sky'),
      ',': () => { o.ev = clamp(o.ev - .25, -8, 8); renderHud(); }, '.': () => { o.ev = clamp(o.ev + .25, -8, 8); renderHud(); }, a: () => act('auto'), h: () => act('hud'),
      r: resetView, '=': () => act('zoomin'), '+': () => act('zoomin'), '-': () => { o.zoom = clamp(o.zoom / 1.5, .6, 5000); o.spp = 0; paintClock(); },
      l: () => puzzle ? toast('Labels are hidden during a level.') : act('labels'), e: () => runSearch('E'), t: () => runSearch('T'),
      g: () => !puzzle && act('targets'), Tab: () => puzzle ? go('theory') : toast('Tab opens the theory in a level.'),
    }[lower];
    if (m) { e.preventDefault(); m(); }
  } else if (scr === 'theory') {
    const m = { s: 't-star', r: 't-rings', v: 't-viewer', Delete: 't-del', Backspace: 't-del', z: 't-undo', y: 't-redo', n: 't-add', x: 't-clear', Enter: 't-check' }[lower];
    if (m) { e.preventDefault(); tAct(m); }
    else if (k === 'Tab') { e.preventDefault(); go('pobserve'); }
    else if (k === 'Escape' || lower === 'm') go('maps');
  } else if (scr === 'settings' || scr === 'controls') {
    if (k === 'Escape') go(S.back || 'title');
    else if (scr === 'settings' && digit >= 0 && digit < 4) { S.settingsTab = digit; render(); }
  }
});
addEventListener('resize', () => { if (S.screen === 'theory') renderTheory(); });

let last = performance.now();
function loop(now) {
  const dt = Math.min(.1, (now - last) / 1000); last = now;
  if (S.screen === 'observe' || S.screen === 'pobserve') tickObserve(dt); else tickPreviews(dt);
  requestAnimationFrame(loop);
}

const start = new URLSearchParams(location.search).get('screen');
S.rail = innerWidth >= 1500;
go(start && (ORDER.includes(start)) ? start : 'title');
requestAnimationFrame(loop);
