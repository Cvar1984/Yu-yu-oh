"""Deck search over save A's collection: one-card swaps.

Each round screens SWAPS random swaps (one card out, one owned card in,
within the copy and ban list limits; 300 games per enemy deck, seeds
9101-9106), confirms the best 5 against the current deck (1,500 per enemy,
seeds 9301-9306), and keeps the best swap if it gains at least ACCEPT
points. Stops after 2 rounds without a gain or after ROUNDS rounds.
Decks are scored by deck_eval.sh's damage score (owner, 2026-11-22: a
win or loss by deck-out counts as a draw, so a deck must win by damage),
and a deck is never tried if a lock card that stops both players'
monsters would stop most of its own (SELF_LOCKS).
Report a result on other seeds (tools/deck_eval.sh <deck> 2000 <base>).

Usage, from the repository root after `cargo build --release -p ygo-cli`:
  SEED=7 SWAPS=150 ACCEPT=0.75 ROUNDS=15 \
    python3 -I tools/deck_search.py . <start deck.json> <out dir>
The out dir gets log.txt and best.json."""
import json, random, subprocess, sys, os
repo, start, out = sys.argv[1:4]
here = os.path.dirname(os.path.abspath(__file__))  # tools/
cards = {c["id"]: c for c in json.load(open(f"{repo}/data/cards.json"))["cards"]}
coll = {x["id"]: x["copies"] for x in json.load(open(f"{repo}/data/collection.json"))["cards"]}
ban = {x["id"]: x["limit"] for x in json.load(open(f"{repo}/data/banlist.json"))["limits"]}
# The 8 EXTRA cards still unscripted (crates/ygo-cards/tests/registry.rs).
UNSCRIPTED = {2833249, 62867251, 77622396, 70344351, 73915051, 59560625, 68005187, 80604092}
def limit(i): return min(coll.get(i, 0), ban.get(i, 3), 3)

# Lock cards that stop both players' monsters from attacking, with the
# monsters they stop: a deck may not hold one that stops more than half of
# its own monsters (owner, 2026-11-22).
SELF_LOCKS = {
    85742772: lambda c: (c["level"] or 0) >= 4,  # Gravity Bind
    44656491: lambda c: (c["atk"] or 0) >= 1500,  # Messenger of Peace
}

def self_locked(counts):
    monsters = [(cards[i], n) for i, n in counts.items() if n and "Monster" in cards[i]["type"]]
    total = sum(n for _, n in monsters)
    for lock, stops in SELF_LOCKS.items():
        if counts.get(lock, 0) and 2 * sum(n for c, n in monsters if stops(c)) > total:
            return True
    return False
pool = [i for i in coll if cards[i]["type"] != "Fusion Monster" and i not in UNSCRIPTED and limit(i) > 0]
# ADD_ATTRIBUTE=DARK keeps a theme: only monsters of that Attribute (and
# any Spell or Trap) may be added.
theme = os.environ.get("ADD_ATTRIBUTE")
if theme:
    pool = [i for i in pool if "Monster" not in cards[i]["type"] or cards[i]["attribute"] == theme]
deck0 = json.load(open(start))
cur = {e["id"]: e["count"] for e in deck0["main"]}
rng = random.Random(int(os.environ.get("SEED", "7")))

def write(counts, path):
    d = dict(deck0); d["name"] = "craft candidate"
    d["main"] = [{"id": i, "name": cards[i]["name"], "count": n} for i, n in sorted(counts.items()) if n]
    json.dump(d, open(path, "w"), indent=1)

def evaluate(counts, n, base):
    p = f"{out}/cand.json"; write(counts, p)
    r = subprocess.run([f"{here}/deck_eval.sh", p, str(n), str(base)], capture_output=True, text=True).stdout
    return float(r.split("|")[0].split()[2])

def swaps(counts, k):
    seen, res = set(), []
    while len(res) < k:
        rem = rng.choice([i for i, n in counts.items() if n])
        add = rng.choice([i for i in pool if counts.get(i, 0) < limit(i) and i != rem])
        if (rem, add) in seen: continue
        seen.add((rem, add)); c = dict(counts); c[rem] -= 1; c[add] = c.get(add, 0) + 1
        if self_locked(c): continue
        res.append(((rem, add), c))
    return res

log = open(f"{out}/log.txt", "a")
def say(*a):
    print(*a, file=log, flush=True)
base_score = evaluate(cur, 1500, 9300)
say("start", round(100 * base_score, 2))
idle = 0
for rnd in range(1, int(os.environ.get("ROUNDS", "15")) + 1):
    screened = sorted(((evaluate(c, 300, 9100), m, c) for m, c in swaps(cur, int(os.environ.get("SWAPS", "80")))), key=lambda x: -x[0])[:5]
    best = None
    for s, m, c in screened:
        v = evaluate(c, 1500, 9300)
        if best is None or v > best[0]: best = (v, m, c)
    v, (rem, add), c = best
    gain = 100 * (v - base_score)
    say(f"round {rnd}: best swap -{cards[rem]['name']} +{cards[add]['name']}: {100*v:.2f} ({gain:+.2f})")
    if gain >= float(os.environ.get("ACCEPT", "1.0")):
        cur, base_score, idle = c, v, 0
        write(cur, f"{out}/best.json"); say("  accepted")
    else:
        idle += 1
        if idle >= 2: break
write(cur, f"{out}/best.json"); say("done", round(100 * base_score, 2))
