"""Nov 19 (2nd update): 385 owned cards incl. the 66 new ones, 13-card limited list, sim v8."""
import sys, csv, json, random
sys.path.insert(0, ".")
import ygo_sim8 as V8
import ygo_sim7 as V7
from ygo_sim2 import Player, build, C
from collections import Counter

ROWS = {r["name"]: r for r in csv.DictReader(open("coll4/collection.csv"))}
OWNED = {n: int(r["copies"]) for n, r in ROWS.items()}
LIMITED = {"Upstart Goblin", "Pot of Greed", "Sangan", "Witch of the Black Forest", "Cyber Jar", "Raigeki",
           "Swords of Revealing Light", "Monster Reborn", "Left Arm of the Forbidden One", "Change of Heart",
           "Delinquent Duo",                       # red "1" badges, owner's screenshot 2026-11-19
           "Confiscation", "Card Destruction"}     # red "1" badges, owner's 'All' list 2026-11-19
def cap(n): return min(3, OWNED.get(n, 0), 1 if n in LIMITED else 3)

def auto_extra(spec):
    if "Polymerization" not in spec:
        return []
    ex = []
    for f, mats in V7.FUSIONS.items():
        if OWNED.get(f) and all(spec.get(m, 0) >= mats.count(m) for m in set(mats)):
            ex += [f] * min(OWNED[f], 3)
    return sorted(ex, key=lambda f: -C[f][2])[:15]

def legal(spec, extra=()):
    for n, k in spec.items():
        assert n in C, n
        assert "Fusion" not in ROWS.get(n, {"type": ""})["type"], f"{n} is a Fusion Monster (Extra Deck only)"
        assert k <= cap(n), f"{n}: {k} > allowed {cap(n)}"
    assert 40 <= sum(spec.values()) <= 60 and len(extra) <= 15
    return build(spec)

def mk(role, spec, extra=None):
    extra = auto_extra(spec) if extra is None else extra
    return (role, legal(spec, extra), list(extra))

CURRENT_USER = {"Monster Reborn": 1, "Summoned Skull": 1, "Change of Heart": 1, "Solemn Judgment": 1, "Magic Jammer": 1,
    "Spellbinding Circle": 1, "Delinquent Duo": 1, "Raigeki": 1, "Swords of Revealing Light": 1, "Tribute to the Doomed": 2,
    "Penguin Soldier": 1, "Scapegoat": 1, "Shadow Spell": 1, "Fissure": 1, "Pot of Greed": 1, "Sangan": 1, "Dark Elf": 2,
    "Witch of the Black Forest": 1, "Armored Lizard": 3, "Kojikocy": 3, "Jirai Gumo": 3, "7 Colored Fish": 3,
    "Blue-Winged Crown": 2, "Paralyzing Potion": 2, "Stim-Pack": 3, "Guardian of the Throne Room": 1}
assert sum(CURRENT_USER.values()) == 40
L = lambda f: json.load(open(f))
DECKS = {
    "YOUR CURRENT (screenshot)": mk("ctrl", CURRENT_USER),
    "P0 Sangan Control (my last pick)": mk("ctrl", L("coll3/prev_legal.json")),
    "W2 WATER improved": mk("ctrl", L("coll3/opt_best_from_water.json")),
    "S1 STALL improved": mk("ctrl", L("coll3/opt_stall.json")),
}

def players(rng, a, b, decks):
    pa = Player(a, decks[a][0], decks[a][1], rng); pa.extra = list(decks[a][2])
    pb = Player(b, decks[b][0], decks[b][1], rng); pb.extra = list(decks[b][2])
    return pa, pb

def match(a, b, n, seed=31, decks=None):
    decks = decks or DECKS
    rng = random.Random(seed); w = Counter()
    for i in range(n):
        pa, pb = players(rng, a, b, decks)
        d = V8.Duel8(pa, pb, rng) if i % 2 == 0 else V8.Duel8(pb, pa, rng)
        w[d.run()[0]] += 1
    return w
