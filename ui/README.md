# Stargaze UI

A standalone HTML/CSS/JavaScript app for iterating on Stargaze's interface. It lives in this repository's `ui/` directory and runs independently of the Rust project.

## Run locally

Requires Node.js 20 or newer. The dev server has no runtime dependencies:

```sh
cd ui # from the repository root
npm run dev
```

Open **http://localhost:5188/**. Editing `index.html`, `style.css`, or files in `src/` reloads the browser automatically. The active level, observation controls, settings, and diagrams survive reloads through browser storage. Changing a settings draft only takes effect after Apply.

Explore follows the native app's opt-in behavior:

```sh
npm run dev -- --explore
```

Or open **http://localhost:5188/?explore=1**. An alternate port works with `npm run dev -- --port 5190`.

## Screens

| URL hash | Screen |
| --- | --- |
| `#title` | Main menu: Play, Settings, Quit; Explore when enabled |
| `#levels` | Three current field studies, browser-local progress, resume and discard |
| `#explore` | Named systems, search and favorites; opt-in only |
| `#observe` | Field-study brief and the current four-panel HUD |
| `#theory` | Orbital diagram, inspector, traits, viewer, drag ordering, undo/redo |
| `#settings` | Quality presets, rendering settings and HUD scale |
| `#controls` | Keyboard and mouse reference |
| `#texture` | Texture-editor interface study |

The clear, discard, keep/leave, quit and result dialogs open through their normal controls. Most native shortcuts also work: arrow keys / Enter in menus, Tab for theory, Space for playback, H for the HUD, comma/period for exposure, and S/R/V/Z/Y/X in the editor. Add diagram nodes by right-clicking empty space; select and drag to change orbit order.

## Scope of the preview

The renderer is intentionally excluded. Observation backgrounds are static captures from the Rust project's documentation; speed, FOV, exposure, atmosphere and texture controls demonstrate interface state rather than change a live sky. Locking near the centre previews a body lock; locking elsewhere previews background stars.

The results screen uses a representative **72.4/100 fixture** to explore its layout. It does not evaluate a hidden orbital solution. Actual simulation, native rendering and scoring remain in Rust for the later implementation.

Progress is stored under `stargaze-ui-preview-v1` in this browser's local storage. It never reads or writes the native app's save files. Undo/redo history lasts for the current browser session; the diagram itself persists.

## Source map

- `index.html`: document and app mounts.
- `style.css`: the current Celestial Blueprint design, fonts, responsive layout and HUD geometry.
- `src/app.js`: screen templates, routing, keyboard/mouse input and semantic `dispatch(action, payload)` commands.
- `src/model.js`: preview data, settings, persistence, playback and diagram editing. This is the replaceable boundary for a future Rust adapter.
- `server.mjs`: localhost-only static server and live reload. No bundler or production server is required.
- `assets/`: embedded fonts, their licenses and existing native-renderer captures.

## Checks

```sh
npm ci
npm run check
npm test
npm run test:browser
```

Browser tests use `/usr/bin/chromium` locally by default; set `BROWSER_EXECUTABLE` for a different installed Chromium executable. CI installs and uses Playwright's bundled Chromium. They cover navigation, settings drafts, persisted progress, theory edits, dialogs, Explore gating and HUD bounds at multiple window sizes. Screenshots are written to ignored `test-results/` for visual review.
