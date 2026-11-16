//! M1 step 3: Normal Summon, Set, Tribute Summon and their limits.

mod common;

use common::{
    BLUE_EYES, MYSTICAL_ELF, SANGAN, SUMMONED_SKULL, in_hand, next_turn, play, shared, shared_with,
    stacked, summon, summons_offered,
};
use ygo_data::CardKind;
use ygo_engine::{Action, Event, Game, InstanceId, Location, Phase, PlayerId, Position};

const P0: PlayerId = PlayerId(0);

#[test]
fn cards_used_by_tests_are_what_we_think() {
    let shared = shared();
    let stats = |id| shared.pool.get(shared.pool.index_of(id).unwrap()).clone();
    assert_eq!(
        (stats(MYSTICAL_ELF).kind, stats(MYSTICAL_ELF).level),
        (CardKind::Normal, 4)
    );
    assert_eq!(
        (stats(SUMMONED_SKULL).kind, stats(SUMMONED_SKULL).level),
        (CardKind::Normal, 6)
    );
    assert_eq!(
        (stats(BLUE_EYES).kind, stats(BLUE_EYES).level),
        (CardKind::Normal, 8)
    );
    assert_eq!(stats(SANGAN).kind, CardKind::Effect);
}

#[test]
fn an_unshuffled_deck_is_drawn_in_listed_order() {
    let g = Game::new(
        shared(),
        &stacked([&[SUMMONED_SKULL, BLUE_EYES], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let hand = &g.player(P0).hand;
    assert_eq!(hand[..2], [InstanceId(0), InstanceId(1)]);
    assert_eq!(g.stats(hand[0]).id, SUMMONED_SKULL);
    assert_eq!(g.stats(hand[1]).id, BLUE_EYES);
    assert_eq!(g.stats(hand[2]).id, MYSTICAL_ELF);
}

#[test]
fn normal_summon_a_level_four_monster() {
    let mut g = Game::new(
        shared(),
        &stacked([&[SUMMONED_SKULL, BLUE_EYES], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    let offered = summons_offered(&g);
    // Three Elves in hand, each face-up or face-down; no tributes available
    // for Summoned Skull or Blue-Eyes.
    assert_eq!(offered.len(), 6, "{offered:#?}");
    assert!(offered.iter().all(|a| matches!(
        a,
        Action::Summon { card, tributes, .. } if g.stats(*card).id == MYSTICAL_ELF && tributes.is_empty()
    )));

    let events = play(&mut g, summon(elf));
    assert_eq!(
        events[0],
        Event::Summoned {
            player: P0,
            card: elf,
            position: Position::FaceUpAttack
        }
    );
    let inst = g.instance(elf);
    assert_eq!(inst.location, Location::MonsterZone(0));
    assert_eq!(inst.position, Position::FaceUpAttack);
    assert_eq!(inst.turn_entered_field, Some(1));
    assert_eq!(inst.turn_set, None);
    assert_eq!(g.player(P0).monster_zones[0], Some(elf));
    assert_eq!(g.player(P0).hand.len(), 4);
    assert_eq!(g.player(P0).summons_used, 1);
    assert_eq!(
        g.legal_actions(),
        vec![Action::EndTurn],
        "one summon per turn"
    );
}

#[test]
fn set_is_face_down_defense_and_uses_the_summon() {
    let mut g = Game::new(shared(), &stacked([&[], &[]], MYSTICAL_ELF)).unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: Vec::new(),
            face_down: true,
        },
    );
    let inst = g.instance(elf);
    assert_eq!(inst.position, Position::FaceDownDefense);
    assert_eq!(inst.turn_set, Some(1));
    assert_eq!(g.legal_actions(), vec![Action::EndTurn]);
}

#[test]
fn tribute_summon_needs_one_tribute_for_level_six_and_two_for_level_eight() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    SUMMONED_SKULL,
                    BLUE_EYES,
                    MYSTICAL_ELF,
                ],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let elf1 = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf1));
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.turn(), 3);
    let elf2 = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf2));
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.turn(), 5);

    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let blue_eyes = in_hand(&g, P0, BLUE_EYES);
    let offered = summons_offered(&g);
    let for_card = |id| {
        offered
            .iter()
            .filter(|a| matches!(a, Action::Summon { card, .. } if g.stats(*card).id == id))
            .count()
    };
    assert_eq!(
        for_card(SUMMONED_SKULL),
        4,
        "2 single tributes x 2 positions"
    );
    assert_eq!(for_card(BLUE_EYES), 2, "1 pair x 2 positions");
    assert!(offered.contains(&Action::Summon {
        card: skull,
        tributes: vec![elf1],
        face_down: false
    }));

    let events = play(
        &mut g,
        Action::Summon {
            card: blue_eyes,
            tributes: vec![elf1, elf2],
            face_down: false,
        },
    );
    assert_eq!(
        g.instance(blue_eyes).location,
        Location::MonsterZone(0),
        "lowest freed zone"
    );
    for elf in [elf1, elf2] {
        assert_eq!(g.instance(elf).location, Location::Graveyard);
        assert!(events.contains(&Event::Tributed {
            player: P0,
            card: elf
        }));
        assert!(
            events.contains(&Event::SentToGraveyard {
                card: elf,
                from: Location::MonsterZone(0)
            }) || events.contains(&Event::SentToGraveyard {
                card: elf,
                from: Location::MonsterZone(1)
            })
        );
    }
    assert_eq!(g.player(P0).graveyard, vec![elf1, elf2]);
    assert_eq!(g.player(P0).monsters().count(), 1);
    assert!(summons_offered(&g).is_empty(), "the summon is used");
}

