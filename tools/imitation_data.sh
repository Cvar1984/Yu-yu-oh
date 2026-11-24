#!/usr/bin/env bash
# Imitation data (M6 step 4): lookahead games (mirrors and cross pairs),
# then search games, recorded for `ygo imitate`.
# Usage: tools/imitation_data.sh <out dir> <log file>
set -eu
out=$(realpath -m "$1"); log=$(realpath -m "$2")
cd "$(dirname "$0")/.."
mkdir -p $out/lookahead $out/search
: > $log
d=data/decks
mirrors="enemy_e1_toon enemy_e2_special_summon enemy_e3_removal enemy_e4_big_beatdown enemy_e5_dark_plasma_zone enemy_e6_gate_guardian"
seed=100
for m in $mirrors; do
  ./target/release/ygo sim $d/$m.json $d/$m.json -n 500 --seed $seed --threads 4 --bot-a lookahead:beatdown --bot-b lookahead:control --record $out/lookahead 2>&1 | grep -E "overall|speed" | sed "s/^/$m: /" >> $log
  seed=$((seed+1000))
done
for pair in "e1_toon e2_special_summon" "e3_removal e4_big_beatdown" "e5_dark_plasma_zone e6_gate_guardian" "e2_special_summon e3_removal" "e4_big_beatdown e5_dark_plasma_zone" "e6_gate_guardian e1_toon"; do
  set -- $pair
  ./target/release/ygo sim $d/enemy_$1.json $d/enemy_$2.json -n 300 --seed $seed --threads 4 --bot-a lookahead:control --bot-b lookahead:beatdown --record $out/lookahead 2>&1 | grep -E "overall|speed" | sed "s/^/$1-$2: /" >> $log
  seed=$((seed+1000))
done
for m in $mirrors; do
  ./target/release/ygo sim $d/$m.json $d/$m.json -n 200 --seed $seed --threads 4 --bot-a search --bot-b search:control --record $out/search 2>&1 | grep -E "overall|speed" | sed "s/^/search $m: /" >> $log
  seed=$((seed+1000))
done
echo DONE >> $log
