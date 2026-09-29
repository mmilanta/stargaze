# Stargaze menu concepts

Four standalone, interactive HTML prototypes on branch `menu-ui`:

| Concept | Direction | Typography | Palette |
| --- | --- | --- | --- |
| Amber Terminal | Compact pixel observatory | VT323 | Charcoal, amber, warm ivory |
| Glacier Arcade | Structured pixel console | Silkscreen | Midnight navy, ice blue, mint |
| Material Midnight | Material-inspired dark workspace | DM Sans | Graphite, lavender, muted violet |
| Material Daylight | Material-inspired light workspace | DM Sans | Warm white, sage, forest green |

Open `index.html` directly, or serve this directory:

```sh
python3 -m http.server 8765 --bind 127.0.0.1
```

Then open http://localhost:8765. Fonts and images are bundled; no build step or external requests are required. These are plain HTML/CSS/JS explorations of Material design, rather than React/MUI implementations.

Try selecting a system, searching, favoriting, switching between Explore and Puzzle, and entering the observation view. The HUD supports play/pause, speed, previous/next day, labels, exposure, automatic exposure, and a stars toggle. Number keys select systems; Enter opens the selected system; M/Escape switches between menu and observation. Keyboard focus is visible. Puzzle mode hides system names, filenames, physical details, and preview images. All simulation changes are illustrative browser state, not a connection to the Rust renderer.

The Saturn, Moon, and eclipse images are existing project renders. They are explicitly labeled as reference renders when used for a different system; they are not new renders of the selected configuration. Font licenses are in `assets/`. `previews/` contains desktop screenshots for the comparison page.

The Rust app is unchanged. The worktree starts at commit `26d67fa`; uncommitted changes in the main workspace are not copied into this branch.
