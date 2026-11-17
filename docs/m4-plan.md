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
