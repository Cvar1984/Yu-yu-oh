# The 440-card pool (handoff update, 2026-11-20)

The owner's game lists 440 cards, not 374: 66 EXTRA cards sit outside the
LOB, MRD and SRL tabs (`docs/handoff.md`, "Update 2026-11-20"). The new
data bundle ships all of them. This reopens two gates that had passed:
M0 (all cards load) and M3 (every non-Normal card scripted, with a
rulings test). Both must pass again before M6 starts.

## What the bundle changed, and what was taken

| Bundle file | Taken | Notes |
| --- | --- | --- |
| `data/cards.json` | yes | 440 cards, `schema_version` 2: new `in_game_index` (LOB, MRD, SRL, EXTRA) per card, `note` on the file, `set_name` on EXTRA printings |
| `data/banlist.json` | yes | 13 cards at 1, enemies included |
| `data/collection.json` | yes | Save A, 385 cards, 1117 copies; new `unique` and `copies` fields, checked on load |
| `data/collection_save_b.json` | yes | Save B, 201 cards, 421 copies |
| `data/extra_cards.txt` | yes | The 66 EXTRA names, for `tools/refresh_cards.py` |
| `data/decks/*` | yes, but one | 12 new decks with a `for` field (`enemy`, `save A`, `save B`); `exodia_stall_full_pool_40.json` stays out (owner, 2026-11-18: too strong); `save_a_f2_40` and `save_a_discard_reborn_40` were then removed as stale (owner, 2026-11-20) |
| `tools/refresh_cards.py` | yes | Fetches the EXTRA cards by name; writes schema 2 |
| `reference/` | yes | `effects_catalog.json`, `rulings.md`, prototype sim v2 to v9 and its tests |
| `images/` | locally | 440 of each size, git-ignored |
| `config/rules.toml` | **no** | An older template with "open" keys and the earlier house rules (no Main Phase 2 summons, 5 Spell/Trap plays a turn). The owner's answers of 2026-11-18 and 2026-11-20 are newer and stay. |
| `README.md` | no | The bundle's own readme |

`reference/rulings.md` also lists those earlier house rules; the owner's
answers in CLAUDE.md win where they differ. Its card rulings are checked
against the modern text card by card below.

## The 44 new scripts

