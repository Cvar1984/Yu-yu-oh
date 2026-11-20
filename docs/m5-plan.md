# M5 plan: search bot (information-set MCTS)

Gate (from the handoff): the search bot beats every heuristic bot over at
least 1,000 games. Read as: for each of the six styles, 1,000 games or
more, half with each player first, and a 95% interval for the search
bot's win rate entirely above 50%.

M4 gate: passed, the owner played full games (owner, 2026-11-20).

## Scope

| In M5 | Later |
| --- | --- |
| Sampling the cards a player cannot see | Inferring an unknown deck list |
| Search bot: information-set MCTS with heuristic playouts | Learned policy and value guiding the search (M6) |
| Search settings: playouts, thinking time, horizon | League and ratings (M7) |
| `ygo sim --threads`, `ygo play --think` | |

## Design

**What a bot gets.** `Player::choose` now takes a `PlayerView` instead of
a bare observation. The view holds the decider's observation and the
legal actions, and keeps the real game private: the only way to play
forward is `PlayerView::sample(seed)`, which returns a whole `Game` that
looks exactly the same to the decider, with every card they cannot see
dealt again at random and a fresh random generator (so coin tosses and
shuffles to come are unknown too). The rule "bots see only what their
player sees" still holds by type.

**Sampling.** A card is hidden from the decider when their observation
does not show its identity: the opponent's hand and face-down cards,
both Decks (the decider does not know their own Deck order either), the
opponent's Extra Deck. Each player's hidden cards are redealt among that
player's own hidden places, each place taking only what may sit there: a
monster to a face-down Monster Zone, a Spell or Trap to a Spell & Trap
Zone, a Field Spell to the Field Zone, Fusion Monsters to the Extra Deck,
anything else to the hand and Deck. Instances keep their place, position
and history; only their card changes. A deal that would give a hidden
hand all of Exodia is drawn again, since that player would have won.

The sample uses the real decks' cards, so **the bot knows the opponent's
deck list** (which cards, not where they are). This is the standard
assumption for this kind of search; inferring a deck list from what has
been seen is left for later. Cards that were shown once and went back
into hiding (a card added to the hand from the Deck) are forgotten.

