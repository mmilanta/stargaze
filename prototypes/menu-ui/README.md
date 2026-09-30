# Stargaze menu concepts

Eight standalone, interactive HTML prototypes on branch `menu-ui`:

| Concept | Direction | Typography | Palette |
| --- | --- | --- | --- |
| Amber Terminal | Compact pixel observatory | VT323 | Charcoal, amber, warm ivory |
| Glacier Arcade | Structured pixel console | Silkscreen | Midnight navy, ice blue, mint |
| Celestial Blueprint | Circular drafting chart and horizontal system index | IBM Plex Mono + DM Sans | Cobalt, chalk, signal yellow |
| Vermilion Dispatch | Oversized poster and asymmetric destination column | DM Sans + IBM Plex Mono | Vermilion, ink, cream |
| Phosphor Console | Terminal tree, optical feed, working command prompt | VT323 + IBM Plex Mono | Black, acid lime, mint |
| Celestial Survey | Three-column exploration desk and compass preview | DM Sans + IBM Plex Mono | Cobalt, chalk, turquoise |
| Celestial Archive | Printed atlas, etched chart, destination plates | Newsreader + IBM Plex Mono | Ivory, navy, vermilion |
| Celestial Nocturne | Clickable constellation of mystery field studies | Newsreader + IBM Plex Mono | Indigo, silver, saffron |

**Celestial Blueprint · every screen** (`suite.html`) takes the Blueprint direction through every menu page in the game. It covers the title screen (new), system browser, observation HUD, maps, field-study HUD, theory editor, the clear / check / leave dialogs, settings (new) and controls (new), plus invalid-config, empty, load-failure, hidden-HUD, eclipse-search and no-day states. Press `` ` `` for the screen index, `?` for per-screen notes (what the game has today vs. what is proposed), and `[` / `]` to step through the screens. `suite.html?screen=theory` opens a screen directly. It includes the idea.md wishes: decade speed steps, a key on every button, Theory in the bottom bar, a keep-or-discard prompt when leaving a field study, a Horizon / Stars orientation next to the lock, and the full-page theory editor. Scoring uses a JS port of `game::score`.

**Button motion** (`buttons.html`) is a small study of 20 hover and press animations in the Blueprint style. Each one is shown on the primary, outline and HUD buttons. Keys shown on the buttons play their press animation, and a speed control slows everything down for inspection.

Open `index.html` directly, or serve this directory:

```sh
python3 -m http.server 8765 --bind 127.0.0.1
```

Then open http://localhost:8765. Fonts and images are bundled; no build step or external requests are required. These are standalone plain HTML/CSS/JS prototypes. Amber and Glacier preserve the original pixel direction; the other concepts explore different layouts and interaction styles. The three Celestial variants are designed for desktop gameplay; all earlier concepts are preserved.

Try selecting a system, searching, favoriting, switching between Explore and Puzzle, and entering the observation view. The HUD supports play/pause, speed, previous/next day, labels, exposure, automatic exposure, and a stars toggle. Number keys select systems; Enter opens the selected system; M/Escape switches between menu and observation. Keyboard focus is visible. Puzzle mode hides system names, filenames, physical details, and preview images. All simulation changes are illustrative browser state, not a connection to the Rust renderer.

The Saturn, Moon, and eclipse images are existing project renders. They are explicitly labeled as reference renders when used for a different system; they are not new renders of the selected configuration. Font licenses are in `assets/`. `previews/` contains desktop screenshots for the comparison page.

The Phosphor prompt supports `help`, `open 1` through `open 5`, `puzzle`, `explore`, `menu`, `resume`, and `clear`. All Celestial charts are decorative, not computed maps of orbital positions. The new variants have working grid, compass, and preview scale controls. Nocturne starts in Puzzle mode and offers a clickable constellation as a level selector; its central world is an abstract illustration that reveals no map geometry.

The gallery presents Celestial Blueprint and its three variants first, followed by every earlier concept.

The Rust app is unchanged. The worktree starts at commit `26d67fa`; uncommitted changes in the main workspace are not copied into this branch.
