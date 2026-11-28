# M7 plan: the league

Status (2026-11-28): draft, waiting for the owner's approval; M7 starts
once the M6 gate report is written (`docs/m6-plan.md`).

Gate (handoff, owner 2026-11-28): **the ratings table runs unattended for 50
generations.**

Owner's answers (2026-11-28):

- An agent is a deck played by the shared network (`data/networks/best.bpk`);
  its style comes from its deck. Per-style networks can join later from the
  owner's GPU machine.
- Mutated decks use the full 535-card pool under the ban list and deck rules;
  the owner's decks are fixed members that never mutate or leave.
- The gate run is 50 generations.

## Members

| Kind | Who | Mutates | Can be eliminated |
| --- | --- | --- | --- |
| Fixed | The 13 shipped decks: the six enemy decks (the first fixed opponents) and the owner's seven save A/B decks | no | no |
| Population | 13 agents, one seeded from each shipped deck by one card swap; later, mutants of the best | no (each agent's deck is frozen; mutants are new agents) | yes |
| Hall of fame | Past champions: each generation's best population agent joins; at most 8, the oldest then loses its protection | no | no, while in the hall |
| Exploiter | One a generation, built to beat the current leader; at most 2 alive | no | yes |

Every agent keeps a lineage (the shipped deck it descends from), a parent,
its birth generation and its kind. All play with `network` (no search: about
45 games a second on this container's 4 cores; the search bot is 10 times
slower).

## A generation

Agents never change once made, so a pair's games are played once and kept.

1. **Games.** Every pair not yet played plays 20 games (each first in half,
   seeds from the league seed and the pair). Generation 0 plays all 325 pairs
   of the first 26 members (about 3 minutes); later generations only the
   newcomers against everyone (about 2,000 games, about a minute). Scoring:
   a win by Life Points or Exodia counts 1; a deck-out win counts a half like
   a draw (owner, 2026-11-22: a deck should win by playing).
2. **Ratings.** Glicko-2, one rating period per generation, over that
   generation's games. The table ranks by the conservative rating (rating
   minus twice the deviation) and shows each agent's win rate with a 95%
   interval, overall and split by who went first.
3. **Hall of fame.** The best population agent joins the hall if it is not
   there yet.
4. **Elimination.** The two lowest population agents (not fixed, not in the
   hall) leave, but never the last living agent of a lineage, so every style
   stays in the league. Removed agents stay in the file's archive with their
   last rating.
5. **Replacement.** Two mutants of top population agents (each parent drawn
   from the best four): 1 to 3 card swaps in the Main Deck. A swap takes out
   one copy and puts in a card from the pool (half the time another copy of a
   card already in the deck), keeping the deck legal (`check_deck`: 40 to 60
   cards, copy limits, ban list).
6. **Exploiter.** From a random agent of the top half: 6 candidate mutants,
   40 games each against the leader (the best member of any kind); the best
   candidate that beats its parent's score against the leader joins as an
   exploiter. With more than 2 alive, the lowest one leaves.

## Files and commands

- `league/<name>.json` (git-ignored): seed, generation, members (decks,
  lineage, kind, rating, deviation, volatility), every pair's results, the
  hall, the archive. Written after each generation (write then rename), so a
  stopped run resumes where it stopped; the same seed gives the same league.
- `league/<name>.md`: after each generation, its summary and the table.
- `ygo league run --league league/main.json --generations 50 [--games 20]
  [--population 13] [--threads N] [--seed S]`: starts or resumes.
- `ygo league table --league league/main.json [--export <dir>]`: the table;
  `--export` writes the top decks in the `data/decks` format.

## Where the code goes

- `ygo-train::league`: members, Glicko-2, mutation, selection, the league
  file, a generation's steps (games come from a function it is given, so tests
  run it with the random bot and few games).
- `ygo-cli`: `sim`'s game loop split into a quiet `play_match` (also used by
  `sim`) and the report; `ygo league run` and `ygo league table`.

## Tests

- Glicko-2 against Glickman's worked example (1500, RD 200, three games:
  1464.06, RD 151.52, volatility 0.05999).
- Mutation: every mutant is legal under the ban list and deck rules; fixed
  members are never mutated or removed; the last agent of a lineage stays.
- A small league (random bot, 4 games a pair, 3 generations): runs, saves,
  resumes, and gives the same file as an unbroken run with the same seed.

## Steps

1. League library and its tests.
2. `play_match`, `ygo league run` and `ygo league table`; a short run.
3. The gate: 50 generations unattended here; report the table, the top decks
   against the fixed decks, the time per generation, and CLAUDE.md.
