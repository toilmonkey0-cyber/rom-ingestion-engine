import os
os.environ["NEEDLE_TELEMETRY"] = "0"
os.environ["DO_NOT_TRACK"] = "1"

import time
from typing import Literal
from pydantic import BaseModel
import needle

class GameClassification(BaseModel):
    platform: Literal["psx", "saturn", "dreamcast", "segacd", "pcecd"]
    is_multidisc: bool
    disc_number: Literal["single", "1", "2", "3", "4", "5", "6", "7", "8"]
    region: Literal["USA", "Europe", "Japan", "World"]

# Same fixtures the Rust jev_test.rs uses, plus messy real-world shapes
cases = [
    "Final Fantasy VII (USA) (Disc 2).cue",
    "Shenmue (USA) (Disc 1).gdi",
    "Sonic CD (USA).cue",
    "Akumajou Dracula X - Chi no Rondo (Japan).cue",
    "Nights into Dreams (Japan).cue",
    "Metal Gear Solid (USA) (Disc 1)",            # no extension
    "gran_turismo_scene_rip_psx_track1.bin",      # scene-style mess
    "Panzer Dragoon Saga (USA) Disc 3 of 4.cue",  # 'of N' disc style
]

t0 = time.time()
first = True
for c in cases:
    t = time.time()
    try:
        r = needle.extract(c, GameClassification)
        dt = time.time() - t
        load = " (incl. model load)" if first else ""
        first = False
        print(f"{dt:6.2f}s{load:17} {c!r:58} -> {r}")
    except Exception as e:
        print(f"ERROR on {c!r}: {type(e).__name__}: {e}")
print(f"\ntotal wall: {time.time()-t0:.1f}s")
