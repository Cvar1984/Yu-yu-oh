# Pharaoh's Servant scripts (2026-11-21)

The owner added Pharaoh's Servant (PSV) as the game's fourth set: 105
cards, 95 new to the pool (535 cards). 10 PSV cards were already EXTRA
cards and keep their scripts. The 70 new non-Normal cards are scripted
here, on the modern text, each with a rulings test in
`crates/ygo-cards/tests/psv_*.rs`. Scripts live in `psv_monsters`,
`psv_spells` and `psv_traps`. The registry test's `PENDING` list shrinks
with every batch.

Owner's answers (2026-11-21): do everything now; save A owns 90 of the
105 (screen of 2026-11-21), recorded at 1 copy each until the counts are
read.

## Batches

Batch 1 uses the engine as it is, plus `turn_face_up_quietly` (a
monster turned face-up without its FLIP effect). Done: 41 cards,
`tests/psv_1.rs`.

| Card | Kind | Status |
| --- | --- | --- |
| Rain of Mercy, Bite Shoes, 4-Starred Ladybug of Doom, Bubonic Vermin | one-shot | done |
| Mad Sword Beast, Buster Blader, Gravity Bind, Insect Barrier | continuous | done |
| Limiter Removal, Monster Recovery, Insect Imitation, The Shallow Grave | Spells | done |
| Nobleman of Extermination, Chain Destruction, Michizure, Gust, Driving Snow | removal | done |
| Numinous Healer, Attack and Receive, Minor Goblin Official, Burning Land | LP | done |
| Invitation to a Dark Sleep, The Fiend Megacyber, Gearfried the Iron Knight, Drill Bug | monsters | done |
| Call of the Haunted, Solemn Wishes, Skull Invitation, Type Zero Magic Crusher | Continuous Traps | done |
| 7 Completed, Earthshaker, Inspection, Bombardment Beetle | choices | done |
| Ceasefire, Shadow of Eyes, Magic Drain, Appropriate, Forced Requisition | windows | done |
| Infinite Dismissal, Mirror Wall, Kiseitai | battle and End Phase | done |

Batch 2 adds turn and battle features: a skipped Draw Phase, Standby
Phase or turn (Time Seal, Solomon's Lawbook, Gamble), a second attack
(Hayabusa Knight), the monsters a card destroyed by battle (Sword Hunter,
Vampire Baby), a position lock that lasts into the next turn (Goblin
Attack Force), a "no Spells or Traps" lock (Cold Wave), a turn player's
revealed hand (Respect Play), and the field-wide shuffles of Morphing
Jar #2 and Major Riot, and Thousand-Eyes Restrict.

Batch 3 adds effect negation and current Types: Jinzo, Imperial Order,
Armored Glass, World Suppression, Mystic Probe, Metal Detector,
Prohibition, DNA Surgery, The Regulation of Tribe, Parasite Paracide,
The Legendary Fisherman, Deepsea Warrior.

Batch 4 adds zone locks, delayed returns and borrowed cards: Ground
Collapse, Dimensionhole, Lightforce Sword, Graverobber, Magical Hats.

## Simplifications

Batch 1:

- Bite Shoes, Bombardment Beetle, The Shallow Grave (the opponent's
  monster) and 7 Completed (ATK or DEF) choose on resolution, not on
  activation.
- Kiseitai is a mandatory trigger in the attack-declaration window; the
  attack is cancelled, so no damage calculation.
- The Fiend Megacyber's Special Summon is a summon procedure.
- Infinite Dismissal notes each Level 3 or lower Normal or Flip Summon
  with a chain link and schedules its destruction for that End Phase; the
  destruction happens even if Infinite Dismissal left the field first.
- Mirror Wall halves the printed ATK (a modifier cannot read the current
  ATK it changes).
- Skull Invitation counts the cards of one window together and burns
  each owner once for them.
- Solemn Wishes and Appropriate see a Draw Phase draw in the Standby
  Phase window that follows it.
- Type Zero Magic Crusher's effect is a Main Phase action.
- Chain Destruction sends the cards from the hand and Deck to the
  Graveyard without a "destroyed" event.
- Call of the Haunted remembers its monster in its counter; it destroys
  the monster when it is sent from the field to the Graveyard (not when
  banished or returned).
