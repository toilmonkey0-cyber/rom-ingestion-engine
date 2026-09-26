import os
os.environ["NEEDLE_TELEMETRY"] = "0"
os.environ["DO_NOT_TRACK"] = "1"

import time, json
from typing import Literal
from pydantic import BaseModel
import needle

class GameClassification(BaseModel):
    platform: Literal["psx", "saturn", "dreamcast", "segacd", "pcecd"]
    is_multidisc: bool
    disc_number: Literal["single", "1", "2", "3", "4", "5", "6", "7", "8"]
    region: Literal["USA", "Europe", "Japan", "World"]

agent = needle.Needle(tools=[GameClassification])

cases = [
    "Final Fantasy VII (USA) (Disc 2).cue",
    "Shenmue (USA) (Disc 1).gdi",
    "Sonic CD (USA).cue",
    "Akumajou Dracula X - Chi no Rondo (Japan).cue",
    "Nights into Dreams (Japan).cue",
    "Metal Gear Solid (USA) (Disc 1)",
    "gran_turismo_scene_rip_psx_track1.bin",
    "Panzer Dragoon Saga (USA) Disc 3 of 4.cue",
]

for c in cases:
    r = agent.complete(c)
    calls = r.get("function_calls") or []
    args = calls[0]["arguments"] if calls else None
    held = r.get("suppressed_calls") or []
    print(f"{c!r}")
    print(f"   conf={r.get('confidence')} args={json.dumps(args)} suppressed={len(held)}")
    print(f"   reasoning={r.get('reasoning')!r}")