5 monsters, 18 Spells and 21 Traps. Three can never resolve in this pool,
because the card they need is not in it: White Dragon Ritual (Paladin of
White Dragon), The Flute of Summoning Dragon (Lord of D.) and Gryphon Wing
(Harpie's Feather Duster). They still get scripts with their real
conditions, so they are dead cards, as in the game.

**Batch 1, existing engine operations** (draws, damage, Life Points,
equips, destruction, banishing, searches, flips, rituals): Mysterious
Puppeteer, The Wicked Worm Beast, Penguin Soldier, Trap Master, Black
Luster Soldier, Black Luster Ritual, White Dragon Ritual, Ancient
Telescope, Card Destruction, De-Spell, Dian Keto the Cure Master,
Graceful Charity, Invigoration, Last Will, Nobleman of Crossout, Ookazi,
Premature Burial, Sword of Dark Destruction, The Flute of Summoning
Dragon, The Inexperienced Spy, Backup Soldier, Burst Breath, Castle
Walls, Disappear, Dust Tornado, Enchanted Javelin, Gift of The Mystical
Elf, Gryphon Wing, Jar of Greed, Just Desserts, Raigeki Break,
Reinforcements, Shadow Spell, The Eye of Truth, Waboku.

**Batch 2, new engine features:**

| Card | Needs |
| --- | --- |
| Scapegoat | Tokens: instances created during the game, a card outside the pool, never in the Graveyard or hand, not Tributed for a Tribute Summon; 4 free Monster Zones |
| Fairy Meteor Crush | Piercing battle damage |
| Reverse Trap | Stat changes reversed until the End Phase |
| D. Tribe | A type change for a turn |
| Soul Exchange | Tributing a monster the opponent controls; no Battle Phase that turn |
| Ultimate Offering | An extra Normal Summon or Set at once, for 500 LP |
| Light of Intervention | No Sets; a monster that would be Set is summoned in face-up Defense Position |
| Riryoku Field | Recognising a Spell that targets exactly one monster |
| Shift | Moving an attack or a Spell/Trap's single target |

## The prototype's acceptance cases

The handoff asks for `test_v8.py` (40 cases) and `test_v9.py` (37) to be
ported first: they are the engine's acceptance cases. Many are already
covered by the M2 and M3 rulings tests; the rest become Rust tests, each
following the modern text where the prototype's house rules differ. Two
engine bugs the prototype found get a test each: a flip effect that
removes the attacker or the target ends that battle with no damage
calculation, and a negated Spell goes to the Graveyard exactly once.

## Steps

1. **Data (M0 again)**: the bundle in, `ygo-data` reads schema 2, all 440
   cards, both collections and every deck load and check; the registry
   test lists the 44 pending scripts and fails if the list goes stale.
2. **Batch 1 scripts**, with rulings tests.
3. **Batch 2**: the engine features, then their scripts and tests.
4. **Acceptance cases** from the prototype, and the two bug tests.
5. **Bots**: card values for the new cards; the bots and the M5 gate
   numbers rerun on the new pool, and against the six enemy decks.
6. **Docs**: CLAUDE.md, this plan's progress.

## Progress

| Step | Commit | Result |
| --- | --- | --- |
| 1. Data | `874320f` | 440 cards, 13 limited, 2 collections, 15 decks load and check; 44 scripts pending |
| 2. Batch 1 | batch 1 | 36 cards scripted (Fairy Meteor Crush moved here from batch 2), 29 tests in `tests/extra.rs`; 8 pending |
| 3. Batch 2 A | `1fe2f51` | D. Tribe, Riryoku Field, Shift; 6 tests in `tests/extra_2.rs`; 5 pending |
| 4. Batch 2 B | `bb687a8` | Reverse Trap, Light of Intervention, Soul Exchange; 4 more tests; 2 pending |
| 5. Batch 2 C | `ce4b30d` | Scapegoat (Tokens), Ultimate Offering; 6 more tests and a card-text test; none pending: every non-Normal card of the 535 is scripted |
| 6. Acceptance cases | `5ab8941` | 76 of the prototype's 77 cases covered and passing (`docs/acceptance.md`), 6 new tests in `tests/acceptance.rs` and 1 in `ygo-ai/tests/lookahead.rs`, both prototype bugs pinned; the engine now replays an attack when the defender's number of monsters changes; 1 bot case open (discard plan) |
| 6b. Discard plan | `f27cfee` | revive potential in the position score (`[revive]` in `card_values.toml`, style `revive_worth`): 77 of 77; no strength change in 6,000 mirror games |
| 7. Bots | this commit | card values kept (the prototype's later values gain nothing overall, `2ca3915`); M5 gate holds on 535 cards: search beats every heuristic style, 61.6% to 73.0%, and wins every enemy-deck mirror, 54.8% to 69.4% |

Batch 1 added to the engine: piercing damage (`Effect::pierce`), stopping
an attack already declared (`cancel_attack`; Spellbinding Circle now
calls it too, as `reference/rulings.md` asks), a turn-long battle shield
(Waboku), a one-sided hand reveal (`Tag::RevealOpponentHand`), private
looks (`look_at`, `Event::LookedAt`), Setting from the hand by an effect,
`CardStats.flip` and the per-turn count of monsters sent to the
Graveyard. Simplifications, like those listed in `docs/m3-plan.md`:

- **Backup Soldier** picks its cards on resolution, as many as it can up
  to three, like Soul Release, instead of targeting.
- **Last Will** checks its condition on activation and summons on
  resolution; the text's "once during this turn" window for a later
  summon is not kept.
- **The Inexperienced Spy** looks at a random card: the pick is blind, so
  the odds are the same.
- **Shadow Spell** goes to the Graveyard with its monster as an Equip
  does, rather than being destroyed by its own effect.

Batch 2 (2026-11-23), in three parts. Owner's answer: the Sheep Token's
stats go in a data file, `data/tokens.json`.

- **A**: D. Tribe, Riryoku Field, Shift. Engine: lingering
  `LingeringKind::Race` (the Type comes from the source script's `race`
  hook, applied after continuous ones; it ends when the monster leaves
  the field), `change_race_until_end_phase`, `attack_targets`,
  `retarget_attack` and `Event::AttackRetargeted`. Fairy's Hand Mirror's
  single-target checks are shared (`remaining::single_monster_target`,
  `other_single_targets`). Simplifications: D. Tribe needs a face-up
  monster to affect, and a monster turned face-down and back up keeps
  the Dragon Type that turn; Shift answers an attack only in the
  attack-declaration window with no chain open.

- **B**: Reverse Trap, Light of Intervention, Soul Exchange. Engine:
  lingering `StatChangesReversed` (applied in `stat_mod` and
  `stats_with`), `MustTribute` and `NoBattlePhase`;
  `Tag::StatsSetOrScaled` on the cards whose changes set, swap or
  multiply (Sanga, Kazejin and Suijin, Karate Man, Cocoon of Evolution,
  Relinquished, Thousand-Eyes Restrict, Megamorph, Mirror Wall, Limiter
  Removal, Shield & Sword); `Tag::NoFaceDownMonsters` with
  `face_down_forbidden`; `SummonKind::Normal` carries its position;
  `CardScript::with_tag`. Simplifications: Reverse Trap reverses by
  source card, so a card would need the tag for all its changes (none of
  the tagged cards also adds); under Light of Intervention a face-down
  Special Summon is made in face-up Defense Position, a turn face-down
  does nothing, and Magical Hats and Darkness Approaches cannot be
  activated; Soul Exchange binds Tribute Summons only (not Ritual
  Summons or Tribute costs), and is activated only in Main Phase 1,
  before any Battle Phase.
- **C**: Scapegoat and Ultimate Offering. Data: `data/tokens.json` (named
  by `[cards] tokens` in `rules.toml`; `CardDb::with_tokens`, `Token`,
  `TokenFile`); the pool gives Tokens `CardStats.token` (treated as Normal
  Monsters, `is_main_deck` false) and no deck may hold one
  (`SetupError::TokenInDeck`). Engine: `special_summon_token` reuses a
  gone Token instance or appends one (instance ids stay below 256);
  a Token sent anywhere off the field goes to `Location::Gone` instead
  (`Event::TokenRemoved`); `Tag::NoTributeSummon` on the Sheep Token;
  `PlayerState.summoned` and lingering `NoSummons` (Scapegoat's lock,
  Sets stay open); `normal_summon_by_effect`; a face-up Spell or Trap's
  Quick effect may be activated, in windows and as a Main Phase action.
  The browser page loads `tokens.json` too (`init` takes `tokens`), and
  the play screen describes a Token. Simplifications: a Token leaving the
  field never touches the Graveyard (no "sent to the Graveyard" event);
  Scapegoat's lock starts when it resolves, after its Tokens (a negated
  Scapegoat locks nothing); Ultimate Offering's summon happens inside the
  chain and opens no summon windows of its own, does not use Soul
  Exchange's monster. A monster Set during an attack replays it (the
  replay rule of step 4).
  Speed: callgrind counts 1.1% more instructions for the same 300 random
  games (Removal against Big Beatdown) than before batch 2.

