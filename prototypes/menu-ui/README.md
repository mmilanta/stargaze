# Stargaze menu concepts

Five standalone, interactive HTML prototypes on branch `menu-ui`:

| Concept | Direction | Typography | Palette |
| --- | --- | --- | --- |
| Amber Terminal | Compact pixel observatory | VT323 | Charcoal, amber, warm ivory |
| Glacier Arcade | Structured pixel console | Silkscreen | Midnight navy, ice blue, mint |
| Celestial Blueprint | Circular drafting chart and horizontal system index | IBM Plex Mono + DM Sans | Cobalt, chalk, signal yellow |
| Vermilion Dispatch | Oversized poster and asymmetric destination column | DM Sans + IBM Plex Mono | Vermilion, ink, cream |
| Phosphor Console | Terminal tree, optical feed, working command prompt | VT323 + IBM Plex Mono | Black, acid lime, mint |

Open `index.html` directly, or serve this directory:

```sh
python3 -m http.server 8765 --bind 127.0.0.1
```

Then open http://localhost:8765. Fonts and images are bundled; no build step or external requests are required. These are standalone plain HTML/CSS/JS prototypes. Amber and Glacier preserve the original pixel direction; the three new concepts explore different layouts and interaction styles.

Try selecting a system, searching, favoriting, switching between Explore and Puzzle, and entering the observation view. The HUD supports play/pause, speed, previous/next day, labels, exposure, automatic exposure, and a stars toggle. Number keys select systems; Enter opens the selected system; M/Escape switches between menu and observation. Keyboard focus is visible. Puzzle mode hides system names, filenames, physical details, and preview images. All simulation changes are illustrative browser state, not a connection to the Rust renderer.

The Saturn, Moon, and eclipse images are existing project renders. They are explicitly labeled as reference renders when used for a different system; they are not new renders of the selected configuration. Font licenses are in `assets/`. `previews/` contains desktop screenshots for the comparison page.

The Phosphor prompt supports `help`, `open 1` through `open 5`, `puzzle`, `explore`, `menu`, `resume`, and `clear`. The Blueprint sky chart is decorative, not a computed map of orbital positions.

The Rust app is unchanged. The worktree starts at commit `26d67fa`; uncommitted changes in the main workspace are not copied into this branch.
