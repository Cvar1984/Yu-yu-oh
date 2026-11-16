# Handoff: Rust Yu-Gi-Oh! AI Bot & Deck Builder (LOB/MRD/SRL)

Nov 16, 2026 · @Cvar1984

## Goal and scope

Build a Rust project with three products sharing one rules engine. The card pool is the 374 cards of Legend of Blue Eyes (LOB), Metal Raiders (MRD) and Spell Ruler (SRL):

1. **Engine**: a rules-accurate, deterministic game engine with full chain rules and configurable house rules.
2. **Bot**: an AI that improves by self-play and by playing humans, across many playstyles, keeping the strongest through league play ("last one standing").
3. **Deck builder**: searches for the strongest legal deck under a card collection and a ban list, scored by the bot's own matches.

Done means:

- The engine passes a rulings test suite.
- The bot beats the heuristic baseline and every earlier checkpoint.
- The deck builder returns legal decks with measured win rates.
- A human can play the bot through the CLI and the browser.

Out of scope for now: cards outside the three sets, online matchmaking, and integration with the VR game the owner plays in.

## Decisions already made

The owner has settled these. Change them only with the owner.

| Topic | Decision |
| --- | --- |
| Language | Rust for engine, AI, deck builder, CLI and server |
| Card pool | LOB, MRD, SRL: 374 cards, shipped as JSON with card images |
| Rules fidelity | Full official chain rules: spell speeds, chains, priority, responses on both players' turns |
| Card limits | Read from `banlist.json`, which changes when the game updates; no limits hard-coded |
| Summon limits | A config, not code. Default: 1 Normal or Tribute Summon (or Set) per turn, as the owner's game plays |
| Main Phase 2 | In the owner's game you can activate and Set Spells/Traps in Main Phase 2 but not summon. Make it a config switch |
| Interfaces | Engine as a portable library API, a CLI for quick access, a browser UI as the main human interface |
| Learning | Self-play plus games against humans; many playstyles compete and the weakest are eliminated |

## Game rules spec

The engine runs official rules, and every house rule the owner named is a key in `rules.toml` that the engine reads at game start. Defaults below match the owner's game where known. Keys marked "open" are guesses until the owner answers the open questions at the end.

**Turn structure:** Draw Phase, Standby Phase, Main Phase 1, Battle Phase (Start, Battle, Damage, End steps), Main Phase 2, End Phase.

```toml
# rules.toml — defaults
[deck]
main_min = 40
main_max = 60
extra_max = 15          # Fusion monsters
default_copy_limit = 3  # per card name; banlist.json overrides

[field]
monster_zones = 5
spell_trap_zones = 5
field_spells_on_field = 1   # 1 total on the whole field; a new one replaces it (owner: "max 1 environment change")

[hand]
max_size = 6
hand_limit_check = "end_phase"  # open: or "always"

[life]
starting_lp = 8000

[turn]
first_player_skips_draw = true      # open
first_player_can_attack = false     # open
spell_trap_plays_per_turn = 5       # owner rule; open: do Sets and quick responses count?

[summon]
normal_or_tribute_per_turn = 1      # Normal Summon, Tribute Summon or Set
tributes_level_5_6 = 1
tributes_level_7_plus = 2
summon_in_main_phase_2 = false      # owner's game
flip_summon_counts_toward_limit = false   # open
special_summon_counts_toward_limit = false # open; true = owner's earlier "1 monster per turn" rule
max_monsters_entering_per_turn = 0  # 0 = no cap; 1 = owner's earlier strict rule

[cards]
text_version = "modern"   # open: "modern" official text or "original_2002"
banlist = "data/banlist.json"

[win]
deck_out_loses = true     # lose when you must draw from an empty Deck
exodia = true
```

Official rules apply wherever `rules.toml` says nothing. Rule changes must never need a recompile; a game record stores the full rules config and seed so any game replays exactly.

## Card data

All card data ships as JSON in the starter bundle `ygo-rust-ai-data.zip`, so ban changes and card fixes never need a rebuild. The engine loads these files at start and validates them.