#[test]
fn full_zones_block_summons_but_not_tribute_summons() {
    let shared = shared_with(|r| {
        r.hand.opening_size = 7;
        r.hand.max_size = 7;
        r.summon.normal_or_tribute_per_turn = 7;
    });
    let mut g = Game::new(shared, &stacked([&[MYSTICAL_ELF; 6], &[]], SUMMONED_SKULL)).unwrap();
    for _ in 0..5 {
        let elf = in_hand(&g, P0, MYSTICAL_ELF);
        play(&mut g, summon(elf));
    }
    assert!(g.player(P0).monster_zones.iter().all(|z| z.is_some()));
    let offered = summons_offered(&g);
    assert!(
        offered
            .iter()
            .all(|a| matches!(a, Action::Summon { tributes, .. } if tributes.len() == 1))
    );
    assert_eq!(offered.len(), 10, "Skull: 5 tributes x 2 positions, no Elf");

    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let tribute = g.player(P0).monster_zones[2].unwrap();
    play(
        &mut g,
        Action::Summon {
            card: skull,
            tributes: vec![tribute],
            face_down: false,
        },
    );
    assert_eq!(g.instance(skull).location, Location::MonsterZone(2));
}

#[test]
fn flip_normal_or_tribute_per_turn() {
    let shared = shared_with(|r| r.summon.normal_or_tribute_per_turn = 2);
    let mut g = Game::new(shared, &stacked([&[], &[]], MYSTICAL_ELF)).unwrap();
    for _ in 0..2 {
        let elf = in_hand(&g, P0, MYSTICAL_ELF);
        play(&mut g, summon(elf));
    }
    assert!(summons_offered(&g).is_empty());
    assert_eq!(g.player(P0).monsters().count(), 2);
}

#[test]
fn flip_max_monsters_entering_per_turn() {
    let shared = shared_with(|r| {
        r.summon.normal_or_tribute_per_turn = 2;
        r.summon.max_monsters_entering_per_turn = 1;
    });
    let mut g = Game::new(shared, &stacked([&[], &[]], MYSTICAL_ELF)).unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        summons_offered(&g).is_empty(),
        "the cap wins over the summon count"
    );
}

#[test]
fn flip_tribute_counts() {
    let shared = shared_with(|r| {
        r.summon.tributes_level_5_6 = 2;
        r.summon.tributes_level_7_plus = 2;
        r.summon.normal_or_tribute_per_turn = 3;
    });
    let mut g = Game::new(
        shared,
        &stacked(
            [
                &[MYSTICAL_ELF, MYSTICAL_ELF, SUMMONED_SKULL, BLUE_EYES],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    for _ in 0..2 {
        let elf = in_hand(&g, P0, MYSTICAL_ELF);
        play(&mut g, summon(elf));
    }
    let offered = summons_offered(&g);
    let tributes_for = |id| -> Vec<usize> {
        offered
            .iter()
            .filter_map(|a| match a {
                Action::Summon {
                    card,
                    tributes,
                    face_down: false,
                } if g.stats(*card).id == id => Some(tributes.len()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        tributes_for(SUMMONED_SKULL),
        vec![2],
        "level 6 now needs two"
    );
    assert_eq!(
        tributes_for(BLUE_EYES),
        vec![2],
        "level 8 needs two: one pair"
    );
}

#[test]
fn effect_monsters_offer_no_actions_in_m1() {
    let g = Game::new(shared(), &stacked([&[], &[]], SANGAN)).unwrap();
    assert_eq!(g.phase(), Phase::Main1);
    assert_eq!(g.legal_actions(), vec![Action::EndTurn]);
}

// `summon_in_main_phase_2` is flipped in the battle tests (step 5), since
// Main Phase 2 is only reachable through the Battle Phase.
