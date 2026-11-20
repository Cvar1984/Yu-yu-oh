"""Hill-climb with owned + limited caps, optional theme constraint, auto Extra Deck (sim v7)."""
import sys, csv, json, random
sys.path.insert(0, ".")
import ygo_sim3 as V3
import ygo_sim7 as V7
import ygo_sim2 as S
from ygo_sim2 import Player, build, C
import run_v7 as R

MODELED = set(V3.FIELDS) | set(V3.EQUIPS) | set(V3.BURN) | {
    "Raigeki", "Pot of Greed", "Change of Heart", "Swords of Revealing Light", "Tribute to the Doomed",
    "Paralyzing Potion", "Solemn Judgment", "Fissure", "Stim-Pack", "Spellbinding Circle", "Delinquent Duo",
    "Magic Jammer", "Upstart Goblin", "Monster Reborn", "Polymerization", "Seven Tools of the Bandit",
    "Gravekeeper's Servant", "Toll", "Ring of Magnetism", "Chorus of Sanctuary", "Horn of Light",
    "Red Medicine", "Goblin's Secret Remedy"}
EXCLUDE = {"Boar Soldier", "Cyber Jar", "Wall Shadow", "Crab Turtle", "Hungry Burger", "Performance of Sword",
           "Relinquished", "Kuriboh", "Cocoon of Evolution", "Big Eye", "Ancient One of the Deep Forest",
           "Left Arm of the Forbidden One", "Toon World"}
FIXED = {"Fake Trap", "Suijin"}  # effects not simulated: keep the owner's copies, never judge them
ALL = [n for n, r in R.ROWS.items() if n not in EXCLUDE and n in C and "Fusion" not in r["type"]
       and "Ritual" not in r["type"] and ("Monster" in r["type"] or n in MODELED)]
THEMES = {
    "water": lambda n: not S.is_mon(n) or V3.ATTR.get(n) == "WATER" or n in ("Sangan", "Witch of the Black Forest", "Jirai Gumo", "Mask of Darkness", "Hane-Hane"),
    "stall": lambda n: not S.is_mon(n) or S.is_wall(n) or n in ("Jirai Gumo", "Dark Elf", "Barrel Dragon", "Mask of Darkness", "Hane-Hane", "Sangan", "Witch of the Black Forest", "Labyrinth Wall"),
    "any": lambda n: True,
}

def evaluate(spec, role, n, gaunt, seed=43):
    D = dict(R.DECKS); D["ME"] = (role, build(spec), R.auto_extra(spec)); res = {}
    for g in gaunt:
        res[g] = round(R.match("ME", g, n, seed=seed, decks=D)["ME"] / n, 3)
    return sum(res.values()) / len(res), res

def climb(start, role, theme, iters, n, seed, gaunt):
    rng = random.Random(seed); cur = dict(start); best, br = evaluate(cur, role, n, gaunt)
    pool = [c for c in ALL if THEMES[theme](c) and c not in FIXED]
    print(f"start {best:.3f} {br}", flush=True)
    for it in range(iters):
        out = rng.choice([k for k in cur if k not in FIXED])
        cands = [k for k in pool if cur.get(k, 0) < R.cap(k) and k != out]
        if not cands: continue
        inn = rng.choice(cands)
        cand = dict(cur); cand[out] -= 1
        if not cand[out]: del cand[out]
        cand[inn] = cand.get(inn, 0) + 1
        sc, r = evaluate(cand, role, n, gaunt)
        if sc > best + 0.006:
            cur, best, br = cand, sc, r
            print(f"it{it}: -{out} +{inn} -> {best:.3f}", flush=True)
    return cur, best, br

if __name__ == "__main__":
    tag, start_name, role, theme, iters, n, seed = sys.argv[1:8]
    start = {"WATER": R.WATER_USER, "STALL": R.STALL_USER}.get(start_name) or json.load(open(start_name))
    # remove cards the sim can't play from the start list, replacing them 1-for-1 later in the climb
    start = dict(start)
    for x in list(start):
        if x in EXCLUDE:
            k = start.pop(x)
            fill = next(c for c in ["Fissure", "Tribute to the Doomed", "Paralyzing Potion", "Spellbinding Circle"] if start.get(c, 0) + k <= R.cap(c))
            start[fill] = start.get(fill, 0) + k
    gaunt = ["YOUR WATER (slot 1)", "YOUR STALL-ATTACK (slot 3), control AI", "PREV BEST: Sangan Control (made legal)", "OLD Umiiruka WATER (Nov 16)"]
    cur, best, br = climb(start, role, theme, int(iters), int(n), int(seed), gaunt)
    print("FINAL", round(best, 3), br)
    for k, v in sorted(cur.items(), key=lambda x: (-x[1], x[0])): print(f"  {v} {k}")
    print("EXTRA", R.auto_extra(cur))
    json.dump(cur, open(f"coll3/opt_{tag}.json", "w"), indent=1)
