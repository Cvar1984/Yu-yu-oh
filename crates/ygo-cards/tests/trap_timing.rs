//! When Trap Cards may be activated, by the TCG rules (owner, 2026-11-27:
//! "check the international rules for each Trap card"). Each test walks
//! a real sequence of play, passing every response window, and records in
//! which windows player 0 was offered which Trap.

mod psv_common;

use psv_common::*;
use ygo_cards::extra_traps::{REINFORCEMENTS, WABOKU};
use ygo_cards::spells::{DARK_HOLE, MONSTER_REBORN};
use ygo_cards::traps::{MIRROR_FORCE, SOLEMN_JUDGMENT, TRAP_HOLE};
use ygo_data::CardId;
use ygo_engine::testing::{CELTIC_GUARDIAN, in_hand, play, summon};
use ygo_engine::{Action, DecisionKind, Game, Location, WindowKind};

/// Passes every response until a decision that is not a response; returns
/// the windows in which player 0 could respond, each with whether each of
/// `traps` was offered there.
fn walk(g: &mut Game, traps: &[CardId]) -> Vec<(WindowKind, Vec<bool>)> {
    let mut seen = Vec::new();
    while let Some(d) = g.pending_decision() {
        if d.kind != DecisionKind::Respond {
            break;
        }
        if d.player == P0 {
            let kind = g.window().map(|(k, _)| k).expect("a response has a window");
            let offered = traps
                .iter()
                .map(|&t| !activations(g, t).is_empty())
                .collect();
            seen.push((kind, offered));
        }
        play(g, Action::Pass);
    }
    seen
}

fn offered_in(seen: &[(WindowKind, Vec<bool>)], trap: usize) -> Vec<WindowKind> {
    seen.iter()
        .filter(|(_, o)| o[trap])
        .map(|(k, _)| *k)
        .collect()
}

#[test]
fn mirror_force_only_when_an_opponents_monster_declares_an_attack() {
    // Reinforcements (usable almost any time) makes player 0 a responder
    // in every window, so the walk sees each window.
    let mut g = game(&[MIRROR_FORCE, REINFORCEMENTS], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, MIRROR_FORCE);
    set_card(&mut g, P0, REINFORCEMENTS);
    play(&mut g, Action::EndTurn);
    let mut seen = walk(&mut g, &[MIRROR_FORCE, REINFORCEMENTS]);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    seen.extend(walk(&mut g, &[MIRROR_FORCE, REINFORCEMENTS]));
    play(&mut g, Action::EnterBattlePhase);
    seen.extend(walk(&mut g, &[MIRROR_FORCE, REINFORCEMENTS]));
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    seen.extend(walk(&mut g, &[MIRROR_FORCE, REINFORCEMENTS]));
    assert_eq!(
        offered_in(&seen, 0),
        [WindowKind::AttackDeclared],
        "Mirror Force: only at the attack declaration, not in the Damage Step: {seen:?}"
    );
    assert!(
        offered_in(&seen, 1).contains(&WindowKind::DamageCalculation),
        "Reinforcements may be activated in the Damage Step (it changes ATK): {seen:?}"
    );
}

#[test]
fn waboku_cannot_be_activated_in_the_damage_step() {
    let mut g = game(&[WABOKU], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, WABOKU);
    play(&mut g, Action::EndTurn);
    walk(&mut g, &[WABOKU]);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    play(&mut g, Action::EnterBattlePhase);
    walk(&mut g, &[WABOKU]);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    let seen = walk(&mut g, &[WABOKU]);
    let windows = offered_in(&seen, 0);
    assert!(windows.contains(&WindowKind::AttackDeclared), "{seen:?}");
    assert!(
        !windows.contains(&WindowKind::DamageCalculation),
        "Waboku changes no ATK or DEF: not in the Damage Step: {seen:?}"
    );
}

#[test]
fn trap_hole_answers_a_normal_summon_but_not_a_special_summon() {
    let mut g = game(&[TRAP_HOLE], &[CELTIC_GUARDIAN, DARK_HOLE, MONSTER_REBORN]);
    set_card(&mut g, P0, TRAP_HOLE);
    play(&mut g, Action::EndTurn);
    walk(&mut g, &[TRAP_HOLE]);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    let seen = walk(&mut g, &[TRAP_HOLE]);
    assert_eq!(
        offered_in(&seen, 0),
        [WindowKind::SummonSuccess],
        "after the Normal Summon of a 1000+ ATK monster: {seen:?}"
    );
    // Player 1 clears their own field, then revives Celtic Guardian.
    let dark_hole = activations(&g, DARK_HOLE).remove(0);
    play(&mut g, dark_hole);
    walk(&mut g, &[TRAP_HOLE]);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    let reborn = activations(&g, MONSTER_REBORN)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &[celtic]))
        .expect("Monster Reborn on Celtic Guardian");
    play(&mut g, reborn);
    let mut seen = walk(&mut g, &[TRAP_HOLE]);
    // Monster Reborn asks the position on resolution.
    if g.pending_decision().map(|d| d.kind) == Some(DecisionKind::Choose) {
        play(&mut g, Action::Answer { option: 0 });
        seen.extend(walk(&mut g, &[TRAP_HOLE]));
    }
    assert!(g.on_field(celtic), "revived");
    assert!(
        offered_in(&seen, 0).is_empty(),
        "Trap Hole: Normal or Flip Summons only, not a Special Summon: {seen:?}"
    );
}

#[test]
fn only_counter_traps_answer_a_summon_being_declared() {
    let mut g = game(&[WABOKU, SOLEMN_JUDGMENT], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, WABOKU);
    set_card(&mut g, P0, SOLEMN_JUDGMENT);
    play(&mut g, Action::EndTurn);
    walk(&mut g, &[WABOKU, SOLEMN_JUDGMENT]);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    let seen = walk(&mut g, &[WABOKU, SOLEMN_JUDGMENT]);
    let declared: Vec<_> = seen
        .iter()
        .filter(|(k, _)| *k == WindowKind::SummonDeclared)
        .collect();
    assert!(!declared.is_empty(), "{seen:?}");
    for (_, offered) in declared {
        assert_eq!(
            offered,
            &[false, true],
            "Waboku waits; Solemn Judgment may answer"
        );
    }
    assert!(
        offered_in(&seen, 0).contains(&WindowKind::SummonSuccess),
        "Waboku once the summon succeeds: {seen:?}"
    );
}

#[test]
fn your_own_set_trap_can_be_activated_in_your_main_phase_next_turn() {
    let mut g = game(&[REINFORCEMENTS, CELTIC_GUARDIAN], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, REINFORCEMENTS);
    assert!(
        activations(&g, REINFORCEMENTS).is_empty(),
        "not in the turn it was Set"
    );
    play(&mut g, Action::EndTurn);
    walk(&mut g, &[REINFORCEMENTS]);
    play(&mut g, Action::EndTurn);
    walk(&mut g, &[REINFORCEMENTS]);
    assert_eq!(g.turn_player(), P0);
    assert!(
        !activations(&g, REINFORCEMENTS).is_empty(),
        "a Normal Trap in your own Main Phase"
    );
    activate_on(&mut g, REINFORCEMENTS, &[celtic]);
    assert_eq!(g.atk(celtic), 1900);
}
