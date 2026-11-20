"""Opening-hand odds from deck composition (Monte Carlo shuffles). 6 cards = opening 5 + first draw; 10 = about turn 5."""
import random
import ygo_sim2 as S
import ygo_sim3 as V3
import ygo_sim8 as V8
from ygo_sim2 import C, is_mon

REMOVAL = {"Raigeki", "Fissure", "Tribute to the Doomed", "Raigeki Break", "Dark Hole", "Change of Heart", "Shadow Spell",
           "Spellbinding Circle", "Mirror Force", "Trap Hole", "Paralyzing Potion", "Hane-Hane", "Penguin Soldier",
           "Man-Eater Bug", "Nobleman of Crossout", "Burst Breath", "Swords of Revealing Light"}

def boost_cards(spec):
    """Cards that raise ATK: fields that help >= half the deck's monsters, generic equips, type equips with >= 3 targets."""
    mons = [n for n, k in spec.items() if is_mon(n) for _ in range(k)]
    out = set()
    for n in spec:
        if n in V3.FIELDS and V3.FIELDS[n]:
            fn, a, _ = V3.FIELDS[n][0]
            if sum(1 for m in mons if fn(m)) * 2 >= len(mons):
                out.add(n)
        elif n in V3.EQUIPS:
            race, a, _ = V3.EQUIPS[n]
            if a > 0 and (race is None or sum(1 for m in mons if V3.RACE.get(m) == race) >= 3):
                out.add(n)
        elif n in ("Stim-Pack", "Malevolent Nuzzler", "Axe of Despair", "Fairy Meteor Crush", *V8.ATTR_EQUIPS, "Reinforcements",
                   "Horn of the Unicorn", "Black Pendant", "Megamorph", "Rush Recklessly"):
            out.add(n)
    return out

def metrics(spec, n=40000, seed=1):
    deck = [c for c, k in spec.items() for _ in range(k)]
    boosts = boost_cards(spec)
    summ = lambda c: is_mon(c) and C[c][1] <= 4 and c not in S.NOMI
    rng = random.Random(seed)
    brick = good = boost10 = rem10 = coh = coh_pair = 0
    for _ in range(n):
        rng.shuffle(deck)
        h6, h10 = deck[:6], deck[:10]
        if not any(summ(c) for c in h6): brick += 1
        if any(summ(c) and C[c][2] >= 1600 for c in h6): good += 1
        if any(c in boosts for c in h10): boost10 += 1
        if any(c in REMOVAL for c in h10): rem10 += 1
        if "Change of Heart" in h10:
            coh += 1
            coh_pair += any(is_mon(c) and C[c][1] >= 5 and c not in S.NOMI for c in h10)
    r = {"no summonable monster in 6": brick / n, "1600+ ATK attacker in 6": good / n,
         "ATK boost by 10 cards": boost10 / n, "removal by 10 cards": rem10 / n,
         "boost cards": sorted(boosts)}
    if coh: r["CoH has a tribute monster (10 cards)"] = coh_pair / coh
    return r

if __name__ == "__main__":
    import json, sys
    for f in sys.argv[1:]:
        print(f, {k: (round(v, 3) if isinstance(v, float) else v) for k, v in metrics(json.load(open(f))).items()})
