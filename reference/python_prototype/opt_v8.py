"""Hill-climb (sim v8): owned + 13-card limited caps; half the swaps try a new (Nov 19) card."""
import sys, json, random
sys.path.insert(0, ".")
import ygo_sim2 as S
import ygo_sim8 as V8
import opt_v7 as O7
import run_v8 as R
from ygo_sim2 import build, C

NEW = [c["name"] for c in json.load(open("sets/extra66.json"))["data"]]
MODELED = O7.MODELED | {"Confiscation"} | {n for n in NEW if C[n][0] != "M" and n not in V8.DEAD}
EXCLUDE = O7.EXCLUDE | (S.NOMI - {"Black Luster Soldier"}) | V8.DEAD
FIXED = {"Fake Trap", "Suijin"}
ALL = [n for n, r in R.ROWS.items() if n not in EXCLUDE and n in C and "Fusion" not in r["type"]
       and (("Monster" in r["type"] and ("Ritual" not in r["type"] or n == "Black Luster Soldier")) or n in MODELED)]
NEW_POOL = [n for n in ALL if n in NEW or n == "Confiscation"]

def evaluate(spec, role, n, gaunt, seed=43):
    D = dict(R.DECKS); D["ME"] = (role, build(spec), R.auto_extra(spec)); res = {}
    for g in gaunt:
        res[g] = round(R.match("ME", g, n, seed=seed, decks=D)["ME"] / n, 3)
    return sum(res.values()) / len(res), res

def climb(start, role, iters, n, seed, gaunt, tag):
    rng = random.Random(seed); cur = dict(start); best, br = evaluate(cur, role, n, gaunt)
    print(f"start {best:.3f} {br}", flush=True)
    for it in range(iters):
        out = rng.choice([k for k in cur if k not in FIXED])
        pool = NEW_POOL if rng.random() < 0.5 else ALL
        cands = [k for k in pool if cur.get(k, 0) < R.cap(k) and k != out]
        if not cands: continue
        inn = rng.choice(cands)
        cand = dict(cur); cand[out] -= 1
        if not cand[out]: del cand[out]
        cand[inn] = cand.get(inn, 0) + 1
        sc, r = evaluate(cand, role, n, gaunt)
        if sc > best + 0.006:
            cur, best, br = cand, sc, r
            print(f"it{it}: -{out} +{inn} -> {best:.3f} {br}", flush=True)
            json.dump(cur, open(f"coll4/opt_{tag}.json", "w"), indent=1)
    return cur, best, br

if __name__ == "__main__":
    tag, start_name, iters, n, seed = sys.argv[1:6]
    start = {"CURRENT": R.CURRENT_USER}.get(start_name) or json.load(open(start_name))
    gaunt = list(R.DECKS)
    cur, best, br = climb(dict(start), "ctrl", int(iters), int(n), int(seed), gaunt, tag)
    print("FINAL", round(best, 3), br)
    for k, v in sorted(cur.items(), key=lambda x: (-x[1], x[0])): print(f"  {v} {k}")
    print("EXTRA", R.auto_extra(cur))
    json.dump(cur, open(f"coll4/opt_{tag}.json", "w"), indent=1)
