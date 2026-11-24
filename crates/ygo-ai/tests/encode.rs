//! The neural encoding (`ygo_ai::encode`): every decision of random games
//! from the whole pool encodes with the right sizes, and a sample that
//! looks the same to the decider encodes the same, so no hidden card
//! reaches the network.

use ygo_ai::encode::{ACTION_F, CARD_F, GLOBAL_F, encode};
use ygo_ai::{Player, RandomPlayer};
use ygo_engine::testing::shared_from;
use ygo_engine::{CardIndex, DeckList, Game, GameSetup, PlayerView, Shared};

fn shared() -> std::sync::Arc<Shared> {
    shared_from(ygo_cards::registry())
}

/// A random 40-card Main Deck from the whole pool, at most 3 copies each.
fn random_pool_deck(shared: &Shared, seed: u64) -> DeckList {
    let pool = &shared.pool;
    let ids: Vec<_> = (0..pool.len() as u16)
        .map(|i| pool.get(CardIndex(i)))
        .filter(|s| s.is_main_deck())
        .map(|s| s.id)
        .collect();
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let mut main = Vec::new();
    while main.len() < 40 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let id = ids[(x >> 33) as usize % ids.len()];
        if main.iter().filter(|&&c| c == id).count() < 3 {
            main.push(id);
        }
    }
    DeckList {
        main,
        extra: Vec::new(),
    }
}

#[test]
fn every_decision_of_random_games_encodes() {
    let shared = shared();
    let (mut decisions, mut most_cards, mut most_actions) = (0, 0, 0);
    for seed in 0..30 {
        let decks = [
            random_pool_deck(&shared, seed),
            random_pool_deck(&shared, seed + 500),
        ];
        let mut g = Game::new(shared.clone(), &GameSetup::new(decks, seed)).unwrap();
        let mut bots = [RandomPlayer::new(seed), RandomPlayer::new(seed + 1)];
        while let Some(view) = PlayerView::new(&g) {
            let e = encode(&view);
            assert_eq!(e.cards.len() % CARD_F, 0);
            assert_eq!(e.globals.len(), GLOBAL_F);
            assert_eq!(e.actions.len(), view.actions().len() * ACTION_F);
            assert!(e.card_count() > 0, "at least the decider's cards");
            for x in e.cards.iter().chain(&e.globals).chain(&e.actions) {
                assert!(x.is_finite() && x.abs() <= 10.0, "{x}");
            }
            decisions += 1;
            most_cards = most_cards.max(e.card_count());
            most_actions = most_actions.max(e.action_count());
            let a = bots[view.player().index()].choose(&view);
            g.apply(&a).unwrap();
        }
    }
    assert!(decisions > 1000, "{decisions} decisions");
    eprintln!("{decisions} decisions, up to {most_cards} cards and {most_actions} actions");
}

#[test]
fn a_sample_that_looks_the_same_encodes_the_same() {
    let shared = shared();
    let mut compared = 0;
    for seed in 100..115 {
        let decks = [
            random_pool_deck(&shared, seed),
            random_pool_deck(&shared, seed + 500),
        ];
        let mut g = Game::new(shared.clone(), &GameSetup::new(decks, seed)).unwrap();
        let mut bots = [RandomPlayer::new(seed), RandomPlayer::new(seed + 1)];
        let mut step = 0u64;
        while let Some(view) = PlayerView::new(&g) {
            step += 1;
            let real = encode(&view);
            let sample = view.sample(seed * 10_000 + step);
            let seen = PlayerView::new(&sample).expect("the sample has the same decision");
            let other = encode(&seen);
            assert_eq!(real.cards, other.cards, "cards differ at step {step}");
            assert_eq!(real.globals, other.globals, "game differs at step {step}");
            // Legal actions can depend on a hidden card (documented on
            // `PlayerView::sample`); compare them when they are the same.
            if view.actions() == seen.actions() {
                assert_eq!(real.actions, other.actions, "actions differ at step {step}");
                compared += 1;
            }
            let a = bots[view.player().index()].choose(&view);
            g.apply(&a).unwrap();
        }
    }
    assert!(compared > 500, "{compared} decisions compared");
}
