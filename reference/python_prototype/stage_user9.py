"""Save A decks vs the 6 tuned enemies (sim v9). Climb from a start deck using only owned cards."""
import sys, json, random, glob
sys.path.insert(0, ".")
import enemies as EN, tune_enemy as T, run_v8 as R, ygo_sim9 as V9, ygo_sim2 as S
from ygo_sim2 import C, is_mon
L = lambda f: json.load(open(f))
STILL_UNMODELED = {"Ancient One of the Deep Forest", "Big Eye", "Boar Soldier", "Cocoon of Evolution", "Wall Shadow",
                   "Left Arm of the Forbidden One"}
OK_NOMI = V9.RITUAL_MONS | {"Black Luster Soldier"} | set(V9.TOONS) | {"Gate Guardian", "Harpie Lady Sisters"}
POOL = [n for n, r in R.ROWS.items() if n in C and "Fusion" not in r["type"] and n not in STILL_UNMODELED
        and n not in V9.NOT_SIMULATED and n not in V9.V8.DEAD and n not in S.PIECES
        and ((is_mon(n) and (n not in S.NOMI or n in OK_NOMI)) or (not is_mon(n) and n in T.MODELED))]

def enemies():
    return {json_name.split("_")[-1][:-5]: EN.mk(L(json_name)) for json_name in sorted(glob.glob("coll4/enemy_E*.json"))}

def evaluate(spec, opps, n, seed=55):
    me = R.mk("ctrl", spec)
    res = {k: EN.match(me, o, n, seed) for k, o in opps.items()}
    return sum(res.values()) / len(res), res

if __name__ == "__main__":
    src, tag, iters, n, seed = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
    cur = R.CURRENT_USER if src == "CURRENT" else L(src)
    opps = enemies(); rng = random.Random(seed)
    best, br = evaluate(cur, opps, n); print("start", round(best, 3), br, flush=True)
    for it in range(iters):
        out = rng.choice(list(cur))
        inn = rng.choice([k for k in POOL if cur.get(k, 0) < R.cap(k) and k != out])
        cand = dict(cur); cand[out] -= 1
        if not cand[out]: del cand[out]
        cand[inn] = cand.get(inn, 0) + 1
        if any(m in cand and s not in cand for s, (m, _) in V9.V8.RITUALS.items()):
            continue  # a ritual monster needs its ritual spell
        s, r = evaluate(cand, opps, n)
        if s > best + 0.008:
            cur, best, br = cand, s, r
            print(f"it{it}: -{out} +{inn} -> {best:.3f} {r}", flush=True)
            json.dump(cur, open(f"coll4/v9_{tag}.json", "w"), indent=1)
    json.dump(cur, open(f"coll4/v9_{tag}.json", "w"), indent=1)
    print("FINAL", round(best, 3), br)
