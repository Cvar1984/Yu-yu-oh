# M7 plan: the league

Status (2026-11-28): approved by the owner (built as planned); M6 gate
passed (`docs/m6-plan.md`). Steps 1 to 3 done; the gate run passed (report
below). Open: the population collapsed onto one lineage (below), for the
owner to decide on before M8.

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
4. **Elimination.** The two lowest population agents (not in the hall)
   leave. Every style stays in the league through its fixed deck. Removed
   agents stay in the file's archive with their last rating. (Changed while
   building, 2026-11-28: the planned "never the last agent of a lineage"
   rule protected every agent of a population seeded one per lineage, so
   nobody could ever leave.)
5. **Replacement.** Two mutants of top agents (each parent drawn from the
   four best agents of different lineages, so the population does not
   collapse onto one style): 1 to 3 card swaps in the Main Deck. A swap takes out
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

## Gate report (2026-11-28)

`ygo league run --league league/main.json --generations 50 --threads 4
--seed 1`, one process, nobody touching it: **50 generations, no error, 23.5
minutes** (generation 0, all 325 pairs: 114 s; later ones 19 to 35 s, 26 s
on average). 80,280 league games plus 14,000 in the exploiter searches; 165
agents made, 4,014 pairs played; the league file saved after every
generation. **The gate passes.**

The table after 50 generations (top and the fixed decks; score is against
the whole league, 95% intervals):

| # | Agent | Kind | Rating ±2 dev. | Score | First | Second |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | DARK Plasma #161 (hall) | population | 1547 ±31 | 63.3% ±3.9 | 69.3% | 57.2% |
| 2 | DARK Plasma #154 | population | 1550 ±40 | 61.8% ±3.7 | 67.6% | 55.9% |
| 3 | DARK Plasma #150 (hall) | population | 1554 ±45 | 60.6% ±3.4 | 66.8% | 54.5% |
| 15 | enemy E6 Gate Guardian | fixed | 1482 ±53 | 54.9% ±1.7 | 57.1% | 52.7% |
| 16 | save_a_beatdown_40 | fixed | 1437 ±53 | 49.0% ±1.7 | 54.4% | 43.7% |
| 17 | enemy E5 DARK Plasma | fixed | 1434 ±54 | 53.4% ±1.7 | 57.4% | 49.3% |
| 18 | save_a_tuned_40 | fixed | 1402 ±54 | 48.4% ±1.7 | 53.2% | 43.6% |
| 19 | enemy E2 Special Summon | fixed | 1373 ±53 | 43.0% ±1.7 | 47.1% | 38.9% |
| 20 | save_a_control_no_tribute_40 | fixed | 1367 ±53 | 45.2% ±1.7 | 49.2% | 41.2% |
| 21 | save_a_best_overall_40 | fixed | 1354 ±54 | 44.1% ±1.7 | 50.9% | 37.3% |
| 22 | save_a_crafted_40 | fixed | 1344 ±54 | 43.0% ±1.7 | 48.8% | 37.1% |
| 23 | enemy E3 Removal | fixed | 1297 ±55 | 39.7% ±1.7 | 40.0% | 39.3% |
| 24 | save_a_owner_current_40 | fixed | 1268 ±55 | 35.9% ±1.7 | 41.2% | 30.6% |
| 25 | enemy E4 Big Beatdown | fixed | 1255 ±56 | 34.1% ±1.6 | 37.5% | 30.8% |
| 26 | enemy E1 Toon | fixed | 1247 ±56 | 29.5% ±1.6 | 32.2% | 26.9% |
| 27 | save_b_dark_plasma_40 | fixed | 969 ±67 | 12.8% ±1.2 | 14.3% | 11.3% |

What the run shows:

- **The league finds stronger decks.** The best mutants score 72.7% ±5.4
  against the 13 fixed decks (260 games each); the first champion (#17,
  generation 0) scored 68.8%. Against the best four, the owner's decks score
  31.2% ±10.2 (`save_a_tuned_40`), 36.2% (`save_a_beatdown_40`) and 21.2%
  (`save_a_owner_current_40`). The top deck (#161) is the E5 DARK Plasma
  enemy deck with seven swaps: out Dark Elf, Mystic Plasma Zone, Whiptail
  Crow, Ryu-Kishin Powered, Dark Hole, Shadow Spell, Snatch Steal; in
  Hyosube, Trap Hole, Mirror Force, Princess of Tsurugi, Firegrass, Dust
  Tornado, Graceful Charity.
- **Going first is worth about 5 to 12 points** for every deck.
- **The hall and exploiters work as built**: 8 champions in the hall (the
  oldest still playing), 39 of 50 generations made an exploiter that beat
  its parent against the leader, and three exploiters reached the top 12.
- **The population collapsed onto one lineage.** Every living mutable agent
  descends from the E5 DARK Plasma deck. The other lineages died out
  early: Toon and save B in generation 0, the other save A decks by
  generation 19, Gate Guardian by 46. Every style is still in the league
  through its fixed deck, but only DARK Plasma is being improved. The
  "four best agents of different lineages" rule for parents cannot hold a
  lineage that has no living member. Ways to keep styles evolving (for the
  owner): a quota of at least one living agent per lineage, with the
  population seeded two per lineage; or leave it to M8's per-style search
  (MAP-Elites), which is built for exactly this.
- Ratings settle at a deviation of about ±53 (two deviations) for veterans:
  each period widens it slightly and new games narrow it again. Agents born
  in the last generation show 0 games and ±700 until they play.
