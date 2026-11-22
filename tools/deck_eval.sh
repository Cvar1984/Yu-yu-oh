#!/bin/bash
# Usage: tools/deck_eval.sh <deck.json> <games per enemy> [seed base]
# Plays the deck against the six enemy decks, the lookahead Beatdown bot on
# both sides, seeds base+1 to base+6 (default 9100). Prints the deck's
# wins, the games, and its win rate against each enemy deck.
# Run from the repository root after `cargo build --release -p ygo-cli`.
deck=$1; n=$2; base=${3:-9100}; k=0; W=0; G=0; per=""
for e in data/decks/enemy_e*.json; do
  k=$((k+1))
  line=$(target/release/ygo sim "$deck" "$e" -n "$n" --seed $((base+k)) \
    --bot-a lookahead:beatdown --bot-b lookahead:beatdown --threads 4 2>&1 | grep " overall: A")
  w=$(echo "$line" | sed -E 's/.*\(A ([0-9]+), B.*/\1/')
  W=$((W+w)); G=$((G+n))
  per="$per $(echo "$line" | grep -oE 'A +[0-9.]+%' | grep -oE '[0-9.]+')"
done
echo "$W $G |$per"
