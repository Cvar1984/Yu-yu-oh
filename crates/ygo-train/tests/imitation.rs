//! Imitation (`ygo_train::imitation`): samples are exactly the teacher's
//! decisions with a choice, with the action it took and the result; a
//! short training run reports a baseline and an epoch and plays.

use ygo_ai::neural::{Net, NeuralPlayer, cpu};
use ygo_ai::{Player, RandomPlayer, play_game};
use ygo_engine::testing::{sample_deck, shared_from};
use ygo_engine::{GameRecord, GameResult, GameSetup, PlayerView, Shared};
use ygo_train::imitation::{Options, samples, train};

fn shared() -> std::sync::Arc<Shared> {
    shared_from(ygo_cards::registry())
}

/// A game between two random players, the first labelled "teacher".
fn record(seed: u64) -> GameRecord {
    let decks = [
        sample_deck("enemy_e3_removal.json"),
        sample_deck("enemy_e4_big_beatdown.json"),
    ];
    let mut a = RandomPlayer::new(seed);
    let mut b = RandomPlayer::new(seed + 1);
    let mut players: [&mut dyn Player; 2] = [&mut a, &mut b];
    let labels = ["teacher".to_string(), "random".to_string()];
    play_game(shared(), &GameSetup::new(decks, seed), &mut players, labels)
        .unwrap()
        .1
}

#[test]
fn samples_are_the_teachers_choices_with_the_result() {
    let shared = shared();
    for seed in 0..3 {
        let r = record(seed);
        let got = samples(&r, &shared, &|label| label == "teacher").unwrap();
        // Replay and list the teacher's decisions with a choice.
        let mut game = r.start(&shared).unwrap();
        let mut expected = Vec::new();
        for (i, a) in r.actions.iter().enumerate() {
            let view = PlayerView::new(&game).unwrap();
            if view.player().index() == 0 && view.actions().len() > 1 {
                let index = view.actions().iter().position(|x| *x == a.action).unwrap();
                expected.push(index);
            }
            GameRecord::step(&mut game, i, a).unwrap();
        }
        assert!(!expected.is_empty());
        assert_eq!(got.iter().map(|s| s.action).collect::<Vec<_>>(), expected);
        let value = match r.result {
            Some(GameResult::Win { winner, .. }) if winner.index() == 0 => 1.0,
            Some(GameResult::Win { .. }) => -1.0,
            _ => 0.0,
        };
        assert!(got.iter().all(|s| s.value == value));
        assert!(
            got.iter()
                .all(|s| s.action < s.encoded.action_count() && s.encoded.action_count() > 1)
        );
    }
}

#[test]
fn a_short_run_reports_and_plays() {
    let shared = shared();
    let records: Vec<GameRecord> = (10..14).map(record).collect();
    let opts = Options {
        epochs: 1,
        batch: 32,
        holdout: 4,
        pool: 64,
        ..Options::default()
    };
    let mut epochs = Vec::new();
    let net = train(
        Net::new(&cpu()),
        &records,
        &shared,
        &|label| label == "teacher",
        &opts,
        &mut |e| epochs.push(*e),
    )
    .unwrap();
    assert_eq!(epochs.iter().map(|e| e.epoch).collect::<Vec<_>>(), [0, 1]);
    assert!(epochs[1].samples > 0 && epochs[1].loss.is_finite());
    assert!(epochs[0].held_out > 0, "one record of four held out");
    // The trained network plays a legal game.
    let decks = [
        sample_deck("enemy_e3_removal.json"),
        sample_deck("enemy_e3_removal.json"),
    ];
    let mut a = NeuralPlayer::new(net, cpu(), 0.0, 1);
    let mut b = RandomPlayer::new(1);
    let mut players: [&mut dyn Player; 2] = [&mut a, &mut b];
    let labels = ["neural".to_string(), "random".to_string()];
    let (game, _) = play_game(shared, &GameSetup::new(decks, 3), &mut players, labels).unwrap();
    assert!(game.result().is_some());
}
