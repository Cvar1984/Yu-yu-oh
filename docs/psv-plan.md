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
Jar #2 and Major Riot, and Thousand-Eyes Restrict. Done: 12 cards, `tests/psv_2.rs`.
Engine: `PlayerState.skip_draw`, `skip_standby`, `skip_turn` (and
`Event::TurnSkipped`); `CardInstance.attacks_this_turn` (a count, was a
flag) with `Tag::SecondAttack`; `CardInstance.destroyed_by_battle`;
lingering `PositionLocked` and `NoSpellsOrTraps`;
`Tag::RevealTurnPlayerHand`. Relinquished's absorb is shared with
Thousand-Eyes Restrict (`ritual::absorb_one`), which joins the Fusion
table.

Batch 3 adds effect negation and current Types: Jinzo, Imperial Order,
Armored Glass, World Suppression, Mystic Probe, Metal Detector,
Prohibition, DNA Surgery, The Regulation of Tribe, Parasite Paracide,
The Legendary Fisherman, Deepsea Warrior. Done: 12 cards, `tests/psv_3.rs`. Engine:
continuous hooks `negate` (with `Game::negated`; a negator that is
negated itself stops, one level deep), `forbid`, `race` (with
`Game::race`, which every script's Type check now uses), `spell_immune`
and `unattackable`; lingering `Negated` for "this turn";
`put_face_up_in_deck`; Drew cards join the trigger scan;
`Event::EffectNegated`. `Shared.hooks` and `Game.hooks` record which of
these hooks a game's cards have, so games without them pay nothing:
`ygo sim` stays at 1,734 to 1,890 random games/s against 1,836 to 1,966
before (Toon against Removal, one core).

Batch 4 adds zone locks, delayed returns and borrowed cards: Ground
Collapse, Dimensionhole, Lightforce Sword, Graverobber, Magical Hats. Done:
5 cards, `tests/psv_4.rs`. Engine: lingering `ZoneLocked` (lifted when
its source leaves the field; `free_monster_zone` and summons skip locked
zones), `ReturnToHand` and `ReturnToField` (run as a Standby Phase
starts), `Borrowed` and `HatMonster`; `banish_face_down` (seen only by
its owner, redealt by the sampler like a hand card), `borrow_to_hand`,
`set_as_monster`, `mark_hat`, `shuffle_set_monsters`;
`Event::ReturnedToField`.

All 70 are scripted: the registry's pending list holds only the 8 EXTRA
cards of `docs/pool-440-plan.md`.

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

Batch 2:

- Time Seal skips the draw of the Draw Phase; the phase itself still
  happens.
- Cold Wave's "at the start of Main Phase 1" is read as "in your Main
  Phase 1"; the lock ends in the End Phase of your opponent's turn.
- Morphing Jar #2 reveals the excavated cards; a Level 4 or lower monster
  that cannot be Special Summoned (no zone, or it needs its proper summon)
  goes to the Graveyard with the rest.
- Major Riot reads "returned by your opponent's effect" as: your monster
  went from the field to your hand in a window where a card your opponent
  controls resolved. Each player Special Summons as many monsters as they
  had on the field, or as many as they can.
- Sword Hunter equips as many of its monsters as there are free
  Spell/Trap Zones.
- Vampire Baby asks the position (face-up Attack or Defense).

Batch 3:

- Imperial Order and Jinzo also stop the activation of the cards they
  negate (a Spell under Imperial Order is not offered), instead of
  letting it start and resolve without effect.
- Armored Glass, World Suppression, Mystic Probe and Metal Detector
  negate the cards on the field when they resolve, until the End Phase.
- Prohibition declares the name on resolution, among the names its
  player can see (face-up cards, Graveyards, banished cards, their own
  hand), so a declaration never names a hidden card. Cards with that name
  cannot be summoned, Set or activated unless already on the field.
- Parasite Paracide's Insect effect holds while it is face-up on the
  field of the player who drew it; a Draw Phase draw summons it in the
  Standby Phase window.
- "Unaffected by Spell effects" (The Legendary Fisherman, Deepsea
  Warrior with Umi): Spells cannot target it or destroy it, and Spell
  modifiers (Equips, Field Spells, a Spell's temporary change) do not
  apply to it. With Umi, an attack goes directly to The Legendary
  Fisherman's controller only when it is the only monster there.

Batch 4:

- Ground Collapse chooses its two zones on resolution.
- Dimensionhole returns the monster in the position it had, to its own
  zone.
- Graverobber's Spell goes to your hand face-up (both players know it);
  it can be activated but not Set; activating it costs 2000 LP; unused, it
  goes back to its owner's Graveyard in the End Phase.
- Magical Hats: the three face-down monsters trade identities (their
  instance ids), so the opponent cannot follow which is which; an attack
  under way goes on against its target instance, now any of the three.
  The hats have no effects and are destroyed when the Battle Phase ends,
  however it ends (or in the End Phase, if their destruction was
  prevented).
