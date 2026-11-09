// Browser-side interaction model. Native rendering and the Rust save files are
// deliberately independent; replace this adapter when the design is settled.
export const SPEEDS = [-1000, -100, -10, -1, 0, 1, 10, 100, 1000];
export const STOP = 4;
export const LEVELS = [
  { id: '1-halo-volcano', title: 'Ember summit', system: 'Calyx and Halo — the volcano', ground: 'Basalt', image: 'ground', fov: 70, targets: ['Calyx', 'Aur', 'Igni', 'Umbra', 'Nerid', 'Vantus'], kind: 'planet', target: 'Calyx' },
  { id: '2-erg-dunes', title: 'Shadow season', system: 'Erg — the ringed dune sea', ground: 'Dunes', image: 'desert', fov: 60, targets: ['Ochre', 'Gemma', 'Gemmi', 'Simoom', 'Sirocco'], kind: 'planet', target: 'Ochre' },
  { id: '3-umbra-valley', title: 'The long shadow', system: 'Umbra — the daily eclipse', ground: 'Ice field', image: 'eclipse', fov: 25, targets: ['Umbra', 'Lucero', 'Ascua', 'Cobre', 'Nacar'], kind: 'planet', target: 'Umbra' },
];
export const SYSTEMS = [
  ...LEVELS,
  { id: 'solar-system', title: 'Distant lights', system: 'Solar System', ground: 'Forest', image: 'moon', fov: 1.1, targets: ['Moon', 'Sun', 'Mercury', 'Venus', 'Mars', 'Jupiter', 'Saturn', 'Uranus', 'Neptune'], kind: 'planet', target: 'Moon' },
  { id: 'saturn', title: 'Rings at dusk', system: 'Saturn observatory', ground: 'Rock', image: 'saturn', fov: 30, targets: ['Saturn', 'Sun', 'Titan', 'Rhea'], kind: 'planet', target: 'Saturn' },
];
export const PRESETS = {
  Eco: { resolution: 50, samples: 1, bounces: 2, vegetation: 35 },
  Balanced: { resolution: 75, samples: 1, bounces: 4, vegetation: 70 },
  High: { resolution: 100, samples: 1, bounces: 8, vegetation: 100 },
};
export const DEFAULT_SETTINGS = { ...PRESETS.Balanced, fps: 30, limit: 256, hud: 100 };
export const SETTING_ROWS = [
  { key: 'fps', label: 'Frame rate', desc: 'Maximum frames per second while observing.', values: [15, 30, 60, 90, 120, 240, 0], format: v => v ? `${v} fps` : 'Uncapped' },
  { key: 'resolution', label: 'Render resolution', desc: 'Scale the sky image. Menus and text stay at full resolution.', min: 25, max: 100, step: 25, format: v => `${v}%` },
  { key: 'bounces', label: 'Maximum bounces', desc: 'How many surface interactions each light path can make.', min: 1, max: 64, step: 1, format: v => `${v}` },
  { key: 'samples', label: 'Samples per frame', desc: 'Higher values spend more time tracing each frame.', min: 1, max: 64, step: 1, format: v => `${v} spp` },
  { key: 'limit', label: 'Still-image limit', desc: 'Stop refining a stationary image after this many samples.', values: [64, 128, 256, 512, 1024, 4096, 0], format: v => v ? `${v.toLocaleString()} spp` : 'Continuous' },
  { key: 'vegetation', label: 'Vegetation density', desc: 'Trees and ground cover. Independent of render resolution.', min: 0, max: 100, step: 5, format: v => `${v}%` },
  { key: 'hud', label: 'HUD scale', desc: 'Size of the observation panels on top of display scaling.', min: 75, max: 200, step: 25, format: v => `${v}%` },
];
export const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
export const clone = value => structuredClone(value);
export const systemById = id => SYSTEMS.find(s => s.id === id) || LEVELS[0];

