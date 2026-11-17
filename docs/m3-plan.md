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

`cards.json` holds modern text and every script follows it. Where the
2002 printing plays differently, the script branches on
`game.text_version()` and the plan records the card. Known so far:
Fissure (2002: any monster, modern: face-up only), Cyber Jar and
Painful Choice (choice ownership), Delinquent Duo (random versus
chosen). The list grows as cards are scripted; nothing is branched
until the original-text data exists, see the questions below.

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
