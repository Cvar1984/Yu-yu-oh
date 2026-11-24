#!/usr/bin/env bash
# One bot against all 14 reference bots, the same deck on both sides:
# random, the six plain heuristic styles, the six lookahead styles, and
# the search bot. One line per opponent in the log.
# Usage: tools/neural_ladder.sh <bot> <deck> <log> [games] [search games] [seed]
# e.g.   tools/neural_ladder.sh neural-search:checkpoints/owner-1.bpk data/decks/save_a_owner_current_40.json ladder.txt
# (needs a release build; a checkpoint path in the bot name is made absolute)
set -eu
bot=$1; deck=$(realpath -m "$2"); log=$(realpath -m "$3")
n=${4:-400}; ns=${5:-400}; seed=${6:-7700}
[ -f "$deck" ] || { echo "no deck at $deck" >&2; exit 1; }
case $bot in
  *:*) kind=${bot%%:*}; arg=${bot#*:}
       if [ "${kind#neural}" != "$kind" ]; then
         arg=$(realpath -m "$arg")
         [ -f "$arg" ] || { echo "no checkpoint at $arg" >&2; exit 1; }
         bot=$kind:$arg
       fi ;;
esac
cd "$(dirname "$0")/.."
: > "$log"
styles="beatdown control stall burn exodia patient"
opps=random
for s in $styles; do opps="$opps heuristic:$s"; done
for s in $styles; do opps="$opps lookahead:$s"; done
for o in $opps search; do
  k=$n; [ "$o" = search ] && k=$ns
  r=$(./target/release/ygo sim "$deck" "$deck" -n "$k" --seed "$seed" --threads 4 \
      --bot-a "$bot" --bot-b "$o" 2>&1 | grep -E "overall" | sed 's/.*overall: A *//')
  echo "$o | $r" >> "$log"
done
echo DONE >> "$log"