export function freshTheory() {
  return { nodes: [{ id: 0, parent: null, x: 500, y: 330, star: false, rings: false }], selected: 0, viewer: null, next: 1, undo: [], redo: [], attempts: [], best: null, message: 'Select a parent, then right-click empty space to add an orbiting object.' };
}
export function freshObservation(id = LEVELS[0].id, puzzle = true) {
  return { id, puzzle, minutes: 730.5 * 1440, speed: STOP, resume: 5, hud: true, labels: !puzzle, targets: false, air: false, density: 1, tint: 0, ev: 0, auto: true, lock: null, fov: systemById(id).fov, spp: 128, ground: systemById(id).ground };
}
export function createState(saved = {}) {
  if (!saved || typeof saved !== 'object' || Array.isArray(saved)) saved = {};
  const settings = { ...DEFAULT_SETTINGS };
  for (const row of SETTING_ROWS) {
    const value = saved.settings?.[row.key];
    if (Number.isFinite(value)) settings[row.key] = row.values ? (row.values.includes(value) ? value : settings[row.key]) : clamp(value, row.min, row.max);
  }
  const theories = {};
  for (const [id, t] of Object.entries(saved.theories || {})) {
    if (!SYSTEMS.some(s => s.id === id) || !validTheory(t)) continue;
    theories[id] = { ...freshTheory(), ...clone(t), undo: [], redo: [] };
  }
  const stored = saved.observation || {};
  const observation = freshObservation(SYSTEMS.some(s => s.id === stored.id) ? stored.id : LEVELS[0].id, stored.puzzle !== false);
  for (const [key, lo, hi] of [['minutes', -1e12, 1e12], ['ev', -8, 8], ['fov', .001, 90], ['density', 0, 3], ['tint', -180, 180], ['spp', 0, 16777216]]) {
    if (Number.isFinite(stored[key])) observation[key] = clamp(stored[key], lo, hi);
  }
  for (const key of ['hud', 'labels', 'auto', 'air', 'targets']) if (typeof stored[key] === 'boolean') observation[key] = stored[key];
  for (const key of ['speed', 'resume']) if (Number.isInteger(stored[key]) && stored[key] >= 0 && stored[key] < SPEEDS.length) observation[key] = stored[key];
  if (['star', 'planet', 'stars'].includes(stored.lock?.kind) && typeof stored.lock.name === 'string') observation.lock = { kind: stored.lock.kind, name: stored.lock.name.slice(0, 100) };
  const active = SYSTEMS.some(s => s.id === saved.active) ? saved.active : null;
  return { screen: 'title', back: 'title', selected: 0, titleSelected: 0, settings, draft: clone(settings), theories, favorites: Array.isArray(saved.favorites) ? saved.favorites.filter(v => typeof v === 'string') : [], observation, active, modal: null, search: '', favoritesOnly: false, settingSelected: 0, texture: { style: 'cratered', scale: 1, roughness: .7, contrast: 1, rotating: false } };
}
export function validTheory(t) {
  if (!t || !Array.isArray(t.nodes) || !t.nodes.length || t.nodes.length > 64) return false;
  const ids = new Set();
  for (const n of t.nodes) {
    if (!Number.isInteger(n.id) || ids.has(n.id) || !Number.isFinite(n.x) || !Number.isFinite(n.y)) return false;
    if (n.parent !== null && !ids.has(n.parent)) return false;
    ids.add(n.id);
  }
  return t.nodes[0].parent === null && ids.has(t.selected) && (t.viewer === null || ids.has(t.viewer));
}
export function persistentState(s) {
  const theories = Object.fromEntries(Object.entries(s.theories).map(([id, t]) => [id, { ...t, undo: [], redo: [] }]));
  return { settings: s.settings, theories, favorites: s.favorites, active: s.active, observation: s.observation };
}
export function progress(t) {
  const objects = t ? t.nodes.length - 1 : 0;
  const attempts = t?.attempts?.length || 0;
  return { objects, attempts, best: t?.best ?? null, started: objects > 0 || attempts > 0 || t?.viewer != null || !!t?.nodes.some(n => n.star || n.rings), solved: (t?.best || 0) >= 99.9 };
}
export function theory(s) { return s.theories[s.active || LEVELS[0].id] ||= freshTheory(); }
function snapshot(t) { return clone({ nodes: t.nodes, selected: t.selected, viewer: t.viewer, next: t.next }); }
export function remember(t) { t.undo.push(snapshot(t)); if (t.undo.length > 100) t.undo.shift(); t.redo = []; }
export function history(t, redo = false) {
  const from = redo ? t.redo : t.undo, to = redo ? t.undo : t.redo;
  if (!from.length) return false;
  to.push(snapshot(t)); Object.assign(t, from.pop()); t.message = redo ? 'Edit redone.' : 'Edit undone.'; return true;
}
export function orbitRank(t, node) {
  if (node.parent === null) return 0;
  const p = t.nodes.find(n => n.id === node.parent);
  return t.nodes.filter(n => n.parent === node.parent).sort((a, b) => Math.hypot(a.x - p.x, a.y - p.y) - Math.hypot(b.x - p.x, b.y - p.y) || a.id - b.id).findIndex(n => n.id === node.id) + 1;
}
export function addNode(t, x, y) {
  if (t.nodes.length >= 64) { t.message = 'Diagram limit: 64 objects. Delete a branch to make room.'; return false; }
  remember(t); const id = t.next++;
  t.nodes.push({ id, parent: t.selected, x: clamp(x, 20, 980), y: clamp(y, 30, 630), star: false, rings: false });
  t.selected = id; t.message = 'Added. Distance from its parent sets the orbit order.'; return true;
}
export function deleteBranch(t) {
  const node = t.nodes.find(n => n.id === t.selected);
  if (!node || node.parent === null) { t.message = 'The centre stays. Select an orbiting object to delete its branch.'; return false; }
  remember(t); const removed = new Set([node.id]);
  for (const n of t.nodes) if (removed.has(n.parent)) removed.add(n.id);
  t.nodes = t.nodes.filter(n => !removed.has(n.id));
  if (removed.has(t.viewer)) t.viewer = null;
  t.selected = node.parent; t.message = 'Branch deleted. Undo restores it.'; return true;
}
export function clearTheory(t) {
  remember(t); Object.assign(t, { nodes: freshTheory().nodes, selected: 0, viewer: null, next: 1 });
  t.message = 'Theory cleared. Undo restores it.';
}
export function toggleTrait(t, trait) {
  if (!['star', 'rings'].includes(trait)) return;
  remember(t); const n = t.nodes.find(n => n.id === t.selected); n[trait] = !n[trait];
  t.message = `${trait === 'star' ? 'Star' : 'Rings'} ${n[trait] ? 'added' : 'removed'}.`;
}
export function changeSpeed(o, delta) { o.speed = clamp(o.speed + delta, 0, SPEEDS.length - 1); if (o.speed !== STOP) o.resume = o.speed; }
export function togglePlayback(o) { if (o.speed === STOP) o.speed = o.resume === STOP ? 5 : o.resume; else { o.resume = o.speed; o.speed = STOP; } }
export function presetName(settings) { return Object.entries(PRESETS).find(([, p]) => Object.entries(p).every(([k, v]) => settings[k] === v))?.[0] || 'Custom'; }
export function clock(minutes) {
  const total = Math.floor(minutes), minute = ((total % 1440) + 1440) % 1440;
  return `day ${Math.floor(total / 1440)} · ${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`;
}
export function speedLabel(speed) { const v = SPEEDS[speed]; return `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toLocaleString('en-US')} min/s`; }

// Representative result data for the design preview. Scoring remains in Rust;
// the browser never pretends that a screenshot contains a simulated solution.
export function previewCheck(t) {
  if (t.viewer === null) { t.message = 'Choose an object and mark Viewer here before checking.'; return null; }
  const attempt = { total: 72.4, topology: 80, traits: 72, viewer: false, number: t.attempts.length + 1 };
  t.attempts.push(attempt); t.best = Math.max(t.best || 0, attempt.total); return attempt;
}
