import { LEVELS, SYSTEMS, SPEEDS, STOP, PRESETS, SETTING_ROWS, clone, clamp, createState, persistentState, systemById, freshObservation, freshTheory, progress, theory, remember, history, orbitRank, addNode, deleteBranch, clearTheory, toggleTrait, changeSpeed, togglePlayback, presetName, clock, speedLabel, previewCheck } from './model.js';

const $ = selector => document.querySelector(selector);
const esc = text => String(text).replace(/[&<>"']/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]));
const storageKey = 'stargaze-ui-preview-v1';
let saved = {};
try { saved = JSON.parse(localStorage.getItem(storageKey) || '{}'); } catch { /* A broken browser preview save starts fresh. */ }
const state = createState(saved);
let runtime = {};
try { runtime = await (await fetch('/runtime.json')).json(); } catch { /* Static hosting works too. */ }
const exploreEnabled = runtime.explore || new URLSearchParams(location.search).get('explore') === '1';
if (!exploreEnabled && !state.observation.puzzle) { state.observation = freshObservation(); state.active = LEVELS[0].id; }
const root = $('#app');
let storageNotice = false;
function persist() {
  try { localStorage.setItem(storageKey, JSON.stringify(persistentState(state))); }
  catch { if (!storageNotice) { storageNotice = true; toast('Browser storage is unavailable. Changes will last for this session.'); } }
}
const icon = (name, cls = '') => {
  const paths = {
    logo: '<circle cx="12" cy="12" r="4"/><ellipse cx="12" cy="12" rx="11" ry="4" transform="rotate(-35 12 12)"/>',
    wheel: '<rect x="6" y="2" width="12" height="20" rx="5"/><path d="M6 10h12M12 2v8"/><path d="M12 5v2" stroke-width="2.5"/>',
    play: '<path d="M8 5v14l11-7z" fill="currentColor" stroke="none"/>',
    pause: '<path d="M8 5v14M16 5v14" stroke-width="3"/>',
    star: '<circle cx="12" cy="12" r="4"/><path d="M12 1v4M12 19v4M1 12h4M19 12h4M4 4l3 3M17 17l3 3M4 20l3-3M17 7l3-3"/>',
    planet: '<circle cx="12" cy="12" r="8" fill="currentColor" opacity=".22"/><path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor"/><circle cx="12" cy="12" r="8"/>',
    stars: '<path d="M6 10v8M2 14h8M15 1v10M10 6h10M20 14v8M16 18h8"/>',
    rings: '<circle cx="12" cy="12" r="5"/><ellipse cx="12" cy="12" rx="11" ry="3" transform="rotate(-25 12 12)"/>',
    leftMouse: '<rect x="5" y="1" width="14" height="22" rx="6"/><path d="M5 10h14M12 1v9"/><path d="M6 9V7a6 6 0 0 1 5-5v7z" fill="currentColor"/>',
    rightMouse: '<rect x="5" y="1" width="14" height="22" rx="6"/><path d="M5 10h14M12 1v9"/><path d="M18 9V7a6 6 0 0 0-5-5v7z" fill="currentColor"/>',
  };
  return `<svg class="icon ${cls}" viewBox="0 0 24 24" aria-hidden="true">${paths[name] || ''}</svg>`;
};
const key = (label, code = label) => `<kbd data-key="${esc(code)}">${esc(label)}</kbd>`;
const button = (text, action, extra = '', cls = '') => `<button type="button" class="button ${cls}" data-action="${action}" ${extra}>${text}</button>`;
const toggle = (on, label, action) => `<button type="button" role="switch" aria-checked="${on}" aria-label="${label}" class="toggle ${on ? 'on' : ''}" data-action="${action}"><span></span></button>`;
const menuItems = () => [['levels', 'Play', 'Your levels'], ...(exploreEnabled ? [['explore', 'Explore', 'Any system, freely']] : []), ['settings', 'Settings', 'Rendering, HUD scale'], ['quit', 'Quit', '']];
const currentTheory = () => theory(state);
const currentSystem = () => systemById(state.observation.id);
const activeTitle = () => systemById(state.active).title;
function toast(text) {
  $('#toast').textContent = text; $('#toast').hidden = false;
  clearTimeout(toast.timer); toast.timer = setTimeout(() => { $('#toast').hidden = true; }, 3200);
}
function navigate(screen, { back = true, replace = false } = {}) {
  if (screen === 'explore' && !exploreEnabled) screen = 'levels';
  if (!['title', 'levels', 'explore', 'observe', 'theory', 'settings', 'controls', 'texture'].includes(screen)) screen = 'title';
  if (screen === 'settings' && state.screen !== 'settings') { state.draft = clone(state.settings); state.settingSelected = 0; }
  if (back && ['settings', 'controls'].includes(screen) && !['settings', 'controls'].includes(state.screen)) state.back = state.screen;
  if (screen === 'theory' && !state.active) state.active = LEVELS[0].id;
  state.screen = screen; state.modal = null; state.selected = 0;
  if (location.hash !== `#${screen}`) window.history[replace || !location.hash ? 'replaceState' : 'pushState'](null, '', `${location.pathname}${location.search}#${screen}`);
  render();
}
function page(title, eyebrow, body, footer = '') {
  return `<section class="page"><header class="page-header"><div class="eyebrow">${eyebrow}</div><h1>${title}</h1></header><div class="page-body">${body}</div><footer class="page-footer">${footer || `${button(`Back ${key('Esc', 'Escape')}`, 'title', '', 'quiet')}<span class="footer-spacer"></span>${state.active ? button(`Resume ${key('M', 'KeyM')}`, 'resume', '', 'quiet') : ''}${button(`Controls ${key('C', 'KeyC')}`, 'controls', '', 'quiet')}`}</footer></section>`;
}
function titleScreen() {
  return `<section class="title-screen"><div class="title-inner"><h1 class="wordmark">${icon('logo')}<span>stargaze<span class="wordmark-dot">.</span></span></h1><nav class="title-menu" aria-label="Main menu">${menuItems().map(([action, label, desc], i) => `<button class="title-item ${state.titleSelected === i ? 'selected' : ''}" data-action="${action}" data-title-index="${i}"><span class="title-caret" aria-hidden="true">▸</span><span><b>${label}</b>${desc ? `<small>${desc}</small>` : ''}</span></button>`).join('')}</nav></div></section>`;
}
function visibleSystems() { return SYSTEMS.filter(s => (!state.favoritesOnly || state.favorites.includes(s.id)) && `${s.system} ${s.id}`.toLowerCase().includes(state.search.toLowerCase())); }
function levelsScreen(explore = false) {
  const entries = explore ? visibleSystems() : LEVELS;
  const filter = explore ? `<div class="list-filter"><label class="search-field"><span aria-hidden="true">⌕</span><input id="system-search" type="search" placeholder="Search systems" value="${esc(state.search)}" aria-label="Search systems"></label>${button(`${state.favoritesOnly ? '★' : '☆'} Favorites`, 'favorites-filter', `aria-pressed="${state.favoritesOnly}"`, state.favoritesOnly ? 'active' : '')}</div>` : '';
  const rows = entries.map((item, index) => {
    const p = progress(state.theories[item.id]);
    return `<article class="level-row ${state.selected === index ? 'selected' : ''}" data-row="${index}" data-id="${item.id}"><span class="row-number">${String(index + 1).padStart(2, '0')}</span><div class="level-copy"><h2>${esc(explore ? item.system : item.title)}</h2>${explore ? `<p>${esc(item.ground)} · ${item.fov}° FOV <span class="filename">${item.id}.yaml</span></p>` : `<div class="level-status"><span class="level-progress" aria-label="Best score ${p.best ?? 0} of 100"><i style="width:${p.best || 0}%"></i></span><span>${p.solved ? 'Solved' : p.started ? 'In progress' : 'Not started'}</span><span>${p.best !== null ? `best ${p.best.toFixed(1)}` : ''}${p.objects ? ` · ${p.objects} objects drawn` : ''}</span></div>`}</div>${button(`${explore ? 'Explore' : p.started ? 'Resume' : 'Play'} ${key('Enter')}`, explore ? 'open-system' : 'play-level', `data-id="${item.id}"`, 'primary')}${explore ? button(state.favorites.includes(item.id) ? '★' : '☆', 'favorite', `data-id="${item.id}" aria-label="${state.favorites.includes(item.id) ? 'Unfavorite' : 'Favorite'} ${esc(item.system)}" aria-pressed="${state.favorites.includes(item.id)}"`, 'favorite') : button(`Discard ${key('D', 'KeyD')}`, 'ask-discard', `data-id="${item.id}" ${p.started ? '' : 'disabled'}`, 'discard')}</article>`;
  }).join('');
  return page(explore ? 'Explore' : 'Levels', explore ? 'Any system · labels and targets on' : 'Play · reconstruct unknown skies', `${filter}<div class="level-list">${rows || '<div class="empty-state"><h2>No systems found</h2><p>Try a different search or turn off the favorites filter.</p></div>'}</div>`);
}
function settingsScreen() {
  const selectedPreset = presetName(state.draft);
  const preset = `<div class="setting-row"><div><h3>Quality preset</h3><p>Sets resolution, light detail, and vegetation together.</p></div><div class="preset-group" aria-label="Quality preset">${Object.keys(PRESETS).map(name => button(name, 'preset', `data-value="${name}" aria-pressed="${selectedPreset === name}"`, selectedPreset === name ? 'active' : '')).join('')}<span class="custom ${selectedPreset === 'Custom' ? 'active' : ''}">Custom</span></div></div>`;
  const rows = SETTING_ROWS.map((row, index) => `<div class="setting-row ${state.settingSelected === index ? 'selected-setting' : ''}" data-setting-row="${index}"><label for="setting-${row.key}"><h3>${row.label}</h3><p>${row.desc}</p></label><div class="setting-control"><input id="setting-${row.key}" type="range" data-setting="${row.key}" aria-label="${row.label}" min="${row.values ? 0 : row.min}" max="${row.values ? row.values.length - 1 : row.max}" step="${row.values ? 1 : row.step}" value="${row.values ? row.values.indexOf(state.draft[row.key]) : state.draft[row.key]}"><output for="setting-${row.key}">${row.format(state.draft[row.key])}</output></div></div>`);
  const body = `<div class="settings-body"><section><h2>Rendering</h2><p class="section-desc">Stop time and stop moving to let the image converge.</p>${preset}${rows.slice(0, 6).join('')}</section><section><h2>Interface</h2><p class="section-desc">Interface changes keep the sky image intact.</p>${rows[6]}</section></div>`;
  return page('Settings', 'Calibrate the telescope', body, `<span id="settings-dirty">${JSON.stringify(state.settings) !== JSON.stringify(state.draft) ? 'Unsaved changes' : ''}</span><span class="footer-spacer"></span>${button(`Back ${key('Esc', 'Escape')}`, 'back')}${button(`Apply ${key('Enter')}`, 'apply-settings', '', 'primary')}`);
}
function controlsScreen() {
  const groups = [
    ['Menus', [['↑ ↓', 'Select an item'], ['Enter →', 'Open selected'], ['Esc ←', 'Back'], ['M', 'Open / resume'], ['C', 'Controls'], ['T', 'Title screen'], ...(exploreEnabled ? [['P', 'Explore / Levels'], ['/', 'Search systems'], ['F', 'Favorite system'], ['B', 'Favorites filter']] : [])]],
    ['Observation', [['mouse', 'Look around'], ['right-click', 'Lock body / background sky'], ['left-click', 'Release lock'], ['scroll', 'Zoom'], ['← →', 'Signed playback speed'], ['Space', 'Pause / resume'], ['PgDn PgUp', '−1 / +1 local day'], [', .', 'Exposure ±0.25 EV'], ['A', 'Auto exposure'], ['H', 'Hide / show HUD'], ['P', 'Settings'], ['Tab', 'Observe / theory']]],
    ['Theory editor', [['right-click', 'Add orbiting object'], ['drag', 'Reorder orbits'], ['S', 'Is star'], ['R', 'Has rings'], ['V', 'Viewer here'], ['Del', 'Delete branch'], ['Z Y', 'Undo / redo'], ['X', 'Clear all'], ['Enter', 'Check theory'], ['Tab', 'Observe'], ['M', 'Maps']]],
  ];
  return page('Controls', 'Every key, one page', `<div class="controls-grid">${groups.map(([title, rows]) => `<section><h2>${title}</h2>${rows.map(([keys, label]) => `<div class="control-reference"><span>${keys.split(' ').map(k => key(k)).join(' ')}</span><span>${label}</span></div>`).join('')}</section>`).join('')}</div>`, `${button(`Back ${key('Esc', 'Escape')}`, 'back')}`);
}
function hudKey(label, code, action, text = '') { return `<button class="hud-action" data-action="${action}">${key(label, code)}${text ? `<span>${text}</span>` : ''}</button>`; }
function speedBars() { return `<span class="speed-bars" role="img" aria-label="Playback speed ${speedLabel(state.observation.speed)}">${SPEEDS.map((_, index) => `<i style="height:${3 + Math.abs(index - STOP) * 2}px" class="${index === state.observation.speed ? 'current' : ''}"></i>`).join('')}</span>`; }
function lockName() {
  const o = state.observation;
  return !o.lock ? '' : o.lock.kind === 'stars' ? 'Background sky' : o.puzzle || !o.labels ? o.lock.kind === 'star' ? 'Star' : 'Planet / moon' : o.lock.name;
}
function observationHud() {
  const o = state.observation;
  if (!o.hud) return `<div class="hud-return panel">${hudKey('H', 'KeyH', 'hud', 'show HUD')}</div>`;
  const system = currentSystem();
  return `<div class="observation-hud" style="--hud-scale:${Math.min(state.settings.hud / 100, innerWidth / 1100, innerHeight / 560)}">
    ${o.puzzle ? `<aside class="field-brief panel"><div class="eyebrow">Field study / observe the unknown</div><h2>${esc(system.title)}</h2><ul><li>Watch which lights move together</li><li>Draw their orbits in Theory ${key('Tab')}</li><li>Mark where you are standing</li></ul></aside>` : ''}
    <aside class="hud-legend panel" aria-label="Observation controls">${!o.puzzle ? '<div class="eyebrow">Observatory / Explore</div>' : ''}<div class="hud-line">${hudKey('M', 'KeyM', 'maps', o.puzzle ? 'maps' : 'menu')}${o.puzzle ? hudKey('Tab', 'Tab', 'theory', 'theory') : ''}${hudKey('P', 'KeyP', 'settings', 'settings')}${hudKey('H', 'KeyH', 'hud', 'hide HUD')}</div>${!o.puzzle ? `<div class="hud-line">${hudKey('L', 'KeyL', 'labels', 'labels')}${hudKey('G', 'KeyG', 'targets', 'targets')}</div><div class="hud-line">${hudKey('B', 'KeyB', 'ground', `ground: ${o.ground}`)}${hudKey('J', 'KeyJ', 'air', 'air')}</div>` : ''}<div class="hud-line view-line"><span id="convergence">${o.spp.toLocaleString()} spp · ${o.speed === STOP ? 'ready' : 'converging'}</span><span>·</span><span id="fov-readout">FOV ${o.fov.toFixed(3)}°</span><span class="wheel-hint">${icon('wheel')}zoom</span></div></aside>
    ${!o.puzzle && o.targets ? `<aside class="target-panel panel"><h3>Targets</h3>${system.targets.map((name, index) => button(`${key(String((index + 1) % 10), `Digit${(index + 1) % 10}`)} ${esc(name)}`, 'aim', `data-value="${esc(name)}"`, 'target-item')).join('')}</aside>` : ''}
    ${!o.puzzle && o.air ? `<aside class="air-panel panel"><h3>Atmosphere</h3><label>Density <output>${o.density.toFixed(2)}×</output><input aria-label="Atmosphere density" type="range" min="0" max="3" step=".05" value="${o.density}" data-observation="density"></label><label>Hue <output>${o.tint}°</output><input aria-label="Atmosphere hue" type="range" min="-180" max="180" step="5" value="${o.tint}" data-observation="tint"></label></aside>` : ''}
    <section class="time-panel panel" aria-label="Time controls"><div class="time-readout"><span id="clock">${clock(o.minutes)}</span><span>·</span><span id="speed-readout">${speedLabel(o.speed)}</span></div><div class="time-columns"><span>−1 day</span>${speedBars()}<span>+1 day</span>${hudKey('PgDn', 'PageDown', 'day-back')}<div class="playback-buttons">${hudKey('←', 'ArrowLeft', 'slower')}<button class="hud-action" data-action="playback" aria-label="${o.speed === STOP ? 'Resume' : 'Pause'} playback"><kbd data-key="Space">${icon(o.speed === STOP ? 'play' : 'pause')}Space</kbd></button>${hudKey('→', 'ArrowRight', 'faster')}</div>${hudKey('PgUp', 'PageUp', 'day-forward')}</div></section>
    <section class="lock-panel panel ${o.lock ? 'following' : ''}" aria-label="Lock status"><span class="lock-reticle">${o.lock ? icon(o.lock.kind) : ''}</span><div><div class="lock-heading"><span class="eyebrow">${o.lock ? 'Following' : 'Free look'}</span>${o.lock ? `<span class="release-hint">${icon('leftMouse')}release</span>` : ''}</div><span class="lock-name">${o.lock ? esc(lockName()) : `${icon('rightMouse')}right-click to lock`}</span></div></section>
    <section class="exposure-panel panel" aria-label="Exposure controls"><div class="exposure-columns"><span>−</span><span>exposure</span><span>+</span>${hudKey(',', 'Comma', 'ev-down')}<input aria-label="Exposure compensation" id="exposure-slider" type="range" min="-8" max="8" step=".25" value="${o.ev}">${hudKey('.', 'Period', 'ev-up')}</div><div class="exposure-footer"><output id="ev-readout" class="${o.auto ? 'muted' : ''}">${o.ev >= 0 ? '+' : ''}${o.ev.toFixed(2)} EV</output><button class="auto-label ${o.auto ? 'on' : ''}" data-action="auto">[A]uto</button>${toggle(o.auto, 'Auto exposure', 'auto')}</div></section>
  </div>`;
}
function observationScreen() {
  const o = state.observation, system = currentSystem();
  return `<section class="observation-screen" id="observation-view" aria-label="${o.puzzle ? 'Field study' : 'Observatory'}"><div class="native-preview" style="background-image:url('assets/${system.image}.png')"></div><div class="aim-reticle" aria-hidden="true"></div>${o.lock && o.lock.kind !== 'stars' ? '<div class="sky-lock-brackets" aria-hidden="true"></div>' : ''}${!o.puzzle && o.labels ? `<span class="sky-body-label">${esc(o.lock?.name || system.target)}</span>` : ''}<div id="hud-root">${observationHud()}</div></section>`;
}
const nodeLabel = (t, n) => n.parent === null ? 'C' : String(orbitRank(t, n));
function theorySvg() {
  const t = currentTheory();
  let svg = '<defs><pattern id="diagram-grid" width="32" height="32" patternUnits="userSpaceOnUse"><circle cx="1" cy="1" r=".7" fill="#b7c7ff" opacity=".2"/></pattern></defs><rect width="1000" height="660" fill="url(#diagram-grid)"/>';
  for (const n of t.nodes.filter(n => n.parent !== null)) {
    const p = t.nodes.find(p => p.id === n.parent);
    const radius = Math.hypot(n.x - p.x, n.y - p.y);
    svg += `<circle cx="${p.x}" cy="${p.y}" r="${radius}" fill="none" stroke="var(--line)" stroke-dasharray="3 7"/><path d="M${p.x} ${p.y}L${n.x} ${n.y}" stroke="var(--line)" stroke-width="1"/>`;
  }
  for (const n of t.nodes) {
    svg += `<g data-node="${n.id}" class="theory-node ${t.selected === n.id ? 'selected' : ''}" role="button" tabindex="0" aria-label="Object ${nodeLabel(t, n)}${n.star ? ', star' : ''}${n.rings ? ', rings' : ''}${t.viewer === n.id ? ', viewer' : ''}" transform="translate(${n.x} ${n.y})"><circle class="node-selection" r="26"/><circle class="node-disc ${n.star ? 'is-star' : ''}" r="18"/>`;
    if (n.star) for (let ray = 0; ray < 8; ray++) { const a = ray * Math.PI / 4; svg += `<path class="star-ray" d="M${Math.cos(a) * 24} ${Math.sin(a) * 24}L${Math.cos(a) * 32} ${Math.sin(a) * 32}"/>`; }
    if (n.rings) svg += '<ellipse rx="32" ry="10" fill="none" stroke="var(--text)" transform="rotate(-20)"/>';
    svg += `<text y="4.5" text-anchor="middle">${nodeLabel(t, n)}</text>${t.viewer === n.id ? '<g class="viewer-marker"><rect x="-18" y="35" width="36" height="17"/><text y="47" text-anchor="middle">YOU</text></g>' : ''}</g>`;
  }
  return svg;
}
function inspector() {
  const t = currentTheory(), n = t.nodes.find(n => n.id === t.selected), parent = t.nodes.find(p => p.id === n.parent);
  return `<header><span>Selected object</span><b>${nodeLabel(t, n)}</b></header><dl><dt>Role</dt><dd>${n.parent === null ? 'Centre' : n.star ? 'Star' : 'Orbiting body'}</dd><dt>Orbits</dt><dd>${parent ? nodeLabel(t, parent) : '—'}</dd><dt>Orbit order</dt><dd>${n.parent === null ? '—' : `${orbitRank(t, n)} of ${t.nodes.filter(p => p.parent === n.parent).length}`}</dd><dt>Satellites</dt><dd>${t.nodes.filter(p => p.parent === n.id).length}</dd><dt>Viewer</dt><dd class="${t.viewer === n.id ? 'accent-green' : ''}">${t.viewer === n.id ? 'you are here' : 'not here'}</dd></dl><div class="inspector-traits">${button(`${icon('star')} Is star ${toggleStatic(n.star)}`, 'trait-star', `aria-pressed="${n.star}"`, n.star ? 'active' : '')}${button(`${icon('rings')} Has rings ${toggleStatic(n.rings)}`, 'trait-rings', `aria-pressed="${n.rings}"`, n.rings ? 'active' : '')}</div>`;
}
const toggleStatic = on => `<span class="toggle static ${on ? 'on' : ''}"><span></span></span>`;
function theoryScreen() {
  const t = currentTheory();
  return `<section class="theory-screen"><header class="theory-header"><h1>Your theory</h1><span class="eyebrow">${esc(activeTitle())}</span><p>Select a parent, right-click to add. Drag to order orbits.</p><span class="theory-count">${t.nodes.length} / 64 objects</span></header><div class="theory-workspace"><div class="diagram-wrap" id="diagram-wrap"><svg id="diagram" viewBox="0 0 1000 660" aria-label="Orbital theory diagram">${theorySvg()}</svg><div class="diagram-help">Orbit 1 = innermost <span>right-click empty space to add</span></div></div><aside class="inspector" id="inspector">${inspector()}</aside></div><div class="theory-status" id="theory-message" role="status">${esc(t.message)}</div><footer class="theory-toolbar"><div>${button(`Maps ${key('M', 'KeyM')}`, 'maps')}${button(`Observe ${key('Tab')}`, 'observe')}</div><div>${button(`${icon('star')} Is star ${key('S', 'KeyS')}`, 'trait-star')}${button(`${icon('rings')} Has rings ${key('R', 'KeyR')}`, 'trait-rings')}${button(`Viewer here ${key('V', 'KeyV')}`, 'viewer', `aria-pressed="${t.viewer === t.selected}"`, t.viewer === t.selected ? 'active' : '')}</div><div>${button(`Undo ${key('Z', 'KeyZ')}`, 'undo', t.undo.length ? '' : 'disabled')}${button(`Redo ${key('Y', 'KeyY')}`, 'redo', t.redo.length ? '' : 'disabled')}${button(`Delete ${key('Del', 'Delete')}`, 'delete', t.selected === 0 ? 'disabled' : '')}</div><div class="toolbar-end">${button(`Clear all ${key('X', 'KeyX')}`, 'ask-clear')}${button(`Check theory ${key('Enter')}`, 'check', '', 'primary')}</div></footer></section>`;
}
function textureScreen() {
  const t = state.texture;
  return `<section class="texture-screen"><div class="texture-preview" style="background-image:url('assets/${t.style === 'gas_giant' ? 'saturn' : 'moon'}.png')"></div><aside class="texture-controls panel"><div class="eyebrow">Surface study</div><h1>Planet textures</h1><label>Surface style<select id="texture-style">${['plain', 'gas_giant', 'cratered', 'rocky', 'icy', 'volcanic', 'terran'].map(style => `<option value="${style}" ${style === t.style ? 'selected' : ''}>${style.replace('_', ' ')}</option>`).join('')}</select></label>${[['scale', 'Feature scale', .25, 4, .05], ['roughness', 'Roughness', 0, 1, .01], ['contrast', 'Contrast', .25, 2, .05]].map(([name, label, min, max, step]) => `<label>${label}<output>${t[name].toFixed(2)}</output><input data-texture="${name}" type="range" min="${min}" max="${max}" step="${step}" value="${t[name]}"></label>`).join('')}${button(`${t.rotating ? icon('pause') : icon('play')} ${t.rotating ? 'Pause' : 'Rotate'}`, 'texture-rotate')}${button('Back', 'title')}</aside></section>`;
}
function render() {
  document.title = `Stargaze${state.screen === 'title' ? '' : ` · ${state.screen === 'observe' ? state.observation.puzzle ? 'Field study' : 'Observatory' : state.screen[0].toUpperCase() + state.screen.slice(1)}`}`;
  root.innerHTML = ({ title: titleScreen, levels: () => levelsScreen(false), explore: () => levelsScreen(true), settings: settingsScreen, controls: controlsScreen, observe: observationScreen, theory: theoryScreen, texture: textureScreen })[state.screen]();
  renderModal(); syncPressed();
}
let previousFocus;
function showModal(type, extra = {}) { previousFocus = document.activeElement; state.modal = { type, ...extra }; renderModal(); }
function renderModal() {
  const holder = $('#modal-root'), modal = state.modal;
  if (!modal) { holder.innerHTML = ''; return; }
  let eyebrow = 'Your theory', title, content, actions;
  const t = currentTheory();
  if (modal.type === 'clear') {
    title = 'Clear the entire theory?'; content = `<p>Removes all orbiting objects and the viewer. The centre stays.</p><p>Undo can restore everything.</p>`;
    actions = button(`Cancel ${key('Esc', 'Escape')}`, 'cancel-modal') + button(`Clear all ${key('Enter')}`, 'confirm-clear', '', 'primary danger');
  } else if (modal.type === 'discard') {
    eyebrow = 'Discard progress'; title = `Discard ${systemById(modal.id).title}?`;
    const p = progress(state.theories[modal.id]);
    content = `<p>Your theory, ${p.attempts} check${p.attempts === 1 ? '' : 's'}, and best score will be removed. This level starts fresh.</p><p>This cannot be undone.</p>`;
    actions = button(`Cancel ${key('Esc', 'Escape')}`, 'cancel-modal') + button(`Discard ${key('Enter')}`, 'confirm-discard', '', 'primary danger');
  } else if (modal.type === 'leave') {
    eyebrow = 'Leave field study'; title = 'Keep your progress?'; content = '<p>Your theory, checks, and best score can be kept for when you return.</p>';
    actions = button(`Cancel ${key('Esc', 'Escape')}`, 'cancel-modal') + button('Discard & leave', 'leave-discard', '', 'danger') + button('Keep & leave', 'leave-keep', '', 'primary');
  } else if (modal.type === 'result') {
    const score = t.attempts.at(-1);
    eyebrow = `Check ${t.attempts.length} / ${activeTitle()}`; title = 'Check results';
    content = `<div class="result-layout"><div class="score-dial" style="--score:${score.total}%"><div><strong>${score.total.toFixed(1)}</strong><span>/ 100</span></div></div><div class="score-breakdown">${[['Ordered structure', score.topology, '60%'], ['Star / ring traits', score.traits, '20%'], ['Viewer location', score.viewer ? 100 : 0, '20%']].map(([name, value, weight]) => `<div><span>${name}<small>${weight} of score</small></span><b>${value.toFixed(1)}%</b><i><em style="width:${value}%"></em></i></div>`).join('')}</div></div><p class="muted">Best so far: ${t.best.toFixed(1)} · ${t.attempts.length} check${t.attempts.length === 1 ? '' : 's'}</p><details class="attempt-history"><summary>Attempt history</summary>${t.attempts.map((a, i) => `<p><span>Check ${i + 1}</span><span>${a.total.toFixed(1)} / 100</span></p>`).reverse().join('')}</details>`;
    actions = button('Maps', 'result-maps') + button('Observe', 'result-observe') + button(`Back to theory ${key('Esc', 'Escape')}`, 'cancel-modal', '', 'primary');
  } else if (modal.type === 'quit') {
    eyebrow = 'Stargaze'; title = 'Leave Stargaze?'; content = '<p>Your progress is saved in this browser. You can close this tab and return later.</p>';
    actions = button('Stay here', 'cancel-modal') + button('Return to title', 'quit-title', '', 'primary');
  }
  holder.innerHTML = `<div class="modal-backdrop"><section class="modal ${modal.type === 'result' ? 'result-modal' : ''}" role="dialog" aria-modal="true" aria-labelledby="modal-title"><div class="eyebrow">${esc(eyebrow)}</div><h2 id="modal-title">${esc(title)}</h2>${content}<footer>${actions}</footer></section></div>`;
  queueMicrotask(() => holder.querySelector('button')?.focus());
}
function closeModal() { state.modal = null; renderModal(); if (previousFocus?.isConnected) previousFocus.focus(); }
function start(id, puzzle) {
  if (puzzle) { state.active = id; state.theories[id] ||= freshTheory(); }
  else state.active = null;
  state.observation = freshObservation(id, puzzle); persist(); navigate('observe');
}
function requestLeave(target) { if (state.observation.puzzle && state.active && progress(currentTheory()).started) showModal('leave', { target }); else navigate(target); }

// Semantic UI commands form the boundary for a later native Rust adapter.
export function dispatch(action, payload = {}) {
  const o = state.observation;
  const t = state.screen === 'theory' || state.modal ? currentTheory() : null;
  switch (action) {
    case 'title': requestLeave('title'); return;
    case 'levels': navigate('levels'); return;
    case 'explore': navigate('explore'); return;
    case 'settings': navigate('settings'); return;
    case 'controls': navigate('controls'); return;
    case 'quit': persist(); showModal('quit'); return;
    case 'quit-title': closeModal(); navigate('title'); return;
    case 'maps': requestLeave(o.puzzle ? 'levels' : 'explore'); return;
    case 'resume': case 'observe': navigate('observe'); return;
    case 'theory': if (o.puzzle) navigate('theory'); return;
    case 'back': navigate(state.back || 'title', { back: false }); return;
    case 'play-level': start(payload.id, true); return;
    case 'open-system': if (exploreEnabled) start(payload.id, false); return;
    case 'favorite': state.favorites = state.favorites.includes(payload.id) ? state.favorites.filter(id => id !== payload.id) : [...state.favorites, payload.id]; persist(); break;
    case 'favorites-filter': state.favoritesOnly = !state.favoritesOnly; state.selected = 0; break;
    case 'ask-discard': showModal('discard', { id: payload.id }); return;
    case 'confirm-discard': delete state.theories[state.modal.id]; closeModal(); persist(); break;
    case 'cancel-modal': closeModal(); return;
    case 'ask-clear': showModal('clear'); return;
    case 'confirm-clear': clearTheory(t); closeModal(); persist(); break;
    case 'leave-keep': case 'leave-discard': {
      const target = state.modal.target; if (action === 'leave-discard') delete state.theories[state.active];
      closeModal(); persist(); navigate(target); return;
    }
    case 'result-observe': closeModal(); navigate('observe'); return;
    case 'result-maps': closeModal(); persist(); navigate('levels'); return;
    case 'preset': Object.assign(state.draft, PRESETS[payload.value]); break;
    case 'apply-settings': state.settings = clone(state.draft); persist(); navigate(state.back || 'title', { back: false }); toast('Settings applied'); return;
    case 'hud': o.hud = !o.hud; break;
    case 'labels': if (!o.puzzle) o.labels = !o.labels; break;
    case 'targets': if (!o.puzzle) { o.targets = !o.targets; o.air = false; } break;
    case 'air': if (!o.puzzle) { o.air = !o.air; o.targets = false; } break;
    case 'ground': o.ground = ['Forest', 'Basalt', 'Dunes', 'Ice field'][(['Forest', 'Basalt', 'Dunes', 'Ice field'].indexOf(o.ground) + 1) % 4]; break;
    case 'slower': changeSpeed(o, -1); break;
    case 'faster': changeSpeed(o, 1); break;
    case 'playback': togglePlayback(o); break;
    case 'day-back': o.minutes -= 1440; o.spp = 0; break;
    case 'day-forward': o.minutes += 1440; o.spp = 0; break;
    case 'ev-down': o.ev = clamp(o.ev - .25, -8, 8); o.auto = false; break;
    case 'ev-up': o.ev = clamp(o.ev + .25, -8, 8); o.auto = false; break;
    case 'auto': o.auto = !o.auto; break;
    case 'aim': o.lock = null; o.aim = payload.value; toast(`Aimed at ${payload.value} · lock released`); break;
    case 'viewer': remember(t); t.viewer = t.selected; t.message = 'Viewer placed on the selected object.'; persist(); break;
    case 'trait-star': toggleTrait(t, 'star'); persist(); break;
    case 'trait-rings': toggleTrait(t, 'rings'); persist(); break;
    case 'undo': history(t); persist(); break;
    case 'redo': history(t, true); persist(); break;
    case 'delete': deleteBranch(t); persist(); break;
    case 'check': if (previewCheck(t)) { persist(); showModal('result'); } else { updateTheory(); toast(t.message); } return;
    case 'texture-rotate': state.texture.rotating = !state.texture.rotating; break;
    default: return;
  }
  render();
}

document.addEventListener('click', event => {
  const target = event.target.closest('[data-action]');
  if (target && !target.disabled) { dispatch(target.dataset.action, { ...target.dataset }); return; }
  if (state.screen === 'observe' && event.target.closest('#observation-view') && !event.target.closest('.panel')) { state.observation.lock = null; render(); }
});
document.addEventListener('pointerover', event => {
  const row = event.target.closest('[data-row]');
  if (row) { state.selected = Number(row.dataset.row); document.querySelectorAll('[data-row]').forEach(el => el.classList.toggle('selected', el === row)); }
  const title = event.target.closest('[data-title-index]');
  if (title) { state.titleSelected = Number(title.dataset.titleIndex); document.querySelectorAll('[data-title-index]').forEach(el => el.classList.toggle('selected', el === title)); }
});
document.addEventListener('input', event => {
  const input = event.target;
  if (input.id === 'system-search') { const start = input.selectionStart; state.search = input.value; state.selected = 0; root.innerHTML = levelsScreen(true); $('#system-search').focus(); try { $('#system-search').setSelectionRange(start, start); } catch { /* Search input selection is browser-specific. */ } }
  if (input.dataset.setting) {
    const row = SETTING_ROWS.find(row => row.key === input.dataset.setting);
    state.draft[row.key] = row.values ? row.values[Number(input.value)] : Number(input.value);
    input.nextElementSibling.textContent = row.format(state.draft[row.key]);
    $('#settings-dirty').textContent = JSON.stringify(state.settings) !== JSON.stringify(state.draft) ? 'Unsaved changes' : '';
    document.querySelectorAll('[data-action="preset"]').forEach(el => { const active = el.dataset.value === presetName(state.draft); el.classList.toggle('active', active); el.setAttribute('aria-pressed', active); });
    $('.custom').classList.toggle('active', presetName(state.draft) === 'Custom');
  }
  if (input.id === 'exposure-slider') {
    state.observation.ev = Number(input.value); state.observation.auto = false;
    const output = $('#ev-readout'); output.textContent = `${state.observation.ev >= 0 ? '+' : ''}${state.observation.ev.toFixed(2)} EV`; output.classList.remove('muted');
    $('.auto-label').classList.remove('on'); const sw = $('.exposure-footer [role="switch"]'); sw.classList.remove('on'); sw.setAttribute('aria-checked', 'false');
  }
  if (input.dataset.observation) { state.observation[input.dataset.observation] = Number(input.value); input.previousElementSibling.textContent = input.dataset.observation === 'density' ? `${Number(input.value).toFixed(2)}×` : `${input.value}°`; }
  if (input.dataset.texture) { state.texture[input.dataset.texture] = Number(input.value); input.previousElementSibling.textContent = Number(input.value).toFixed(2); }
});
document.addEventListener('change', event => { if (event.target.id === 'texture-style') { state.texture.style = event.target.value; render(); } });

let drag = null;
function diagramPoint(event) { const svg = $('#diagram'); return new DOMPoint(event.clientX, event.clientY).matrixTransform(svg.getScreenCTM().inverse()); }
function updateTheory() {
  if (!$('#diagram')) return;
  $('#diagram').innerHTML = theorySvg(); $('#inspector').innerHTML = inspector(); $('#theory-message').textContent = currentTheory().message;
  $('.theory-count').textContent = `${currentTheory().nodes.length} / 64 objects`;
}
document.addEventListener('contextmenu', event => {
  if (event.target.closest('#diagram-wrap')) {
    event.preventDefault(); const p = diagramPoint(event), t = currentTheory();
    if (!event.target.closest('[data-node]')) { addNode(t, p.x, p.y); persist(); render(); }
  } else if (state.screen === 'observe' && !event.target.closest('.panel')) {
    event.preventDefault(); const o = state.observation;
    const inCenter = Math.abs(event.clientX - innerWidth / 2) < innerWidth * .22 && Math.abs(event.clientY - innerHeight / 2) < innerHeight * .25;
    const name = o.aim || currentSystem().target;
    o.lock = inCenter ? { kind: /sun|aur|igni|lucero|gemma|gemmi/i.test(name) ? 'star' : 'planet', name } : { kind: 'stars', name: 'Background sky' };
    render();
  }
});
document.addEventListener('pointerdown', event => {
  const node = event.target.closest('[data-node]');
  if (!node || event.button !== 0) return;
  const t = currentTheory(); t.selected = Number(node.dataset.node);
  const p = diagramPoint(event), n = t.nodes.find(n => n.id === t.selected);
  drag = { id: n.id, dx: n.x - p.x, dy: n.y - p.y, started: false, startX: p.x, startY: p.y };
  $('#diagram-wrap').setPointerCapture(event.pointerId); updateTheory();
});
document.addEventListener('pointermove', event => {
  if (!drag) return;
  const p = diagramPoint(event), t = currentTheory();
  if (!drag.started && Math.hypot(p.x - drag.startX, p.y - drag.startY) < 2) return;
  if (!drag.started) { remember(t); drag.started = true; }
  const n = t.nodes.find(n => n.id === drag.id);
  n.x = clamp(p.x + drag.dx, 20, 980); n.y = clamp(p.y + drag.dy, 30, 630); updateTheory();
});
document.addEventListener('pointerup', () => { if (drag) { drag = null; persist(); render(); } });
document.addEventListener('wheel', event => {
  if (state.screen !== 'observe' || event.target.closest('.panel')) return;
  event.preventDefault(); const o = state.observation;
  o.fov = clamp(o.fov * Math.pow(.78, -event.deltaY / 100), .001, 90); o.spp = 0;
  const readout = $('#fov-readout'); if (readout) readout.textContent = `FOV ${o.fov.toFixed(3)}°`;
}, { passive: false });

const held = new Set();
function syncPressed() { document.querySelectorAll('[data-key]').forEach(el => el.classList.toggle('pressed', held.has(el.dataset.key))); }
document.addEventListener('keyup', event => { held.delete(event.code); syncPressed(); });
addEventListener('blur', () => { held.clear(); syncPressed(); drag = null; });
const observingKeys = { KeyM: 'maps', Escape: 'maps', KeyP: 'settings', KeyH: 'hud', KeyL: 'labels', KeyG: 'targets', KeyB: 'ground', KeyJ: 'air', ArrowLeft: 'slower', ArrowRight: 'faster', Space: 'playback', PageDown: 'day-back', PageUp: 'day-forward', Comma: 'ev-down', Period: 'ev-up', KeyA: 'auto', Tab: 'theory' };
const theoryKeys = { Tab: 'observe', KeyM: 'maps', Escape: 'observe', KeyV: 'viewer', KeyS: 'trait-star', KeyR: 'trait-rings', KeyZ: 'undo', KeyY: 'redo', Delete: 'delete', Backspace: 'delete', KeyX: 'ask-clear', Enter: 'check' };
document.addEventListener('keydown', event => {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  held.add(event.code); syncPressed();
  if (state.modal) {
    if (event.key === 'Escape') { event.preventDefault(); closeModal(); return; }
    if (event.key === 'Tab') {
      const buttons = [...document.querySelectorAll('#modal-root button')], index = buttons.indexOf(document.activeElement);
      event.preventDefault(); buttons[(index + (event.shiftKey ? buttons.length - 1 : 1)) % buttons.length]?.focus(); return;
    }
    if (event.key === 'Enter' && document.activeElement?.tagName !== 'BUTTON') { event.preventDefault(); const commands = { clear: 'confirm-clear', discard: 'confirm-discard', leave: 'leave-keep', quit: 'quit-title' }; if (commands[state.modal.type]) dispatch(commands[state.modal.type]); }
    return;
  }
  if (event.target.matches('input, select, textarea')) { if (event.key === 'Escape') event.target.blur(); return; }
  if (state.screen === 'title') {
    if (['ArrowUp', 'ArrowDown'].includes(event.code)) { event.preventDefault(); state.titleSelected = (state.titleSelected + (event.code === 'ArrowDown' ? 1 : menuItems().length - 1)) % menuItems().length; render(); }
    else if (['Enter', 'ArrowRight'].includes(event.code)) { event.preventDefault(); dispatch(menuItems()[state.titleSelected][0]); }
    else if (event.code === 'KeyC') dispatch('controls');
    return;
  }
  if (state.screen === 'observe' || state.screen === 'theory') {
    const action = (state.screen === 'theory' ? theoryKeys : observingKeys)[event.code];
    if (action) { event.preventDefault(); if (!event.repeat || ['Comma', 'Period'].includes(event.code)) dispatch(action); }
    if (state.screen === 'observe' && state.observation.targets && !state.observation.puzzle && /^Digit/.test(event.code)) {
      const index = (Number(event.code.slice(5)) + 9) % 10; const name = currentSystem().targets[index]; if (name) dispatch('aim', { value: name });
    }
    return;
  }
  if (state.screen === 'settings') {
    if (event.code === 'Escape') { event.preventDefault(); dispatch('back'); }
    if (event.code === 'Enter') { event.preventDefault(); dispatch('apply-settings'); }
    if (['ArrowUp', 'ArrowDown'].includes(event.code)) { event.preventDefault(); state.settingSelected = clamp(state.settingSelected + (event.code === 'ArrowDown' ? 1 : -1), 0, SETTING_ROWS.length - 1); render(); document.querySelector(`[data-setting-row="${state.settingSelected}"]`)?.scrollIntoView({ block: 'nearest' }); }
    if (['ArrowLeft', 'ArrowRight'].includes(event.code)) {
      event.preventDefault(); const row = SETTING_ROWS[state.settingSelected], delta = event.code === 'ArrowRight' ? 1 : -1;
      state.draft[row.key] = row.values ? row.values[clamp(row.values.indexOf(state.draft[row.key]) + delta, 0, row.values.length - 1)] : clamp(state.draft[row.key] + delta * row.step, row.min, row.max); render();
    }
    return;
  }
  if (['Escape', 'ArrowLeft', 'KeyT'].includes(event.code)) { event.preventDefault(); navigate(state.screen === 'controls' ? state.back : 'title', { back: false }); return; }
  if (event.code === 'KeyC') { dispatch('controls'); return; }
  if (event.code === 'KeyG') { dispatch('settings'); return; }
  if (event.code === 'KeyM' && state.active) { dispatch('resume'); return; }
  if (['levels', 'explore'].includes(state.screen)) {
    const entries = state.screen === 'levels' ? LEVELS : visibleSystems();
    if (['ArrowUp', 'ArrowDown'].includes(event.code)) { event.preventDefault(); state.selected = (state.selected + (event.code === 'ArrowDown' ? 1 : entries.length - 1)) % Math.max(1, entries.length); render(); document.querySelector(`[data-row="${state.selected}"]`)?.scrollIntoView({ block: 'nearest' }); }
    if (['Enter', 'ArrowRight'].includes(event.code) && entries[state.selected]) { event.preventDefault(); start(entries[state.selected].id, state.screen === 'levels'); }
    if (event.code === 'KeyD' && state.screen === 'levels' && progress(state.theories[entries[state.selected]?.id]).started) dispatch('ask-discard', { id: entries[state.selected].id });
    if (event.code === 'KeyP' && exploreEnabled) navigate(state.screen === 'levels' ? 'explore' : 'levels');
    if (event.code === 'Slash' && state.screen === 'explore') { event.preventDefault(); $('#system-search').focus(); }
    if (event.code === 'KeyF' && state.screen === 'explore' && entries[state.selected]) dispatch('favorite', { id: entries[state.selected].id });
    if (event.code === 'KeyB' && state.screen === 'explore') dispatch('favorites-filter');
  }
});

setInterval(() => {
  if (state.screen !== 'observe' || state.modal || document.hidden) return;
  const o = state.observation;
  o.minutes += SPEEDS[o.speed] * .25;
  o.spp = o.speed === STOP ? Math.min(state.settings.limit || 16777216, o.spp + 1) : 1;
  if ($('#clock')) $('#clock').textContent = clock(o.minutes);
  if ($('#convergence')) $('#convergence').textContent = `${o.spp.toLocaleString()} spp · ${o.speed === STOP && o.spp >= (state.settings.limit || 16777216) ? 'ready' : 'converging'}`;
}, 250);
addEventListener('hashchange', () => navigate(location.hash.slice(1), { back: false, replace: true }));
addEventListener('beforeunload', persist);
addEventListener('resize', () => { if (state.screen === 'observe') render(); });
const requestedScreen = location.hash.slice(1) || 'title';
if (requestedScreen === 'theory' || (requestedScreen === 'observe' && state.observation.puzzle)) { state.active ||= LEVELS[0].id; state.theories[state.active] ||= freshTheory(); }
navigate(requestedScreen, { back: false, replace: true });
// Keep the design's navigation intact while providing a small inspection API.
window.stargaze = { state, dispatch, navigate, exploreEnabled };
const events = new EventSource('/__events');
events.onmessage = event => { if (event.data === 'reload') { persist(); location.reload(); } };