## Step 5: card values and bot reruns on the 535-card pool (2026-11-24)

**Card values.** `card_values.toml` holds the 51 values of the
prototype's first table (`ygo_sim2.py`); its later versions
(`ygo_sim3.py` to `ygo_sim9.py`) value 103 more cards, among them most
EXTRA cards (Graceful Charity 90, Premature Burial 82, Scapegoat 70) and
the Toons; none of the 51 changed. PSV came after the prototype. The 103
were tried as a style override on the lookahead bot, against the same bot
with today's values (`lookahead:beatdown`, mirrors, 1,000 games each,
seed 88):

| Deck | New values win |
| --- | --- |
| Enemy E1 Toon | 45.2% ±3.1 |
| Enemy E2 Special Summon | 48.8% ±3.1 |
| Enemy E3 Removal | 50.9% ±3.1 |
| Enemy E4 Big Beatdown | 49.5% ±3.1 |
| Enemy E5 DARK + Plasma Zone | 51.9% ±3.1 |
| Enemy E6 Gate Guardian | 51.7% ±3.1 |
| Save A Beatdown | 54.1% ±3.1 |

Pooled 3,521 of 7,000 (50.3%): no gain overall, a loss on the Toon deck
and a gain on save A's Beatdown deck. Leaving Graceful Charity at its
default did not change that (44.4% and 54.3%); leaving Toon World at its
default brought the Toon deck to 47.1%. The prototype's values were set
for its own, simpler bot; tuning them card by card against one deck would
fit the noise. **Kept as they are**: new cards use the defaults (Spells
and Traps 30, monsters by their stats), and the lookahead does the rest.

**M5 gate rerun.** The search bot (200 playouts, horizon 2) against each
plain heuristic style, 1,000 games each, `--seed 5000`, each player first
in half, as in `docs/m5-plan.md`. The gate's deck (the owner's 45 of
2026-11-16) was removed as stale, so this uses the owner's current deck,
`save_a_owner_current_40`, mirrored:

