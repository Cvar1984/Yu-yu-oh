import sys, random, re, json, traceback
sys.path.insert(0, ".")
import run_v8 as R, ygo_sim9 as V9, ygo_sim2 as S
from ygo_sim2 import Player, C, is_mon
from collections import Counter
ALLC = {}
for f in ["Legend_of_Blue_Eyes_White_Dragon", "Metal_Raiders", "Spell_Ruler", "extra66"]:
    for c in json.load(open(f"sets/{f}.json"))["data"]: ALLC[c["name"]] = c
pool = [n for n, c in ALLC.items() if n in C and "Fusion" not in c["type"] and n not in S.PIECES]
cap = lambda n: 1 if n in R.LIMITED else 3
viol, crashes = Counter(), 0
class Chk(V9.Duel9):
    def end_phase(self, pl):
        super().end_phase(pl)
        for side in self.p:
            if len(side.mons) > 5: viol["mon zone>5"] += 1
            if len(side.st) > 5: viol["st zone>5"] += 1
            if V9.V8.TOKEN in side.hand + side.gy + side.deck: viol["token off-field"] += 1
        if len(pl.hand) > 6: viol["hand>6"] += 1
        if pl.st_plays > 5: viol["st plays>5"] += 1
rng = random.Random(9)
def rdeck():
    spec = Counter()
    while sum(spec.values()) < 40:
        n = rng.choice(pool)
        if spec[n] < cap(n): spec[n] += 1
    return S.build(dict(spec))
t0 = __import__("time").time()
for g in range(3000):
    a = Player("A", "ctrl" if g % 4 else "stall", rdeck(), rng); b = Player("B", "ctrl", rdeck(), rng)
    a.extra = b.extra = []
    d = Chk(a, b, rng, log=True)
    try: d.run()
    except Exception:
        crashes += 1
        if crashes <= 3: traceback.print_exc()
        continue
    per = Counter()
    for l in d.lines:
        m = re.match(r"^T(\d+) (\S+) (summons |tribute summons |sets a monster|tributes .* to Set)", l)
        if m: per[(m.group(1), m.group(2))] += 1
        for n in S.NOMI:
            if re.search(rf"(?<!Special )summons {re.escape(n)}$", l) and "Ritual" not in l: viol["NOMI normal summoned: " + n] += 1
    viol[">1 normal summon/turn"] += sum(1 for (t, p), v in per.items() if v > 1 and not any(f"T{t} {p} pays 500" in l for l in d.lines))
print("crashes:", crashes, " time", round(__import__("time").time() - t0), "s")
print("violations:", {k: v for k, v in viol.items() if v} or "none")
