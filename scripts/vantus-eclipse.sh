#!/bin/sh
# Iri eclipses Aur on Vantus, viewed from Halo across 2.722377 AU.
# Start paused; allow the image to accumulate, or use the HUD's +/-1m buttons.
set -eu
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
# These search helpers would otherwise override the exact time/view below.
unset STARGAZE_FIND_ECLIPSE STARGAZE_FIND_PHOBOS
export STARGAZE_SYSTEM=binary
export STARGAZE_TIME=444.478
export STARGAZE_AIM=8
export STARGAZE_NO_ADVANCE=1
export STARGAZE_FOV=0.005323985
export STARGAZE_EXPOSURE=1
exec cargo run --release
