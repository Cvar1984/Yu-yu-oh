# M3 plan: every remaining non-Normal card

Gate (from the handoff): all 228 non-Normal cards scripted, a rulings
test per card. After M2, 199 cards remain (the four Toon monsters carry
only their summon restriction so far and count among them).

## What the pool needs

| Group | Cards | New machinery |
| --- | --- | --- |
| Equip Spells that give a type +300 ATK/DEF, and the attribute or type Field Spells | 26 Equips, 11 Fields | none: a table drives them |
| Burn, Life Point gain and other one-shot Normal Spells and Traps | 25 | temporary stat changes that end at the End Phase; banished cards |
| Hand reveal and opponent-choice cards (Confiscation, The Forceful Sentry, Delinquent Duo, Painful Choice, Ceremonial Bell, Hiro's Shadow Scout, Big Eye) | 7 | a `Choose` whose chooser is the other player; revealed hands in observations; random picks from the seeded RNG |
| Polymerization and the 25 Fusion Monsters | 26 | Fusion Summon: material matching by card id, materials to the Graveyard, summon from the Extra Deck, under `fusion_summons` |
| The 4 Ritual Spells and 4 Ritual Monsters | 8 | Ritual Summon: tributes whose levels total at least the monster's, summon from the hand, under `ritual_summons`; Relinquished's absorb effect |
| Effect Monsters: direct attackers, conditional stats, recruiters that Special Summon on battle destruction, battle-damage triggers | 34 | direct-attack hook on the attack rules; Special Summon from the Deck with a search |
| Effect Monsters: ignition effects, coin tosses, position-change triggers, Standby Phase effects, attack costs, the Damage Step trio (Sanga, Kazejin, Suijin) and the summon-restricted monsters with their enablers (Gate Guardian, the Moths with Cocoon of Evolution, Harpie Lady Sisters with Elegant Egotist, Wall Shadow with Magical Labyrinth) | 36 | `Ignition` effects as Main Phase actions; a coin-toss operation; an attack-declaration cost hook; Special Summon from the hand by a card's own condition |
| FLIP monsters | 15 | reordering the top of the Deck (Big Eye); Cyber Jar's mass Special Summon under each summon setting |
| Toon World and the 4 Toon monsters | 5 | Special Summon from the hand while Toon World is up, direct attacks when the opponent has no Toons, destruction when Toon World leaves |
| Remaining Traps, Continuous Spells and special Equips (Horn of Heaven, Spellbinding Circle, Dragon Capture Jar, Fake Trap, Fairy's Hand Mirror, Robbin' Goblin, House of Adhesive Tape, Eatgaboon, Gravekeeper's Servant, Toll, Chain Energy, Chorus of Sanctuary, Snatch Steal, Megamorph, Stim-Pack, Germ Infection, Paralyzing Potion, Ring of Magnetism, Axe of Despair, Black Pendant, Horn of the Unicorn, Horn of Light, Sword of Deep-Seated) | 23 | attack-target restrictions and attack replays; costs to declare an attack and to Summon or activate; redirecting a Spell's target; a negation that tributes |

## Engine changes, all in step 1

- `Location::Banished` and a per-player banished pile; `banish(card)`.
- `EffectKind::Ignition` offered as a Main Phase action of the turn player, once per turn where the card says so (`CardInstance.used_this_turn`).
- `Flow::Choose` gains `chooser: PlayerId`; the chooser's observation reveals the options. `Side.hand` reveals cards while a reveal effect is active.
- `coin(&mut self) -> bool` and `random_pick` on the game's RNG, so replays stay exact.
- Temporary modifiers: `Lingering` gains a `StatMod { card, atk, def }` kind that expires at the End Phase.
- Attack rules take hooks from continuous effects: `can_attack_directly`, `must_attack(target)` (Ring of Magnetism), `attack_cost` (Toll, Gravekeeper's Servant, Dark Elf), and an attack whose target left the field replays against the remaining monsters instead of being cancelled.
- Fusion and Ritual Summon operations; `special_summon` learns to pull from the Extra Deck, the hand and the Deck.
- `Tag::DirectAttacker`, `Tag::Toon`, `Tag::OncePerTurn` where a tag is enough.

## Steps

Each step is one commit with its tests. Table-driven cards get one
parameterised test that checks every row (bonus present for a matching
monster, absent for another), which still gives each card its own
assertion.

1. **Engine machinery** listed above, with engine tests on synthetic scripts.
2. **Tables**: 26 type Equips, 11 Field Spells. Also the attribute-booster monsters (Star Boy, Milus Radiant, Bladefly, Witch's Apprentice, Hoshiningen, Little Chimera) and the Battleguards, since they are the same shape.
3. **One-shot Spells and Traps**: burn, Life Point gain, Remove Trap, Stop Defense, Block Attack, Heavy Storm, Giant Trunade, Eternal Rest, Two-Pronged Attack, Snake Fang, Rush Recklessly, The Reliable Guardian, Shield & Sword, Curse of Fiend, Darkness Approaches, The Cheerful Coffin, Share the Pain, Final Destiny, Soul Release, Gravedigger Ghoul, Tailor of the Fickle.
4. **Hand reveal and opponent choice**: the 7 cards above.
5. **Fusion**: Polymerization and the 25 Fusion Monsters from a materials table.
6. **Ritual**: the 4 Spells and 4 monsters.
7. **Effect Monsters A**: direct attackers, conditional stats (Shadow Ghoul, Muka Muka, Flash Assailant, Maha Vailo, Insect Soldiers of the Sky), recruiters and battle-destruction triggers (Giant Rat, UFO Turtle, Shining Angel, Mother Grizzly, Flying Kamakiri #1, Mystic Tomato, Giant Germ, Nimble Momonga), battle-damage triggers (White Magical Hat, Masked Sorcerer, The Bistro Butcher), summon triggers (Senju, Sonic Bird), discard and mill triggers (Penguin Knight, Electric Snake, Minar), control-change triggers (Ameba, Griggle).
8. **Effect Monsters B**: ignition effects (Catapult Turtle, Cannon Soldier, Time Wizard, Barrel Dragon, Karate Man, The Little Swordsman of Aile, Blast Juggler, Thunder Dragon), Jirai Gumo, Dark Elf, position-change triggers (Crass Clown, Dream Clown, Yado Karu, Tainted Wisdom), Standby effects (Pumpking, Mushroom Man #2, Dark Zebra), Sanga, Kazejin, Suijin, Steel Scorpion, Electric Lizard, Kotodama, Boar Soldier, The Unhappy Maiden, Banisher of the Light, and the summon-restricted monsters with their enablers.
9. **FLIP monsters**: the 15, with Cyber Jar tested under every summon setting.
10. **Toons**: Toon World and the 4 monsters.
11. **Remaining Traps, Continuous Spells and special Equips**: the 23 listed above.
12. **Completeness, fuzz, speed, docs**: a test that every non-Normal id in the pool has a script; the million-game fuzz with the sample decks and with a full-pool random deck; speed re-measured; CLAUDE.md and this plan updated; gate report.

## Text versions

`cards.json` holds modern text and every script follows it. The plan
promised a list of cards whose 2002 printing plays differently; it was
not built, because `cards.json` has no 2002 text to compare against and
a list from memory would be guesswork. Building it, and branching the
scripts on `game.text_version()`, waits for an original-text data file.

## Owner's answers (2026-11-17)

- Cyber Jar Special Summons as many revealed Level 4 or lower monsters
  as the summon settings allow, the controller choosing which; the rest
  go to the hand.
- Toons play by the modern text: Special Summon from the hand while
  Toon World is up, direct attacks when the opponent controls no Toon
  monster, destroyed when Toon World leaves, no attack in the turn they
  are summoned.
- Original 2002 text: scripts follow modern text only; the list of
  cards whose 2002 printing differs is kept here and branched once an
  original-text data file exists.

## Progress

All twelve steps are done; the gate report is at the end of this file.

What changed from the plan while building:

- Step 1's machinery grew in later steps, each time a card needed it:
  battle-damage and destruction-replacement hooks (Relinquished), a
  record of which player's effect discarded or milled a card (Electric
  Snake, Minar, Penguin Knight), timed destruction and attack bans
  (Steel Scorpion, Electric Lizard), stat changes that end with damage
  calculation (Sanga, Kazejin, Suijin), banish-instead (Banisher of the
  Light), a position lock (Dragon Capture Jar), a second Battle Phase
  (Weather Report), control swaps (Invader of the Throne), questions
  either player answers (Cyber Jar), destruction shields (Fake Trap),
  retargeting a chain link (Fairy's Hand Mirror) and the hand-play toll
  (Chain Energy).
- Battle damage is now applied before battle destruction; the rules
  treat them as simultaneous, and Relinquished's reflection needs the
  field intact.
- Set Traps can be activated directly in the Main Phase, not only in a
  response window.
- Dragon Capture Jar moved from step 11 to step 9, next to Dragon Piper.
- Penguin Knight's test moved to step 11, where Gravekeeper's Servant
  can mill it.

Simplifications, each a candidate for a later rulings pass:

- Catapult Turtle burns half the tributed monster's printed ATK, not its
  ATK on the field.
- Time Wizard and Jirai Gumo do not ask for the coin call; a call is a
  50/50 either way.
- Castle of Dark Illusions boosts every Zombie, not only those on the
  field when it flipped.
- Spear Cretin revives in Attack Position only; Cyber Jar asks for the
  position.
- Insect Soldiers of the Sky gain their 1,000 ATK for the whole attack,
  not only the Damage Step.
- A Toon's tribute summon still needs a free Monster Zone before the
  tributes are paid.
- Snatch Steal returns control when it goes to the Graveyard, but not
  when it returns to the hand.
- Fake Trap recognises destruction aimed at its controller's Traps by
  targets, plus Heavy Storm and Final Destiny by name.
- Ritual Spells offer tribute sets of up to three monsters.
- Still open from M2: Monster Reborn revives in Attack Position only,
  Field Spells activate from the hand only.

## Gate report (2026-11-17)

Gate: all 228 non-Normal cards scripted, a rulings test per card.

| Check | Evidence |
| --- | --- |
| Every non-Normal card scripted | `every_non_normal_card_in_the_pool_has_a_script` in `crates/ygo-cards/tests/registry.rs` |
| A rulings test per card | 172 card tests (263 in the workspace); table-driven cards are checked row by row, and every Fusion Monster whose materials exist (23 of 25) and every Ritual monster is actually summoned in a test |
| Sample decks, random play | 1,000,000 games over four deck pairs, no panics, no refused action, no draws |
| Whole pool, random play | 5,000 property cases with random legal decks drawn from all 374 cards, invariants checked after every action; it found one stale invariant (hidden hands under Ceremonial Bell) |
| Speed | 1,700 to 2,200 games/s, 300,000 to 330,000 actions/s, one core, release |

Random-play results, 250,000 games per pair, half with each player first:

| A | B | A wins | A first | B first | Mean turns |
| --- | --- | --- | --- | --- | --- |
| Owner's main 45 | Umiiruka WATER 40 | 35.4% ±0.2 | 35.2% | 35.6% | 44 |
| Defense-tuned 45 | Exodia stall 40 | 89.9% ±0.1 | 89.2% | 90.6% | 29 |
| Owner's main 45 | Owner's main 45 | 50.1% ±0.2 | 49.7% | 50.5% | 38 |
| Umiiruka WATER 40 | Defense-tuned 45 | 68.2% ±0.2 | 68.4% | 68.0% | 42 |

Reading these numbers: the mirror match is on 50% again. With every
effect live, the Umiiruka deck pulls further ahead of the owner's deck
(64.6% against it, up from 56% at M2), close to the prototype's 60%
for that deck. Random play still cannot pilot combo or stall plans, so
the Exodia deck stays near 10%; the heuristic bots of M4 are the next
real measure.
