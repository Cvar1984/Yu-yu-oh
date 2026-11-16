//! The engine's invariants under random play with every M2 script active.

use proptest::prelude::*;
use ygo_engine::testing::{check_invariants, sample_deck, shared_from};
use ygo_engine::{DeckList, Game, GameSetup};

fn decks() -> Vec<DeckList> {
    vec![
        sample_deck("owner_main_45.json"),
        sample_deck("defense_tuned_45.json"),
        sample_deck("umiiruka_water_40.json"),
        sample_deck("exodia_stall_full_pool_40.json"),
    ]
}

fn run(deck_a: usize, deck_b: usize, seed: u64, choices: &[u32]) {
    let decks = decks();
    let setup = GameSetup::new([decks[deck_a].clone(), decks[deck_b].clone()], seed);
    let mut g = Game::new(shared_from(ygo_cards::registry()), &setup).unwrap();
    check_invariants(&g);
    for &c in choices {
        if g.result().is_some() {
            break;
        }
        let actions = g.legal_actions();
        let action = actions[c as usize % actions.len()].clone();
        let before = g.clone();
        g.apply(&action)
            .unwrap_or_else(|e| panic!("{action:?}: {e}"));
        check_invariants(&g);
        let mut again = before;
        again.apply(&action).unwrap();
        assert_eq!(
            again, g,
            "the same action on the same state gives the same state"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn invariants_hold_with_scripts_active(
        deck_a in 0usize..4,
        deck_b in 0usize..4,
        seed in any::<u64>(),
        choices in prop::collection::vec(any::<u32>(), 0..500),
    ) {
        run(deck_a, deck_b, seed, &choices);
    }
}

#[test]
fn scripted_random_games_finish() {
    let decks = decks();
    let shared = shared_from(ygo_cards::registry());
    let mut lcg = 99u64;
    for seed in 0..40u64 {
        let a = &decks[seed as usize % decks.len()];
        let b = &decks[(seed as usize / 4) % decks.len()];
        let mut g = Game::new(
            shared.clone(),
            &GameSetup::new([a.clone(), b.clone()], seed),
        )
        .unwrap();
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
    }
}
