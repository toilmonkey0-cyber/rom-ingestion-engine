# Needle Spike Notes — 2026-09-26

Spike goal: prove (or disprove) that a local Needle engine can replace the
opt-in BYOK TypeSafe Jev tier for filename classification, with a path to a
domain fine-tune. Status: integration merged behind an optional parameter;
fine-tune pipeline proven; tuned-model numbers pending final run.

## What shipped in this repo

- `src-tauri/src/classifier/needle.rs` — `NeedleClient` (serve-mode HTTP) and
  `parse_needle_response` mirroring the Jev parser's vocabulary defenses.
- `src-tauri/tests/needle_test.rs` — 7 tests incl. mock-server protocol test.
- `scan_and_plan` gained `needle_base_url: Option<String>`; classification
  chain is now Redump → Needle (local) → Jev (BYOK) → regex fallback.
  `NeedleError::NoCall` (engine refuses: junk/off-topic) falls through
  silently; transport errors surface as a status note.
- `ClassificationSource::NeedleAI` end-to-end (serde `needleai`, plan-builder
  precedence, frontend type + dry-run badge).
- Full Rust suite green (all test binaries, 0 failed); frontend vitest 39/39.

## Integration contract (learned from the live sidecar)

- Sidecar: `needle.exe --model needle3.cact --tools tools.json --serve`
  (engine 1.3 MB, weights 35 MB, ~81 MB peak RAM, p50 ~170 ms/file over HTTP
  incl. reset round-trip, ~700 tok/s decode on this laptop CPU).
- **The engine owns one process-global conversation.** Every file MUST be
  `POST /reset` then `POST /complete {"input":"<folder>/<filename>"}`.
  Without reset, answers leak across files ("carries over from history").
- **Input text is the bare filename (folder-prefixed).** Verbs leak into
  grounded fields — the word "Classify" made the model set
  `is_multidisc: true`. Instructions belong in the tool description only.
- `suppressed_calls` holds calls the confidence gate withheld; arguments are
  present, so the parser uses them at 0.5x confidence.
- `confidence` is `null` for local-LoRA weights (calibration head ships only
  with base/platform-trained weights). Parser defaults it to 0.75 so
  `needs_review` gating still behaves.
- The decode grammar constrains answers to the schema enums; the Rust parser
  still validates (out-of-vocab → Unknown + 0.25x confidence, region falls
  back to Redump tag extraction, disc 1..8 only) so a buggy sidecar cannot
  inject an unvalidated platform or path-unsafe string.
- Spawn any Cactus component with `NEEDLE_TELEMETRY=0 DO_NOT_TRACK=1`.
  Inference itself never touches the network.

## Version traps (cost real time — do not relearn)

- pip `cactus-needle` 3.0.5 requests an engine wheel `3.0.2` that is NOT
  published on HF (only 3.0.0/3.0.1 are). **Pin `cactus-needle==3.0.1`.**
- `[train]` extras (flax→orbax) blow past Windows MAX_PATH with ~250-char
  test-fixture paths; a short venv prefix does not help. Train in WSL
  (`pip3 install --user --break-system-packages 'cactus-needle[train]==3.0.1'`).
  WSL can fail to create its VM under memory pressure — retry later.
- Windows engine + weights via HF repo `Cactus-Compute/needle3`:
  `windows-x86_64/{needle.exe,libneedle.a,needle.h}` + `needle3.cact`.
  `needle download windows-x86_64 --out dir` works from the pip package.
- redump.org is unreachable from this network; libretro DAT mirrors are gone
  (libretro-dats restructured to a downloader; libretro-database `dat/` is
  nearly empty). The corpus was synthesized instead (see below).

## Corpus + eval methodology

- `gen_data.py` (spike scratch, not committed): curated famous-title seeds
  per platform + synthetic tag mangling (region tag variants `(U)/[E]/PAL/…`,
  disc tag variants, scene junk, folder hints, mixed separators/case).
  Labels mirror `redump.rs` parsing exactly. `.gdi` only ever labels
  dreamcast (it is a Dreamcast-only descriptor — generator bug fixed).
- Split by title hash: train 1778 / val 59 / test 83 (test titles unseen).
  Negatives (firmware/BIOS packs/saves) labeled with empty `answers`.
- `eval_sidecar.py`: scores any sidecar field-by-field + a regex baseline
  that mirrors the app Fallback tier (filename-only).

