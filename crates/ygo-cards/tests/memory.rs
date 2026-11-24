//! What a player remembers and knows beyond the open board
//! (`ygo-engine/src/memory.rs`): cards seen and hidden again, their own
//! Deck's contents, and Types changed by effects.

use ygo_cards::extra_traps::D_TRIBE;
use ygo_cards::flips::HANE_HANE;
use ygo_cards::oneshot::DARKNESS_APPROACHES;
use ygo_data::CardId;
use ygo_engine::testing::{CELTIC_GUARDIAN, MYSTICAL_ELF, check_invariants, next_turn, play};
use ygo_engine::{Action, DecisionKind, InstanceId, PlayerView, VisibleCard};

mod psv_common;
use psv_common::*;

fn known(c: &VisibleCard) -> bool {
    matches!(c, VisibleCard::Known { .. })
}

/// Flip Summons `card` and resolves its FLIP effect on `target`.
fn flip_onto(g: &mut ygo_engine::Game, card: InstanceId, target: InstanceId) {
    play(g, Action::FlipSummon { card });
    loop {
        let d = g.pending_decision().unwrap();
        let hit = g
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![target]));
        match hit {
            Some(a) if d.kind == DecisionKind::Triggers => {
                play(g, a);
                ygo_engine::testing::pass_all(g);
                return;
            }
            _ => {
                play(g, Action::Pass);
            }
        }
    }
}

// A monster bounced to the hand is remembered there, and forgotten once
// it leaves the hand face-down.
#[test]
fn a_bounced_monster_is_remembered_until_it_moves() {
    let mut g = game(&[HANE_HANE], &[CELTIC_GUARDIAN]);
    let hane = set_monster(&mut g, P0, HANE_HANE);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    flip_onto(&mut g, hane, celtic);
    assert!(g.player(P1).hand.contains(&celtic));
    let seen = g.observe(P0);
    for c in &seen.opponent().hand {
        assert_eq!(known(c), c.instance() == celtic, "{c:?}");
    }
    check_invariants(&g);
    next_turn(&mut g);
    let set = in_hand_set(&mut g, P1, celtic);
    let seen = g.observe(P0);
    let zone = seen
        .opponent()
        .monster_zones
        .iter()
        .flatten()
        .find(|c| c.instance() == set)
        .unwrap();
    assert!(!known(zone), "Set from the hand: it could be any hand card");
}

/// Sets `card` face-down from `player`'s hand.
fn in_hand_set(
    g: &mut ygo_engine::Game,
    player: ygo_engine::PlayerId,
    card: InstanceId,
) -> InstanceId {
    assert!(g.player(player).hand.contains(&card));
    play(
        g,
        Action::Summon {
            card,
            tributes: vec![],
            face_down: true,
        },
    );
    ygo_engine::testing::pass_all(g);
    card
}

// A face-up monster turned face-down stays known to the opponent, and a
// sample of the game keeps it.
#[test]
fn a_monster_turned_face_down_stays_known_in_samples() {
    let mut g = game(
        &[DARKNESS_APPROACHES, MYSTICAL_ELF, MYSTICAL_ELF],
        &[CELTIC_GUARDIAN],
    );
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, DARKNESS_APPROACHES);
    assert!(!g.instance(celtic).position.is_face_up());
    assert!(g.remembers(P0, celtic));
    let view = PlayerView::new(&g).unwrap();
    assert_eq!(view.player(), P0);
    for seed in 0..20 {
        let sample = view.sample(seed);
        assert_eq!(sample.stats(celtic).id, CELTIC_GUARDIAN, "seed {seed}");
    }
    // Shuffled identities are forgotten (Magical Hats), even when the
    // shuffle happens to leave a card in place.
    g.shuffle_set_monsters(&[celtic]);
    assert!(!g.remembers(P0, celtic));
}

// A player knows what is left in their own Deck, never its order.
#[test]
fn the_view_lists_the_deciders_own_deck_sorted() {
    let g = game(&[CELTIC_GUARDIAN], &[]);
    let view = PlayerView::new(&g).unwrap();
    let mut expected: Vec<CardId> = g
        .deck(view.player())
        .iter()
        .map(|&c| g.stats(c).id)
        .collect();
    expected.sort_unstable();
    assert_eq!(view.own_deck(), expected);
    assert_eq!(view.own_deck().len(), view.observation().me().deck_count);
}

// A Type changed by an effect is in the observation.
#[test]
fn a_changed_type_is_observed() {
    let mut g = game(&[CELTIC_GUARDIAN, D_TRIBE], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, D_TRIBE);
    next_turn(&mut g);
    next_turn(&mut g);
    assert!(g.observe(P1).types.is_empty());
    activate(&mut g, D_TRIBE);
    for viewer in [P0, P1] {
        assert_eq!(
            g.observe(viewer).types,
            vec![(celtic, "Dragon".to_string())]
        );
    }
}
