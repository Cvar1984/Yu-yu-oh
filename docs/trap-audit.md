# Trap audit against the TCG rules (2026-11-27)

Owner (2026-11-27): "Trap card activation is wrong … check the international
rules for each Trap card." Every one of the 71 Trap Cards in the pool (44
Normal, 21 Continuous, 6 Counter) was read against its official card text
(`data/cards.json`, from YGOPRODeck) and the TCG rulings for when it may be
activated, what it targets and what it does. Rulings sites (Yugipedia, the
official database) are not reachable from this container; the card text is,
and each finding below is proven by a rulings test.

## Findings and fixes

| Card | Found | Fix |
| --- | --- | --- |
| Imperial Order | Paid 700 LP only in its controller's Standby Phase; the text says "Once per turn, during the Standby Phase", every Standby Phase. | Pays in each Standby Phase, both players'. |
| Imperial Order | Spells could not be activated while it was face-up (a simplification in `docs/psv-plan.md`). By the TCG rulings Spells may be activated; their effects are negated. | `Tag::NegatesEffectsOnly`: a card it negates may still be activated from the hand or while Set and resolves without effect. Jinzo keeps stopping Trap activations, as its text says. |
| Backup Soldier | Picked its cards on resolution; the text targets "up to 3" on activation (so the opponent can answer the targets). | Targets 1 to 3 monsters on activation; adds those still in the Graveyard. |
| Enchanted Javelin | Only the opponent's attacking monster; the text says "1 attacking monster", either player's. | Any attacking monster. |

Left as it was: an Equip Spell activated under Imperial Order resolves
without effect and is not equipped (by the rulings it equips with its
effects negated); it then leaves the field like any Equip Spell without a
target.

## The general activation rules, tested

`crates/ygo-cards/tests/trap_timing.rs` walks real play and records in which
windows a player is offered each Trap. With `traps.rs` (Set-this-turn rule,
Trap Hole, chains and speeds) these hold:

- Mirror Force only when an opponent's monster declares an attack, not in
  the Damage Step.
- Trap Hole after a Normal (or Tribute) or Flip Summon, never after a
  Special Summon or a Set.
- While a summon is being declared only Counter Traps may answer (Solemn
  Judgment yes, Waboku no); Normal Traps wait for the summon to succeed.
- In the Damage Step only cards that change ATK or DEF (Reinforcements),
  not Waboku.
- A Set Trap cannot be activated in the turn it was Set
  (`trap_activation_turn_set = false`), and can be in its controller's own
  Main Phase from the next turn.

## Cards found right (68)

- `traps.rs`: Trap Hole, Solemn Judgment, Magic Jammer, Seven Tools of the
  Bandit, Mirror Force.
- `oneshot.rs`: Two-Pronged Attack, Snake Fang.
- `remaining.rs`: Fake Trap, Horn of Heaven, Robbin' Goblin, Spellbinding
  Circle, Fairy's Hand Mirror, House of Adhesive Tape, Eatgaboon (both on a
  face-up Normal, Tribute or Flip Summon by the opponent).
- `flips.rs`: Dragon Capture Jar.
- `psv_traps.rs` (all but Imperial Order): Appropriate, Armored Glass, DNA
  Surgery, Metal Detector, Mystic Probe, The Regulation of Tribe, World
  Suppression, Attack and Receive, Call of the Haunted, Ceasefire, Chain
  Destruction, Driving Snow, Earthshaker, Forced Requisition, Gamble,
  Graverobber, Lightforce Sword, Magical Hats, Major Riot, Respect Play,
  Solomon's Lawbook, Time Seal, Gravity Bind, Gust, Infinite Dismissal,
  Magic Drain, Michizure, Minor Goblin Official, Mirror Wall, Numinous
  Healer, Shadow of Eyes, Skull Invitation, Solemn Wishes, Type Zero Magic
  Crusher.
- `extra_traps.rs` (all but Backup Soldier and Enchanted Javelin): Gift of
  The Mystical Elf, The Eye of Truth, Dust Tornado, Light of Intervention,
  Shift, Burst Breath, Castle Walls, D. Tribe, Disappear, Gryphon Wing, Jar
  of Greed, Just Desserts, Raigeki Break, Reinforcements, Reverse Trap,
  Riryoku Field, Shadow Spell, Ultimate Offering, Waboku.