| File | Contents |
| --- | --- |
| `data/cards.json` | 374 cards: 146 Normal, 95 Effect, 25 Fusion and 4 Ritual Monsters, 89 Spells, 15 Traps |
| `data/banlist.json` | Card limits (0, 1 or 2 copies); only Pot of Greed, Raigeki, Swords of Revealing Light and Change of Heart (limit 1) observed so far |
| `data/collection.json` | The owner's 226 cards (427 copies), read from in-game screenshots |
| `data/decks/*.json` | Four decks from the prototype, with results in `notes` |
| `config/rules.toml` | The house rules above |
| `images/full`, `images/small` | One JPEG per card id, 374 of each |
| `tools/refresh_cards.py` | Rebuilds `cards.json` and fetches missing images |

One `cards.json` entry (text shortened):

```json
{"id": 39111158, "name": "Tri-Horned Dragon", "type": "Normal Monster", "frame_type": "normal",
 "text": "An unworthy dragon with three sharp horns...", "level": 8, "atk": 2850, "def": 2350,
 "race": "Dragon", "attribute": "DARK",
 "printings": [{"set": "LOB", "number": 0, "code": "LOB-000", "rarity": "Secret Rare"}],
 "images": {"full": "images/full/39111158.jpg", "small": "images/small/39111158.jpg",
            "url_full": "https://images.ygoprodeck.com/images/cards/39111158.jpg", "url_small": "..."}}
```

Rules for the data layer:

