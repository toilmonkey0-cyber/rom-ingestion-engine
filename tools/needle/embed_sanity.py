import os
os.environ["NEEDLE_TELEMETRY"] = "0"
os.environ["DO_NOT_TRACK"] = "1"

import math
import needle

agent = needle.Needle()  # base weights

def cos(a, b):
    num = sum(x * y for x, y in zip(a, b))
    da = math.sqrt(sum(x * x for x in a))
    db = math.sqrt(sum(y * y for y in b))
    return num / (da * db)

t0 = agent.embed("Final Fantasy VII")
print("dim:", len(t0), "type:", type(t0[0]))

ff8 = agent.embed("Final Fantasy VIII")
ct = agent.embed("Chrono Trigger")
gt = agent.embed("Gran Turismo")
sat = agent.embed("Panzer Dragoon Saga")

print(f"FF7 vs FF8       : {cos(t0, ff8):.4f}  (same series, same platform)")
print(f"FF7 vs ChronoTrig: {cos(t0, ct):.4f}  (same genre era, cross platform)")
print(f"FF7 vs GranTur   : {cos(t0, gt):.4f}  (different genre)")
print(f"FF7 vs PanzerDrag: {cos(t0, sat):.4f}  (different platform)")
print(f"FF7 vs messy     : {cos(t0, agent.embed('final_fantasy_vii_disc2_usa_rip')):.4f}")
