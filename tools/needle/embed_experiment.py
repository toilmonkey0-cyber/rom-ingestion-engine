"""Embedding-index experiment: platform via needle_embed nearest-neighbor.

Index: canonical titles from the train split (same hash split as the
fine-tune corpus) embedded with base weights.
Query: test.jsonl rows (unseen titles), stem cleaned before embedding.
Also runs: full-index (production-like, title present in DAT) variant,
and a combined score (NN platform + tuned-sidecar tags).
"""
import json, math, os, re, sys, time
import urllib.request

os.environ["NEEDLE_TELEMETRY"] = "0"
os.environ["DO_NOT_TRACK"] = "1"
import needle
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_data import TITLES, split_key

SIDECAR = "http://127.0.0.1:8080"

# ---------------- stem cleaning (mirrors redump.rs clean_canonical_title) ---
JUNK_WORDS = {
    "rip", "proper", "repack", "track", "track1", "track01", "disc", "disk",
    "bin", "cue", "iso", "img", "gdi", "chd", "7z", "zip", "usa", "us",
    "europe", "eu", "japan", "jpn", "world", "ntsc", "pal", "rev", "v1",
}
def clean_stem(q: str) -> str:
    s = q.replace("\\", "/").split("/")[-1]          # drop folder hint
    s = re.sub(r"\.[a-z0-9]{1,4}$", "", s, flags=re.I)  # drop extension
    s = re.sub(r"[\(\[\{][^\)\]\}]*[\)\]\}]", " ", s)   # drop tag groups
    s = re.sub(r"[\_\.]+", " ", s)                      # separators -> space
    s = re.sub(r"[-]+", " ", s)
    tokens = [t for t in re.split(r"\s+", s.lower()) if t and t not in JUNK_WORDS and not re.fullmatch(r"\d+", t)]
    return " ".join(tokens) if tokens else s.lower().strip()

def cos(a, b):
    num = sum(x * y for x, y in zip(a, b))
    da = math.sqrt(sum(x * x for x in a))
    db = math.sqrt(sum(y * y for y in b))
    return num / (da * db) if da and db else 0.0

# ---------------- sidecar helpers (tags from the tuned model) --------------
def post(url, payload=None):
    data = json.dumps(payload).encode() if payload is not None else b""
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=60) as r:
        body = r.read().decode()
    return json.loads(body) if body else {}

def sidecar_tags(query):
    post(f"{SIDECAR}/reset")
    resp = post(f"{SIDECAR}/complete", {"input": query})
    calls = resp.get("function_calls") or resp.get("suppressed_calls") or []
    return calls[0]["arguments"] if calls else None

# ---------------- build index ----------------------------------------------
agent = needle.Needle()  # base weights
t0 = time.time()
index = []  # (title, platform, vector)
for platform, titles in TITLES.items():
    for title in sorted(set(titles)):
        if split_key(title, platform) >= 0.75:   # eval/val titles stay out
            continue
        index.append((title, platform, agent.embed(title.lower())))
print(f"index: {len(index)} titles across {len(TITLES)} platforms ({time.time()-t0:.1f}s)")

full_index = []  # production-like: every title indexed
for platform, titles in TITLES.items():
    for title in sorted(set(titles)):
        full_index.append((title, platform, agent.embed(title.lower())))

def nn_platform(vec, pool, k=1):
    scored = sorted(((cos(vec, v), p, t) for (t, p, v) in pool), reverse=True)
    from collections import Counter
    votes = Counter(p for _, p, _ in scored[:k])
    top_score, top_p, top_t = scored[0]
    winner, n = votes.most_common(1)[0]
    margin = top_score  # nearest-neighbor similarity as confidence proxy
    return winner, top_p, top_score, top_t

# ---------------- evaluate -------------------------------------------------
rows = [json.loads(l) for l in open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "data", "test.jsonl"), encoding="utf-8")]

res = {"n": 0, "nn_unseen": 0, "nn_full": 0, "combined_exact": 0, "no_call": 0}
per_platform = {}
thresholds = [0.0, 0.955, 0.965, 0.975, 0.985, 0.995]
thr_stats = {t: {"cov": 0, "ok": 0, "cov_f": 0, "ok_f": 0} for t in thresholds}
t0 = time.time()
for row in rows:
    gold = row["answers"][0]["arguments"] if row["answers"] else None
    if gold is None:
        continue  # negatives: tags side handles refusal (measured before)
    res["n"] += 1
    stem = clean_stem(row["query"])
    vec = agent.embed(stem)

    pred_u, _, sim_u, _ = nn_platform(vec, index)
    pred_f, _, sim_f, match_t = nn_platform(vec, full_index)
    if pred_u == gold["platform"]: res["nn_unseen"] += 1
    if pred_f == gold["platform"]: res["nn_full"] += 1
    per_platform.setdefault(gold["platform"], [0, 0])
    per_platform[gold["platform"]][0] += int(pred_u == gold["platform"])
    per_platform[gold["platform"]][1] += 1

    for t in thresholds:
        if sim_u >= t:
            thr_stats[t]["cov"] += 1
            thr_stats[t]["ok"] += int(pred_u == gold["platform"])
        if sim_f >= t:
            thr_stats[t]["cov_f"] += 1
            thr_stats[t]["ok_f"] += int(pred_f == gold["platform"])

    tags = sidecar_tags(row["query"])
    if tags is None:
        res["no_call"] += 1
        continue
    combined = dict(tags)
    combined["platform"] = pred_u   # unseen-index variant is the honest one
    if (combined["platform"] == gold["platform"]
            and combined["region"] == gold["region"]
            and str(combined["disc_number"]) == str(gold["disc_number"])
            and bool(combined["is_multidisc"]) == bool(gold["is_multidisc"])):
        res["combined_exact"] += 1

n = res["n"]
print(f"\nqueries: {n}  ({time.time()-t0:.1f}s)")
print(f"NN k=1 platform (index without eval titles) : {res['nn_unseen']}/{n} = {res['nn_unseen']/n*100:.1f}%")
print(f"NN k=1 platform (full index, prod-like)     : {res['nn_full']}/{n} = {res['nn_full']/n*100:.1f}%")
print(f"combined exact-all (NN + tuned tags)        : {res['combined_exact']}/{n} = {res['combined_exact']/n*100:.1f}%")
print(f"sidecar no-call: {res['no_call']}")
print("\nper-platform (unseen index, k=1):")
for p, (ok, tot) in sorted(per_platform.items()):
    print(f"  {p:10s}: {ok}/{tot} = {ok/tot*100:.0f}%")
print("\nthreshold sweep (sim >= T -> answer, else needs_review):")
print("   T     | unseen cov  acc@cov | full cov  acc@cov")
for t in thresholds:
    s = thr_stats[t]
    cov, ok = s["cov"], s["ok"]
    cov_f, ok_f = s["cov_f"], s["ok_f"]
    print(f"  {t:.3f}  |  {cov/n*100:5.1f}%   {ok/max(cov,1)*100:5.1f}%   | {cov_f/n*100:5.1f}%  {ok_f/max(cov_f,1)*100:5.1f}%")

# show a few misses for intuition
print("\nexample misses (unseen index, k=1):")
shown = 0
for row in rows:
    gold = row["answers"][0]["arguments"] if row["answers"] else None
    if gold is None:
        continue
    stem = clean_stem(row["query"])
    pred, _, score, match_t = nn_platform(agent.embed(stem), index)
    if pred != gold["platform"] and shown < 6:
        print(f"  '{stem}' gold={gold['platform']:10s} pred={pred:10s} sim={score:.4f} nearest='{match_t}'")
        shown += 1
