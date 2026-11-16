# M2 plan: chains and 30 key cards

Gate (from the handoff): rulings tests for the key cards pass. This is
the hardest engine milestone; every later result depends on it.

## Scope

M2 adds Spell and Trap plays, the chain, response windows, triggers,
continuous effects and 30 scripted cards chosen so that every effect
mechanism the 228 non-Normal cards need is exercised at least once.
Fusion and Ritual Summons, hand-reveal choices (Confiscation, The
Forceful Sentry) and the long tail of cards are M3.

| Mechanism | Cards that exercise it |
| --- | --- |
| Normal Spell, no targets | Pot of Greed, Raigeki, Dark Hole, Fissure, Upstart Goblin |
| Normal Spell with targets at activation | Monster Reborn, Change of Heart |
| Cost paid at activation | Tribute to the Doomed (discard), Solemn Judgment (half LP), Magic Jammer (discard), Seven Tools of the Bandit (1000 LP), Kuriboh (discard) |
| Quick-Play Spell, speed 2, from hand on own turn or Set | Mystical Space Typhoon |
| Equip Spell with stat modifiers, leaves with the monster | Malevolent Nuzzler, Legendary Sword |
| Field Spell under the owner's one-on-the-field rule | Umi |
| Continuous Spell with a Standby Phase cost and a restriction | Messenger of Peace |
| Lingering effect with a duration | Swords of Revealing Light |
| Normal Trap responding to an event | Mirror Force (attack declared), Trap Hole (Normal or Flip Summon succeeded) |
| Counter Trap, speed 3, negation | Solemn Judgment (summon or activation), Magic Jammer (Spell), Seven Tools of the Bandit (Trap) |
| Mandatory trigger in the Graveyard with a deck search choice, text-version branch | Sangan, Witch of the Black Forest |
| Flip effects, by Flip Summon and by battle in the Damage Step | Man-Eater Bug, Magician of Faith |
| Quick monster effect during damage calculation | Kuriboh |
| Alternate win condition, `win.exodia` | Exodia the Forbidden One and its four limbs |
| Summon restriction flag | Toon Mermaid (cannot be Normal Summoned or Set) |

Thirty cards: 13 Spells, 5 Traps, 12 monsters.

## Rules keys M2 implements

`turn.trap_activation_turn_set`, `turn.spell_target_monster_set_this_turn`,
`summon.special_summon_counts_toward_limit`,
`summon.max_monsters_entering_per_turn` (now also for Special Summons),
`field.field_spells_on_field`, `field.spell_trap_zones`, `win.exodia`,
`cards.text_version` (Sangan and Witch: "when" under original text,
"if" under modern). Each gets a flip test. `fusion_summons` and
`ritual_summons` wait for M3.

## Design

**Scripts are data plus function pointers.** `ygo-engine` defines the
types; `ygo-cards` fills a `Registry` (card id to `&'static CardScript`)
and `Shared` carries it. Function pointers, not closures: no captured
state, nothing to clone with the game, and WebAssembly-friendly.

```text
CardScript { tags: &[Tag], effects: &[Effect] }
Tag: CannotNormalSummon | ExodiaPiece | ExodiaHead
Effect {
  kind:   Ignition | Activate | Trigger { mandatory } | Flip | Quick | Continuous
  speed:  1 | 2 | 3
  timing: fn(&Game, Source, &Window) -> bool    # may it start now, given the window
  targets: Option<fn(&Game, Source) -> Vec<Vec<InstanceId>>>   # legal target sets
  cost:    Option<fn(&mut Game, &mut EffectCtx)>              # paid at activation
  resolve: fn(&mut Game, &mut EffectCtx) -> Flow
  modify:  Option<fn(&Game, Source, InstanceId) -> StatMod>   # continuous
}
Flow: Done | Choose { options: Vec<InstanceId>, then: fn(&mut Game, &mut EffectCtx, InstanceId) -> Flow }
```

