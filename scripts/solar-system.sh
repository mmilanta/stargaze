#!/bin/sh
# Solar system from Saturn's modeled surface at 20 degrees north.
set -eu
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
export STARGAZE_SYSTEM=solar
exec cargo run --release
