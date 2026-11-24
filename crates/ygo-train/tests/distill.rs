//! Distillation (`ygo_train::distill`): the teacher labels exactly the
//! student's decisions with a choice, its shares sum to 1, with `mix` 1 the
//! teacher's move is played, the samples carry the labels, and training
//! on them moves the network towards the teacher.

use std::sync::Arc;

use ygo_ai::neural::{Net, NeuralPlayer, cpu};
use ygo_ai::{HeuristicPlayer, Player, RandomPlayer, SearchPlayer, SearchSettings};
use ygo_data::{CardValues, Styles};
use ygo_engine::testing::{bundle, root, sample_deck, shared_from};
use ygo_engine::{GameRecord, GameSetup, PlayerId, PlayerView, Shared};
use ygo_train::distill::{Labelled, play_labelled, samples};
use ygo_train::imitation::{Options, train_on};

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn teacher(net: &Net, seed: u64) -> SearchPlayer {
    let b = bundle();
    let values = CardValues::load(&root().join("config/card_values.toml"), &b.cards).unwrap();
    let styles = Styles::load(&root().join("config/styles.toml"), &b.cards).unwrap();
    let rollout = HeuristicPlayer::new(
        shared(),
        &b.cards,
        &values,
        styles.get("beatdown").unwrap(),
        seed,
    );
    let settings = SearchSettings {
        iterations: 8,
        ..SearchSettings::default()
    };
    SearchPlayer::new(rollout, settings, seed).with_prior(Box::new(NeuralPlayer::new(
        net.clone(),
        cpu(),
        0.0,
        seed,
    )))
}

fn game(net: &Net, seed: u64, side: PlayerId, mix: f64) -> Labelled {
    let decks = [
        sample_deck("enemy_e3_removal.json"),
        sample_deck("enemy_e4_big_beatdown.json"),
    ];
    let mut student = NeuralPlayer::new(net.clone(), cpu(), 0.0, seed);
    let mut opponent = RandomPlayer::new(seed);
    let mut labels = ["random".to_string(), "random".to_string()];
    labels[side.index()] = "student".to_string();
    play_labelled(
        shared(),
        &GameSetup::new(decks, seed),
        &mut student,
        &mut teacher(net, seed),
        &mut opponent,
        side,
        labels,
        mix,
    )
    .unwrap()
}

#[test]
fn the_teacher_labels_exactly_the_students_choices() {
    let net = Net::new(&cpu());
    let shared = shared();
    for (seed, side) in [(1, PlayerId(0)), (2, PlayerId(1))] {
        let g = game(&net, seed, side, 0.0);
        assert!(g.record.result.is_some());
        // Replay: the labelled indices are the student's decisions with a choice.
        let mut state = g.record.start(&shared).unwrap();
        let mut expected = Vec::new();
        for (i, a) in g.record.actions.iter().enumerate() {
            let view = PlayerView::new(&state).unwrap();
            if view.player() == side && view.actions().len() > 1 {
                expected.push((i, view.actions().len()));
            }
            GameRecord::step(&mut state, i, a).unwrap();
        }
        assert!(!expected.is_empty());
        let got: Vec<(usize, usize)> = g.targets.iter().map(|(i, t)| (*i, t.len())).collect();
        assert_eq!(got, expected);
        for (_, t) in &g.targets {
            assert!((t.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{t:?}");
        }
        let s = samples(&g, &shared).unwrap();
        assert_eq!(s.len(), g.targets.len());
        for (sample, (_, t)) in s.iter().zip(&g.targets) {
            assert_eq!(&sample.policy, t);
            assert_eq!(sample.encoded.action_count(), t.len());
            assert_eq!(t[sample.action], t.iter().copied().fold(0.0, f32::max));
        }
    }
}

#[test]
fn with_mix_one_the_teacher_moves() {
    let net = Net::new(&cpu());
    let shared = shared();
    let side = PlayerId(0);
    let g = game(&net, 3, side, 1.0);
    // Every labelled decision played what a fresh teacher with the same
    // seed would pick: its most visited move.
    let mut state = g.record.start(&shared).unwrap();
    let mut t = teacher(&net, 3);
    let mut labelled = g.targets.iter().peekable();
    for (i, a) in g.record.actions.iter().enumerate() {
        if labelled.next_if(|(j, _)| *j == i).is_some() {
            let view = PlayerView::new(&state).unwrap();
            assert_eq!(t.choose(&view), a.action, "decision {i}");
        }
        GameRecord::step(&mut state, i, a).unwrap();
    }
}

#[test]
fn training_on_the_labels_moves_the_network_towards_the_teacher() {
    let net = Net::new(&cpu());
    let shared = shared();
    let games: Vec<Labelled> = (10..14)
        .map(|s| game(&net, s, PlayerId((s % 2) as u8), 0.5))
        .collect();
    let opts = Options {
        epochs: 3,
        batch: 32,
        holdout: 0,
        pool: 64,
        learning_rate: 3e-3,
        ..Options::default()
    };
    let mut losses = Vec::new();
    train_on(
        net,
        games.len(),
        &|i| samples(&games[i], &shared),
        &opts,
        &mut |e| losses.push(e.loss),
    )
    .unwrap();
    assert!(losses[1].is_finite() && losses[3] < losses[1], "{losses:?}");
}
