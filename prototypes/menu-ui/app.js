const themes = {
  amber: { name: 'Amber Terminal', number: '01', kind: 'PIXEL SERIES', note: 'A warmer observatory. VT323 lettering, amber highlights, hard edges, and quiet instrument markings. Keeps the numbered rows and keyboard-first rhythm of the current menu.' },
  glacier: { name: 'Glacier Arcade', number: '02', kind: 'PIXEL SERIES', note: 'An icy flight console. Silkscreen lettering, midnight blue, mint status lights, and inset panels. A more geometric, spacious take on the existing pixel menu.' },
  blueprint: { name: 'Celestial Blueprint', number: '03', kind: 'NEW TERRITORY', note: 'A celestial drafting table. Cobalt paper, chalk lines, signal yellow, and IBM Plex Mono. A circular sky chart takes center stage, with a horizontal system index underneath. The chart is decorative, not a computed orbital map.', title: 'Find your coordinates.', subtitle: 'A chart, a destination, and a sky still to be discovered.', eyebrow: 'CELESTIAL ATLAS / PLATE 003' },
  vermilion: { name: 'Vermilion Dispatch', number: '04', kind: 'NEW TERRITORY', note: 'A bold space expedition poster. Vermilion, near-black ink, warm cream, and oversized DM Sans lettering. An image-led dispatch fills the left side; a dark destination index occupies the right. Hard rules replace conventional cards.', title: 'PICK YOUR SKY.', subtitle: 'Five destinations. Zero ordinary nights.', eyebrow: 'STARGAZE EXPLORATION BUREAU / DISPATCH 004' },
  phosphor: { name: 'Phosphor Console', number: '05', kind: 'NEW TERRITORY', note: 'A remote observatory terminal. Black, acid lime, mint, VT323 display lettering, and IBM Plex Mono readouts. Select a system from the file tree, or type commands into the working prompt. Try help, open 2, puzzle, explore, and menu.', title: 'REMOTE OBSERVATORY', subtitle: 'Connection established. The night shift is yours.', eyebrow: 'STARGAZE://OBSERVATORY / SESSION 005' },
};
const theme = document.body.dataset.theme;
const concept = themes[theme];
const icons = {
  star: '<path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2-5.5-2.9-5.5 2.9 1-6.2L3 9.6l6.2-.9Z"/>',
  orbit: '<circle cx="12" cy="12" r="4"/><ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(-35 12 12)"/>',
  arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
  back: '<path d="M20 12H4m6-6-6 6 6 6"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',
  book: '<path d="M4 4h7l1 2 1-2h7v15h-7l-1 2-1-2H4Zm8 2v15"/>',
  settings: '<path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/>',
  pause: '<path d="M8 5v14M16 5v14"/>',
  play: '<path d="m8 5 11 7-11 7Z"/>',
  chevron: '<path d="m9 5 7 7-7 7"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>',
  close: '<path d="m6 6 12 12M6 18 18 6"/>',
};
const icon = name => `<svg viewBox="0 0 24 24" aria-hidden="true">${icons[name]}</svg>`;
const systems = [
  { id: 'solar-system', name: 'Solar System', short: 'Our celestial neighborhood', puzzle: 'Distant lights', tag: 'Classic', camera: 'Earth', stars: '1 star', fov: '1.1°', desc: 'Start close to home. Watch the Moon from Earth, then explore the familiar dance of eight planets and their moons.', image: 'saturn', caption: 'Saturn · bundled reference render', stamp: 'SOL / 001', category: 'classic' },
  { id: 'halo', name: 'Calyx & Halo', short: 'Two suns. One amber horizon.', puzzle: 'Amber horizon', tag: 'Binary', camera: 'Halo', stars: '2 stars', fov: '80°', desc: 'Stand on a tidally locked moon beneath the rings of Calyx. Two suns move through a sky that rewards a little patience.', image: 'saturn', caption: 'Ring study · Saturn reference render', stamp: 'HAL / 002', category: 'binary' },
  { id: 'median-resonance', name: 'Median', short: 'Three moons in a clockwork sky', puzzle: 'Clockwork sky', tag: 'Resonance', camera: 'Cadence', stars: '2 stars', fov: '50°', desc: 'Observe a 1:2:4 chain of resonant moons through the dense atmosphere of Cadence. Find the repeating rhythm in their paths.', image: 'eclipse', caption: 'Eclipse study · Vantus reference render', stamp: 'MED / 003', category: 'binary' },
  { id: 'vesper', name: 'Vesper', short: 'Find the bright wanderers', puzzle: 'Bright wanderers', tag: 'Discovery', camera: 'Vesper', stars: 'Night sky', fov: '60°', desc: 'Begin with an open night sky. Find Cinder, Aureole, and the two moons above the horizon by watching their reflected light.', image: 'moon', caption: 'Moon study · Luna reference render', stamp: 'VES / 004', category: 'discovery' },
  { id: 'puzzle', name: 'First field study', short: 'Observe. Sketch. Reconstruct.', puzzle: 'First field study', tag: 'Field study', camera: 'Lookout', stars: '1 star', fov: '35°', desc: 'A small observation field for building a theory. Watch the changing sky and work out how the unseen orbits fit together.', image: 'eclipse', caption: 'Eclipse study · Vantus reference render', stamp: 'FLD / 005', category: 'discovery' },
];
let favorites;
try { favorites = new Set(JSON.parse(localStorage.getItem('stargaze-menu-favorites') || '["halo"]')); } catch { favorites = new Set(['halo']); }
let selected = systems[0];
let puzzle = false;
let filter = 'all';
let observing = false;
let paused = true;
let labels = false;
let day = 0;
let speed = 60;
let automatic = true;
let ev = 0;
let steady = true;
let toastTimer;

