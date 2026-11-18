//! M1 step 7: invariants that must hold after every action of every game,
//! checked over random play.

mod common;

use common::{normal_deck, sample_deck, shared};
use proptest::prelude::*;
use ygo_engine::{DeckList, Game, GameSetup, InstanceId};

fn decks() -> Vec<DeckList> {
    vec![
        normal_deck(40),
        sample_deck("owner_main_45.json"),
        sample_deck("defense_tuned_45.json"),
        sample_deck("umiiruka_water_40.json"),
    ]
}

use common::check_invariants;

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
        deck_a in 0usize..4,
        deck_b in 0usize..4,
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
