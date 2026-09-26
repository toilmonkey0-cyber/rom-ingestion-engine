"""Score a Needle sidecar (base or tuned weights) on the held-out test set.

Usage: python eval_sidecar.py [--url http://127.0.0.1:8080] [--data test.jsonl] [--label base]
"""
import argparse, json, os, re, sys, time
import urllib.request

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

def post(url, payload=None):
    data = json.dumps(payload).encode() if payload is not None else b""
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=60) as r:
        body = r.read().decode()
    return json.loads(body) if body else {}

# --- regex baseline mirroring the app's Fallback tier (filename-only) -------
REGION_MAP = {
    "usa": "USA", "u": "USA", "us": "USA", "north america": "USA", "ntsc": "USA", "ntsc-u": "USA",
    "europe": "Europe", "e": "Europe", "eu": "Europe", "pal": "Europe", "euro": "Europe",
    "japan": "Japan", "j": "Japan", "jpn": "Japan", "ntsc-j": "Japan",
    "world": "World", "w": "World",
}
def baseline_region(q):
    for m in re.finditer(r"[\(\[]([^()\[\]]+)[\)\]]", q):
        t = m.group(1).strip().lower()
        if t in REGION_MAP:
            return REGION_MAP[t]
    for w in re.split(r"[\s_\-/\.]+", q):
        if w.lower() in REGION_MAP:
            return REGION_MAP[w.lower()]
    return None

def baseline_disc(q):
    m = re.search(r"(?:disc|disk|cd)[\s_]*(\d)", q, re.IGNORECASE)
    return m.group(1) if m else "single"

def baseline_platform(q):
    # .gdi is the only extension that implies a platform by itself
    return "dreamcast" if q.lower().endswith(".gdi") else None

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", default="http://127.0.0.1:8080")
    ap.add_argument("--data", default=os.path.join(SCRIPT_DIR, "data", "test.jsonl"))
    ap.add_argument("--label", default="needle")
    args = ap.parse_args()

    rows = [json.loads(l) for l in open(args.data, encoding="utf-8")]
    stats = {"n": 0, "platform": 0, "region": 0, "disc": 0, "multidisc": 0, "exact": 0,
             "no_call": 0, "neg_correct": 0, "neg_total": 0}
    base = dict(stats)
    confusions = {}
    latencies = []
    withheld = 0

    for row in rows:
        q = row["query"]
        gold_args = row["answers"][0]["arguments"] if row["answers"] else None
        stats["n"] += 1
        base["n"] += 1

        # ---- baseline ----
        if gold_args is None:
            base["neg_total"] += 1  # regex baseline can't refuse; counts wrong
        else:
            br = baseline_region(q) or "Unknown"
            bd = baseline_disc(q)
            bp = baseline_platform(q)
            bm = bd != "single"
            if bp == gold_args["platform"]: base["platform"] += 1
            if br == gold_args["region"]: base["region"] += 1
            if bd == gold_args["disc_number"]: base["disc"] += 1
            if bm == gold_args["is_multidisc"]: base["multidisc"] += 1

        # ---- sidecar ----
        t0 = time.time()
        try:
            post(f"{args.url}/reset")
            resp = post(f"{args.url}/complete", {"input": q})
        except Exception as e:
            print(f"REQUEST FAILED on {q!r}: {e}", file=sys.stderr)
            continue
        latencies.append(time.time() - t0)

        calls = resp.get("function_calls") or []
        if not calls:
            calls = resp.get("suppressed_calls") or []
            if calls:
                withheld += 1
        pred = calls[0]["arguments"] if calls else None

        if gold_args is None:
            stats["neg_total"] += 1
            if pred is None:
                stats["neg_correct"] += 1
                stats["exact"] += 1
            continue

        if pred is None:
            stats["no_call"] += 1
            continue
        if pred.get("platform") == gold_args["platform"]:
            stats["platform"] += 1
        else:
            key = (gold_args["platform"], pred.get("platform"))
            confusions[key] = confusions.get(key, 0) + 1
        if pred.get("region") == gold_args["region"]: stats["region"] += 1
        if str(pred.get("disc_number")) == str(gold_args["disc_number"]): stats["disc"] += 1
        if bool(pred.get("is_multidisc")) == bool(gold_args["is_multidisc"]): stats["multidisc"] += 1
        if (pred.get("platform") == gold_args["platform"] and pred.get("region") == gold_args["region"]
                and str(pred.get("disc_number")) == str(gold_args["disc_number"])
                and bool(pred.get("is_multidisc")) == bool(gold_args["is_multidisc"])):
            stats["exact"] += 1

    def report(name, s):
        n = s["n"]
        pos = n - s["neg_total"]
        print(f"\n== {name} ==  n={n} (positives={pos}, negatives={s['neg_total']})")
        print(f"  platform  : {s['platform']}/{pos} = {s['platform']/max(pos,1)*100:.1f}%")
        print(f"  region    : {s['region']}/{pos} = {s['region']/max(pos,1)*100:.1f}%")
        print(f"  disc      : {s['disc']}/{pos} = {s['disc']/max(pos,1)*100:.1f}%")
        print(f"  multidisc : {s['multidisc']}/{pos} = {s['multidisc']/max(pos,1)*100:.1f}%")
        print(f"  exact-all : {s['exact']}/{n} = {s['exact']/max(n,1)*100:.1f}%")
        print(f"  junk refused correctly: {s['neg_correct']}/{s['neg_total']}")

    print(f"sidecar: {args.url}  data: {args.data.split(chr(92))[-1]}  label: {args.label}")
    print(f"latency ms: p50={sorted(latencies)[len(latencies)//2]*1000:.0f} max={max(latencies)*1000:.0f}  withheld={withheld}")
    report(f"{args.label}", stats)
    report("regex-baseline (app Fallback tier, filename-only)", base)
    if confusions:
        print("\ntop platform confusions (gold -> pred):")
        for (g, p), c in sorted(confusions.items(), key=lambda kv: -kv[1])[:8]:
            print(f"  {g} -> {p}: {c}")

if __name__ == "__main__":
    main()
