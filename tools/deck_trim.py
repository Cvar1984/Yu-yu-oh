#!/usr/bin/env python3
"""Trims a card pool to its best deck, judged by the trained network.

Starting from the pool (a deck file of up to 60 cards), each round tries
removing one copy of every card, scores each candidate against the
opponent decks, and keeps the best removal if it beats the deck as it
stands; it stops at 40 cards or when no removal helps. The network plays
both sides (`ygo sim --bot-a network --bot-b network`); every candidate
meets the same seeds. Score: wins by Life Points or Exodia count 1,
deck-out wins (either side's) and draws count half (owner, 2026-11-22:
a deck should win by playing, not by decking out).

Usage (release build first):
  tools/deck_trim.py <pool.json> <out.json> <opponent decks...> [--games N] [--seed S]
"""
import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
YGO = ROOT / "target" / "release" / "ygo"
MIN_SIZE = 40


def size(deck):
    return sum(e["count"] for e in deck["main"])


def score(deck, opponents, games, seed, tmp):
    """Mean score over the opponents, 0 to 1."""
    path = Path(tmp) / "candidate.json"
    path.write_text(json.dumps(deck))
    total, n = 0.0, 0
    for i, opp in enumerate(opponents):
        out = subprocess.run(
            [str(YGO), "sim", str(path), str(opp), "-n", str(games), "--seed", str(seed + i * 100_000),
             "--threads", "4", "--bot-a", "network", "--bot-b", "network"],
            cwd=ROOT, capture_output=True, text=True, check=True).stdout
        a = re.search(r"A wins by: Life Points (\d+), deck-out (\d+), Exodia (\d+)", out)
        b = re.search(r"B wins by: Life Points (\d+), deck-out (\d+), Exodia (\d+)", out)
        draws = int(re.search(r"overall: .*draws (\d+), n (\d+)", out).group(1))
        lp, a_out, ex = map(int, a.groups())
        b_out = int(b.group(2))
        total += lp + ex + 0.5 * (a_out + b_out + draws)
        n += games
    return total / n


def without_one(deck, index):
    d = json.loads(json.dumps(deck))
    d["main"][index]["count"] -= 1
    d["main"] = [e for e in d["main"] if e["count"] > 0]
    return d


def main():
    p = argparse.ArgumentParser()
    p.add_argument("pool")
    p.add_argument("out")
    p.add_argument("opponents", nargs="+")
    p.add_argument("--games", type=int, default=400, help="games per opponent")
    p.add_argument("--seed", type=int, default=1)
    args = p.parse_args()
    deck = json.loads(Path(args.pool).read_text())
    opponents = [Path(o).resolve() for o in args.opponents]
    with tempfile.TemporaryDirectory() as tmp:
        best = score(deck, opponents, args.games, args.seed, tmp)
        print(f"start: {size(deck)} cards, score {best:.4f}", flush=True)
        while size(deck) > MIN_SIZE:
            trials = []
            for i, e in enumerate(deck["main"]):
                s = score(without_one(deck, i), opponents, args.games, args.seed, tmp)
                trials.append((s, i))
                print(f"  without one {e['name']}: {s:.4f}", flush=True)
            s, i = max(trials)
            if s <= best:
                print(f"no removal helps; stop at {size(deck)} cards", flush=True)
                break
            print(f"remove one {deck['main'][i]['name']}: {best:.4f} -> {s:.4f}", flush=True)
            deck, best = without_one(deck, i), s
    Path(args.out).write_text(json.dumps(deck, indent=1, ensure_ascii=False) + "\n")
    print(f"best: {size(deck)} cards, score {best:.4f}, saved to {args.out}", flush=True)


if __name__ == "__main__":
    sys.exit(main())
