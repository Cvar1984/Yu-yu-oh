"""Six enemy decks from all 440 in-game cards (max 3 copies, the 13-card limited list)."""
import sys, json, random
sys.path.insert(0, ".")
import ygo_sim9 as V9
import ygo_sim2 as S
import run_v8 as R
from ygo_sim2 import Player, C, build, is_mon
from collections import Counter

ALLC = {}
for f in ["Legend_of_Blue_Eyes_White_Dragon", "Metal_Raiders", "Spell_Ruler", "extra66"]:
    for c in json.load(open(f"sets/{f}.json"))["data"]:
        ALLC[c["name"]] = c
FUSIONS = json.load(open("coll4/fusions.json"))
def cap(n): return 1 if n in R.LIMITED else 3
STAPLE_T = {"Mirror Force": 2, "Trap Hole": 2, "Solemn Judgment": 1, "Waboku": 2}
E = {
 "E1 Toon": {"Toon World": 3, "Toon Summoned Skull": 3, "Blue-Eyes Toon Dragon": 2, "Manga Ryu-Ran": 2, "Toon Mermaid": 3,
     "Mystic Tomato": 3, "Giant Rat": 2, "Sangan": 1, "Witch of the Black Forest": 1,
     "Pot of Greed": 1, "Graceful Charity": 3, "Raigeki": 1, "Dark Hole": 1, "Monster Reborn": 1, "Change of Heart": 1,
     "Swords of Revealing Light": 1, "Heavy Storm": 1, "Mystical Space Typhoon": 2, **STAPLE_T, "Solemn Judgment": 2},
 "E2 Special Summon": {"Black Luster Soldier": 2, "Black Luster Ritual": 2, "Relinquished": 1, "Black Illusion Ritual": 1,
     "Crab Turtle": 1, "Turtle Oath": 1, "Senju of the Thousand Hands": 3, "Sonic Bird": 2, "Mystic Tomato": 3,
     "Shining Angel": 3, "Cyber Jar": 1, "Sangan": 1, "Witch of the Black Forest": 1,
     "Monster Reborn": 1, "Premature Burial": 1, "Last Will": 1, "Scapegoat": 1, "Graceful Charity": 3, "Pot of Greed": 1,
     "Raigeki": 1, "Change of Heart": 1, "Swords of Revealing Light": 1, "Mirror Force": 2, "Trap Hole": 2, "Waboku": 2,
     "Solemn Judgment": 1},
 "E3 Removal": {"Man-Eater Bug": 3, "Penguin Soldier": 3, "Sangan": 1, "Witch of the Black Forest": 1, "Jirai Gumo": 3,
     "La Jinn the Mystical Genie of the Lamp": 1, "Summoned Skull": 3, "Cyber Jar": 1,
     "Raigeki": 1, "Dark Hole": 1, "Fissure": 3, "Tribute to the Doomed": 3, "Heavy Storm": 1, "Mystical Space Typhoon": 1,
     "Nobleman of Crossout": 2, "Pot of Greed": 1, "Graceful Charity": 2, "Change of Heart": 1,
     "Raigeki Break": 2, "Trap Hole": 2, "Mirror Force": 3, "Dust Tornado": 1},
 "E4 Big Beatdown": {"Blue-Eyes White Dragon": 3, "Red-Eyes Black Dragon": 2, "Summoned Skull": 3, "Jirai Gumo": 3,
     "La Jinn the Mystical Genie of the Lamp": 1, "7 Colored Fish": 3, "Battle Ox": 2,
     "Axe of Despair": 3, "Megamorph": 2, "Horn of the Unicorn": 2, "Rush Recklessly": 1, "Monster Reborn": 1,
     "Pot of Greed": 1, "Raigeki": 1, "Dark Hole": 1, "Change of Heart": 1, "Swords of Revealing Light": 1,
     "Graceful Charity": 2, **STAPLE_T},
 "E5 DARK + Plasma Zone": {"Mystic Plasma Zone": 3, "Witch's Apprentice": 2, "Dark Elf": 3, "La Jinn the Mystical Genie of the Lamp": 3,
     "Ryu-Kishin Powered": 3, "Whiptail Crow": 3, "Mystic Tomato": 3, "Sangan": 1, "Witch of the Black Forest": 1,
     "Summoned Skull": 3, "Pot of Greed": 1, "Graceful Charity": 2, "Raigeki": 1, "Dark Hole": 1, "Change of Heart": 1,
     "Monster Reborn": 1, "Swords of Revealing Light": 1, **STAPLE_T},
 "E6 Gate Guardian": {"Sanga of the Thunder": 2, "Kazejin": 2, "Suijin": 2, "Gate Guardian": 1, "Jirai Gumo": 3,
     "7 Colored Fish": 3, "La Jinn the Mystical Genie of the Lamp": 3, "Mystic Tomato": 3, "Sangan": 1,
     "Witch of the Black Forest": 1, "Monster Reborn": 1, "Premature Burial": 1, "Pot of Greed": 1, "Graceful Charity": 3,
     "Raigeki": 1, "Dark Hole": 1, "Change of Heart": 1, "Swords of Revealing Light": 1, "Umi": 2, **STAPLE_T},
}
def legal(spec):
    for n, k in spec.items():
        assert n in ALLC and n in C, n
        assert "Fusion" not in ALLC[n]["type"], n
        assert k <= cap(n), (n, k)
    assert sum(spec.values()) == 40, sum(spec.values())
    return build(spec)

def mk(spec): return ("ctrl", legal(spec), [])

def match(da, db, n, seed):
    rng = random.Random(seed); w = Counter()
    for i in range(n):
        pa = Player("A", da[0], da[1], rng); pa.extra = list(da[2])
        pb = Player("B", db[0], db[1], rng); pb.extra = list(db[2])
        d = V9.Duel9(pa, pb, rng) if i % 2 == 0 else V9.Duel9(pb, pa, rng)
        w[d.run()[0]] += 1
    return w["A"] / n

if __name__ == "__main__":
    for k, v in E.items():
        legal(v); print(k, "ok", Counter("M" if is_mon(n) else C[n][0] for n, c in v.items() for _ in range(c)))
