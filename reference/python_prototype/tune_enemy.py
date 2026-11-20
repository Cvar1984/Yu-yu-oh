"""Climb one enemy deck: its theme cards are fixed, the rest may be any simulated card from all 440."""
import sys, json, random
sys.path.insert(0, ".")
import enemies as EN, opt_v8 as O8, ygo_sim9 as V9, ygo_sim2 as S, run_v8 as R
from ygo_sim2 import C, is_mon
L = lambda f: json.load(open(f))
ENGINE = {"Axe of Despair", "Dark Hole", "Heavy Storm", "Mirror Force", "Mystical Space Typhoon", "Snatch Steal", "Trap Hole",
          "Giant Trunade", "The Forceful Sentry", "Messenger of Peace", "Robbin' Goblin"}
V9_ST = {"Toon World", "Black Pendant", "Horn of the Unicorn", "Megamorph", "Rush Recklessly", "Sogen", "Luminous Spark",
         "Vile Germs", "Final Flame", "Remove Trap", "Stop Defense", "Two-Pronged Attack", "Dragon Capture Jar", "Horn of Heaven",
         "Final Destiny", "Share the Pain", "Gravedigger Ghoul", "Soul Release", "Snake Fang", "The Reliable Guardian",
         "Germ Infection", "Elegant Egotist", *V9.V8.RITUALS}
MODELED = O8.MODELED | ENGINE | V9_ST | {"Confiscation"}
EXCLUDE = O8.EXCLUDE | set(V9.NOT_SIMULATED) | V9.V8.DEAD | set(S.PIECES)
POOL = [n for n, c in EN.ALLC.items() if n in C and "Fusion" not in c["type"] and n not in EXCLUDE and
        ((is_mon(n) and n not in S.NOMI) or (not is_mon(n) and n in MODELED))]
CORE = {"E1 Toon": {"Toon World", *V9.TOONS}, "E2 Special Summon": {"Black Luster Soldier", "Black Luster Ritual", "Relinquished",
        "Black Illusion Ritual", "Crab Turtle", "Turtle Oath", "Senju of the Thousand Hands", "Sonic Bird", "Monster Reborn", "Premature Burial"},
        "E3 Removal": {"Raigeki", "Dark Hole", "Fissure", "Tribute to the Doomed", "Mirror Force", "Trap Hole"},
        "E4 Big Beatdown": {"Blue-Eyes White Dragon", "Red-Eyes Black Dragon", "Summoned Skull", "Axe of Despair", "Megamorph"},
        "E5 DARK + Plasma Zone": {"Mystic Plasma Zone", "Witch's Apprentice", "Dark Elf", "Summoned Skull"},
        "E6 Gate Guardian": {"Gate Guardian", *V9.GG_PIECES}}

def evaluate(spec, opps, n, seed):
    me = EN.mk(spec)
    return sum(EN.match(me, o, n, seed) for o in opps.values()) / len(opps)

if __name__ == "__main__":
    name, iters, n, seed = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
    opps = {k: EN.mk(v) for k, v in EN.E.items() if k != name}
    opps["F2"] = R.mk("ctrl", L("coll4/F2.json")); opps["W2"] = R.mk("ctrl", L("coll3/opt_best_from_water.json"))
    cur = dict(EN.E[name]); rng = random.Random(seed)
    json.dump(cur, open(f"coll4/enemy_{name.split()[0]}.json", "w"), indent=1)
    best = evaluate(cur, opps, n, 77); print("start", round(best, 3), flush=True)
    for it in range(iters):
        out = rng.choice([k for k in cur if k not in CORE[name]])
        inn = rng.choice([k for k in POOL if cur.get(k, 0) < EN.cap(k) and k != out])
        cand = dict(cur); cand[out] -= 1
        if not cand[out]: del cand[out]
        cand[inn] = cand.get(inn, 0) + 1
        s = evaluate(cand, opps, n, 77)
        if s > best + 0.01:
            cur, best = cand, s
            print(f"it{it}: -{out} +{inn} -> {best:.3f}", flush=True)
            json.dump(cur, open(f"coll4/enemy_{name.split()[0]}.json", "w"), indent=1)
    print("FINAL", round(best, 3))
    json.dump(cur, open(f"coll4/enemy_{name.split()[0]}.json", "w"), indent=1)