document.getElementById('app').innerHTML = `
  <header class="concept-bar">
    <a class="all-concepts" href="index.html">${icon('back')}<span>All concepts</span></a>
    <nav class="concept-switcher" aria-label="Design concepts">${Object.entries(themes).map(([key, value]) => `<a href="${key}.html" ${key === theme ? 'aria-current="page"' : ''}><span>${value.number}</span> ${value.name}</a>`).join('')}</nav>
    <button class="concept-info" id="concept-info" aria-label="About this concept">i</button>
  </header>
  <div class="workspace">
    <main class="main-shell" id="main">
      <header class="app-header">
        <a href="index.html" class="brand">${icon('orbit')}<span>stargaze<span class="brand-dot">.</span></span></a>
        <div class="header-right"><span class="session-status"><i></i> Observation paused</span><button id="resume" class="button secondary">Resume <kbd>Esc</kbd></button></div>
      </header>
      <section class="intro">
        <div><p class="eyebrow">${concept.eyebrow || (theme === 'amber' ? 'THE NIGHT IS YOURS' : 'OBSERVATORY / SYSTEM LIBRARY')}</p><h1 id="page-title">${concept.title || 'A sky worth exploring.'}</h1><p class="intro-copy" id="intro-copy">${concept.subtitle || 'Choose a world. Settle in. See what moves.'}</p></div>
        <div class="mode-toggle" role="group" aria-label="Observation mode"><button class="active" data-mode="explore" aria-pressed="true">${icon('orbit')} Explore</button><button data-mode="puzzle" aria-pressed="false">${icon('book')} Puzzle</button></div>
      </section>
      <div class="library-layout">
        <section class="library" aria-label="System library">
          <div class="section-heading"><span id="library-title">Solar systems</span><span class="counter" id="counter">05 AVAILABLE</span></div>
          <label class="search">${icon('search')}<input id="search" type="search" placeholder="Find a solar system…" aria-label="Search systems"><kbd>/</kbd></label>
          <div class="filters" aria-label="Library filters"><button class="active" data-filter="all" aria-pressed="true">All systems</button><button data-filter="favorites" aria-pressed="false">${icon('star')} Favorites</button><button data-filter="binary" aria-pressed="false">Binary</button></div>
          <div id="system-list" class="system-list"></div>
          <div class="library-footer"><span><i></i> ${theme === 'glacier' ? 'ALL SYSTEMS ONLINE' : '5 worlds. Endless perspectives.'}</span><span class="keyboard-hint"><kbd>1–5</kbd> select</span></div>
        </section>
        <section class="feature" aria-label="Selected system">
          <div class="preview-top"><span class="preview-label">SYSTEM PREVIEW</span><span class="preview-coordinate" id="stamp">SOL / 001</span><button id="feature-favorite" class="icon-button" aria-label="Favorite selected system" aria-pressed="false">${icon('star')}</button></div>
          <div class="planet-stage" id="planet-stage"><div class="star-field" aria-hidden="true"></div><div class="orbit-guide one" aria-hidden="true"></div><div class="orbit-guide two" aria-hidden="true"></div><img id="planet-image" src="assets/saturn.png" alt="Reference render of Saturn and its rings"><div class="mystery" aria-hidden="true">?</div><span class="stage-marker marker-left" aria-hidden="true">+</span><span class="stage-marker marker-right" aria-hidden="true">+</span><p class="image-caption" id="image-caption">Saturn · bundled reference render</p></div>
          <div class="feature-copy"><div class="feature-kicker"><span id="feature-tag">Classic</span><span id="feature-file">solar-system.yaml</span></div><h2 id="feature-title">Solar System</h2><p id="feature-description"></p><dl class="metrics" id="metrics"><div><dt>Observatory</dt><dd id="camera">Earth</dd></div><div><dt>Sky</dt><dd id="stars">1 star</dd></div><div><dt>Field of view</dt><dd id="fov">1.1°</dd></div></dl><button id="launch" class="button primary"><span>Enter observatory</span>${icon('arrow')}</button><p class="launch-hint"><kbd>Enter</kbd> to explore <span>·</span> <span id="launch-note">A new perspective awaits</span></p></div>
        </section>
      </div>
      <section class="observation-view" aria-label="Observation preview" hidden>
        <div class="observation-heading"><p class="eyebrow">BROWSER OBSERVATION DEMO</p><h1 id="observed-title"></h1><p>Illustrative controls · static reference image</p></div>
        <img id="observed-image" src="assets/saturn.png" alt="Bundled observation reference render"><span class="observed-label" id="observed-label" hidden>Saturn</span>
        <button class="button secondary observation-return" id="return-menu">${icon('back')} Choose another sky <kbd>M</kbd></button>
      </section>
      <footer class="workspace-footer"><span class="footer-note">${icon('orbit')} A little patience. A whole universe.</span><span class="footer-shortcuts"><kbd>M</kbd> menu <span>·</span> <kbd>Esc</kbd> resume</span></footer>
      <section class="hud" aria-label="Observation controls">
        <button id="menu-button" class="hud-menu">${icon('orbit')}<span>Menu</span></button>
        <div class="hud-group"><button id="labels" class="hud-toggle" aria-pressed="false">Labels <span class="switch"></span></button></div>
        <div class="hud-group playback"><button id="slower" aria-label="Decrease playback speed">−</button><span id="speed">60 min/s</span><button id="faster" aria-label="Increase playback speed">+</button><button id="play" class="play-button" aria-label="Play simulation">${icon('play')}</button></div>
        <div class="hud-group day-controls"><button id="previous-day" aria-label="Previous day">−1 day</button><span id="day">Day 000</span><button id="next-day" aria-label="Next day">+1 day</button></div>
        <div class="hud-group exposure">${icon('sun')}<label for="exposure">Exposure</label><input id="exposure" type="range" min="-8" max="8" step="0.25" value="0"><output id="ev">+0 EV</output><button id="auto" class="auto active" aria-pressed="true">Auto</button></div>
        <button id="steady" class="hud-toggle stars-toggle" aria-pressed="true">Stars <span class="switch"></span></button>
      </section>
    </main>
  </div>
  <footer class="prototype-footer"><span><i></i> INTERACTIVE HTML CONCEPT</span><span>${concept.number} / ${concept.name} <span class="prototype-separator">·</span> ${concept.kind}</span></footer>
  <div id="toast" class="toast" role="status"></div>
  <dialog id="about"><button id="close-about" class="icon-button" aria-label="Close concept details">${icon('close')}</button><p class="eyebrow">CONCEPT ${concept.number} / ${concept.kind}</p><h2>${concept.name}</h2><p>${concept.note}</p><p class="dialog-detail">Try selecting systems, switching modes, searching, and entering the observation. The controls are an interactive browser prototype.</p><a href="index.html" class="button primary">Compare all five ${icon('arrow')}</a></dialog>
`;