- **Card id is the key everywhere.** It is the card's official password; deck, collection and ban list files all refer to it.
- **Effects stay out of `cards.json`.** It holds what the card says; how the engine plays it lives in effect scripts keyed by id (see Engine design). The ban list is separate so it can change without touching card data.
- **The ban list is the game's, kept by hand.** YGOPRODeck's ban data is the modern TCG list, not the owner's game. Every limit the game shows goes into `banlist.json`.
- **Images are local.** The [YGOPRODeck API guide](https://ygoprodeck.com/api-guide/) asks users to download each image once and host it themselves, and limits the API to 20 requests per second. `refresh_cards.py` respects both.
- **Card names, text and art are owned by Konami.** Keep the images for personal use.

## Architecture

One Cargo workspace. The engine crate knows nothing about AI or UI, so the same rules run in training, the CLI, the server and the browser.

| Crate | Job | Depends on |
| --- | --- | --- |
| `ygo-data` | Load and validate `cards.json`, `banlist.json`, `collection.json`, decks and `rules.toml` | serde, serde\_json, toml |
| `ygo-engine` | Game state, rules, chains, effect scripts, legal actions, replays. No I/O, no threads | ygo-data |
| `ygo-cards` | One effect script per non-vanilla card, registered by card id | ygo-engine |
| `ygo-ai` | Observation encoding, heuristic bots, search, neural policy/value, playstyles | ygo-engine |
| `ygo-train` | Self-play workers, replay buffer, training loop, league and ratings, checkpoints | ygo-ai |
| `ygo-deckbuilder` | Deck search under collection and ban list, scored by matches | ygo-ai |
| `ygo-cli` | Play, simulate, train, run the league, build decks, check decks | all of the above |
| `ygo-server` | HTTP and WebSocket API for the browser; logs human games for training | ygo-engine, ygo-ai |
| `web/` | Browser board using the card images; talks to `ygo-server` | the server's protocol |

Portability: `ygo-engine` must also build to WebAssembly (no OS calls, no system clock, randomness only from a passed-in seed), so the browser can run games offline later.

Suggested crates, to confirm in a first spike: `rand_chacha` (seeded RNG), `rayon` (parallel self-play), `clap` (CLI), `axum` + `tokio` (server), `proptest` and `criterion` (tests and benchmarks). For neural nets, compare `burn`, `candle` and `tch-rs` on the owner's hardware before choosing (see open questions).

## Engine design

The engine is a deterministic state machine: the same rules, decks, seed and decisions always give the same game. Every choice a player makes, including chain responses, targets, costs and yes/no prompts, goes through one decision interface, so bots and humans use the same API.

```rust
pub trait Game {
    fn new(rules: &Rules, decks: [Deck; 2], seed: u64) -> Self;
    fn pending_decision(&self) -> Option<Decision>;   // who must choose, and what kind of choice
    fn legal_actions(&self) -> Vec<Action>;           // every legal answer to that decision
    fn apply(&mut self, action: &Action) -> Result<Vec<Event>, RuleError>;
    fn observe(&self, player: PlayerId) -> Observation; // hidden information removed
    fn result(&self) -> Option<GameResult>;
}
```

Requirements:

1. **Full chain rules.** Spell speeds 1, 2 and 3, chain building and backwards resolution, priority passing, simultaneous triggers in the right order, "when" vs "if" triggers (missing the timing), costs paid before effects, negation, and Damage Step limits.
2. **Cheap to copy.** Search copies states thousands of times per move, so keep state in flat vectors and indices, with no `Rc` or `RefCell` in game state.
3. **Hidden information.** `observe` shows a player only public zones, their own hand and their own face-down cards. Bots never read the full state during a game.
4. **Effect scripts.** The 146 Normal Monsters need no script. The other 228 cards each get a script keyed by card id, written in Rust against a small effect API (target, destroy, move, change stats, summon, negate, draw, discard, and so on). Simple effects can come from a data table later; start with Rust for correctness.
5. **Continuous effects in layers.** Field Spells, equips and lingering effects such as Swords of Revealing Light recompute ATK/DEF and restrictions from the base stats each time, never by mutating them.
6. **House rules from config.** Every limit in `rules.toml` is checked in one place. For example, Cyber Jar's "Special Summon all revealed monsters" must follow whatever the summon settings allow.
7. **Game records.** Each game writes rules, decks, seed and every action as JSON lines. Replays, the browser viewer and training all read this format.
8. **Speed target.** Measure random-play games per second per core from the first milestone onward and keep it in CI, because training speed depends on it.

## AI: self-play, playstyles and the league

Build the bot in three layers, each a stronger opponent for the next, and keep every one as a baseline.

1. **Heuristic bots.** Rule-based players, one per playstyle, ported from the Python prototype's logic. They are the first opponents and a sanity check for the engine.
2. **Search.** Information-set Monte Carlo tree search: sample the opponent's hidden cards consistent with what has been seen, search each sample, and combine the results. It plays well with no training at all.
3. **Learned policy and value.** A neural network scores each legal action and estimates the win chance from an observation. It guides the search (AlphaZero-style) and is trained on self-play games. Score each legal action from its own features rather than using a fixed output per action, because the legal actions change every turn.

Represent each card by its features (type, level, ATK, DEF, attribute, race, effect tags), not by a learned slot per card id. Then a ban list change or a newly scripted card does not break a trained model.

**Playstyles.** Each agent is a deck plus a policy plus a style. A style is a set of reward and behavior weights. Start with these:

| Style | Plays for |
| --- | --- |
| Beatdown | Fast damage with Level 4 attackers and equips |
| Control | Removal and card advantage, attacking only when safe |
| Stall / deck-out | Walls, Swords, Gravekeeper's Servant; wins when the opponent can't draw |
| Burn | Direct damage spells and effects |
| Alternate win | Exodia, where the copy limits allow it |
| Patient big-summon | The owner's style: Tribute Summons only when protected, for example with Solemn Judgment set |

**League ("last one standing").** A population of agents plays rounds against each other. Rate everyone with Glicko-2 or TrueSkill and report win rates with 95% confidence intervals, split by who went first. After each generation:

1. Remove the bottom agents.
2. Replace them with mutated copies of the top agents (new deck tweaks, style weights, training settings).
3. Keep a hall of fame of past champions in every round, so the league doesn't forget how to beat old strategies.
4. Keep "exploiter" agents whose only job is to beat the current leader. The prototype found rock-paper-scissors cycles (Exodia beats control, anti-Exodia beats Exodia, control beats anti-Exodia), and without exploiters a league can circle forever.

**Learning from humans.** Every human game played through the CLI or browser is saved in the game-record format. Use it two ways: humans enter the league with their own ratings, and their moves form a dataset for imitation learning, which gives the network a head start before self-play.

## Deck builder

The deck builder searches for the deck with the best win rate against the current league, using only cards the player owns and the ban list allows.

- **Constraints:** owned copies from `collection.json` (or the full pool), limits from `banlist.json`, deck size and Extra Deck limits from `rules.toml`.
- **Search:** start from seed decks per playstyle, then try card swaps and keep a swap only if the win rate improves by more than the noise. This is how the prototype found its decks.
- **Variety:** keep the best deck in each style bucket (MAP-Elites), not only the single best, so every playstyle stays in the league.
- **Co-evolution:** decks and policies improve together. A deck is scored with a policy trained on it, because a policy that can't pilot a card makes that card look useless.
- **Saving compute:** test every candidate on the same seeds, and stop a test early once it is clearly better or worse (sequential testing).
- **Output:** deck JSON in the `data/decks` format, plus a report with its win rate against each league member, first and second player separately.

The prototype showed one trap to avoid. Bot quality changed deck rankings more than once: an AI that tributed its own Red-Eyes, or used Painful Choice badly, made good cards look bad. Re-check deck rankings whenever the bot gets stronger.

## Interfaces

All three interfaces drive the same `Game` API and write the same game records.

**Engine API (portable).** A Rust library first. Add a WebAssembly build of `ygo-engine` so the browser can run games locally, and a C ABI later only if another program needs it.

**CLI (`ygo`).** Quick access to everything:

| Command | Does |
| --- | --- |
| `ygo play --deck <file> --bot <name>` | Human vs bot in the terminal |
| `ygo sim <deckA> <deckB> -n 10000` | Bot vs bot, prints win rates with confidence intervals |
| `ygo league run` / `ygo league table` | Runs league generations / shows ratings |
| `ygo train --config <file>` | Starts or resumes self-play training |
| `ygo build-deck --collection data/collection.json --style stall` | Runs the deck builder |
| `ygo check-deck <file>` | Checks a deck against the collection, ban list and rules |
| `ygo replay <game.jsonl>` | Steps through a recorded game |
| `ygo cards refresh` | Same job as `tools/refresh_cards.py` |

**Browser (main human interface).** `ygo-server` serves the board and card images and runs games over a WebSocket: the server sends an observation and the pending decision, and the browser answers with one legal action. The browser never computes rules. It shows chain links as they stack, so a human can respond at every point the rules allow.

## Testing and evaluation

The bot can only be as good as the rules it trains on, so the engine is tested before any learning starts.

- **Rulings tests:** a scenario test per scripted card, plus chain cases. Examples: Solemn Judgment negating a Tribute Summon, Trap Hole not triggering on a Set, Mirror Force on attack declaration, Sangan's search when tributed, Cyber Jar under each summon setting, a Field Spell replacing another.
- **Invariant tests (proptest):** zone limits never exceeded; hand at or below the limit after the End Phase; total card count conserved (nothing duplicated or lost); the same seed and actions always give the same game.
- **Fuzzing:** millions of random-legal-action games with no panics and no illegal states.
- **Config tests:** every `rules.toml` switch has a test that flips it and checks the behavior changes.
- **Bot evaluation:** a fixed ladder of the random bot, heuristic bots, the prototype's best decks and earlier checkpoints. A new checkpoint is promoted only if it beats the current best over enough games for a 95% confidence interval that excludes 50%.
- **Regression:** the prototype's matchups give rough expected results to compare against, while remembering its rules were simplified (see the next section).

## Milestones

Each milestone ends at a gate; don't start the next until it passes. The engine milestones come first because every later result depends on correct rules.

| # | Milestone | Gate to pass |
| --- | --- | --- |
| M0 | Data layer: load and validate all JSON and `rules.toml` | All 374 cards, the ban list, the collection and the 4 sample decks load; `check-deck` flags illegal decks |
| M1 | Core engine without chains: phases, summons, tributes, battle, Normal Monsters | Random games finish; invariant tests and fuzzing pass; speed measured |
| M2 | Chain system plus about 30 key cards (Raigeki, Pot of Greed, Change of Heart, Monster Reborn, Swords, Mirror Force, Trap Hole, Solemn Judgment, Sangan, Witch, Exodia pieces) | Rulings tests for those cards pass |
| M3 | All 228 non-Normal cards scripted | A rulings test per card |
| M4 | CLI: human vs heuristic bots, game records, replay | The owner plays a full game in the terminal |
| M5 | Search bot (information-set MCTS) | Beats every heuristic bot over at least 1,000 games |
| M6 | Neural policy and value, self-play training | Each promoted checkpoint beats the previous one |
| M7 | League with playstyles, elimination, hall of fame, exploiters | Ratings table runs unattended for many generations |
| M8 | Deck builder with co-evolution | Finds decks that beat the prototype's best decks from the same card pool |
| M9 | Browser UI and server; human games feed training | The owner plays the bot in the browser |

## Findings from the Python prototype

A Python simulator (in `reference/python_prototype`) already tested decks under the house rules. Its rules were simplified: one response per action instead of real chains, and simple heuristic bots. Treat its numbers as starting hypotheses for the Rust bot to confirm or overturn.

| Finding | Result |
| --- | --- |
| Strongest full-pool deck (3 copies of everything allowed) | Exodia with stall cards: 64% overall, but only 40% vs a dedicated anti-Exodia deck |
| Cycle in the full pool | Exodia beats control, anti-Exodia beats Exodia, control beats anti-Exodia about 93% of the time |
| Strongest deck from the owner's collection | Umiiruka WATER beatdown, 40 cards: 60% vs other decks from the collection |
| Owner's 45-card deck, attacking | 39% vs decks from the collection |
| Owner's 45-card deck, defending until deck-out | 38% |
| Defense-tuned 45 (attack cards swapped for walls) | 50% |
| Tribute Summoning Red-Eyes only when "safe" | 32%, vs 38% when never summoning it; about 30% of those summons were gone before the owner's next turn |
| Decks from the collection vs full-pool decks | 1 to 7%: the collection has 1 copy of each power card |

Things the prototype learned the hard way:

- The Exodia result depends on 3 copies of each piece. The owner's game limits some cards, so the ban list must be complete before trusting it.
- Cyber Jar's text Special Summons several monsters at once, which a strict summon limit forbids. The engine needs a defined behavior for each summon setting.
- AI mistakes changed deck rankings more than once (tributing its own big monster, a bad Painful Choice policy, Solemn Judgment saved for the wrong cards).
- Bigger isn't better even for stall: a 60-card stall deck did worse (45%) than the tuned 45, because its single copies of key cards came up less often.

## Open questions for the owner

Each answer sets a key in `rules.toml` or a project choice. Until then the defaults above apply.

- [ ] Does the player going first skip their first draw, and can they attack on turn 1?
- [ ] Hand limit 6: only checked in the End Phase (discard down), or can you never hold more than 6?
- [ ] Does the 5 Spell/Trap limit per turn count Sets, and cards activated on the opponent's turn?
- [ ] Field Spells: one on the whole field, with a new one destroying the old, even if the other player owns it?
- [ ] Do Flip Summons and Special Summons (Monster Reborn, Fusion, Cyber Jar) count toward any per-turn monster limit in your game?
- [ ] Are Fusion Summons (Polymerization) and Ritual Summons allowed?
- [ ] Which card text does the game use: modern official text, or the original 2002 printing? (Sangan and Witch of the Black Forest differ, for example.)
- [ ] The full ban list: which other cards show a 0, 1 or 2 badge in the game?
- [ ] Is losing when you can't draw (deck-out) a loss in your game?
- [ ] Training hardware: NVIDIA GPU, AMD GPU, Apple Silicon, or CPU only? This decides the neural-net crate.
- [ ] Can games from the VR game be exported, or will human games come only from this project's CLI and browser?
- [ ] How long may the bot think per decision when playing a human?

Sources: [YGOPRODeck API guide](https://ygoprodeck.com/api-guide/) (card data, images, rate limit). Card data fetched 2026-11-16.