| Opponent | Search wins | Search first | Search second | Mean turns |
| --- | --- | --- | --- | --- |
| beatdown | 61.6% ±3.0 | 64.0% ±4.2 | 59.2% ±4.3 | 19.7 |
| control | 64.4% ±3.0 | 68.6% ±4.1 | 60.2% ±4.3 | 19.9 |
| stall | 71.7% ±2.8 | 76.0% ±3.7 | 67.4% ±4.1 | 21.5 |
| burn | 63.9% ±3.0 | 66.2% ±4.1 | 61.6% ±4.3 | 19.8 |
| exodia | 73.0% ±2.8 | 76.2% ±3.7 | 69.8% ±4.0 | 23.1 |
| patient | 70.1% ±2.8 | 73.6% ±3.9 | 66.6% ±4.1 | 20.7 |

**The gate holds**: the lowest bound of any interval is 58.6%. The margins
are below M5's 72% to 79%, but on another deck and a pool with 161 more
cards, so the two are not comparable. No draws.

The search bot against plain Beatdown in each enemy deck's mirror, 500
games each, same seed:

| Deck | Search wins | Deck-out wins (search / Beatdown) |
| --- | --- | --- |
| E1 Toon | 54.8% ±4.4 | 55 / 34 |
| E2 Special Summon | 60.0% ±4.3 | 23 / 53 |
| E3 Removal | 55.4% ±4.4 | 49 / 14 |
| E4 Big Beatdown | 65.2% ±4.2 | 26 / 12 |
| E5 DARK + Plasma Zone | 69.4% ±4.0 | 1 / 1 |
| E6 Gate Guardian | 61.0% ±4.3 | 2 / 1 |

It wins every one, narrowly with the slow decks (Toon and Removal, about
30 turns), where deck-outs decide a fifth of the games: the search's
scoring still ignores the Deck (`docs/m5-plan.md`, What it does not do
yet). Speed: 6 to 20 games/s on 4 threads of this container.

## Deck crafting from save A (2026-11-20)

The owner asked for the best deck from save A's cards. Two findings so
far, measured against the six enemy decks (the heuristic bot pilots the
enemies):

- **The heuristic bot misleads a deck search.** A hill climb scored by
  the heuristic bot on both sides raised `save_a_best_overall_40` from
  39.1% to 49.1% (6,000 games, fresh seeds). With the search bot
  piloting, the same crafted deck scored 51.3% against the original's
  54.1% (1,200 games each): the gain was the heuristic bot's quirks.
  Deck search must be scored with the search bot, as the handoff warns.
- **Search-bot swaps for the dead Scapegoat** (unscripted until batch 2),
  150 games per enemy, same seeds: unchanged 54.8%; Solemn Judgment 58.0,
  Dust Tornado 58.0, Raigeki Break 57.8, Premature Burial 57.7, Hane-Hane
  57.0, Nobleman of Crossout 55.4, Kuriboh 54.4, Card Destruction 53.9,
  Confiscation 52.2, Upstart Goblin 49.6. Differences under about 4
  points are noise at this sample size; any live card beats the dead
  slot by about 3.

**Validation (resumed 2026-11-20).** Three decks, each piloted by the
search bot (defaults: 200 iterations, horizon 2) against the six enemy
decks piloted by the Beatdown heuristic bot, 600 games per enemy, the
same seed per enemy for every deck (`--seed 9101` to `9106`,
`--threads 4`):

| Enemy | Best overall | A: Solemn for Scapegoat | B: A, Dust Tornado and Raigeki Break for the Horns |
| --- | --- | --- | --- |
| e1 Toon | 63.0 | 65.0 | 72.8 |
| e2 Special Summon | 57.2 | 56.3 | 62.3 |
| e3 Removal | 57.3 | 60.0 | 66.7 |
| e4 Big beatdown | 73.7 | 75.3 | 77.3 |
| e5 Dark Plasma zone | 29.0 | 29.7 | 33.0 |
| e6 Gate Guardian | 35.5 | 40.3 | 37.5 |
| **All, 3,600 games** | **52.6 ±1.6** | **54.4 ±1.6** | **58.3 ±1.6** |

B is saved as `data/decks/save_a_crafted_40.json`: 5.7 points over the
original (204 more wins of 3,600), better against five of the six
enemies, and its interval does not overlap the original's. A alone is
within noise of the original. The weak spots stay the Dark Plasma zone
and Gate Guardian decks (about a third won). Scripting Scapegoat (batch
2) changes the baseline: rerun this comparison then.

## Deck crafting with the lookahead bot (2026-11-22)

The search bot's playouts misplayed many cards (`ygo audit`: the plain
heuristic bot erred in 19.4% of its activations, answers and choices on
the real decks), so deck results favoured decks it happened to play
well. Decks are now measured with the lookahead bot on both sides
(`lookahead:beatdown`), 2,000 games against each enemy deck
(`tools/deck_eval.sh`). The enemies play better too, so every win rate
is lower than with the old protocol.

