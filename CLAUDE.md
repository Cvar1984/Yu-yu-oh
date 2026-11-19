# Yu-yu-oh: Rust Yu-Gi-Oh! engine, bot and deck builder (LOB / MRD / SRL)

Full spec: `docs/handoff.md`. This file holds what every session must know.

## Decisions already made (change only with the owner)

- Rust for everything: engine, AI, deck builder, CLI and server. One Cargo workspace, edition 2024.
- Card pool is the 374 cards of LOB, MRD and SRL, shipped as JSON. Card id (the official password) is the key in every file.
- Full official chain rules. House rules live in `config/rules.toml`; copy limits live in `data/banlist.json`. Nothing about limits or rules is hard-coded; a rule change must never need a recompile.
- `ygo-engine` knows nothing about AI or UI, does no I/O, and takes randomness only from a seed, so it also builds to WebAssembly.
- Card images are Konami-owned and never committed: `images/` is git-ignored.
- Effects never go into `cards.json`; they are Rust scripts in `ygo-cards`, keyed by card id.

## Layout

| Path | Job | May depend on |
| --- | --- | --- |
| `crates/ygo-data` | Load and validate cards, ban list, collection, decks, rules | serde, serde_json, toml |
| `crates/ygo-engine` | Game state, rules, chains, effect API, legal actions, replays | ygo-data |
| `crates/ygo-cards` | One effect script per non-Normal card | ygo-engine |
| `crates/ygo-ai` | Observations, heuristic bots, search, neural policy | ygo-engine |
| `crates/ygo-train` | Self-play, replay buffer, league, checkpoints | ygo-ai |
| `crates/ygo-deckbuilder` | Deck search under collection and ban list | ygo-ai |
| `crates/ygo-play` | A human against a bot without a screen: `PlaySession`, card text (`describe`), bots by name, deck checks | ygo-ai, ygo-cards |
| `crates/ygo-wasm` | The game for a web page: JSON commands in, JSON views out; WebAssembly exports for `wasm32` only | ygo-play |
| `crates/ygo-cli` | The `ygo` binary | all of the above |
| `crates/ygo-server` | HTTP and WebSocket API for the browser | ygo-engine, ygo-ai |
| `web/` | Browser board (M9, over the server's protocol) | the server's protocol |
| `web/play/` | Standalone browser game: the engine in WebAssembly, built by `scripts/build-web.sh` into `web/play/dist/` (git-ignored) | ygo-wasm |
| `config/rules.toml` | House rules, also names the card and ban list files | |
| `config/styles.toml`, `config/card_values.toml` | Heuristic bot styles and card values (read by `ygo-data`) | |
| `records/` | Game records (git-ignored) | |
| `data/` | JSON data; see `data/README.md` | |

Milestones M0 to M9 and their gates are in `docs/handoff.md`. Do not start a milestone before the previous gate passes. Status: M0 to M3 gates passed. All 228 non-Normal cards are scripted with rulings tests (`docs/m3-plan.md`, which also lists the simplifications made). M4 (records, `ygo play`, heuristic bots, `ygo replay`) is built and its checks pass (`docs/m4-plan.md`, gate report); its gate is the owner playing a full game, with `ygo play` or in the browser game (`web/play/`). The rules and all card scripts were then moved to the modern official rules and text (`docs/card-audit.md`: findings, the three fix batches, and the exceptions left on purpose); the owner audits that work. Next after that: M5.

## Conventions

- Keep it simple: no abstraction before the second use, no dependency before it is needed. Empty crates stay empty until their milestone.
- Data formats are strict (`deny_unknown_fields`). A changed bundle must fail loudly, then we adapt the types on purpose.
- Deck legality is a report (`Vec<DeckProblem>`), not an error. Loading failures are `DataError`.
- Paths inside `rules.toml` are relative to `--data-root` (default: the working directory).
- Every behaviour switch in `rules.toml` gets a test that flips it. Every scripted card gets a rulings test.
- CI runs `cargo fmt --check`, `cargo clippy --workspace --all-targets` with `-D warnings`, and `cargo test --workspace`. Run `scripts/check.sh` before every commit; it fails on the first problem.
- The workflow lives in `docs/ci-workflow.yml` until someone with the GitHub `workflow` scope moves it to `.github/workflows/ci.yml`; sessions here cannot push workflow files.

## Commands

```sh
scripts/check.sh                       # fmt, clippy -D warnings, tests
cargo run -p ygo-cli -- check-data [--collection data/collection.json]
cargo run -p ygo-cli -- check-deck data/decks/<deck>.json [--collection data/collection.json]
cargo run --release -p ygo-cli -- sim data/decks/<a>.json data/decks/<b>.json -n 2000 [--seed S] \
    [--bot-a random|heuristic:<style>] [--bot-b ...] [--record <dir>]
cargo run --release -p ygo-cli -- play --deck data/decks/<deck>.json [--bot heuristic:beatdown] \
    [--bot-deck <file>] [--seed S] [--first me|bot|random] [--record <file>]
cargo run --release -p ygo-cli -- replay <record.jsonl> [--view p0|p1|all] [--turn N]
scripts/build-web.sh                   # browser game into web/play/dist (needs: rustup target add wasm32-unknown-unknown)
python3 -m http.server -d web/play/dist  # play it locally
```

## Engine notes (ygo-engine)

- `Game` is a struct, not a trait; `Shared` (rules + `CardPool`) is built once and held by `Arc`, so a game clone is flat vectors plus a pointer bump. No `Rc`, no `RefCell`, no lifetimes in state.
- Every choice is an `Action`; `legal_actions` enumerates all of them (tribute combinations included) and `apply` refuses anything else. Phases without a decision advance automatically.
- Any Main Deck monster can be Normal Summoned unless its script carries `Tag::CannotNormalSummon`. Every non-Normal card has a script; `crates/ygo-cards/tests/registry.rs` fails if one is missing.
- Work that waits on the players sits on a stack of `Pending` items: response windows, a declared summon before its negation window closes, a declared attack, the damage calculation, a chain to resolve. `advance` works the top item and stops only when a player has a real choice; a window whose player has nothing to activate is passed automatically.
- Windows open after Sets and position changes, at summon declaration and success, at attack declaration, inside the Damage Step before damage calculation, after battle, after each chain link and after a chain resolves, and at the Standby, Battle-start, Battle-end and End Phases. Trigger and Flip effects are collected when a window opens, in SEGOC order.
- `scripts/check.sh` must pass before every commit; `cargo test --no-fail-fast` shows every failing binary at once.
- `GameSetup.shuffle = false` keeps decks in listed order for scenario tests; `tests/common/mod.rs` has `stacked`, `shared_with` (rules with one change) and known card ids.
- Instance ids are assigned in a seeded random order so an id never reveals a face-down card's identity. `observe(player)` is the only view bots get.
- Modern-rules state: a Set completes with no negation window; Gate Guardian, the Moths and the Toons use summon procedures (`Action::ProcedureSummon`, `SummonKind::Procedure`, with a summon-negation window); each instance knows whether it was properly summoned (`special_summonable`, `special_summon_properly`, `room_after_leaving`); a `ChainLink` records `card_activation` and the cost's `value`; Field Spells can be Set; optional "When" triggers set `misses_timing`; once-per-turn names and name locks live in `PlayerState` (`use_name_once`, `lock_name_this_turn`) and clear at turn start. `tests/modern_rules.rs` and `tests/card_fixes.rs` in `ygo-cards` show each rule with real cards.
- Speed baseline (after the modern-rules work, release, one core of this cloud container, all 228 scripts active): 1,215 to 1,812 random games/s, 265,000 to 320,000 actions/s; the Beatdown bot against itself 2,860 games/s. M4 was 1,870 to 2,580 games/s (350,000 to 400,000 actions/s) and Beatdown 3,777; the drop comes from the new name, procedure and proper-summon checks. M3 was 1,700 to 2,200, M2 2,600 to 3,500 and M1 7,300. Profiling (`valgrind --tool=callgrind` on `ygo sim`) at M2 showed a third of the time in `observe` and a fifth in allocation; M3 adds continuous-hook scans on every attack, summon and Graveyard move. An observation buffer reused across decisions, and caching the face-up continuous effects per state, are the next wins. Re-measure with `ygo sim` after engine changes and keep the numbers here current.

## Records, play and bots (ygo-engine, ygo-ai, ygo-cli)

- A game record is JSON Lines: a header with the full `Rules`, the `GameSetup` and player labels, one line per action, then the result. It replays under its own rules, so `rules.toml` changes never break old records. `GameRecord::note` goes before `apply`.
- Bots implement `ygo_ai::Player` and see only the `Observation`. `HeuristicPlayer` scores each legal action; its weights come from a style in `config/styles.toml` and card worth from `config/card_values.toml` (style values override). Card knowledge goes in those files, not in code. Use engine tags (Exodia pieces) when a rule of thumb needs a card property.
- `ygo_cli::bots::Bots` turns `random` or `heuristic:<style>` into a player for `play` and `sim`. Evaluate any bot change with `ygo sim` (2,000 games, 95% interval) and record the numbers.
- `ygo play` and `ygo sim` refuse a deck that fails the check-deck report (cards, ban list, deck rules; not the collection).
- The browser game sends JSON commands to `ygo_wasm::Host::handle` (tests call it natively); its view is built from the human's observation only, so hidden cards never reach the page. A game resumes from its seed and the human's choices. The published page ships a card file without images or print records, and no Konami art.
- `PlaySession` and `ReplaySession` hold the logic; `tui.rs` only draws and reads keys, so tests drive sessions and check screens with ratatui's `TestBackend`. Anything shown to a player is rendered from that player's observation (`describe.rs`).

## Writing a card script (ygo-cards)

- One `CardScript` per card id, registered in the module for its group: `spells`, `traps`, `monsters`, `continuous` (the 30 key cards), `tables` (table-driven Equips, Field Spells and boosters), `oneshot`, `reveal`, `fusion`, `ritual`, `monsters_a`, `monsters_b`, `flips`, `toons`, `remaining`. Function pointers only, no closures capturing state; everything an effect needs is in `EffectCtx` or read back from the `Game`.
- `Effect::activate(speed, resolve)` for Spells and Traps, `Effect::trigger(mandatory, resolve)` for triggers, `Effect::flip(resolve)` for FLIP effects, `Effect::continuous()` with `modify` or `restrict` for static effects. Override `timing`, `targets`, `cost_choices` and `cost` as needed; set `damage_step: true` for effects that may start inside the Damage Step.
- `timing` sees the open `Window` (its kind and events); `targets` and `cost_choices` enumerate every legal set and the engine turns them into `Activate` actions. A resolution that needs a pick returns `Flow::Choose` (cards) or `Flow::Ask` (labelled options) with a continuation; `ChooseBy` and `AskBy` let the other player answer. A trigger that needs facts from its window (Kotodama, Trap Hole) must capture them in `targets`, because the window is gone by resolution.
- Continuous hooks besides `modify` and `restrict`: `on_attack` (attack costs), `attack_lock`, `on_battle_damage`, `replace_battle_destruction`, `lock_position`. Tags: `CannotNormalSummon`, `DirectAttacker`, `Toon`, `RevealHands`, `BanishInsteadOfGraveyard`, `HandPlayToll`, `StaysOnField`, `ControlWhileEquipped` (Snatch Steal), `StaysWithoutTarget` (Spellbinding Circle), the Exodia tags. Effect flags: `once_per_turn`, `damage_step`, `in_standby`, `misses_timing`. `EffectKind::Procedure` is a summon procedure, not an effect: it never starts a chain.
- Modern text rules for scripts: when the text names no position for a Special Summon, ask it (`positions.rs`: `face_up`, `attack_or_set`); compare current ATK (`g.atk`), never printed ATK; LP costs use `can_pay_lp` (paying all your LP is legal); "Special Summon" effects check `special_summonable` and use `special_summon_properly` only for the card's own proper summon; a cost that must hand a number to the resolution sets `ctx.value`. Choices the text makes on resolution (Polymerization, The Cheerful Coffin) are `Flow::Choose`, not targets.
- A card's `counter` survives leaving the field and resets when it enters the field again.
- Engine operations for scripts live in `ops.rs` (and a few in `chain.rs` and `battle.rs`): draw, destroy, discard, mill, mill_by, banish, tribute, special_summon, fusion_summon, ritual_summon, change_control, swap_control, set_position, flip_face_up, equip, attach, absorb_as_equip, gain_lp, pay_lp, take_damage, add_to_hand, return_to_deck, place_on_top_of_deck, reorder_top, shuffle_deck, reveal, coin, random_index, modify_until_end_phase, modify_during_damage_calculation, destroy_at_end_of_turn, forbid_attacks_until, grant_extra_battle_phase, end_battle_phase, shield_until_chain_ends, retarget, set_counter, negate_activation, negate_summon, prevent_battle_damage, special_summon_properly, forbid_position_change_this_turn, use_name_once, lock_name_this_turn, clear_marks, and queries such as atk, def, on_field, spell_targetable, special_summonable, room_after_leaving, can_pay_lp, name_used, marked_by, pending_summon, last_link, current_attack, window, top_of_deck, hands_revealed.
- Every scripted card gets a rulings test in `crates/ygo-cards/tests`, built on `ygo_engine::testing` (stacked decks, `play`, `next_turn`, `pass_all`, `with_rules`).

## Data formats (from the data bundle, `schema_version` 1)

Every file carries `"schema_version": 1`; `ygo-data` rejects any other version. `name` fields next to ids are for humans and are never checked.

- `cards.json`: `{ schema_version, source, sets, count, cards: [Card] }`. `count` must equal the array length. `type` uses YGOPRODeck spelling; the sets contain Normal, Effect, Flip Effect, Toon, Fusion, Ritual and Ritual Effect Monsters plus Spell and Trap Cards. Spells and Traps have `null` level, ATK, DEF and attribute, and `race` holds their subtype.
- `banlist.json`: `{ schema_version, name, updated, default_limit, note, limits: [ { id, name, limit } ] }`. `default_limit` is informational and must equal `deck.default_copy_limit` in `rules.toml`; every listed limit must be below it.
- `collection.json`: `{ schema_version, owner, read_from, totals: { set: "owned/total" }, cards: [ { id, name, copies } ] }`.
- `decks/*.json`: `{ schema_version, name, notes, main: [ { id, name, count } ], extra: [ ... ] }`. Copy limits count Main and Extra Deck together.
- `tools/refresh_cards.py [root]` rebuilds `cards.json` and fetches missing images from YGOPRODeck; it never touches the ban list. `reference/python_prototype/` is the simplified Python simulator, reference only.

## Owner's answers (2026-11-16, updated 2026-11-18)

- 2026-11-18: the basic rules and every card follow the modern official rules and modern card text. The earlier house rules stay as switches in `rules.toml`, now set to the modern values (hand limit, Trap timing, Set-monster targeting, Main Phase 2 summons, Field Spells).

- Turn 1: the first player does not draw (`first_player_skips_draw = true`), may Normal Summon or Set a monster, Set and activate Spells and Traps, but may not attack (`first_player_can_attack = false`).
- Hand: no cap during a turn (Pot of Greed may push it past 6). At your End Phase, discard down to 6 (`hand_limit_check = "end_phase"`, modern; `"after_draw"` was the earlier house rule).
- Spells and Traps per turn: no count. The only limit is the 5 zones (`spell_trap_zones`); the earlier `spell_trap_plays_per_turn` key was removed.
- A Trap cannot be activated in the turn it was Set (`trap_activation_turn_set = false`, modern; `true` was the earlier house rule).
- Spells may target a monster Set this turn (`spell_target_monster_set_this_turn = true`, modern; `false` was the earlier house rule). Normal Summons and Sets are allowed in Main Phase 2 (`summon_in_main_phase_2 = true`).
- Field Spells: one per player; a new one replaces only your own (`field_spells_on_field = 2`, modern; `1` was the earlier house rule).
- Flip and Special Summons do not count toward a per-turn limit, but this may change, so both stay config keys.
- Fusion and Ritual Summons are allowed (`fusion_summons`, `ritual_summons`).
- Card text: support both modern and original 2002 text. `cards.json` holds only modern text; the original text needs a second data source, keyed by id, before `text_version = "original_2002"` can do anything. Effect scripts whose rulings differ by printing (Sangan, Witch of the Black Forest) must branch on this key.
- Ban list (2026-11-17): Pot of Greed, Change of Heart and Raigeki are limited to 1 per deck (`data/banlist.json`). Swords of Revealing Light, the fourth badge seen earlier, stays unlimited. The Exodia stall sample deck was removed (owner, 2026-11-18: too strong).
- Deck-out is a loss.
- Training hardware: likely NVIDIA. Pick the neural-net crate at M6 with a spike on CUDA (candidates: burn, candle, tch-rs).
- VR games cannot be exported; human games come only from this project's CLI and browser.
- Bot thinking time against a human is adjustable: a bot setting, not a rule, introduced at M5.
- Cyber Jar (2026-11-17): Special Summons as many revealed monsters as the summon settings allow, controller's choice, rest to the hand. Toons play by the modern text. Scripts follow modern text only until an original-text data file exists; the list of cards whose 2002 printing differs is built once that file exists (see `docs/m3-plan.md`, Text versions).

## Still open

Nothing. Every question from the handoff is answered above; new ones go here.
