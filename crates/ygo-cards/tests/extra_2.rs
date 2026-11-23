//! Rulings tests for the last EXTRA cards (pool-440 plan, batch 2), on the
//! modern text. The board helpers are the PSV tests' (`psv_common`).

use ygo_cards::extra_traps::*;
use ygo_cards::oneshot::RUSH_RECKLESSLY;
use ygo_cards::spells::{CHANGE_OF_HEART, DARK_HOLE};
use ygo_engine::testing::{CELTIC_GUARDIAN, MYSTICAL_ELF, in_hand, next_turn, play};
use ygo_engine::{Action, DecisionKind, Location};

mod psv_common;
use psv_common::*;

// D. Tribe: the face-up monsters you control at resolution are Dragons
// until the End Phase, so Dragon cards (Burst Breath) see them.
#[test]
fn d_tribe_makes_your_face_up_monsters_dragons_this_turn() {
    let mut g = game(&[CELTIC_GUARDIAN, D_TRIBE, BURST_BREATH, CYBER_FALCON], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, D_TRIBE);
    set_card(&mut g, P0, BURST_BREATH);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.race(celtic), "Warrior");
    assert!(
        activations(&g, BURST_BREATH).is_empty(),
        "no Dragon to Tribute"
    );
    activate(&mut g, D_TRIBE);
    assert_eq!(g.race(celtic), "Dragon");
    assert!(!activations(&g, BURST_BREATH).is_empty());
    let falcon = summon_now(&mut g, P0, CYBER_FALCON);
    assert_eq!(g.race(falcon), "Machine", "summoned after the resolution");
    next_turn(&mut g);
    assert_eq!(g.race(celtic), "Warrior", "ends in the End Phase");
}

// Riryoku Field: a Spell that targets exactly 1 monster on the field is
// negated and destroyed.
#[test]
fn riryoku_field_negates_a_spell_that_targets_one_monster() {
    let mut g = game(&[CELTIC_GUARDIAN, RUSH_RECKLESSLY], &[RIRYOKU_FIELD]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let riryoku = set_card(&mut g, P1, RIRYOKU_FIELD);
    next_turn(&mut g);
    let rush = in_hand(&g, P0, RUSH_RECKLESSLY);
    let a = activations(&g, RUSH_RECKLESSLY).remove(0);
    play(&mut g, a);
    respond(&mut g, P1, RIRYOKU_FIELD, None);
    assert_eq!(g.atk(celtic), 1400, "Rush Recklessly was negated");
    assert_eq!(g.instance(rush).location, Location::Graveyard);
    assert_eq!(g.instance(riryoku).location, Location::Graveyard);
}

#[test]
fn riryoku_field_ignores_a_spell_without_a_single_monster_target() {
    let mut g = game(&[CELTIC_GUARDIAN, DARK_HOLE], &[RIRYOKU_FIELD]);
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    set_card(&mut g, P1, RIRYOKU_FIELD);
    next_turn(&mut g);
    let a = activations(&g, DARK_HOLE).remove(0);
    play(&mut g, a);
    while let Some(d) = g.pending_decision() {
        if d.kind != DecisionKind::Respond {
            break;
        }
        assert!(activations(&g, RIRYOKU_FIELD).is_empty());
        play(&mut g, Action::Pass);
    }
}

// Shift, attack: the attack moves to another monster the attacker could
// attack, with no replay.
#[test]
fn shift_moves_an_attack_to_another_of_your_monsters() {
    let mut g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF, SHIFT], &[KOUMORI_DRAGON]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let elf = set_monster(&mut g, P0, MYSTICAL_ELF);
    set_card(&mut g, P0, SHIFT);
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    attack(&mut g, koumori, Some(celtic));
    respond(&mut g, P0, SHIFT, Some(&[elf]));
    assert_eq!(g.instance(celtic).location, Location::MonsterZone(0));
    assert_eq!(lp(&g, P1), 7500, "1500 ATK into the Elf's 2000 DEF");
    assert!(g.on_field(elf) && g.on_field(koumori));
}

// Shift, Spell: the opponent's single-target Spell takes the new target.
#[test]
fn shift_moves_a_spell_to_another_of_your_monsters() {
    let mut g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF, SHIFT], &[CHANGE_OF_HEART]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let elf = summon_now(&mut g, P0, MYSTICAL_ELF);
    set_card(&mut g, P0, SHIFT);
    next_turn(&mut g);
    let a = activations(&g, CHANGE_OF_HEART)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, SHIFT, None);
    assert_eq!(
        g.instance(elf).controller,
        P1,
        "Change of Heart took the Elf"
    );
    assert_eq!(g.instance(celtic).controller, P0);
}

#[test]
fn shift_needs_a_second_monster() {
    let mut g = game(&[CELTIC_GUARDIAN, SHIFT], &[KOUMORI_DRAGON]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, SHIFT);
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    attack(&mut g, koumori, Some(celtic));
    while let Some(d) = g.pending_decision() {
        if d.kind != DecisionKind::Respond {
            break;
        }
        assert!(activations(&g, SHIFT).is_empty());
        play(&mut g, Action::Pass);
    }
}
