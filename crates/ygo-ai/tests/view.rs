//! `PlayerView::sample`: a sampled game looks exactly like the real one
//! to the decider, keeps every card with its owner, and redeals what the
//! decider cannot see.

use std::collections::BTreeMap;
use std::sync::Arc;

use ygo_ai::{Player, RandomPlayer};
use ygo_engine::testing::{sample_deck, shared_from};
use ygo_engine::{CardIndex, Game, GameSetup, Location, PlayerView, Shared};

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn new_game(seed: u64) -> Game {
    let decks = [
        sample_deck("save_a_owner_current_40.json"),
        sample_deck("save_b_dark_plasma_40.json"),
    ];
    Game::new(shared(), &GameSetup::new(decks, seed)).unwrap()
}

/// Each player's cards, counted by card.
fn cards_by_owner(g: &Game) -> [BTreeMap<CardIndex, usize>; 2] {
    let mut out = [BTreeMap::new(), BTreeMap::new()];
    for inst in g.instances() {
        *out[inst.owner.index()].entry(inst.card).or_default() += 1;
    }
    out
}

#[test]
fn a_sample_looks_the_same_to_the_decider_and_keeps_every_card() {
    let mut decisions = 0;
    let mut redealt = 0;
    for seed in 0..30 {
        let mut g = new_game(seed);
        let mut bots = [RandomPlayer::new(seed), RandomPlayer::new(seed + 100)];
        while let Some(view) = PlayerView::new(&g) {
            decisions += 1;
            let sample = view.sample(seed * 1000 + decisions);
            assert_eq!(
                &sample.observe(view.player()),
                view.observation(),
                "seed {seed}: the decider tells the sample from the game"
            );
            assert_eq!(sample.actions(), view.actions(), "seed {seed}");
            assert_eq!(cards_by_owner(&sample), cards_by_owner(&g));
            if sample.instances() != g.instances() {
                redealt += 1;
            }
            for (a, b) in sample.instances().iter().zip(g.instances()) {
                assert_eq!(
                    (a.owner, a.location, a.position),
                    (b.owner, b.location, b.position),
                    "only identities move"
                );
                let stats = sample.shared().pool.get(a.card);
                match a.location {
                    Location::MonsterZone(_) => assert!(stats.is_monster()),
                    Location::SpellTrapZone(_) | Location::FieldZone => {
                        assert!(!stats.is_monster())
                    }
                    Location::Extra => assert!(stats.is_extra_deck()),
                    Location::Hand | Location::Deck => assert!(!stats.is_extra_deck()),
                    _ => {}
                }
            }
            let i = view.player().index();
            let action = bots[i].choose(&view);
            g.apply(&action).unwrap();
        }
    }
    assert!(
        redealt * 10 > decisions * 9,
        "{redealt} of {decisions} samples redealt"
    );
}

#[test]
fn samples_vary_with_the_seed_and_repeat_with_it() {
    let g = new_game(5);
    let view = PlayerView::new(&g).unwrap();
    let opp = view.player().opponent();
    let hand = |s: &Game| -> Vec<CardIndex> {
        s.player(opp)
            .hand
            .iter()
            .map(|&id| s.instance(id).card)
            .collect()
    };
    assert_eq!(view.sample(1), view.sample(1));
    let hands: std::collections::BTreeSet<_> = (0..20).map(|s| hand(&view.sample(s))).collect();
    assert!(hands.len() > 10, "{} distinct hands", hands.len());
}

#[test]
fn the_deciders_own_cards_stay_put_except_the_deck_order() {
    let g = new_game(9);
    let view = PlayerView::new(&g).unwrap();
    let me = view.player();
    let sample = view.sample(3);
    for &id in &g.player(me).hand {
        assert_eq!(sample.instance(id).card, g.instance(id).card);
    }
    let deck = |s: &Game| -> Vec<CardIndex> {
        s.player(me)
            .deck
            .iter()
            .map(|&id| s.instance(id).card)
            .collect()
    };
    assert_ne!(deck(&sample), deck(&g), "own deck order is unknown");
    let mut a = deck(&sample);
    let mut b = deck(&g);
    a.sort();
    b.sort();
    assert_eq!(a, b);
}
