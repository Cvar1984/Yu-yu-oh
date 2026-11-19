# Card and rules audit (2026-11-18)

The owner's standard, 2026-11-18: the basic rules and every card follow
the modern official rules and the modern card text in `data/cards.json`.
This audit compares each of the 228 card scripts, and the engine rules
they rely on, against that standard. Each finding lists its fix; the
fixes were made in three batches, and their status is under Fix status.

Verdicts: **OK** matches modern text and rulings. **Wrong** plays
differently from the text. **Partial** right in the common case, wrong in
a named case.

## Summary

Of the 228 card scripts, **176 match their modern text**, **38 are
partly wrong** (right in the common case, wrong in a named one) and **14
are wrong**. On top of that, **13 rules in the engine itself (C1 to C13)
differ from the modern rules**, and those reach many cards at once: a
card can follow its own text and still play wrong because of them.

The ones a player meets most often:

- **C2**: Solemn Judgment and Horn of Heaven can negate a monster Set.
- **C9**: every Special Summon without a stated position (Monster Reborn,
  Fusion and Ritual Summons, Elegant Egotist and others) puts the monster
  in Attack Position; the player should choose Attack or Defense.
- **C6**: an effect that changes a monster's position uses up its
  once-per-turn manual change.
- **C1 and C8**: Gate Guardian, the Moths and the Toons are summoned by
  a chain effect instead of a summon procedure, and any monster can be
  revived without ever being properly summoned.
- **C12**: two 0-ATK monsters in Attack Position destroy each other.
- **C10**: a bounced Fusion Monster goes to the hand.
- **Card-specific**: Sangan and Witch of the Black Forest miss their
  name lock and once-per-turn; Mystical Space Typhoon cannot hit a Field
  Spell; Fissure and Trap Hole read printed ATK instead of current ATK.

A word on method: this audit compares the code with the modern text and
rulings as I understand them, card by card; the same person wrote the
scripts, so a misunderstanding shared by both would not show here. The
owner's own examples from play are the best check on it.

## Fix plan

Each batch ends with tests that state the modern text, `scripts/check.sh`,
the bot evaluation rerun with `ygo sim`, and the web game rebuilt.

1. **Core rules a player meets every game**: C2, C12, C9, C6, C10, C11,
   C4.
2. **Core rules that need new engine state**: C1 (properly summoned),
   C8 (summon procedures), C3 (card activation versus effect), C7
   (battle-damage triggers as chain links), C5 (Set Field Spells), C13
   (missing the timing).
3. **Card-specific fixes**: every Wrong and Partial row below that the
   core batches do not already settle, module by module.

## Fix status (2026-11-19)

All three batches are done: every Wrong and Partial row in this report
is fixed, apart from the three exceptions below. `scripts/check.sh`
passes with 323 tests.

| Batch | Commit | Rows | Tests |
| --- | --- | --- | --- |
| 1. Everyday core rules | `2629a0b` | C2, C4, C6, C9, C10, C11, C12 | `tests/modern_rules.rs` |
| 2. Core rules with new engine state | `1badb9d` | C1, C3, C5, C7, C8, C13 | `tests/modern_rules.rs` |
| 3. Card-specific fixes | `19b47b9` | every remaining Wrong and Partial card row | `tests/card_fixes.rs`, and the module tests moved to the modern text |

What changed in the engine for these fixes: a Set completes with no
negation window; summon procedures (`Action::ProcedureSummon`, with a
summon-negation window) for Gate Guardian, the Moths and the Toons;
"properly summoned" per instance (`special_summonable`,
`special_summon_properly`); a chain link records whether a card was
activated (`card_activation`) and a number its cost left for the
resolution (`value`, Catapult Turtle); Set Field Spells; optional "When"
triggers miss the timing (`misses_timing`); once-per-turn names and name
locks (`use_name_once`, `lock_name_this_turn`); `can_pay_lp`; Extra Deck
monsters return to the Extra Deck; a monster turned face-down loses its
Equips; position questions (`positions.rs`); battle-damage triggers on
`Event::BattleDamageDealt`; marks (`LingeringKind::Marked`, Castle of Dark
Illusions); tags `ControlWhileEquipped` (Snatch Steal) and
`StaysWithoutTarget` (Spellbinding Circle).