## Base-model numbers (test set, n=83, before fine-tune)

| metric | base Needle | regex baseline (Fallback tier) |
|---|---|---|
| platform | 16.2% | 1.2% (only `.gdi` implies dreamcast) |
| region | 41.2% | 100% |
| disc | 62.5% | 100% |
| multidisc | 37.5% | 100% |
| exact-all | 0% | 0% |
| junk refused | 0/3 | n/a |

Reading: on cleanly-tagged synthetic names the regex already nails tags, so
the model's only possible value-add is **platform from title knowledge** —
and the base model lacks the domain (its top confusion predicts `pcecd` for
everything). The fine-tune must (a) reach regex parity on tags, (b) add
platform knowledge, (c) refuse junk. The earlier ad-hoc probe
(`gran_turismo_scene_rip_psx_track1.bin` → correct psx) shows the model CAN
read scene-style hints the regex ignores; the synthetic eval under-weights
that strength — a follow-up eval should use real messy library names.

## Fine-tune pipeline (WSL)

```
needle finetune data/train.jsonl --epochs 4 --out adapter.safetensors
needle build checkpoints/needle3.safetensors --lora adapter.safetensors --out romclass.cact
# serve tuned weights: needle.exe --model romclass.cact --tools tools.json --serve
```

Local LoRA = 4-bit merge at export; confidence head dropped (`null`).
Production option if calibrated confidence is needed: one hosted platform
fine-tune (keeps head + 2-bit) or gate on grammar validity + tier agreement.

## Tuned-model results (run completed 2026-09-26)

3 epochs on 1,778 examples in WSL: train loss 0.0396, val 0.0526;
`romclass.cact` 63 MB (W4A8, 20 layers). Training notes: JAX preallocates
~75% of RAM by default and the first run was OOM-killed at 6.5 GB RSS inside
the ~7 GB WSL VM — set `XLA_PYTHON_CLIENT_PREALLOCATE=false` and use
`--max-len 512 --batch-size 8 --workers 2`; full run ≈ 3.5 h on this CPU.

What the fine-tune delivered, from live probes against the tuned sidecar:

- **Tags: solved.** Canonical probes parse perfectly — `Final Fantasy VII
  (USA) (Disc 2)` → disc 2/multi/USA; `(Japan)`/`(Europe)` regions; `Disc 1`
  sets. Heavy mangling from the synthetic eval (abbrev tags, `of N`, cd1)
  still trips at ~40–60% — more/better data fixes this, the behavior is
  installed.
- **Surface-token shortcuts: learned.** `gar_turismo_scene_rip_psx_track1.bin`
  → psx (it reads `psx` from the name, which the regex tier cannot).
- **Title→platform knowledge: NOT installed.** Unseen titles collapse to a
  dominant class (eval 31% platform, everything→psx in one run,
  everything→dreamcast in another), and even training-seen titles answer
  wrong when the exact training variant differs. A 1.7k-example LoRA on a
  small model learns the *format*, and cannot install a per-title lookup
  table with robustness.

**Architectural conclusion (the spike's main finding): split the labor.**

1. Fine-tuned Needle = tag parser + junk refuser + confidence (generation).
2. `needle_embed` = title→platform/canonical-name nearest-neighbor against a
   prebuilt Redump title index (lookup). This mirrors Needle's own tool
   retrieval head and is the same embedding pattern proposed for Card
   Studio's DAT-less sorting.

Next experiment for promotion: build the embedding index from the builtin
Redump titles + real DATs, classify platform by NN over cleaned stems, and
let the tuned model handle region/disc/multidisc/junk. If NN platform + tuned
tags both hit high 90s on a real-world messy eval, Needle replaces the Jev
tier outright.

## Open items for production promotion

1. Tuned-model eval numbers (in flight) + retry with real Redump DATs for
   the corpus (user's browser can fetch what this network cannot).
2. Sidecar lifecycle in the app: managed download/cache like chdman
   (`chdman/downloader.rs` pattern), spawn with telemetry env vars,
   health-check before enabling the tier.
3. `tools.json` embedded as an included asset so app and training share one
   schema definition.
4. UI: endpoint field (default `http://127.0.0.1:8080`) + Needle badge copy.
5. Watch mode (`watch.rs`) still Redump+fallback only — natural Needle
   consumer once promoted.
