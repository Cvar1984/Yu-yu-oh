//! The engine's invariants under random play with every M2 script active.

use proptest::prelude::*;
use ygo_data::CardId;
use ygo_engine::testing::{check_invariants, sample_deck, shared_from};
use ygo_engine::{CardIndex, DeckList, Game, GameSetup};

fn decks() -> Vec<DeckList> {
    vec![
        sample_deck("save_a_owner_current_40.json"),
        sample_deck("save_a_best_overall_40.json"),
        sample_deck("save_b_dark_plasma_40.json"),
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
        deck_a in 0usize..3,
        deck_b in 0usize..3,
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

/// A legal random deck from the whole pool: `main` Main Deck cards, at
/// most three of each, and up to 15 Fusion Monsters.
fn random_deck(seed: u64, main: usize) -> DeckList {
    let shared = shared_from(ygo_cards::registry());
    let pool = &shared.pool;
    let (mut mains, mut extras): (Vec<CardId>, Vec<CardId>) = (Vec::new(), Vec::new());
    for i in 0..pool.len() as u16 {
        let c = pool.get(CardIndex(i));
        if c.is_extra_deck() {
            extras.push(c.id);
        } else if c.is_main_deck() {
            mains.push(c.id);
        }
    }
    let mut state = seed | 1;
    let mut next = |n: usize| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 33) as usize) % n
    };
    let mut deck = DeckList::default();
    while deck.main.len() < main {
        let id = mains[next(mains.len())];
        if deck.main.iter().filter(|&&c| c == id).count() < 3 {
            deck.main.push(id);
        }
    }
    for _ in 0..next(16) {
        deck.extra.push(extras[next(extras.len())]);
    }
    deck
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Random decks from the whole card pool exercise every script, not only the
    /// ones in the sample decks.
    #[test]
    fn invariants_hold_with_random_whole_pool_decks(
        deck_seed in any::<u64>(),
        seed in any::<u64>(),
        size in 40usize..=60,
        choices in prop::collection::vec(any::<u32>(), 0..600),
    ) {
        let decks = [random_deck(deck_seed, size), random_deck(deck_seed ^ 0x9E37, 40)];
        let setup = GameSetup::new(decks, seed);
        let mut g = Game::new(shared_from(ygo_cards::registry()), &setup).unwrap();
        check_invariants(&g);
        for &c in &choices {
            if g.result().is_some() {
                break;
            }
            let actions = g.legal_actions();
            let action = actions[c as usize % actions.len()].clone();
            let before = g.clone();
            g.apply(&action).unwrap_or_else(|e| panic!("{action:?}: {e}"));
            check_invariants(&g);
            let mut again = before;
            again.apply(&action).unwrap();
            prop_assert_eq!(&again, &g);
        }
    }
}

#[test]
fn whole_pool_random_games_finish() {
    let shared = shared_from(ygo_cards::registry());
    let mut lcg = 7u64;
    for seed in 0..60u64 {
        let decks = [
            random_deck(seed, 40 + (seed as usize % 21)),
            random_deck(seed + 1000, 40),
        ];
        let mut g = Game::new(shared.clone(), &GameSetup::new(decks, seed)).unwrap();
        let mut steps = 0;
        while g.result().is_none() {
            let actions = g.legal_actions();
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let action = actions[(lcg >> 33) as usize % actions.len()].clone();
            g.apply(&action)
                .unwrap_or_else(|e| panic!("seed {seed}: {action:?}: {e}"));
            steps += 1;
            assert!(steps < 200_000, "seed {seed} never ends");
        }
        check_invariants(&g);
    }
}
