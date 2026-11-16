//! M1 step 7: invariants that must hold after every action of every game,
//! checked over random play.

mod common;

use std::collections::BTreeSet;

use common::{normal_deck, sample_deck, shared};
use proptest::prelude::*;
use ygo_data::HandLimitCheck;
use ygo_engine::{
    DeckList, Game, GameSetup, InstanceId, Location, Phase, PlayerId, Position, VisibleCard,
};

fn decks() -> Vec<DeckList> {
    vec![
        normal_deck(40),
        sample_deck("owner_main_45.json"),
        sample_deck("defense_tuned_45.json"),
        sample_deck("umiiruka_water_40.json"),
        sample_deck("exodia_stall_full_pool_40.json"),
    ]
}

/// Every rule that must hold in any reachable state.
fn check_invariants(g: &Game) {
    let rules = &g.shared().rules;
    let mut seen = BTreeSet::new();
    for p in PlayerId::BOTH {
        let s = g.player(p);
        let zones = [
            (Location::Deck, s.deck.clone()),
            (Location::Hand, s.hand.clone()),
            (Location::Graveyard, s.graveyard.clone()),
            (Location::Extra, s.extra.clone()),
        ];
        for (zone, ids) in zones {
            for id in ids {
                assert!(seen.insert(id), "{id:?} is in two places");
                let inst = g.instance(id);
                assert_eq!(inst.location, zone, "{id:?}");
                assert_eq!(
                    inst.controller, p,
                    "{id:?} off-field cards are with their owner"
                );
                assert_eq!(inst.owner, p);
            }
        }
        assert_eq!(s.monster_zones.len(), rules.field.monster_zones);
        assert_eq!(s.spell_trap_zones.len(), rules.field.spell_trap_zones);
        for (i, zone) in s.monster_zones.iter().enumerate() {
            if let Some(id) = zone {
                assert!(seen.insert(*id), "{id:?} is in two places");
                let inst = g.instance(*id);
                assert_eq!(inst.location, Location::MonsterZone(i as u8));
                assert_eq!(inst.controller, p);
                assert!(g.stats(*id).is_monster());
                assert_ne!(
                    inst.position,
                    Position::FaceDown,
                    "monsters use the Defense positions"
                );
                assert!(inst.turn_entered_field.is_some());
            }
        }
        for (i, zone) in s.spell_trap_zones.iter().enumerate() {
            if let Some(id) = zone {
                assert!(seen.insert(*id), "{id:?} is in two places");
                let inst = g.instance(*id);
                assert_eq!(inst.location, Location::SpellTrapZone(i as u8));
                assert_eq!(inst.controller, p);
                assert!(!g.stats(*id).is_monster());
                assert!(matches!(
                    inst.position,
                    Position::FaceDown | Position::FaceUp
                ));
            }
        }
        assert!(s.lp <= rules.life.starting_lp);
        assert!(usize::from(s.summons_used) <= rules.summon.normal_or_tribute_per_turn);
        if rules.hand.hand_limit_check == HandLimitCheck::AfterDraw
            && matches!(g.phase(), Phase::Main1 | Phase::Battle(_) | Phase::Main2)
        {
            assert!(
                s.hand.len() <= rules.hand.max_size,
                "hand limit after the draw"
            );
        }
    }
    assert_eq!(seen.len(), g.instances().len(), "every card is somewhere");
    assert!(g.turn() <= g.max_turns());
    let any_dead = PlayerId::BOTH.iter().any(|p| g.player(*p).lp == 0);
    assert_eq!(
        any_dead,
        matches!(
            g.result(),
            Some(ygo_engine::GameResult::Win {
                reason: ygo_engine::WinReason::LifePoints,
                ..
            }) | Some(ygo_engine::GameResult::Draw {
                reason: ygo_engine::DrawReason::BothLifePointsZero
            })
        )
    );
    assert_eq!(g.result().is_none(), g.pending_decision().is_some());
    if g.pending_decision()
        .is_some_and(|d| d.kind != ygo_engine::DecisionKind::Choose)
    {
        assert!(
            g.chain().is_empty(),
            "a chain resolves before the next decision"
        );
    }
    assert_eq!(g.result().is_none(), !g.legal_actions().is_empty());

    for viewer in PlayerId::BOTH {
        let obs = g.observe(viewer);
        let opp = obs.opponent();
        assert!(
            opp.hand
                .iter()
                .all(|c| matches!(c, VisibleCard::Hidden { .. }))
        );
        for zone in opp.monster_zones.iter().flatten() {
            let face_up = g.instance(zone.instance()).position.is_face_up();
            assert_eq!(matches!(zone, VisibleCard::Known { .. }), face_up);
        }
        assert!(
            obs.me()
                .hand
                .iter()
                .all(|c| matches!(c, VisibleCard::Known { .. }))
        );
    }
}

fn run(deck_a: usize, deck_b: usize, seed: u64, choices: &[u32]) -> Game {
    let decks = decks();
    let setup = GameSetup::new([decks[deck_a].clone(), decks[deck_b].clone()], seed);
    let mut g = Game::new(shared(), &setup).unwrap();
    check_invariants(&g);
    for &c in choices {
        if g.result().is_some() {
            break;
        }
        let actions = g.legal_actions();
        let action = actions[c as usize % actions.len()].clone();
        let before = g.clone();
        g.apply(&action).unwrap();
        check_invariants(&g);
        let mut again = before;
        again.apply(&action).unwrap();
        assert_eq!(
            again, g,
            "the same action on the same state gives the same state"
        );
    }
    g
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn invariants_hold_along_random_play(
        deck_a in 0usize..5,
        deck_b in 0usize..5,
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<u32>(), 0..400),
    ) {
        run(deck_a, deck_b, seed, &choices);
    }
}

#[test]
fn every_offered_action_applies_cleanly() {
    let setup = GameSetup::new([normal_deck(40), sample_deck("owner_main_45.json")], 5);
    let mut g = Game::new(shared(), &setup).unwrap();
    let mut lcg = 12345u64;
    for _ in 0..150 {
        if g.result().is_some() {
            break;
        }
        let actions = g.legal_actions();
        for a in &actions {
            let mut copy = g.clone();
            copy.apply(a).unwrap_or_else(|e| panic!("{a:?}: {e}"));
            check_invariants(&copy);
        }
        lcg = lcg
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let action = actions[(lcg >> 33) as usize % actions.len()].clone();
        g.apply(&action).unwrap();
    }
}

#[test]
fn random_games_finish() {
    let decks = decks();
    let mut lcg = 7u64;
    for seed in 0..60u64 {
        let a = &decks[seed as usize % decks.len()];
        let b = &decks[(seed as usize / 5) % decks.len()];
        let mut g = Game::new(shared(), &GameSetup::new([a.clone(), b.clone()], seed)).unwrap();
        let mut steps = 0;
        while g.result().is_none() {
            let actions = g.legal_actions();
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let action = actions[(lcg >> 33) as usize % actions.len()].clone();
            g.apply(&action).unwrap();
            steps += 1;
            assert!(steps < 100_000, "seed {seed} never ends");
        }
        check_invariants(&g);
        assert!(g.turn() <= GameSetup::DEFAULT_MAX_TURNS);
        let _ = InstanceId(0);
    }
}
