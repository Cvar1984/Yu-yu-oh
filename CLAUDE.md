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
| `crates/ygo-cli` | The `ygo` binary | all of the above |
| `crates/ygo-server` | HTTP and WebSocket API for the browser | ygo-engine, ygo-ai |
| `web/` | Browser board | the server's protocol |
| `config/rules.toml` | House rules, also names the card and ban list files | |
| `data/` | JSON data; see `data/README.md` | |

Milestones M0 to M9 and their gates are in `docs/handoff.md`. Do not start a milestone before the previous gate passes. Status: M0 and M1 gates passed. M1 (`docs/m1-plan.md`) plays Normal Monsters through every phase with summons, positions and battle; random games finish, invariants and fuzzing pass, speed is measured. Next: M2, the chain system and about 30 key cards.

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
cargo run --release -p ygo-cli -- sim data/decks/<a>.json data/decks/<b>.json -n 2000 [--seed S]
```

## Engine notes (ygo-engine)

- `Game` is a struct, not a trait; `Shared` (rules + `CardPool`) is built once and held by `Arc`, so a game clone is flat vectors plus a pointer bump. No `Rc`, no `RefCell`, no lifetimes in state.
- Every choice is an `Action`; `legal_actions` enumerates all of them (tribute combinations included) and `apply` refuses anything else. Phases without a decision advance automatically.
- Cards that need an effect are drawn, held and discarded but offer no actions until M2 scripts them. Only `CardKind::Normal` can be summoned today.
- `GameSetup.shuffle = false` keeps decks in listed order for scenario tests; `tests/common/mod.rs` has `stacked`, `shared_with` (rules with one change) and known card ids.
- Instance ids are assigned in a seeded random order so an id never reveals a face-down card's identity. `observe(player)` is the only view bots get.
- Speed baseline (M1, release, one core of this cloud container): about 7,300 random games/s, 1.5 million actions/s. Re-measure with `ygo sim` after engine changes and keep the number here current.

## Data formats (from the data bundle, `schema_version` 1)

Every file carries `"schema_version": 1`; `ygo-data` rejects any other version. `name` fields next to ids are for humans and are never checked.

- `cards.json`: `{ schema_version, source, sets, count, cards: [Card] }`. `count` must equal the array length. `type` uses YGOPRODeck spelling; the sets contain Normal, Effect, Flip Effect, Toon, Fusion, Ritual and Ritual Effect Monsters plus Spell and Trap Cards. Spells and Traps have `null` level, ATK, DEF and attribute, and `race` holds their subtype.
- `banlist.json`: `{ schema_version, name, updated, default_limit, note, limits: [ { id, name, limit } ] }`. `default_limit` is informational and must equal `deck.default_copy_limit` in `rules.toml`; every listed limit must be below it.
- `collection.json`: `{ schema_version, owner, read_from, totals: { set: "owned/total" }, cards: [ { id, name, copies } ] }`.
- `decks/*.json`: `{ schema_version, name, notes, main: [ { id, name, count } ], extra: [ ... ] }`. Copy limits count Main and Extra Deck together.
- `tools/refresh_cards.py [root]` rebuilds `cards.json` and fetches missing images from YGOPRODeck; it never touches the ban list. `reference/python_prototype/` is the simplified Python simulator, reference only.

## Owner's answers (2026-11-16)

- Turn 1: the first player does not draw (`first_player_skips_draw = true`), may Normal Summon or Set a monster, Set and activate Spells and Traps, but may not attack (`first_player_can_attack = false`).
- Hand: no cap during a turn (Pot of Greed may push it past 6). After your Draw Phase, cards above 6 go to the Graveyard (`hand_limit_check = "after_draw"`; `"end_phase"` is the official timing).
- Spells and Traps per turn: no count. The only limit is the 5 zones (`spell_trap_zones`); the earlier `spell_trap_plays_per_turn` key was removed.
- Traps may be activated in the turn they were Set (`trap_activation_turn_set = true`; `false` is official).
- A monster Set face-down this turn cannot be the target of a Spell, such as an Equip, until the next turn (`spell_target_monster_set_this_turn = false`; `true` is official).
- Field Spells: one on the whole field; a new one destroys the old whoever owns it (`field_spells_on_field = 1`).
- Flip and Special Summons do not count toward a per-turn limit, but this may change, so both stay config keys.
- Fusion and Ritual Summons are allowed (`fusion_summons`, `ritual_summons`).
- Card text: support both modern and original 2002 text. `cards.json` holds only modern text; the original text needs a second data source, keyed by id, before `text_version = "original_2002"` can do anything. Effect scripts whose rulings differ by printing (Sangan, Witch of the Black Forest) must branch on this key.
- Ban list: empty for now (`data/banlist.json`). The four limit-1 badges seen earlier are recorded in its `note` in case the game enforces them.
- Deck-out is a loss.
- Training hardware: likely NVIDIA. Pick the neural-net crate at M6 with a spike on CUDA (candidates: burn, candle, tch-rs).
- VR games cannot be exported; human games come only from this project's CLI and browser.
- Bot thinking time against a human is adjustable: a bot setting, not a rule, introduced at M5.

## Still open

Nothing. Every question from the handoff is answered above; new ones go here.
