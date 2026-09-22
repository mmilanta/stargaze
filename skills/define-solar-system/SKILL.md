---
name: define-solar-system
description: Create or edit Stargaze YAML star systems and surface camera setups from natural-language prompts, including planets, moons, rings, and tidal locking. Use for scene design and configuration in Stargaze.
---

# Define a solar system for Stargaze

Stargaze is a Rust desktop observatory simulator. A telescope attached to a rotating planet or moon views analytic Keplerian orbits. Its progressive GPU path tracer renders finite stars, spheres, ring shadows, eclipses, and an optional atmosphere on the camera's host. Time can be paused or scrubbed; exposure has manual and automatic modes.

## Workflow

- Locate the Stargaze checkout: use the current project when appropriate; this installation is at `/home/marco/Documents/stargaze`. Inspect existing changes before editing.
- Read [the configuration format](references/config-format.md), then inspect `configs/halo.yaml` or `configs/solar-system.yaml` as the closest starting point. Scene definitions belong in YAML; ordinary scene requests need no Rust edits.
- Translate the requested view into bodies, orbits, rings, and a surface camera. Preserve unspecified properties of an existing system. For a new system, choose reasonable defaults and state material assumptions; ask only when a missing choice changes the requested result.
- Edit the requested file or create `configs/<descriptive-name>.yaml`. The default startup reads `configs/halo.yaml`; creating another file does not select it automatically. Include the explicit launch command when delivering a new config.
- Validate from the project root with `cargo run --release -- --check-config configs/<name>.yaml`. Fix reported schema or geometry errors. This command runs without a display or GPU; it validates structure, not long-term orbital stability or artistic framing.
- If launching or visual verification is in scope, use `cargo run --release -- --config configs/<name>.yaml`. Check the target's horizon placement and framing. A valid target can still be underground at an explicitly selected time. Report the file, camera location, assumptions, and validation result.

## Design decisions

For a planet fixed in a moon's sky, use a circular moon orbit and `rotation: {mode: tidal_lock}`. Longitude 180° faces the parent; longitude 0° faces away. Start around latitude 20–35° for visible ground with the planet above the horizon. Incline the moon orbit relative to the planet's equator to expose its rings; a coplanar observer sees them edge-on.

Ring radii are multiples of the planet radius. For rings that stop short of a moon, require `outer_radius * planet.radius_km < moon_axis_km * (1 - eccentricity) - moon.radius_km`, with visible clearance. Moving the moon farther out shrinks the planet in the sky. When keeping the same parent mass, scale its orbital period by `(new_axis / old_axis)^1.5`; tidal spin follows automatically.

Periods are explicit: Stargaze has no mass solver or N-body gravity. Binary stars orbit a shared invisible anchor; their phases should oppose one another and their periods match. Moon orbit angles use the same world frame as planet orbits, not their parent's equator. Eccentric synchronous moons librate naturally.

For real-system requests, retain the supplied approximate physical data or consult authoritative astronomy sources for new values. Distinguish a plausible visualization from a date-accurate ephemeris. Do not silently inflate real radii or distances to improve the composition; change camera FOV or placement instead.
