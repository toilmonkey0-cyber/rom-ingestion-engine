#!/bin/bash
# Fine-tune Needle on the ROM corpus inside WSL, then export a .cact.
# Memory-constrained profile: JAX preallocation off (it grabs ~75% of RAM by
# default and got the first run OOM-killed), short max-len (filenames are
# ~30 tokens), small batch, few workers.
set -e
export NEEDLE_TELEMETRY=0 DO_NOT_TRACK=1
export XLA_PYTHON_CLIENT_PREALLOCATE=false XLA_PYTHON_CLIENT_MEM_FRACTION=0.55
export PATH="$HOME/.local/bin:$PATH"
SPIKE_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SPIKE_DIR"
echo "== finetune =="
needle finetune data/train.jsonl --epochs 3 --max-len 512 --batch-size 8 --workers 2 --out adapter.safetensors 2>&1 | tail -25
echo "== build =="
needle build checkpoints/needle3.safetensors --lora adapter.safetensors --out romclass.cact 2>&1 | tail -8
ls -la adapter.safetensors romclass.cact
