# M1 plan: core engine without chains

Gate (from the handoff): random games finish; invariant tests and fuzzing
pass; random-play speed is measured and kept in CI.

## Scope

In M1 the engine plays the 146 Normal Monsters under the full turn
structure. Decks may contain any card; cards that need an effect (Spells,
Traps, Effect, Fusion and Ritual Monsters) are drawn, held and discarded
but offer no actions. That keeps the four sample decks loadable and means
M2 only adds actions, never reshapes the state.

| In M1 | Deferred |
| --- | --- |
| Draw, Standby, Main 1, Battle (Start, Battle, Damage, End steps), Main 2, End | Chains, spell speeds, priority, response windows (M2) |
| Normal Summon, Set, Tribute Summon and Tribute Set, per-turn summon limit | Setting and activating Spells and Traps (M2) |
| Flip Summon, position change, "Set this turn" tracking | Special Summons, Fusion, Ritual (M2/M3) |
| Attack declaration, damage calculation, destruction, direct attacks | Flip effects at the Damage Step (M2) |
| Hand limit after draw (owner) or at End Phase (official) | Field Spells, Exodia, text versions |
| Deck-out loss, LP 0, draw at a turn cap | Game record files and replay command (M4) |
| Observation with hidden information removed | Observation encoding for the network (M6) |

## Rules keys M1 implements and tests by flipping

`deck.*` (validated at game start), `field.monster_zones`,
`field.spell_trap_zones` (zones exist, stay empty), `hand.max_size`,
`hand.hand_limit_check` (both values), `life.starting_lp`,
`turn.first_player_skips_draw`, `turn.first_player_can_attack`,
`summon.normal_or_tribute_per_turn`, `summon.tributes_level_5_6`,
`summon.tributes_level_7_plus`, `summon.summon_in_main_phase_2`,
`summon.flip_summon_counts_toward_limit`,
`summon.max_monsters_entering_per_turn`, `win.deck_out_loses`.

Keys read but without effect until M2: `trap_activation_turn_set`,
`spell_target_monster_set_this_turn` (the engine already records the turn
each card was Set so M2 can enforce it), `special_summon_counts_toward_limit`,
`fusion_summons`, `ritual_summons`, `field_spells_on_field`, `win.exodia`,
`cards.text_version`.

## Design

**One struct, not a trait.** `Game` is a struct with the methods the
handoff lists (`new`, `pending_decision`, `legal_actions`, `apply`,
`observe`, `result`). A trait is added only when a second implementation
exists.

**Card data.** At start the engine builds a `CardPool`: a dense `Vec` of
the stats it needs (kind, level, ATK, DEF, attribute, race, id), indexed by
a `u16`. `Game` holds it in an `Arc`, which is immutable shared data, not
interior mutability, so clones stay a pointer bump.

**State is flat.** Every physical card in the match is a `CardInstance` in
one `Vec`, addressed by a `u8` index:

```text
CardInstance { card: u16, owner, controller, location, position,
               turn_set, turn_entered_field, position_changed_this_turn,
               attacked_this_turn }
location: Deck | Hand | MonsterZone(i) | SpellTrapZone(i) | Graveyard | Extra
position: FaceUpAttack | FaceUpDefense | FaceDownDefense | FaceDown (S/T)
```

Per player: `deck`, `hand`, `graveyard`, `extra` as `Vec<u8>` in order,
`monster_zones` and `spell_trap_zones` as `Vec<Option<u8>>` sized from the
rules, `lp`, and per-turn counters (summons used, monsters entered).
Game-wide: turn number, turn player, phase, pending decision, RNG state,
result. No `Rc`, no `RefCell`, no references with lifetimes.

**Decisions and actions.** Every choice is an `Action` variant and
`legal_actions` enumerates all of them, so bots and humans never construct
anything the engine did not offer. M1 actions:

```text
Summon { card, tributes: Vec<u8>, face_down: bool }   # Normal, Tribute, Set
FlipSummon { card } | ChangePosition { card }
EnterBattlePhase | Attack { attacker, target: Option<u8> } | EndBattlePhase
EndTurn | Discard { card }                              # hand limit
```

