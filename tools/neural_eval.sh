#!/usr/bin/env bash
# Strength of a checkpoint: against the plain heuristic bot and the
# lookahead bot in each enemy mirror; the lookahead bot against the plain
# one on the same seeds, for comparison.
# Usage: tools/neural_eval.sh <checkpoint> <log file> (needs a release build)
set -eu
ckpt=$(realpath -m "$1"); log=$(realpath -m "$2")
[ -f "$ckpt" ] || { echo "no checkpoint at $ckpt" >&2; exit 1; }
cd "$(dirname "$0")/.."
: > $log
for m in enemy_e1_toon enemy_e2_special_summon enemy_e3_removal enemy_e4_big_beatdown enemy_e5_dark_plasma_zone enemy_e6_gate_guardian; do
  d=data/decks/$m.json
  for pair in "neural:$ckpt heuristic:beatdown" "neural:$ckpt lookahead:beatdown" "lookahead:beatdown heuristic:beatdown"; do
    set -- $pair
    r=$(./target/release/ygo sim $d $d -n 500 --seed 90000 --threads 4 --bot-a $1 --bot-b $2 2>&1 | grep -E "overall" | sed 's/.*overall: A *//')
    echo "$m | ${1%%:*} vs $2 | $r" >> $log
  done
done
echo DONE >> $log