Baselines (seeds 9101-9106, 12,000 games): save_a_crafted_40 51.4%,
save_a_control_no_tribute_40 44.7%, save_a_best_overall_40 43.6%,
save_a_owner_current_40 42.2%, the owner's Dark Summoner deck 38.2%.

`tools/deck_search.py` searches one-card swaps over save A's collection
(485 cards since the 2026-11-22 list), within copy and ban list limits,
without the 8 unscripted EXTRA cards: each round screens random swaps on
300 games per enemy, confirms the best five on 1,500 per enemy against
the current deck on other seeds, and keeps a swap that gains enough. The
results are then measured on seed sets the search never used.

Two searches (from save_a_crafted_40 and from the owner's Dark Summoner
deck) reached 62.3% and 61.1%, but the owner found the decks made no
sense, and they did not: both put Gravity Bind (Level 4 and higher
monsters cannot attack) in decks whose monsters are almost all Level 4,
so it locked their own attackers too. They won by freezing the board:
29% of the searched deck's wins came from the opponent's deck running
out (crafted deck: 0%), and its games lasted 22 turns instead of 14.
The enemy bots never broke the lock (no removal aimed at Gravity Bind,
big monsters played into it), and the search exploited that. Without
Gravity Bind the deck fell to 55.0%. Both decks were removed (owner,
2026-11-22); the bots must learn to break a lock before the search runs
again.


### Breaking the lock (2026-11-22)

Four of the six enemy decks had no card that destroys a face-up Spell or
Trap, so nothing could break Gravity Bind. The owner says the game's
opponents have answers, so each got two (e6 one, e3 already had four):
Mystical Space Typhoon and Dust Tornado, in place of Waboku (e1, e4, e5,
e6) or two Graceful Charity (e2); see each deck's notes. The bots use
them: in an audit of 600 lock-deck games, Mystical Space Typhoon and
Dust Tornado were activated in 668 reviewed decisions, wrongly in 8%.

A bot setting that made a monster unable to attack count for less (so
removing a lock or not locking yourself would score higher) changed
nothing measurable at 0.75, 0.5 or 0.25, neither in the six mirrors nor
against the lock deck, and was taken out again.

The removed lock deck still wins 61.0% against the armed enemy decks
(crafted deck 52.6%), 17% of its wins by deck-out. With the strongest
pilot on both sides (the search bot with lookahead playouts, 150 games)
it beat Big beatdown 74.7% ±7.0 (crafted deck 53.3% ±8.0), 39 of its
112 wins by deck-out, games of 34 turns. Against these opponents the
lock is a real strength of the simulation, not only a bot blind spot;
whether it is a deck the owner wants is the owner's call.

That run also found a sampler bug: a card whose effect is on the chain
while it is face-down (Kiseitai, attacked) was dealt another identity in
the opponent's samples, so its link resolved an effect the new card does
not have and the game panicked. Chain cards and a card waiting on an
answer now keep their identity in samples (`PlayerView::sample`; test in
`crates/ygo-ai/tests/view.rs`).

### Win by damage only (owner, 2026-11-22)

The search now scores a deck by its damage score: a game won or lost by
deck-out counts as a draw (`tools/deck_eval.sh`, third number). On the
same 1,800 games the crafted deck scores 54.3% either way, the lock deck
60.4% by win rate but 55.2% by damage score. The search also never tries
a deck holding a lock card that stops both players' monsters if it would
stop more than half of its own (Gravity Bind: Level 4 or higher;
Messenger of Peace: 1500 ATK or more; `SELF_LOCKS` in
`tools/deck_search.py`); the lock deck fails that rule.

Under these rules the search from save_a_crafted_40 kept six swaps
(Summoned Skull, Rush Recklessly, Magic Jammer, Guardian of the Throne
Room, Spellbinding Circle and Judge Man out; Gearfried the Iron Knight,
Goblin Attack Force, Magic Drain, Man-Eater Bug, Blackland Fire Dragon
and Darkfire Soldier #2 in): `save_a_beatdown_40`, Level 4 attackers
with removal, 61.6% ±0.6 against 52.7% for the crafted deck over 24,000
games on two seed sets the search never used, the same by damage score.
The search from the owner's Dark Summoner deck (40.0%) reached 50.8%,
but four of its six additions left its DARK theme; it is being redone
with `ADD_ATTRIBUTE=DARK` (only DARK monsters, and any Spell or Trap,
may be added).