Tribute combinations are enumerated (at most C(5,2) = 10 per card). Phases
with no choice (Draw, Standby, Start and End steps) advance automatically.
`apply` returns the `Vec<Event>` it produced (Draw, Summon, Flip, Battle,
Damage, Destroyed, Discard, PhaseChanged, TurnChanged, GameOver) and a
`RuleError` for an action that was not legal.

**Determinism.** `GameSetup { rules, decks, seed, first_player, max_turns }`.
Randomness is a seeded ChaCha8 used only to shuffle and, when
`first_player` is `None`, to pick who starts. The same setup and action
list always produce the same events and the same final state.
`max_turns` ends a game as a draw, so games terminate even when
`deck_out_loses` is false.

**Battle rulings in M1.** Attack vs attack: the lower ATK is destroyed and
its controller takes the difference; equal ATK destroys both with no
damage. Attack vs defense: ATK above DEF destroys the defender with no
damage (no piercing exists in these sets); ATK below DEF damages the
attacker by the difference; equal does nothing. A face-down defender is
flipped face up at the start of the Damage Step and stays face up. Each
monster attacks once per Battle Phase; a monster summoned this turn may
attack; direct attacks only when the opponent controls no monsters. The
Damage Step is one atomic `apply` in M1, but it is modelled as a step so
M2 can open response windows inside it without changing the state shape.

**Hidden information.** `observe(player)` returns the public board, that
player's hand, LP, counts of the other player's hand and deck, and
face-down cards as `Hidden`. A property test checks that an observation
never contains an index from the opponent's hand or face-down zones.

**Dependencies added.** `ygo-engine`: ygo-data, serde, rand_core,
rand_chacha; dev: proptest. `ygo-cli`: nothing new.

## Steps

Each step is one commit with its tests; the next starts when the previous
is green.

1. **Skeleton and setup.** `CardPool`, `CardInstance`, `GameSetup`,
   `Game::new` with deck size checks, shuffle, 5-card opening hands, first
   player choice. Tests: setup is deterministic per seed, card count is
   conserved, both hands hold 5.
2. **Turn machine.** Phases, automatic advance, Draw Phase with
   `first_player_skips_draw`, hand limit in both modes, `EndTurn`,
   deck-out, turn cap. Tests: a game of nothing but `EndTurn` ends by
   deck-out at the expected turn; each key flipped changes the outcome.
3. **Summons.** Normal Summon, Set, Tribute Summon and Set, zone capacity,
   summon limit, tributes by level, Main Phase 2 restriction,
   `max_monsters_entering_per_turn`. Tests per rule and per key.
4. **Positions.** Flip Summon and position change: once per turn, not in
   the turn the card entered the field, not after attacking, not during
   the Battle Phase; `flip_summon_counts_toward_limit`.
5. **Battle.** Battle Phase steps, attack legality, the rulings table
   above, direct attacks, LP, win, loss and draw, `first_player_can_attack`.
   One test per row of the rulings table.
6. **Observation, events and serialisation.** `Action`, `Event`,
   `GameSetup` and `GameResult` derive serde; a replay test re-applies a
   recorded action list and compares final states; the hidden-information
   property test.
7. **Random player, invariants and speed.** A `RandomPlayer` in
   `ygo-engine`'s test support (bots proper arrive in M4). Proptest
   invariants: zone limits, hand limit after the check point, card
   conservation, every offered action applies without error, determinism.
   `ygo sim --random -n N [--seed S]` plays random games with the four
   sample decks, prints win rates split by first player and games per
   second. CI runs it with a small N and prints the speed; the baseline
   number is recorded in CLAUDE.md.
8. **Docs.** Update CLAUDE.md status, the engine crate doc and this plan
   with what changed.

A long fuzz run (millions of games, release build) is run by hand before
the gate is declared passed, not on every CI run.

## Owner's confirmations (2026-11-16)

- "No summons in Main Phase 2" covers Normal and Tribute Summons and Sets
  only; Flip Summons and position changes stay allowed there.
- Opening hands are 5 cards (`hand.opening_size`); who goes first is
  random from the seed unless the caller chooses.
- A game that reaches the turn cap is a draw (engine setting, default
  200 turns, not a rule).

## Progress

- Step 1 done: `CardPool`, `CardInstance`, `GameSetup`, `Game::new`
  with deck checks, seeded shuffle and opening hands.
