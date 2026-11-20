# M4 plan: CLI play, heuristic bots, game records, replay

Gate (from the handoff): the owner plays a full game in the terminal.

## Scope

| In M4 | Later |
| --- | --- |
| Game record format, writer, reader, exact replay | Training data pipeline (M6) |
| `ygo play`: a human against a bot in the terminal | Browser UI (M9) |
| Heuristic bots, one per playstyle, styles as config | Search (M5), learned policy (M6) |
| `ygo sim` with any two bots; `ygo replay` | League and ratings (M7) |

## Design

**Game records.** One JSON Lines file per game. Line 1 is a header:
`schema_version`, the engine version, the full `Rules`, the `GameSetup`
(decks, seed, first player, shuffle, turn cap) and a label per player
(`human`, `random`, `heuristic:beatdown`). Then one line per action:
`{ "turn", "player", "action" }`. The last line is the result. Replaying
rebuilds the game from the header and applies each action, failing
loudly if one is illegal, so a record also checks itself. Because the
header carries the rules, a game replays the same after `rules.toml`
changes. The types live in `ygo-engine` (serde only, no I/O); reading
and writing files lives in `ygo-cli`. Records go to `records/`, which
is git-ignored.

**Text rendering.** Two functions: one draws the board from an
`Observation` (LP, hand, both fields with current ATK/DEF, Graveyard
tops and counts, the chain, the open window, revealed cards), one turns
an `Action` into a sentence with card names ("Tribute Summon Summoned
Skull, tributing Mystical Elf"). Both read only the observation, so a
human never sees a hidden card. They live in `ygo-cli` until the server
needs them too.

**`ygo play`.** A full-screen terminal UI (`ratatui` on `crossterm`):
panels for the opponent's field, your field, your hand, the chain and
open window, a log of recent events, and the list of legal actions.
Arrow keys or numbers select an action, Enter plays it, `c` shows the
selected card's full text, `q` quits. The screen is a thin layer over a
`PlaySession` that holds the game, the bot and the record, so tests
drive the session directly and check the drawn screen with ratatui's
test backend, without a real terminal.

```text
ygo play --deck data/decks/owner_main_45.json [--bot heuristic:beatdown] \
         [--bot-deck <file>] [--seed S] [--first me|bot|random] [--record <file>]
```

The default bot is `heuristic:beatdown`. Every game is recorded.

**Heuristic bots.** One `HeuristicPlayer`, parameterised by a style, so
six playstyles are six configurations, not six programs. It scores each
legal action from the observation and plays the best, breaking ties
with its seeded RNG. The scoring follows the prototype's rules of
thumb, generalised from card names to what the engine already knows:

- Summon or Set the monster that best fits the board: attack if it
  beats the strongest known opposing monster, otherwise Set the best
  defender; Tribute Summon only when the new monster is worth more than
  what it costs.
- Attack the strongest monster it can beat, attack directly when it
  can, attack a face-down monster only with enough ATK, never attack
  into a known loss.
- Activate removal when the opponent has something worth removing,
  draw cards freely, keep Traps for their windows.
- Respond with Traps when the window is worth it (a counter trap only
  against a big enough summon or Spell, by style).
- For choices: search or revive the best card, destroy the opponent's
  best, give the opponent the worst when they choose for us.
- Discard the lowest-value card for the hand limit.

Card knowledge comes from two config files, not code:
`config/styles.toml` (weights per style: aggression, caution, how much
Life Points matter, when to Tribute Summon, when to spend a counter
trap) and `config/card_values.toml` (a value per card id, seeded from
the prototype's table, with defaults by card kind and stats for the
rest). Styles: beatdown, control, stall, burn, exodia, and patient (the
owner's: big Tribute Summons only behind a set Solemn Judgment or
similar).

## Steps

1. **Game records**: types, JSON Lines writer and reader, replay with
   legality checks. Test: games from `ygo sim` replay to equal final
   states; a tampered record fails.
2. **Text rendering**: board and action sentences. Tests: known states
   render as expected; a render for one player never contains the name
   of a card hidden from them (fuzzed over random games).
3. **`ygo play`**: `PlaySession`, the full-screen UI, card text,
   recording. Tests: a full game driven through `PlaySession` runs to
   the end and leaves a record that replays; the screen drawn on the
   test backend shows the board and never a hidden card.
4. **Heuristic bot and styles**: `HeuristicPlayer`, the two config
   files, loading and validation in `ygo-data`. Tests: one per rule of
   thumb above, in stacked scenarios.
5. **Bot evaluation**: `ygo sim --bot-a <bot> --bot-b <bot>`. Every
   style must beat the random bot over 2,000 games with a 95% interval
   above 50%, which also sanity-checks the engine, as the handoff asks.
6. **`ygo replay`**: step through a record from either player's view
   or with everything shown; jump to a turn.
7. **Docs and gate**: CLAUDE.md, this plan's progress, a gate report.
   The gate is yours: you play a full game with `ygo play`.

## Owner's answers (2026-11-17)

- `ygo play` is a full-screen terminal UI.
- Card values and style weights live in config files.
- The default bot is `heuristic:beatdown`.

## Progress

| Step | Commit | Result |
| --- | --- | --- |
| 1. Game records | `95d0a0e` | JSON Lines writer, reader, replay under the record's own rules; tampered records fail |
| 2. Text rendering | `19f69eb` | Board and action sentences from the observation; fuzzed: nothing hidden is ever named |
| 3. `ygo play` | `737c859` | `PlaySession` and a full-screen ratatui UI; games replay from their record |
| 4. Heuristic bot | `922e44b` | `HeuristicPlayer`, six styles, `config/styles.toml` and `config/card_values.toml` |
| 5. Bot evaluation | `dca13f8` | `ygo sim --bot-a/--bot-b`; every style beats random (below) |
| 6. `ygo replay` | `bc96bf2` | Step through a record from either view or both; jump to a turn |
| 7. Docs and gate | this commit | CLAUDE.md, this report |

Found and fixed on the way:

- Engine: a monster being summoned face-up was shown to the opponent as
  a hidden hand card during its summon window, so a player deciding on
  Solemn Judgment could not see what was being summoned. It is now
  visible from its declaration; a Set stays hidden.
- Bot: it Set Exodia pieces on the field, where they no longer count.
  With the Exodia win on, pieces are never Summoned or Set (by engine
  tag). The Exodia style on the Exodia deck went from 54.9% to 93.0%
  against random.

## Gate report (2026-11-17)

Gate: the owner plays a full game with `ygo play`. That part is yours;
everything that can be checked without you is below.

```sh
# owner_main_45.json was removed as stale on 2026-11-20; save_a_owner_current_40.json replaces it.
cargo run --release -p ygo-cli -- play --deck data/decks/owner_main_45.json
cargo run --release -p ygo-cli -- replay records/human/<file>.jsonl --view all
```

| Check | Evidence |
| --- | --- |
| A whole game through `ygo play` | `a_human_plays_whole_games_and_the_record_replays` (6 games, random and Beatdown opponents); the real binary was driven in a pseudo-terminal: it draws, plays, quits and saves a record that parses |
| The screen never shows a hidden card | `the_screen_shows_the_board_and_never_a_hidden_card`, the describe fuzz and the replay view test |
| Records replay exactly | Records from `play`, `sim` and the bot tests replay to equal final states; tampered records are refused |
| Every style beats random | 24 runs below, lowest 88.2% ±1.4 |
| Tests | 301 in the workspace, `scripts/check.sh` clean |
| Speed | Random games 1,870 to 2,580 games/s (350,000 to 400,000 actions/s); Beatdown against itself 3,777 games/s; one core, release |

Style against the random bot, 2,000 games each (half with each player
first), both players on the same deck, `--seed 1`, 95% intervals:

| Deck | beatdown | control | stall | burn | exodia | patient |
| --- | --- | --- | --- | --- | --- | --- |
| Owner's main 45 | 96.0 ±0.9 | 95.5 ±0.9 | 95.5 ±0.9 | 95.5 ±0.9 | 95.5 ±0.9 | 95.4 ±0.9 |
| Umiiruka WATER 40 | 95.2 ±0.9 | 93.5 ±1.1 | 92.0 ±1.2 | 94.3 ±1.0 | 91.8 ±1.2 | 91.4 ±1.2 |
| Exodia stall 40 | 89.4 ±1.3 | 88.2 ±1.4 | 88.7 ±1.4 | 88.6 ±1.4 | 93.0 ±1.1 | 88.4 ±1.4 |
| Defense-tuned 45 | 98.0 ±0.6 | 98.2 ±0.6 | 98.9 ±0.5 | 98.2 ±0.6 | 98.6 ±0.5 | 98.6 ±0.5 |

Styles against Beatdown, owner's deck mirror, 2,000 games, `--seed 7`:
control 43.5%, stall 44.4%, burn 44.6%, exodia 43.6%, patient 41.9%
(each ±2.2). Beatdown against itself: 48.6% ±2.2.

Reading these numbers: every style is far above the bar, so the bots
and the engine agree on what good play is. The styles do play
differently (on the owner's deck, mean game length runs from 18.8 turns
for Beatdown to 22.7 for Exodia), but on these decks none beats
Beatdown yet; the weights in `config/styles.toml` are a
first guess, and tuning them is now a config change measured with
`ygo sim`. Only the Exodia style is clearly best on its own deck. The
heuristics read card values, not card effects, so a card whose worth
depends on timing (Dark Hole with your own monsters out) is still
played naively; search in M5 is the planned answer.

### After the modern-rules work (2026-11-19)

Rerun after the three batches in `docs/card-audit.md`, same commands
(`--seed 1`, 2,000 games, same deck both sides, against the random bot).
The Exodia stall deck was removed by the owner on 2026-11-18.

| Deck | beatdown | control | stall | burn | exodia | patient |
| --- | --- | --- | --- | --- | --- | --- |
| Owner's main 45 | 96.7 ±0.8 | 95.9 ±0.9 | 96.7 ±0.8 | 96.5 ±0.8 | 96.0 ±0.9 | 96.5 ±0.8 |
| Umiiruka WATER 40 | 95.0 ±1.0 | 94.3 ±1.0 | 92.0 ±1.2 | 94.8 ±1.0 | 90.5 ±1.3 | 91.7 ±1.2 |
| Defense-tuned 45 | 97.9 ±0.6 | 98.9 ±0.5 | 99.1 ±0.4 | 98.8 ±0.5 | 99.2 ±0.4 | 98.7 ±0.5 |

Every style still wins over 90% against random play; the numbers moved
within about two intervals of the M4 ones. Beatdown against itself on
the owner's deck: 48.1% ±2.2. Speed: 1,215 to 1,812 random games/s
(265,000 to 320,000 actions/s), Beatdown mirror 2,860 games/s, one core.