A resolution that needs a choice (Sangan's search) returns `Flow::Choose`;
the engine turns it into a `Choose` decision for the controller, whose
observation reveals the options, and continues with `then`. No script
ever blocks or holds state outside `EffectCtx` (source, controller,
targets, chosen, the triggering event).

**Windows.** Each point where the rules allow a fast effect is a
`Window` carrying the events that just happened. After every action and
every resolved chain the engine opens a window; the player with priority
(turn player first, then the other) sees `Activate` actions plus `Pass`.
When a player's only legal action is `Pass`, the engine passes for them,
so M1 behaviour and speed are unchanged until Traps exist. The engine
also opens windows at summon negation (before the monster is on the
field), summon success, attack declaration, and inside the Damage Step
at: start, before damage calculation, damage calculation, after damage
calculation, end. Only speed 2 and 3 effects may start in a window, and
in the Damage Step only those that negate or change ATK/DEF (Kuriboh,
counter traps), plus Flip effects that trigger after damage calculation.

**Chain.** `chain: Vec<ChainLink { effect, controller, targets, negated }>`.
A link may be added only with speed at or above the previous link's,
and speed 1 only on an empty chain. After each link the opponent gets
priority; two consecutive passes resolve the chain backwards. Negated
links are skipped. Costs are paid and targets chosen when the link is
added. Cards activated from a Spell/Trap zone go to the Graveyard after
resolution unless they are Continuous, Equip or Field.

**Triggers.** After a chain resolves (or after an action outside a
chain), the engine collects trigger effects whose `timing` accepts the
window's events, orders them by SEGOC (turn player's mandatory, the
opponent's mandatory, turn player's optional, opponent's optional), lets
a controller order their own group when it has more than one (an
`OrderTriggers` decision), and builds the next chain from them.
Optional "when" triggers that are not activated in their window are
missed; under `text_version = "original_2002"` Sangan and Witch are
"when", under modern text they are "if".

**Summons become a two-window flow.** Tributes are paid, the summon
negation window opens (Solemn Judgment), then the monster enters the
field and the summon success window opens (Trap Hole). Special Summon is
a new engine operation used by Monster Reborn, honouring
`max_monsters_entering_per_turn` and `special_summon_counts_toward_limit`.
Any Main Deck monster may be Normal Summoned unless its script carries
`CannotNormalSummon`; Effect Monsters without a script play as vanilla
until M3 scripts them.

**Continuous and lingering.** ATK and DEF are computed on demand from
the base stats plus every `modify` of every face-up Equip, Field and
Continuous card on the field, never by mutating the stats. Equips track
their monster and leave the field with it. Lingering effects such as
Swords live in `game.lingering: Vec<Lingering { source, controller,
restriction, expires }>` and expire at the named End Phase; `Restriction`
is an engine enum (`CannotAttack`, `CannotAttackWithAtkAbove`).

**Observation and actions.** New actions: `SetSpellTrap`, `Activate
{ card, effect, targets }`, `Pass`, `Choose { card }`, `OrderTriggers
{ order }`. New decision kinds: `Respond`, `Choose`, `OrderTriggers`.
The observation adds the chain (every link is public), the lingering
effects, each monster's current ATK and DEF, and the revealed options of
a pending `Choose`.

**Game records** stay a setup plus the action list; replays reproduce
chains because every choice is an action.

## Steps

1. **Effect API and registry.** Types above, `Registry`, `Shared` takes
   it, `ygo-cards` skeleton with an empty registry and the summon
   restriction tag. Lift the Normal-only summon rule. Tests: Sangan is
   summonable as vanilla, Toon Mermaid is not.
2. **Spell plays and a one-link chain.** Spell/Trap zones,
   `SetSpellTrap`, Normal Spell activation from hand or Set, the chain
   structure and backwards resolution with no responses, Special Summon,
   control change with return at the End Phase. Cards: Pot of Greed,
   Raigeki, Dark Hole, Fissure, Upstart Goblin, Monster Reborn, Change
   of Heart, Tribute to the Doomed. Tests per card plus: a Spell cannot
   be activated on the opponent's turn or in the Battle Phase; a
   resolved Spell is in the Graveyard; `spell_target_monster_set_this_turn`.
3. **Traps, windows and counter traps.** Windows with auto-pass, Trap
   activation under `trap_activation_turn_set`, spell speed rules,
   multi-link chains, negation. Cards: Mirror Force, Trap Hole, Solemn
   Judgment, Magic Jammer, Seven Tools of the Bandit, Mystical Space
   Typhoon. Tests: a three-link chain resolves backwards; speed 1 cannot
   respond; Solemn negates a Tribute Summon and the tributes stay paid;
   Trap Hole triggers on Normal and Flip Summons but not on Sets or
   Special Summons; MST destroying a Set Trap in a chain does not stop
   a Trap already in the chain.
4. **Triggers, SEGOC and the Damage Step.** Trigger collection and
   ordering, `Choose` decisions, Damage Step sub-steps. Cards: Sangan,
   Witch of the Black Forest, Man-Eater Bug, Magician of Faith, Kuriboh,
   the five Exodia cards. Tests: Sangan searched when tributed and when
   destroyed by battle, not when discarded from the hand; Raigeki
   destroying Sangan and Witch gives an ordering decision; text-version
   flip changes "if" to "when"; Man-Eater Bug flipped by an attack
   destroys after damage calculation; Kuriboh prevents the damage;
   Exodia wins on the draw and on the search.
5. **Continuous and lingering effects.** Computed ATK/DEF, equips, the
   Field Spell rule, Continuous Spell upkeep, Swords. Cards: Malevolent
   Nuzzler, Legendary Sword, Umi, Messenger of Peace, Swords of
   Revealing Light. Tests: an equip's bonus appears in observations and
   in battle and vanishes when the monster leaves; a second Field Spell
   destroys the first whoever owns it; Swords expires after three of the
   opponent's End Phases; Messenger's cost is paid or the card goes.
6. **Random play, invariants and speed.** Invariants extended (chain
   empty when no decision, zone consistency for Spells and Traps, equips
   point at monsters on the field, computed stats never below zero); the
   million-game fuzz again with the sample decks, which now play their
   Spells and Traps; speed re-measured and the baseline updated. Rulings
   test count: at least one per card.
7. **Docs.** CLAUDE.md status and engine notes, this plan's progress
   and gate report.

## Risks

- The chain and window machinery is where rules bugs hide. Step 3's
  tests are the most important in the milestone, and every later card
  adds a rulings test rather than trusting the machinery.
- Auto-passing windows reveals that a player has no possible response.
  Real engines do the same; bots must not be trained to read it as
  information, which the observation design prevents by not exposing
  whether a pass was automatic.
- Speed will drop with windows and stat computation. The fuzz step
  measures it; the target is to stay within a factor of two of M1.
