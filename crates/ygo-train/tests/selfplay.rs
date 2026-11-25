//! Self-play (`ygo_train::selfplay`): every decision with a choice, of
//! both sides, carries the search's visit shares; the samples carry them
//! with the result for each decider; the record replays; the same seed
//! plays the same game.

use ygo_ai::mcts::MctsSettings;
use ygo_ai::neural::{Net, cpu};
use ygo_engine::testing::{sample_deck, shared_from};
use ygo_engine::{GameRecord, GameResult, GameSetup, PlayerView};
use ygo_train::selfplay::{SelfPlayGame, play, samples};

fn game(net: &Net, seed: u64) -> SelfPlayGame {
    let deck = sample_deck("enemy_e3_removal.json");
    let settings = MctsSettings {
        iterations: 4,
        ..MctsSettings::default()
    };
    play(
        shared_from(ygo_cards::registry()),
        &GameSetup::new([deck.clone(), deck], seed),
        net,
        &settings,
        2,
    )
    .unwrap()
}

#[test]
fn every_choice_of_both_sides_is_labelled_and_the_record_replays() {
    let net = Net::new(&cpu());
    let shared = shared_from(ygo_cards::registry());
    let g = game(&net, 3);
    assert!(g.record.result.is_some());
    assert_eq!(
        game(&net, 3).record.actions,
        g.record.actions,
        "same seed, same game"
    );
    let mut state = g.record.start(&shared).unwrap();
    let mut expected = Vec::new();
    let mut deciders = Vec::new();
    for (i, a) in g.record.actions.iter().enumerate() {
        let view = PlayerView::new(&state).unwrap();
        if view.actions().len() > 1 {
            expected.push((i, view.actions().len()));
            deciders.push(view.player());
        }
        GameRecord::step(&mut state, i, a).unwrap();
    }
    assert!(deciders.iter().any(|p| p.index() == 0) && deciders.iter().any(|p| p.index() == 1));
    let got: Vec<(usize, usize)> = g.targets.iter().map(|(i, t)| (*i, t.len())).collect();
    assert_eq!(got, expected);
    for (_, t) in &g.targets {
        assert!((t.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{t:?}");
    }
    assert_eq!(g.record.replay(&shared).unwrap(), state);

    let s = samples(&g, &shared).unwrap();
    assert_eq!(s.len(), g.targets.len());
    for ((sample, (_, t)), who) in s.iter().zip(&g.targets).zip(&deciders) {
        assert_eq!(&sample.policy, t);
        let value = match g.record.result {
            Some(GameResult::Win { winner, .. }) if winner == *who => 1.0,
            Some(GameResult::Win { .. }) => -1.0,
            _ => 0.0,
        };
        assert_eq!(sample.value, value);
    }
}