**Search.** Single-observer information-set MCTS (Cowling, Powley and
Whitehouse, 2012). Each iteration takes a new sample, walks one shared
tree using only the moves legal in that sample (UCB1, with how often a
move was available in place of the parent's visits), adds one new move,
plays on with the heuristic bot on both sides, scores the result, and
credits it to every move on the way, each from its mover's side. Forced
moves (a single legal action) take no place in the tree. The bot plays
the most visited first move.

**Playouts and scoring.** Whole-game playouts are slow (about 30 turns)
and noisy. A playout instead stops at the start of the turn
`horizon_turns` after the one being decided (default 2: the rest of this
turn and the opponent's next one) and scores the position for player 0:
a finished game is 1, 0 or 0.5; an unfinished one is
`1 / (1 + exp(-diff / 40))`, where `diff` is the difference in Life
Points (50 LP = 1 point) plus card worth from `config/card_values.toml`
(cards on the field in full, a monster's ATK boosts included, cards in
the hand at half). The playout bot is the heuristic bot of a named style
(`search` uses Beatdown, `search:<style>` another).

**Settings.** `SearchSettings`: `iterations` (playouts per decision,
default 200), `time_limit` (stop a decision early; never used in the
browser, which has no clock), `exploration` (0.7), `horizon_turns` (2),
`max_playout_actions`, and the scoring weights `lp_per_point` (50),
`hand_worth` (0.5) and `score_scale` (40). They are bot settings, not
rules. Command-line flags on `ygo sim` and `ygo play`: `--iterations`,
`--think <seconds>` (with no `--iterations`, the bot uses the whole
time) and `--horizon`. The browser game offers `search` with the
defaults; with a playout count and no clock the search is
deterministic, so a resumed game replays the bot's choices exactly.

**Evaluation.** `ygo sim --threads N` plays N games at once; each game
depends only on its seed, so the numbers are the same for any N.

## Steps

1. **Sampling**: `PlayerView`, `sample`, the `Player` trait change.
   Tests: over random games with every card script, at every decision,
   each sample shows the decider the same observation and legal actions,
   keeps every card with its owner, and puts only fitting cards in each
   place; samples vary with the seed and repeat with it.
2. **Search bot**: `SearchPlayer`, `search[:<style>]` in `Bots`, settings
   flags, `ygo sim --threads`.
3. **Tuning**: horizon, playouts, exploration, scoring, measured with
   `ygo sim` against Beatdown.
4. **Gate run**: against every style, 1,000 games each.
5. **Play**: `ygo play --think`, the search bot in the browser game.
6. **Docs**: CLAUDE.md, this plan's progress, a gate report.

## Progress

| Step | Result |
| --- | --- |
| 1. Sampling | `PlayerView`, `sample`; `Player::choose(&PlayerView)`; `tests/view.rs` checks every decision of 30 random games |
| 2. Search bot | `SearchPlayer`, `search[:<style>]`, `--iterations`, `--think`, `--horizon`, `ygo sim --threads` |
| 3. Tuning | Below |
| 4. Gate run | Passed, below |
| 5. Play | `ygo play --bot search [--think S]`; the browser game offers `search` and resumes it exactly (`a_game_resumes_from_its_choices`) |
| 6. Docs | CLAUDE.md, this report |

## Tuning (2026-11-20)

Search against Beatdown, owner's deck mirror, 95% intervals. Playouts to
the end of the game first, then cut at a horizon and scored:

| Setting | Games | Search wins |
| --- | --- | --- |
| Whole-game playouts, 100 per decision | 100 | 56.0% ±9.7 |
| Horizon 1, 200 playouts | 100 | 51.0% ±9.8 |
| Horizon 2, 200 playouts | 100 | 65.0% ±9.3 |
| Horizon 3, 200 playouts | 200 | 63.0% ±6.7 |
| Horizon 4, 200 playouts | 200 | 69.5% ±6.4 |
| Horizon 2, 500 playouts | 200 | 63.5% ±6.7 |
| Horizon 3, 500 playouts | 200 | 69.5% ±6.4 |

More playouts or a longer horizon helped little, so the scoring was
tuned next (horizon 2, 200 playouts, 400 games each, `--seed 100`):

| Scoring | Search wins |
| --- | --- |
| 100 LP a point, hand 0.5, scale 20, exploration 0.7 | 63.7% ±4.7 |
| 50 LP a point | 69.0% ±4.5 |
| 200 LP a point | 55.0% ±4.9 |
| hand 0.25 / 0.8 | 64.5% / 53.5% |
| scale 10 / 40 | 57.8% / 62.5% |
| exploration 0.3 / 1.2 | 64.0% / 59.0% |
| 25 LP a point | 70.8% ±4.5 |
| 50 LP a point, hand 0.25 | 68.2% ±4.6 |
| **50 LP a point, scale 40** (chosen) | 72.2% ±4.4 |
| 50 LP a point, the heuristic's favourite move expanded first | 69.5% ±4.5 |
| 50 LP a point, exploration 0.4 | 69.8% ±4.5 |

Life Points matter about twice as much as the first guess; the rest is
within noise. Expanding the heuristic's favourite move first gained
nothing, so it was left out.

## Gate report (2026-11-20)

Gate: the search bot beats every heuristic bot over at least 1,000 games.
**Passed.** Default settings (200 playouts, horizon 2), owner's deck
mirror, 1,000 games per style, each player first in half, `--seed 5000`
(fresh seeds, not the ones tuned on), 95% intervals:

```sh
# owner_main_45.json was removed as stale on 2026-11-20; it is in git history.
ygo sim data/decks/owner_main_45.json data/decks/owner_main_45.json \
    --bot-a search --bot-b heuristic:<style> -n 1000 --threads 4 --seed 5000
```

| Opponent | Search wins | Search first | Search second | Mean turns |
| --- | --- | --- | --- | --- |
| beatdown | 72.1% ±2.8 | 72.2% ±3.9 | 72.0% ±3.9 | 27.1 |
| control | 75.7% ±2.7 | 77.2% ±3.7 | 74.2% ±3.8 | 29.1 |
| stall | 77.7% ±2.6 | 77.4% ±3.7 | 78.0% ±3.6 | 31.3 |
| burn | 76.2% ±2.6 | 76.4% ±3.7 | 76.0% ±3.7 | 28.6 |
| exodia | 78.8% ±2.5 | 79.6% ±3.5 | 78.0% ±3.6 | 32.3 |
| patient | 76.9% ±2.6 | 79.6% ±3.5 | 74.2% ±3.8 | 29.0 |

The lowest bound of any interval is 69.3%. A check on another deck:
Umiiruka WATER mirror against Beatdown, 400 games, `--seed 9000`: 75.5%
±4.2. No draws anywhere.

| Check | Evidence |
| --- | --- |
| Samples never reveal a hidden card | `tests/view.rs`: identical observation and legal actions at every decision; cards stay with their owner and fit their place |
| The search is repeatable | Same seed, same analysis; `ygo sim` gives the same numbers for any `--threads` |
| Games with the search bot replay | `tests/search.rs`, `ygo-cli` `tests/play.rs` (a human against `search`), the browser resume test |
| It beats its own playout bot where it plays naively | `keeps_dark_hole_while_its_own_monster_rules_the_field` |
| Tests | 332 in the workspace |

Speed (release, the owner's laptop, Intel i3-10110U: 2 cores, 4
threads): search against Beatdown about 3 games/s on one thread (about
0.3 s of thinking per game) and 5 with 4 threads.

What it does not do yet: it knows the opponent's deck list (see
Sampling); its scoring ignores the Graveyard and the Deck, and it loses
more games by deck-out than it wins that way (13 to 34 per 1,000 against
4 to 10); the playouts are only as good as the heuristic bot. M6's learned value and policy are the planned answer
to the last two.
