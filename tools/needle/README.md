# Needle classification pipeline (spike tooling)

Reproducible tooling for the local Needle classification tier — the spike
documented in `docs/needle-spike-2026-09-26.md`. The Rust integration lives
in `src-tauri/src/classifier/needle.rs`; everything here builds the model
and measures it.

## Layout

| file | purpose |
|---|---|
| `tools.json` | the `classify_rom` tool schema — must match what the sidecar serves and what the model was trained on |
| `gen_data.py` | generates the synthetic corpus (`data/*.jsonl`): curated famous-title seeds per platform + tag mangling; train/val/test split by title hash |
| `data/` | train 1,778 / validation 59 / test 83 examples; labels mirror the app's `redump.rs` parsers |
| `adapter.safetensors` | trained LoRA adapter (the irreplaceable artifact — 3.5 h CPU in WSL); rebuild the deployable weights with `needle build` (below) |
| `eval_sidecar.py` | scores any running sidecar field-by-field (platform/region/disc/multidisc/exact + junk refusal) against a regex baseline |
| `embed_experiment.py` | the embedding-index experiment: NN platform via `Needle.embed` (3072-dim), k=1 + threshold sweep |
| `extract_test1.py`, `confidence_test.py`, `embed_sanity.py` | one-shot probes used during the spike (latency, confidence behavior, embedding texture) |
| `setup_wsl.sh`, `finetune_wsl.sh` | WSL training environment install + constrained-profile fine-tune runner |

Not committed (regenerable / re-downloadable):

- `romclass.cact` — rebuilt from the committed adapter + base in ~2 min:
  `needle build checkpoints/needle3.safetensors --lora adapter.safetensors --out romclass.cact`
- base checkpoint `needle3.safetensors` — auto-downloaded from
  `Cactus-Compute/needle3` by the CLI on first use
- engine binaries (`needle.exe`, `libneedle.a`, `needle.h`, `needle3.cact`) —
  `needle download windows-x86_64 --out engine` (pip package, see below)

## Reproduce

```powershell
# runtime (Python 3.13 used in the spike) — PIN THIS VERSION: 3.0.5's
# engine-wheel download 404s; only 3.0.0/3.0.1 wheels are published.
pip install cactus-needle==3.0.1
$env:NEEDLE_TELEMETRY='0'; $env:DO_NOT_TRACK='1'

# fetch the sidecar engine, then serve base or tuned weights
needle download windows-x86_64 --out engine        # via the needle CLI
engine\windows-x86_64\needle.exe --model engine\needle3.cact --tools tools.json --serve
# tuned: rebuild romclass.cact (above), then --model romclass.cact

# score whatever is serving on :8080
python eval_sidecar.py --label tuned

# embedding-index experiment (uses the Python embed API + the sidecar for tags)
python embed_experiment.py
```

Training happens in WSL (the `[train]` extras exceed Windows MAX_PATH, and
JAX's default RAM preallocation OOM-kills the run inside WSL's ~7 GB VM):

```bash
wsl -d Ubuntu -- bash setup_wsl.sh        # one-time install
wsl -d Ubuntu -- bash finetune_wsl.sh     # LoRA -> adapter.safetensors -> romclass.cact
```

## Spike conclusions (short form)

- Tags: tuned model parses region/disc/multidisc perfectly on canonical
  names; the regex tier stays primary (100% on tagged names).
- Platform: solved by the embedding index, not generation — full-index NN is
  92.5% (97.1% at 86% coverage with a 0.985 abstain threshold); k=1 beats
  k=5 voting; clean stems before embedding.
- Junk: the tuned model refuses off-topic input; low NN similarity abstains.
- Serve mode has no `/embed` route — runtime embedding needs the C API
  (`needle_embed` in `needle.h`) linked in-process.