Exceptions, left as they are on purpose:

- **Soul Release and Gravedigger Ghoul** still pick their cards on
  resolution. Targeting on activation would offer every subset of up to
  five (or two) Graveyard cards as a separate action, which floods the
  action list for players and bots alike. The cards still end up where
  the text says; only the opponent's chance to respond to the chosen
  targets is missing.
- **Jirai Gumo and Time Wizard** toss the coin without asking for a call.
  A call does not change the odds, so the result is the same.
- **Fake Trap** knows the effects that destroy several Traps at once by a
  list (Heavy Storm, Final Destiny, and now Dragon Piper's FLIP when its
  controller has a face-up Dragon Capture Jar), not by asking each
  effect what it would destroy. Every such card in this pool is on the
  list.

## Core rules

| # | Finding | Modern rule | Cards affected | Fix |
| --- | --- | --- | --- | --- |
| C1 | No "properly summoned" state. A monster that must first be Special Summoned a certain way (Gate Guardian, Harpie Lady Sisters, Wall Shadow, Larvae Moth, Great Moth, the Toons), or a Fusion or Ritual monster that was not Fusion or Ritual Summoned, can be Special Summoned by any effect. | Such a monster can be Special Summoned by another effect only after it was properly summoned once (and then only from the Graveyard or banishment); it can never be summoned from the hand or Deck by other effects. | Monster Reborn, every "Special Summon from the Deck/Graveyard" effect | Track "properly summoned" per instance; `special_summon` refuses the others; target lists skip them. |
| C2 | Setting a monster opens the same negation window as a summon. | Setting is not a summon: nothing that responds to "would be Summoned" (Solemn Judgment, Horn of Heaven) can respond to a Set. | Solemn Judgment, Horn of Heaven | A Set completes at once, with no summon-negation window. |
| C3 | A chain link does not record whether a card was activated or an effect of a face-up card was used. | "When a Spell/Trap Card is activated" counters negate card activations only, never the effect of a Spell or Trap already face-up (Magical Labyrinth's Tribute, for example). | Solemn Judgment, Magic Jammer, Seven Tools of the Bandit | Mark each link as a card activation or an effect; the counters check it. |
| C4 | Several scripts require more LP than a cost (`lp > 500` to pay 500). | A player may pay an LP cost equal to their remaining LP (and then loses). | Seven Tools of the Bandit, Malevolent Nuzzler, others noted below | One helper, `can_pay_lp`, used by every LP cost. |
| C5 | Field Spells can only be activated from the hand. | A Field Spell may also be Set face-down in the Field Zone and activated later. | All 13 Field Spells | Offer a Set for Field Spells; activate a Set one like any Set Spell. |
| C6 | A position change by a card effect marks the monster as having changed position this turn, so its controller can no longer change it by hand. | Only a manual change uses the once-per-turn position change; a change by an effect does not. | Block Attack, Stop Defense, Curse of Fiend, Dream Clown, Crass Clown, Yado Karu and every effect that changes position | `set_position` from an effect leaves the manual-change flag alone. |
| C7 | "When this card inflicts battle damage" effects are applied at once by a hidden hook instead of as trigger effects. | They are trigger effects: they start a chain after damage calculation, and the opponent may respond (Solemn-type cards cannot, but Quick-Play Spells and Traps can). | White Magical Hat, Masked Sorcerer, The Bistro Butcher, Robbin' Goblin | Turn them into triggers on the battle-damage event. |
| C8 | Gate Guardian, Larvae Moth and Great Moth are Special Summoned by an effect that goes on the chain. | Their own summon from the hand is a summon procedure, not an effect: it does not start a chain, and Solemn Judgment or Horn of Heaven can negate it as a summon. It also counts as the proper summon (C1). | Gate Guardian, Larvae Moth, Great Moth, and the Toon monsters (checked below) | A "summon by procedure" action with a summon-negation window, like a Normal Summon. |
| C9 | Every Special Summon without a stated position puts the monster in face-up Attack Position. | When the text names no position, the player chooses face-up Attack or face-up Defense Position. | Monster Reborn, Elegant Egotist, Magical Labyrinth, Gate Guardian, the Moths, Ritual Summons, Fusion Summons, others below | Ask the position whenever the text does not fix it. |
| C10 | A Fusion monster sent to the hand or the Deck goes there. | An Extra Deck monster that would go to the hand or the Main Deck returns to the Extra Deck instead. | Hane-Hane, Crass Clown, Penguin Knight, any bounce or shuffle | `add_to_hand` and `return_to_deck` send Extra Deck monsters to the Extra Deck. |
| C11 | A monster turned face-down (Darkness Approaches) keeps its Equip Cards. | An Equip Card is destroyed when its monster is turned face-down, as when it leaves the field. | Darkness Approaches with any Equip | Destroy the Equips when a monster goes face-down. |
| C12 | When two Attack Position monsters with equal ATK battle, both are destroyed even at 0 ATK. | If both have 0 ATK, neither is destroyed. | Any 0-ATK monster (Labyrinth Wall in Attack Position, Kotodama, Ceremonial Bell, The Unhappy Maiden, Relinquished without a monster) | No destruction when both ATK are 0. |
| C13 | No optional trigger can "miss the timing". | An optional "When ... you can" trigger whose event was not the last thing to happen cannot be activated. In this pool it matters only for a few Equips sent to the Graveyard in the middle of a chain (Axe of Despair, Malevolent Nuzzler, Horn of Light). | Axe of Despair, Malevolent Nuzzler, Horn of Light | Mark optional "When" triggers and drop them when their event was not last. |

## Spells (`spells.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Pot of Greed | OK | | |
| Raigeki | OK | | |
| Dark Hole | OK | | |
| Fissure | Wrong | Compares printed ATK, not current ATK (Field Spells, Equips and boosts change who is lowest). | Compare `g.atk`. |
| Upstart Goblin | OK | | |
| Tribute to the Doomed | OK | | |
| Monster Reborn | Wrong | Always Attack Position; may revive a monster that was never properly summoned (C1). | Ask Attack or Defense Position; apply C1. |
| Change of Heart | OK | | |

## Traps and Mystical Space Typhoon (`traps.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Mirror Force | OK | | |
| Trap Hole | Partial | Checks printed ATK; a monster at 1000+ ATK only through a Field Spell or boost is missed, and one printed at 1000+ but lowered is caught. | Check current ATK on the field. |
| Solemn Judgment | Wrong | Can negate a monster Set (C2); can negate the effect of a face-up Spell or Trap (C3). | C2, C3. |
| Magic Jammer | Partial | Can negate the effect of a face-up Spell (C3). | C3. |
| Seven Tools of the Bandit | Partial | Can negate the effect of a face-up Trap (C3); refuses to pay at exactly 1000 LP, which modern rules allow. | C3; allow LP equal to the cost. |
| Mystical Space Typhoon | Wrong | Cannot target a Field Spell in the Field Zone. | Add Field Zone cards to its targets. |

## Equip, Field and Continuous Spells (`continuous.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Malevolent Nuzzler | Partial | Will not pay 500 LP at exactly 500 LP (C4). | C4. |
| Legendary Sword | OK | | |
| Umi | OK | Can only be activated from the hand (C5). | C5. |
| Messenger of Peace | OK | | |
| Swords of Revealing Light | OK | | |

## Table cards (`tables.rs`): 14 type Equips, 11 Field Spells, 6 attribute boosters, 2 Battleguards

All 33 match their modern text: type Equips give 300 ATK/DEF to a face-up monster of their type; type Field Spells give 200 ATK/DEF (Yami also takes 200 from Fairies); attribute Field Spells give 500 ATK and take 400 DEF; the boosters (Star Boy, Milus Radiant, Bladefly, Witch's Apprentice, Hoshiningen, Little Chimera) give 500 ATK and take 400 ATK while face-up; each Battleguard gains 500 ATK per face-up partner its controller controls. The Field Spells share C5.

## One-shot Spells and Traps (`oneshot.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Sparks, Hinotama, Final Flame, Tremendous Fire | OK | | |
| Red Medicine, Goblin's Secret Remedy | OK | | |
| Remove Trap | OK | | |
| Stop Defense | OK | Its change uses up the monster's manual change (C6). | C6. |
| Block Attack | OK | Same as Stop Defense (C6). | C6. |
| Heavy Storm | OK | | |
| Giant Trunade | OK | | |
| Eternal Rest | OK | | |
| Two-Pronged Attack | OK | | |
| Snake Fang | OK | | |
| Rush Recklessly | OK | | |
| The Reliable Guardian | OK | | |
| Shield & Sword | OK | | |
| Curse of Fiend | Partial | Leaves face-down Defense Position monsters alone; they are Defense Position monsters, so they change to face-up Attack Position. | Include face-down monsters (flipping them face-up). |
| Darkness Approaches | OK | | |
| The Cheerful Coffin | Partial | Picks the monsters to discard when it is activated; the text discards on resolution, so the picks are made then. | Choose up to 3 monsters at resolution. |
| Share the Pain | OK | | |
| Final Destiny | OK | | |
| Soul Release | Partial | Picks the cards on resolution; modern text targets them on activation ("Target up to 5 cards"), so they can be responded to. | Pick targets on activation. |
| Gravedigger Ghoul | Partial | Same as Soul Release, up to 2 monsters in the opponent's Graveyard. | Pick targets on activation. |
| Tailor of the Fickle | OK | | |

## Cards that reveal or let the other player choose (`reveal.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Confiscation | Partial | Will not pay 1000 LP at exactly 1000 LP (C4). | C4. |
| The Forceful Sentry | OK | | |
| Delinquent Duo | Partial | Same as Confiscation (C4). | C4. |
| Painful Choice | OK | | |
| Ceremonial Bell | OK | | |
| Hiro's Shadow Scout | OK | | |
| Big Eye | OK | | |

## Rituals (`ritual.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Black Illusion Ritual, Commencement Dance, Hamburger Recipe, Turtle Oath | Partial | The Ritual Monster always enters in Attack Position (modern: face-up Attack or face-up Defense, the player's choice); tribute sets stop at three monsters; a set may include a monster whose Levels were not needed (three Level 4s for a Level 6). | Ask the position; any number of tributes; refuse sets with a surplus monster. |
| Performance of Sword, Hungry Burger, Crab Turtle | OK | Revivable without a proper Ritual Summon first (C1). | C1. |
| Relinquished | OK | Same C1 note. | C1. |

## Monsters (`monsters.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Exodia the Forbidden One and the four limbs | OK | | |
| Sangan | Wrong | Missing both modern clauses: the searched card's name cannot be activated (as a card or an effect) for the rest of the turn, and Sangan's effect can be used only once per turn. | Add a name lock lingering until the End Phase; once-per-turn by card name. |
| Witch of the Black Forest | Wrong | Same two missing clauses as Sangan. | As Sangan. |
| Man-Eater Bug | OK | | |
| Magician of Faith | OK | | |
| Kuriboh | OK | | |

## Effect Monsters, batch A (`monsters_a.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Mystic Lamp, Leghul, Ooguchi, Jinzo #7, Rainbow Flower, Queen's Double | OK | | |
| Shadow Ghoul | OK | | |
| Muka Muka | OK | | |
| Flash Assailant | OK | | |
| Maha Vailo | Partial | Counts only Equips its own controller controls; an opponent's Equip on it is still "an Equip Card equipped to this card". | Count every Equip whose target is Maha Vailo. |
| Insect Soldiers of the Sky | Partial | The 1000 ATK lasts the whole attack, not only the Damage Step (known M3 shortcut). | Apply it only while the Damage Step runs. |
| Giant Rat, UFO Turtle, Shining Angel, Mother Grizzly, Flying Kamakiri #1, Mystic Tomato | OK | May recruit a monster that cannot be Special Summoned from the Deck (C1). | C1. |
| Giant Germ | OK | | |
| Nimble Momonga | OK | | |
| White Magical Hat | Partial | Not a chain link (C7). | C7. |
| Masked Sorcerer | Partial | Not a chain link (C7). | C7. |
| The Bistro Butcher | Partial | Not a chain link (C7). | C7. |
| Senju of the Thousand Hands | OK | | |
| Sonic Bird | OK | | |
| Penguin Knight | OK | | |
| Electric Snake | OK | | |
| Minar | OK | | |
| Ameba | OK | | |
| Griggle | OK | | |

## Effect Monsters, batch B (`monsters_b.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Catapult Turtle | Partial | Burns half the Tributed monster's printed ATK; the text says its ATK on the field (known M3 shortcut). | Record the ATK as the cost is paid. |
| Cannon Soldier | OK | | |
| Time Wizard | OK | The coin call is not asked; with a fair coin the odds are the same. | Ask the call, for the feel of the card. |
| Barrel Dragon | OK | | |
| Karate Man | OK | | |
| The Little Swordsman of Aile | OK | | |
| Blast Juggler | OK | | |
| Thunder Dragon | OK | | |
| Jirai Gumo | Partial | The coin toss happens at once as a hidden hook, not as a trigger on attack declaration (as C7); the call is not asked. | A trigger on its attack declaration. |
| Dark Elf | Partial | Cannot attack at exactly 1000 LP (C4). | C4. |
| Crass Clown | OK | | |
| Dream Clown | OK | | |
| Yado Karu | OK | | |
| Tainted Wisdom | OK | | |
| Pumpking the King of Ghosts | Partial | When Castle of Dark Illusions leaves, the count resets only at the next Standby Phase; if Castle returns first, the old gains come back. | Reset the count as soon as Castle leaves the field. |
| Mushroom Man #2 | Partial | Will not pay 500 LP at exactly 500 LP (C4). | C4. |
| Dark Zebra | OK | | |
| Sanga of the Thunder, Kazejin, Suijin | OK | | |
| Steel Scorpion | OK | | |
| Electric Lizard | OK | | |
| Kotodama | OK | | |
| Boar Soldier | OK | | |
| The Unhappy Maiden | Partial | Ends the Battle Phase when destroyed by battle even if it is banished instead (Banisher of the Light); the text needs it sent to the Graveyard. | Check that it reached the Graveyard. |
| Banisher of the Light | OK | | |
| Gate Guardian | Wrong | Summoned by a chain effect (C8); always Attack Position (C9). | C8, C9. |
| Larvae Moth, Great Moth | Wrong | As Gate Guardian (C8, C9). | C8, C9. |
| Cocoon of Evolution | OK | | |
| Harpie Lady Sisters, Wall Shadow | OK | Revivable without a proper summon (C1). | C1. |
| Elegant Egotist | Partial | Always Attack Position (C9). | C9. |
| Magical Labyrinth | Partial | Always Attack Position (C9). | C9. |

## FLIP monsters and Dragon Capture Jar (`flips.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Reaper of the Cards | OK | | |
| Armed Ninja | OK | | |
| Hane-Hane | Partial | A Fusion monster it returns goes to the hand (C10). | C10. |
| Mask of Darkness | OK | | |
| Dragon Piper | OK | | |
| Dragon Capture Jar | OK | | |
| Castle of Dark Illusions | Partial | Boosts every Zombie, including those that arrive later; the text boosts only "those" on the field when it flipped (known M3 shortcut). | Record the Zombies present at the flip. |
| Princess of Tsurugi | OK | | |
| The Immortal of Thunder | OK | | |
| Weather Report | OK | | |
| Invader of the Throne | OK | | |
| Jigen Bakudan | OK | | |
| Cyber Jar | OK | Plays by the owner's ruling (as many as the summon settings allow, controller's choice); a Level 4 or lower Ritual monster could be summoned from the Deck (C1). | C1. |
| Spear Cretin | Partial | Always face-up Attack Position (the text offers face-down Defense too); refuses every Fusion monster, even one properly Fusion Summoned (C1 is the right test). | Ask the position; use C1 instead of the Extra Deck filter. |

## Toons (`toons.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Toon World | Partial | Will not pay 1000 LP at exactly 1000 LP (C4). | C4. |
| Toon Mermaid, Toon Summoned Skull, Blue-Eyes Toon Dragon, Manga Ryu-Ran | Wrong | Summoned by a chain effect instead of a summon procedure (C8); always Attack Position (C9); cannot attack at exactly 500 LP (C4); survive when Toon World is destroyed and banished instead of sent to the Graveyard. | C8, C9, C4; trigger on Toon World's destruction, wherever it goes. |

## Fusion (`fusion.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Polymerization | Partial | Picks the Fusion Monster and materials on activation; modern Polymerization picks them on resolution. Always Attack Position (C9). With five monsters on the field it cannot summon even when the materials would free a zone. | Choose on resolution; ask the position; free the materials before checking for a zone. |
| The 25 Fusion Monsters | OK | No effects of their own; C1 (revival needs a proper Fusion Summon first) and C10 (returned to the Extra Deck, not the hand or Deck). Rabid Horseman and Cyber Saurus cannot be summoned from this pool (a material is missing), as printed. | C1, C10. |

## Remaining Spells, Traps and Equips (`remaining.rs`)

| Card | Verdict | What is wrong | Fix |
| --- | --- | --- | --- |
| Sword of Deep-Seated | OK | | |
| Germ Infection | OK | | |
| Paralyzing Potion | OK | | |
| Ring of Magnetism | OK | | |
| Stim-Pack | OK | | |
| Axe of Despair | OK | | |
| Black Pendant | OK | | |
| Horn of Light | Partial | Will not pay 500 LP at exactly 500 LP (C4). | C4. |
| Horn of the Unicorn | OK | | |
| Megamorph | OK | | |
| Snatch Steal | Partial | Control returns only when Snatch Steal goes to the Graveyard, not when it leaves the field another way (known M3 shortcut). | Return control whenever it leaves the field. |
| Spellbinding Circle | Partial | Goes to the Graveyard whenever its monster leaves the field (bounced, banished); the text destroys it only when that monster is destroyed. | Destroy it on the monster's destruction; otherwise it stays, with no effect. |
| Robbin' Goblin | Partial | Not a chain link (C7). | C7. |
| Gravekeeper's Servant | OK | | |
| Toll | Partial | A player at exactly 500 LP cannot pay to attack (C4). | C4. |
| Chain Energy | OK | | |
| Chorus of Sanctuary | OK | Field Spell (C5). | C5. |
| Fairy's Hand Mirror | OK | | |
| Fake Trap | Partial | Knows mass Trap destruction only by name (Heavy Storm, Final Destiny); misses Dragon Piper, whose FLIP destroys face-up Dragon Capture Jars without targeting (known M3 shortcut). | Ask each effect which cards it would destroy. |
| Horn of Heaven | Wrong | Can negate a monster Set (C2). | C2. |
| House of Adhesive Tape | OK | | |
| Eatgaboon | OK | | |