const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
if (theme === 'blueprint' || theme === 'vermilion') {
  $('.library-layout').prepend($('.feature'));
}
if (theme === 'blueprint') {
  $('#planet-stage').insertAdjacentHTML('beforeend', '<div class="chart-reticle" aria-hidden="true"><span>N / 00°</span><span>E / 90°</span><span>S / 180°</span><span>W / 270°</span></div><span class="chart-note">ILLUSTRATIVE SKY CHART</span>');
}
if (theme === 'vermilion') {
  $('.feature').insertAdjacentHTML('afterbegin', '<div class="dispatch-masthead"><span>THE OBSERVATION DISPATCH</span><span>VOL. 01 / NO. 004</span></div>');
  $('.library').insertAdjacentHTML('afterbegin', '<p class="destination-label">YOUR NEXT STOP ↘</p>');
}
if (theme === 'phosphor') {
  $('.library').insertAdjacentHTML('afterbegin', '<div class="terminal-path">root@stargaze<br><span>└─ /observatory/systems/</span></div>');
  $('.feature').insertAdjacentHTML('afterbegin', '<div class="console-readout"><span>OPTICAL FEED [CONNECTED]</span><span>REFERENCE BUFFER / 01</span></div>');
  $('.workspace-footer').insertAdjacentHTML('beforebegin', '<section class="command-console" aria-label="Observatory command prompt"><div id="command-output" role="status">READY. Type help to list commands.</div><form id="command-form"><label for="command-input">observer@stargaze:~$</label><input id="command-input" autocomplete="off" spellcheck="false" placeholder="open 2" aria-label="Observatory command"><button type="submit" aria-label="Run command">RUN ↵</button></form></section>');
}
function toast(message) {
  $('#toast').textContent = message;
  $('#toast').classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $('#toast').classList.remove('show'), 2400);
}
function saveFavorites() {
  try { localStorage.setItem('stargaze-menu-favorites', JSON.stringify([...favorites])); } catch { /* Works with storage disabled. */ }
}
function toggleFavorite(id) {
  favorites.has(id) ? favorites.delete(id) : favorites.add(id);
  saveFavorites();
  renderList();
  updateFeature();
}
function visibleSystems() {
  const query = $('#search').value.toLowerCase().trim();
  return systems.filter(s => (!query || (puzzle ? s.puzzle : `${s.name} ${s.short}`).toLowerCase().includes(query)) && (filter === 'all' || (filter === 'favorites' ? favorites.has(s.id) : s.category === filter)));
}
function renderList() {
  const visible = visibleSystems();
  $('#counter').textContent = `${String(visible.length).padStart(2, '0')} AVAILABLE`;
  $('#system-list').innerHTML = visible.length ? visible.map(s => `<div class="system-row ${s.id === selected.id ? 'selected' : ''}"><button class="system-select" data-select="${s.id}" aria-pressed="${s.id === selected.id}"><span class="system-number">${String(systems.indexOf(s) + 1).padStart(2, '0')}</span><span class="system-text"><strong>${puzzle ? s.puzzle : s.name}</strong><span>${puzzle ? 'Start a fresh theory' : s.short}</span></span><span class="row-tag">${puzzle ? 'Map' : s.tag}</span><span class="row-arrow">${icon('chevron')}</span></button><button class="row-favorite ${favorites.has(s.id) ? 'favorited' : ''}" data-favorite="${s.id}" aria-label="Favorite ${puzzle ? s.puzzle : s.name}" aria-pressed="${favorites.has(s.id)}">${icon('star')}</button></div>`).join('') : '<div class="empty-state">No skies found.<br><span>Try another search or filter.</span></div>';
  $$('[data-select]').forEach(b => b.addEventListener('click', () => select(b.dataset.select)));
  $$('[data-favorite]').forEach(b => b.addEventListener('click', () => toggleFavorite(b.dataset.favorite)));
}
function updateFeature() {
  $('#feature-title').textContent = puzzle ? selected.puzzle : selected.name;
  $('#feature-description').textContent = puzzle ? 'An uncharted sky, a blank notebook, and your own theory. Observe the motion and reconstruct the system from what you discover.' : selected.desc;
  $('#feature-tag').textContent = puzzle ? 'Observation puzzle' : selected.tag;
  $('#feature-file').textContent = puzzle ? 'Uncharted map' : `${selected.id}.yaml`;
  $('#stamp').textContent = puzzle ? `MAP / ${String(systems.indexOf(selected) + 1).padStart(3, '0')}` : selected.stamp;
  $('#camera').textContent = selected.camera;
  $('#stars').textContent = selected.stars;
  $('#fov').textContent = selected.fov;
  $('#metrics').hidden = puzzle;
  const src = puzzle ? 'assets/moon.png' : `assets/${selected.image}.png`;
  $('#planet-image').src = src;
  $('#planet-image').alt = puzzle ? '' : selected.caption;
  $('#image-caption').textContent = puzzle ? 'A new sky. No spoilers.' : selected.caption;
  $('#launch span').textContent = puzzle ? 'Begin field study' : 'Enter observatory';
  $('#launch-note').textContent = puzzle ? 'Starts a fresh theory' : 'A new perspective awaits';
  $('#feature-favorite').setAttribute('aria-pressed', String(favorites.has(selected.id)));
  $('#feature-favorite').classList.toggle('favorited', favorites.has(selected.id));
}
function select(id) {
  selected = systems.find(s => s.id === id);
  renderList();
  updateFeature();
}
function setMode(mode) {
  puzzle = mode === 'puzzle';
  document.body.classList.toggle('puzzle-mode', puzzle);
  $$('[data-mode]').forEach(b => { b.classList.toggle('active', b.dataset.mode === mode); b.setAttribute('aria-pressed', String(b.dataset.mode === mode)); });
  $('#page-title').textContent = puzzle ? (theme === 'vermilion' ? 'UNKNOWN SKIES.' : 'A mystery in every sky.') : (concept.title || 'A sky worth exploring.');
  $('#intro-copy').textContent = puzzle ? 'Watch the motion. Build your own theory.' : (concept.subtitle || 'Choose a world. Settle in. See what moves.');
  $('#library-title').textContent = puzzle ? 'Uncharted maps' : 'Solar systems';
  $('#search').placeholder = puzzle ? 'Find a map…' : 'Find a solar system…';
  $('#search').setAttribute('aria-label', puzzle ? 'Search maps' : 'Search systems');
  $('#search').value = '';
  filter = 'all';
  $$('[data-filter]').forEach(b => { b.classList.toggle('active', b.dataset.filter === filter); b.setAttribute('aria-pressed', String(b.dataset.filter === filter)); b.hidden = puzzle && b.dataset.filter === 'binary'; });
  renderList();
  updateFeature();
}
function observation(value) {
  observing = value;
  document.body.classList.toggle('is-observing', value);
  $('.observation-view').hidden = !value;
  $('#observed-title').textContent = puzzle ? selected.puzzle : selected.name;
  $('#observed-image').src = puzzle ? 'assets/moon.png' : `assets/${selected.image}.png`;
  $('#observed-image').hidden = puzzle;
  $('#observed-label').textContent = puzzle ? 'Uncharted sky' : selected.caption;
  $('#observed-label').hidden = !labels || puzzle;
  $('#observed-image').style.filter = `brightness(${Math.pow(2, ev / 3)})`;
  $('.session-status').innerHTML = `<i></i> ${value ? (paused ? 'Observation paused' : 'Observing the sky') : 'Observation paused'}`;
  (value ? $('#return-menu') : $('#resume')).focus({ preventScroll: true });
}
function updateDay() { $('#day').textContent = `Day ${day < 0 ? '−' : ''}${String(Math.abs(day)).padStart(3, '0')}`; }

