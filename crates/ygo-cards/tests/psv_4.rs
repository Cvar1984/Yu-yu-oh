//! Rulings tests for Pharaoh's Servant, batch 4 (docs/psv-plan.md): zone
//! locks, delayed returns, borrowed cards and Magical Hats.

use ygo_cards::oneshot::HINOTAMA;
use ygo_cards::psv_spells::*;
use ygo_cards::psv_traps::*;
use ygo_cards::traps::MYSTICAL_SPACE_TYPHOON as MST;
use ygo_engine::testing::{CELTIC_GUARDIAN, in_hand, next_turn, pass_all, play};
use ygo_engine::{Action, DecisionKind, Location, Position, VisibleCard};

mod psv_common;
use psv_common::*;

#[test]
fn ground_collapse_locks_two_empty_zones_while_it_is_on_the_field() {
    let mut g = game(&[GROUND_COLLAPSE], &[MST]);
    let collapse = in_hand(&g, P0, GROUND_COLLAPSE);
    activate(&mut g, GROUND_COLLAPSE);
    answer(&mut g, 0); // your zone 1
    answer(&mut g, 4); // the opponent's zone 1 (fifth of the nine left)
    let mine = summon_now(&mut g, P0, PAD);
    assert_eq!(g.instance(mine).location, Location::MonsterZone(1));
    next_turn(&mut g);
    let theirs = summon_now(&mut g, P1, PAD);
    assert_eq!(g.instance(theirs).location, Location::MonsterZone(1));
    activate_on(&mut g, MST, &[collapse]);
    let next = summon_now(&mut g, P1, PAD);
    assert_eq!(
        g.instance(next).location,
        Location::MonsterZone(0),
        "free again once Ground Collapse is gone"
    );
}

#[test]
fn dimensionhole_banishes_a_monster_until_your_next_standby_phase() {
    let mut g = game(&[CELTIC_GUARDIAN, DIMENSIONHOLE], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    activate_on(&mut g, DIMENSIONHOLE, &[celtic]);
    assert_eq!(g.instance(celtic).location, Location::Banished);
    let pad = summon_now(&mut g, P0, PAD);
    assert_eq!(
        g.instance(pad).location,
        Location::MonsterZone(1),
        "its zone is locked"
    );
    next_turn(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Banished);
    next_turn(&mut g);
    assert_eq!(g.instance(celtic).location, Location::MonsterZone(0));
    assert_eq!(g.instance(celtic).position, Position::FaceUpAttack);
}

#[test]
fn lightforce_sword_banishes_a_hand_card_face_down_for_four_of_the_owners_standby_phases() {
    let mut g = game(&[LIGHTFORCE_SWORD], &[]);
    set_card(&mut g, P0, LIGHTFORCE_SWORD);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, LIGHTFORCE_SWORD);
    let banished = g.banished(P1).to_vec();
    assert_eq!(banished.len(), 1);
    let card = banished[0];
    assert_eq!(g.instance(card).position, Position::FaceDown);
    let seen_by = |g: &ygo_engine::Game, viewer| {
        matches!(
            g.observe(viewer).sides[1].banished[0],
            VisibleCard::Known { .. }
        )
    };
    assert!(!seen_by(&g, P0), "hidden from the activator");
    assert!(seen_by(&g, P1), "known to its owner");
    next_turn(&mut g);
    for turn in [4, 6, 8] {
        assert_eq!(
            (g.turn(), g.instance(card).location),
            (turn, Location::Banished)
        );
        next_turn(&mut g);
        next_turn(&mut g);
    }
    assert_eq!(g.turn(), 10, "the owner's 4th Standby Phase");
    assert_eq!(g.instance(card).location, Location::Hand);
}

#[test]
fn graverobber_lends_an_opponents_spell_for_2000_lp() {
    let mut g = game(&[GRAVEROBBER], &[HINOTAMA, HINOTAMA]);
    set_card(&mut g, P0, GRAVEROBBER);
    next_turn(&mut g);
    activate(&mut g, HINOTAMA);
    next_turn(&mut g);
    activate(&mut g, GRAVEROBBER);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Choose);
    let a = g.legal_actions()[0].clone();
    let Action::Choose { card: spell } = a else {
        panic!("{a:?}")
    };
    choose(&mut g, spell);
    assert!(g.hand(P0).contains(&spell));
    assert!(
        !g.legal_actions()
            .contains(&Action::SetSpellTrap { card: spell }),
        "it can be used, not Set"
    );
    activate(&mut g, HINOTAMA);
    assert_eq!(lp(&g, P0), 7500 - 2000, "Hinotama earlier, then the 2000");
    assert_eq!(lp(&g, P1), 7500);
    assert_eq!(g.instance(spell).location, Location::Graveyard);
    assert!(g.graveyard(P1).contains(&spell), "back to its owner");
}

#[test]
fn graverobber_returns_an_unused_spell_in_the_end_phase() {
    let mut g = game(&[GRAVEROBBER], &[HINOTAMA]);
    set_card(&mut g, P0, GRAVEROBBER);
    next_turn(&mut g);
    activate(&mut g, HINOTAMA);
    next_turn(&mut g);
    activate(&mut g, GRAVEROBBER);
    let a = g.legal_actions()[0].clone();
    let Action::Choose { card: spell } = a else {
        panic!("{a:?}")
    };
    choose(&mut g, spell);
    next_turn(&mut g);
    assert!(g.graveyard(P1).contains(&spell));
    assert_eq!(lp(&g, P0), 7500, "unused: no 2000");
}

#[test]
fn magical_hats_hides_a_monster_among_two_spells_that_vanish_after_the_battle_phase() {
    let mut g = game(
        &with_deck(&[MAGICAL_HATS, CELTIC_GUARDIAN], &[HINOTAMA, HINOTAMA]),
        &[CELTIC_GUARDIAN],
    );
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, MAGICAL_HATS);
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    play(&mut g, Action::EnterBattlePhase);
    respond(&mut g, P0, MAGICAL_HATS, None);
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Choose)
    {
        let a = g.legal_actions()[0].clone();
        play(&mut g, a);
        pass_all(&mut g);
    }
    let field = g.monsters_controlled(P0);
    assert_eq!(field.len(), 3);
    assert!(
        field
            .iter()
            .all(|&m| g.instance(m).position == Position::FaceDownDefense)
    );
    assert_eq!(field.iter().filter(|&&m| g.is_hat(m)).count(), 2);
    let _ = celtic;
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    let left = g.monsters_controlled(P0);
    assert_eq!(left.len(), 1, "the hats are destroyed");
    assert_eq!(g.stats(left[0]).id, CELTIC_GUARDIAN);
    let hinotamas = g
        .graveyard(P0)
        .iter()
        .filter(|&&c| g.stats(c).id == HINOTAMA)
        .count();
    assert_eq!(hinotamas, 2);
}
