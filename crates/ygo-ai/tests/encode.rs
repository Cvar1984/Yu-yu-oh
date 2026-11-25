//! The neural encoding (`ygo_ai::encode`): every decision of random games
//! from the whole pool encodes with the right sizes, and a sample that
//! looks the same to the decider encodes the same, so no hidden card
//! reaches the network.

use ygo_ai::encode::{ACTION_F, CARD_F, GLOBAL_F, LINGER_F, encode};
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
    let (mut decisions, mut most_cards, mut most_actions, mut most_lingering) = (0, 0, 0, 0);
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
            assert_eq!(
                e.lingering.len(),
                view.observation().lingering.len() * LINGER_F
            );
            assert!(
                e.card_count() >= view.own_deck().len(),
                "the decider's Deck is among the cards"
            );
            for x in e
                .cards
                .iter()
                .chain(&e.globals)
                .chain(&e.actions)
                .chain(&e.lingering)
            {
                assert!(x.is_finite() && x.abs() <= 10.0, "{x}");
            }
            decisions += 1;
            most_cards = most_cards.max(e.card_count());
            most_actions = most_actions.max(e.action_count());
            most_lingering = most_lingering.max(e.lingering_count());
            let a = bots[view.player().index()].choose(&view);
            g.apply(&a).unwrap();
        }
    }
    assert!(decisions > 1000, "{decisions} decisions");
    assert!(most_lingering > 0, "some lingering effect was encoded");
    eprintln!(
        "{decisions} decisions, up to {most_cards} cards, {most_actions} actions and {most_lingering} lingering effects"
    );
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
            assert_eq!(
                real.lingering, other.lingering,
                "lingering differs at step {step}"
            );
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

/// The race block of a card vector starts after known/hidden (2), kind
/// (6), subtype (7), stats (5) and the attribute block (8).
const RACE_START: usize = 28;
const WARRIOR: usize = 22;
const DRAGON: usize = 7;

// A Type changed by D. Tribe is the Type the network sees.
#[test]
fn a_changed_type_is_encoded() {
    use ygo_engine::testing::{
        CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, next_turn, pass_all, play, stacked, summon,
    };
    let shared = shared();
    let mut g = Game::new(
        shared,
        &stacked(
            [&[CELTIC_GUARDIAN, ygo_cards::extra_traps::D_TRIBE], &[]],
            MAMMOTH_GRAVEYARD,
        ),
    )
    .unwrap();
    let celtic = ygo_engine::testing::in_hand(&g, ygo_engine::PlayerId(0), CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    let tribe =
        ygo_engine::testing::in_hand(&g, ygo_engine::PlayerId(0), ygo_cards::extra_traps::D_TRIBE);
    play(&mut g, ygo_engine::Action::SetSpellTrap { card: tribe });
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    // The vector of the viewer's face-up monster: place "Monster", mine.
    let monster_race = |g: &Game| -> usize {
        let e = encode(&PlayerView::new(g).unwrap());
        let place = CARD_F - 17;
        let v = e
            .cards
            .chunks(CARD_F)
            .find(|v| v[place + 1] == 1.0 && v[place + 10] == 1.0)
            .expect("a monster of the viewer's");
        (0..27).find(|&i| v[RACE_START + i] == 1.0).unwrap()
    };
    assert_eq!(monster_race(&g), WARRIOR);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, ygo_engine::Action::Activate { card, .. } if *card == tribe))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(monster_race(&g), DRAGON);
}
