#!/usr/bin/env python3
"""The bot ladder (M6 step 6): one bot (A) against each opponent bot (B),
the same deck on both sides, on each of the decks given, each player first
in half the games. Prints A's win rate per deck and pooled, with 95%
intervals (draws count half).

Usage (release build first):
  tools/ladder.py --bot network --games 250 --seed 777000 \
      --opponent random --opponent neural:checkpoints/owner-2.bpk \
      data/decks/save_a_tuned_40.json data/decks/enemy_*.json
"""
import argparse
import math
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
YGO = ROOT / "target" / "release" / "ygo"
OVERALL = re.compile(r"overall: A\s+[\d.]+% .*\(A (\d+), B (\d+), draws (\d+), n (\d+)\)")


def match(bot, opponent, deck, games, seed, threads):
    out = subprocess.run(
        [str(YGO), "sim", deck, deck, "-n", str(games), "--seed", str(seed),
         "--bot-a", bot, "--bot-b", opponent, "--threads", str(threads)],
        cwd=ROOT, capture_output=True, text=True, check=True).stdout
    m = OVERALL.search(out)
    if not m:
        sys.exit(f"no result line from ygo sim:\n{out}")
    a, b, d, n = map(int, m.groups())
    return a, b, d, n


def rate(a, d, n):
    p = (a + d / 2) / n
    return p, 1.96 * math.sqrt(p * (1 - p) / n)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("decks", nargs="+")
    ap.add_argument("--bot", default="network")
    ap.add_argument("--opponent", action="append", required=True)
    ap.add_argument("--games", type=int, default=250, help="games per deck")
    ap.add_argument("--seed", type=int, default=777000)
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    for opponent in args.opponent:
        print(f"## {args.bot} against {opponent}, {args.games} games a deck, seed {args.seed}")
        print("| Deck | A wins | ±95% |\n| --- | --- | --- |")
        total = [0, 0, 0, 0]
        for deck in args.decks:
            a, b, d, n = match(args.bot, opponent, deck, args.games, args.seed, args.threads)
            total = [x + y for x, y in zip(total, (a, b, d, n))]
            p, e = rate(a, d, n)
            print(f"| {Path(deck).stem} | {p:.1%} | {e:.1%} |", flush=True)
        p, e = rate(total[0], total[2], total[3])
        print(f"| **all {total[3]} games** | **{p:.2%}** | {e:.2%} |\n", flush=True)


if __name__ == "__main__":
    main()
