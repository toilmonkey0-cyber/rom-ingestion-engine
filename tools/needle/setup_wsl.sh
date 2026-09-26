#!/bin/bash
set -e
export NEEDLE_TELEMETRY=0 DO_NOT_TRACK=1
pip3 install --quiet --user --break-system-packages 'cactus-needle[train]==3.0.1' 2>&1 | tail -3
export PATH="$HOME/.local/bin:$PATH"
needle --help 2>&1 | head -2
python3 -c 'import jax; print("jax", jax.__version__, jax.devices())'