$('#search').addEventListener('input', renderList);
$$('[data-filter]').forEach(b => b.addEventListener('click', () => {
  filter = b.dataset.filter;
  $$('[data-filter]').forEach(other => { other.classList.toggle('active', other === b); other.setAttribute('aria-pressed', String(other === b)); });
  renderList();
}));
$$('[data-mode]').forEach(b => b.addEventListener('click', () => setMode(b.dataset.mode)));
$('#feature-favorite').addEventListener('click', () => toggleFavorite(selected.id));
$('#launch').addEventListener('click', () => { day = 0; updateDay(); observation(true); });
$('#resume').addEventListener('click', () => observation(true));
$('#return-menu').addEventListener('click', () => observation(false));
$('#menu-button').addEventListener('click', () => observation(!observing));
$('#play').addEventListener('click', () => {
  paused = !paused;
  $('#play').innerHTML = icon(paused ? 'play' : 'pause');
  $('#play').setAttribute('aria-label', paused ? 'Play simulation' : 'Pause simulation');
  $('.session-status').innerHTML = `<i></i> ${!observing || paused ? 'Observation paused' : 'Observing the sky'}`;
  toast(paused ? 'Time paused' : `Time running at ${speed} min/s · demo`);
});
setInterval(() => { if (observing && !paused) { day += speed / 1440; $('#day').textContent = `Day ${day.toFixed(2)}`; } }, 1000);
$('#previous-day').addEventListener('click', () => { day = Math.floor(day) - 1; updateDay(); });
$('#next-day').addEventListener('click', () => { day = Math.floor(day) + 1; updateDay(); });
function updateSpeed(multiplier) { speed = Math.max(1, Math.min(3840, speed * multiplier)); $('#speed').textContent = `${speed} min/s`; }
$('#slower').addEventListener('click', () => updateSpeed(0.5));
$('#faster').addEventListener('click', () => updateSpeed(2));
$('#labels').addEventListener('click', () => { labels = !labels; $('#labels').setAttribute('aria-pressed', String(labels)); $('#observed-label').hidden = !labels || puzzle; toast(`Body labels ${labels ? 'on' : 'off'}`); });
$('#exposure').addEventListener('input', e => { ev = Number(e.target.value); $('#ev').textContent = `${ev >= 0 ? '+' : ''}${ev} EV`; $('#observed-image').style.filter = `brightness(${Math.pow(2, ev / 3)})`; });
$('#auto').addEventListener('click', () => { automatic = !automatic; $('#auto').classList.toggle('active', automatic); $('#auto').setAttribute('aria-pressed', String(automatic)); toast(automatic ? 'Automatic exposure enabled' : 'Manual exposure enabled'); });
$('#steady').addEventListener('click', () => { steady = !steady; $('#steady').setAttribute('aria-pressed', String(steady)); document.body.classList.toggle('twinkle', !steady); toast(steady ? 'Steady background stars' : 'Background stars twinkle'); });
$('#concept-info').addEventListener('click', () => $('#about').showModal());
$('#close-about').addEventListener('click', () => $('#about').close());
$('#about').addEventListener('click', e => { if (e.target === $('#about')) $('#about').close(); });
if (theme === 'phosphor') {
  $('#command-form').addEventListener('submit', e => {
    e.preventDefault();
    const command = $('#command-input').value.trim().toLowerCase();
    const match = command.match(/^open ([1-5])$/);
    let result;
    if (match) { select(systems[Number(match[1]) - 1].id); day = 0; updateDay(); observation(true); result = `LOADED: ${puzzle ? selected.puzzle : selected.name}. Type menu to return.`; }
    else if (command === 'help') result = 'COMMANDS: open 1–5 / puzzle / explore / menu / resume / clear';
    else if (command === 'puzzle' || command === 'explore') { observation(false); setMode(command); result = `MODE: ${command.toUpperCase()}. Select a destination.`; }
    else if (command === 'menu') { observation(false); result = 'SYSTEM INDEX RESTORED.'; }
    else if (command === 'resume') { observation(true); result = 'OPTICAL FEED RESUMED.'; }
    else if (command === 'clear') result = 'READY.';
    else result = 'Command not found. Type help.';
    $('#command-output').textContent = result;
    $('#command-input').value = '';
    $('#command-input').focus({preventScroll:true});
  });
}
document.addEventListener('keydown', e => {
  if ($('#about').open || e.altKey || e.ctrlKey || e.metaKey || ['INPUT', 'TEXTAREA', 'SELECT'].includes(document.activeElement.tagName)) return;
  if (e.key === '/') { e.preventDefault(); $('#search').focus(); }
  if (e.key.toLowerCase() === 'm' || e.key === 'Escape') { e.preventDefault(); observation(!observing); }
  if (!observing && /^[1-5]$/.test(e.key)) select(systems[Number(e.key) - 1].id);
  if (e.key === 'Enter' && !observing && !['BUTTON', 'A'].includes(document.activeElement.tagName)) observation(true);
});
renderList();
updateFeature();
