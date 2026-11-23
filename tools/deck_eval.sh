#!/bin/bash
# Usage: tools/deck_eval.sh <deck.json> <games per enemy> [seed base]
# Plays the deck against the six enemy decks, the lookahead Beatdown bot on
# both sides, seeds base+1 to base+6 (default 9100).
# Prints: wins games damage_score | win rate per enemy
# damage_score counts a game won or lost by deck-out as a draw (owner,
# 2026-11-22: the search must find decks that win by damage), as do draws.
# Run from the repository root after `cargo build --release -p ygo-cli`.
deck=$1; n=$2; base=${3:-9100}; k=0; W=0; G=0; D2=0; per=""
for e in data/decks/enemy_e*.json; do
  k=$((k+1))
  out=$(target/release/ygo sim "$deck" "$e" -n "$n" --seed $((base+k)) \
    --bot-a lookahead:beatdown --bot-b lookahead:beatdown --threads 4 2>&1)
  line=$(echo "$out" | grep " overall: A")
  w=$(echo "$line" | sed -E 's/.*\(A ([0-9]+), B.*/\1/')
  draws=$(echo "$line" | sed -E 's/.*draws ([0-9]+),.*/\1/')
  a_out=$(echo "$out" | grep "A wins by" | sed -E 's/.*deck-out ([0-9]+).*/\1/')
  b_out=$(echo "$out" | grep "B wins by" | sed -E 's/.*deck-out ([0-9]+).*/\1/')
  # Twice the damage score: 2 per win by damage, 1 per deck-out or draw.
  D2=$((D2 + 2*(w - a_out) + a_out + b_out + draws))
  W=$((W+w)); G=$((G+n))
  per="$per $(echo "$line" | grep -oE 'A +[0-9.]+%' | grep -oE '[0-9.]+')"
done
echo "$W $G $(awk -v d=$D2 -v g=$G 'BEGIN{printf "%.4f", d/(2*g)}') |$per"
