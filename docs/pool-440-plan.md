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
| 2. Batch 1 | this commit | 36 cards scripted (Fairy Meteor Crush moved here from batch 2), 29 tests in `tests/extra.rs`; 8 pending |

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

## Deck crafting from save A (paused, 2026-11-20)

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

Next, when resumed: validate the best overall deck with Solemn Judgment
for Scapegoat, and that deck with Dust Tornado and Raigeki Break for the
two Horn of the Unicorn, at 400 or more games per enemy with the search
bot, then save the winner as `save_a_crafted_40.json`. Scripting
Scapegoat (batch 2) changes the baseline.

