//! Rulings tests for Toon World and the Toon monsters.

use std::sync::Arc;

use ygo_cards::toons::*;
use ygo_cards::traps::MYSTICAL_SPACE_TYPHOON;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, in_hand, next_turn, pass_all, play, shared_from, stacked,
    summon, summons_offered, with_rules,
};
use ygo_engine::{Action, Game, InstanceId, Location, PlayerId, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.summon.normal_or_tribute_per_turn = 5;
        r.hand.max_size = 30;
    })
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

fn activate(g: &mut Game, id: CardId) {
    let a = activations(g, id);
    assert!(!a.is_empty(), "{id}: {:?}", g.legal_actions());
    play(g, a[0].clone());
    pass_all(g);
}

fn put(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

fn attacks(g: &Game) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Attack { .. }))
        .collect()
}

#[test]
fn toon_world_costs_1000_and_toons_cannot_be_normal_summoned() {
    let mut g = Game::new(
        shared(),
        &stacked([&[TOON_WORLD, TOON_MERMAID], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let mermaid = in_hand(&g, P0, TOON_MERMAID);
    assert!(
        !summons_offered(&g)
            .iter()
            .any(|a| matches!(a, Action::Summon { card, .. } if *card == mermaid))
    );
    assert!(
        activations(&g, TOON_MERMAID).is_empty(),
        "no Toon World yet"
    );
    activate(&mut g, TOON_WORLD);
    assert_eq!(g.player(P0).lp, 7000);
    activate(&mut g, TOON_MERMAID);
    assert!(g.on_field(mermaid));
}

#[test]
fn a_toon_waits_a_turn_pays_500_and_attacks_directly_over_monsters() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[TOON_WORLD, TOON_MERMAID], &[CELTIC_GUARDIAN]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    activate(&mut g, TOON_WORLD);
    activate(&mut g, TOON_MERMAID);
    let mermaid = g.player(P0).monsters().next().unwrap();
    next_turn(&mut g);
    let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    let offered = attacks(&g);
    assert!(offered.contains(&Action::Attack {
        attacker: mermaid,
        target: None
    }));
    assert!(offered.contains(&Action::Attack {
        attacker: mermaid,
        target: Some(celtic)
    }));
    play(
        &mut g,
        Action::Attack {
            attacker: mermaid,
            target: None,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.player(P0).lp, 7000 - 500);
    assert_eq!(g.player(P1).lp, 8000 - 1400);
}

#[test]
fn a_toon_summoned_this_turn_cannot_attack() {
    let base = shared();
    let early = with_rules(&base, |r| r.turn.first_player_can_attack = true);
    let mut g = Game::new(
        early,
        &stacked([&[TOON_WORLD, TOON_MERMAID], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    activate(&mut g, TOON_WORLD);
    activate(&mut g, TOON_MERMAID);
    play(&mut g, Action::EnterBattlePhase);
    assert!(attacks(&g).is_empty());
}

#[test]
fn bigger_toons_tribute_by_level() {
    for (toon, tributes) in [
        (TOON_SUMMONED_SKULL, 1),
        (BLUE_EYES_TOON_DRAGON, 2),
        (MANGA_RYU_RAN, 2),
    ] {
        let mut g = Game::new(
            shared(),
            &stacked(
                [&[TOON_WORLD, toon, MYSTICAL_ELF, MYSTICAL_ELF], &[]],
                MYSTICAL_ELF,
            ),
        )
        .unwrap();
        activate(&mut g, TOON_WORLD);
        assert!(
            activations(&g, toon).is_empty(),
            "{toon}: nothing to tribute yet"
        );
        put(&mut g, P0, MYSTICAL_ELF);
        put(&mut g, P0, MYSTICAL_ELF);
        let offered = activations(&g, toon);
        assert!(
            offered
                .iter()
                .all(|a| matches!(a, Action::Activate { cost, .. } if cost.len() == tributes)),
            "{toon}: {offered:?}"
        );
        play(&mut g, offered[0].clone());
        pass_all(&mut g);
        assert_eq!(g.player(P0).monsters().count(), 3 - tributes, "{toon}");
        assert!(g.player(P0).monsters().any(|m| g.stats(m).id == toon));
    }
}

#[test]
fn a_toon_facing_a_toon_must_attack_a_toon() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[TOON_WORLD, TOON_MERMAID],
                &[TOON_WORLD, TOON_MERMAID, CELTIC_GUARDIAN],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    activate(&mut g, TOON_WORLD);
    activate(&mut g, TOON_MERMAID);
    let mine = g.player(P0).monsters().next().unwrap();
    next_turn(&mut g);
    activate(&mut g, TOON_WORLD);
    activate(&mut g, TOON_MERMAID);
    let theirs = g.player(P1).monsters().next().unwrap();
    put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(
        attacks(&g),
        vec![Action::Attack {
            attacker: mine,
            target: Some(theirs)
        }]
    );
}

#[test]
fn toons_are_destroyed_when_toon_world_is_destroyed() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[TOON_WORLD, TOON_MERMAID], &[MYSTICAL_SPACE_TYPHOON]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    activate(&mut g, TOON_WORLD);
    activate(&mut g, TOON_MERMAID);
    let mermaid = g.player(P0).monsters().next().unwrap();
    next_turn(&mut g);
    activate(&mut g, MYSTICAL_SPACE_TYPHOON);
    assert_eq!(g.instance(mermaid).location, Location::Graveyard);
}
