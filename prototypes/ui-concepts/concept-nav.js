// Prototype-only switcher between the UI concepts: click the label at the top, press 1–5, or [ and ].
(() => {
  const PAGES = [['orrery', 'Orrery'], ['signal', 'Signal'], ['drift', 'Drift'], ['eyepiece', 'Eyepiece'], ['setting-circles', 'Setting Circles']];
  const here = location.pathname.split('/').pop().replace('.html', '');
  const i = Math.max(0, PAGES.findIndex(([id]) => id === here));
  const go = d => { location.href = PAGES[(i + d + PAGES.length) % PAGES.length][0] + '.html'; };
  const nav = document.createElement('nav');
  nav.style.cssText = 'position:fixed;top:10px;left:50%;transform:translateX(-50%);z-index:99;display:flex;gap:14px;align-items:center;font:400 10px/1 ui-monospace,Menlo,monospace;letter-spacing:.14em;text-transform:uppercase;color:currentColor;opacity:.35;transition:opacity .2s;pointer-events:auto;user-select:none';
  nav.onmouseenter = () => nav.style.opacity = '.9'; nav.onmouseleave = () => nav.style.opacity = '.35';
  const link = (text, onClick, title) => { const a = document.createElement('a'); a.textContent = text; a.title = title || ''; a.style.cssText = 'cursor:pointer;color:inherit;text-decoration:none;padding:4px'; a.onclick = e => { e.stopPropagation(); onClick(); }; return a; };
  nav.append(
    link('◂ ' + PAGES[(i + PAGES.length - 1) % PAGES.length][1], () => go(-1), 'Previous concept  [ '),
    link(`${i + 1} · ${PAGES[i][1]}`, () => { location.href = 'index.html'; }, 'All concepts'),
    link(PAGES[(i + 1) % PAGES.length][1] + ' ▸', () => go(1), 'Next concept  ]'));
  nav.addEventListener('pointerdown', e => e.stopPropagation());
  document.body.append(nav);
  addEventListener('keydown', e => {
    if (e.target.closest?.('input,textarea')) return;
    if (e.key === '[') go(-1); if (e.key === ']') go(1);
    const n = Number(e.key); if (n >= 1 && n <= PAGES.length && n - 1 !== i) location.href = PAGES[n - 1][0] + '.html';
  });
})();
