//! The neural network and bot (`ygo_ai::neural`, M6 step 3): an untrained
//! bot plays whole games with legal actions, a padded batch gives what one
//! decision at a time gives, checkpoints round-trip, and training lowers
//! the loss.
#![cfg(feature = "neural")]

use burn::nn::loss::CrossEntropyLossConfig;
use burn::optim::{AdamConfig, GradientsParams};
use burn::prelude::*;
use ygo_ai::encode::{Encoded, encode};
use ygo_ai::neural::{Inputs, Net, NeuralPlayer, cpu};
use ygo_ai::{Player, RandomPlayer};
use ygo_engine::testing::{sample_deck, shared_from};
use ygo_engine::{Game, GameSetup, PlayerView};

fn new_game(seed: u64) -> Game {
    let decks = [
        sample_deck("enemy_e3_removal.json"),
        sample_deck("enemy_e4_big_beatdown.json"),
    ];
    Game::new(
        shared_from(ygo_cards::registry()),
        &GameSetup::new(decks, seed),
    )
    .unwrap()
}

/// The first `n` decisions of a random game, encoded.
fn decisions(seed: u64, n: usize) -> Vec<Encoded> {
    let mut g = new_game(seed);
    let mut bot = RandomPlayer::new(seed);
    let mut out = Vec::new();
    while let Some(view) = PlayerView::new(&g) {
        if out.len() == n {
            break;
        }
        out.push(encode(&view));
        let a = bot.choose(&view);
        g.apply(&a).unwrap();
    }
    out
}

fn floats<const D: usize>(t: Tensor<D>) -> Vec<f32> {
    t.into_data().try_to_vec::<f32>().unwrap()
}

#[test]
fn an_untrained_bot_plays_whole_games() {
    let net = Net::new(&cpu());
    for seed in 0..3 {
        let mut g = new_game(seed);
        let mut players: [Box<dyn Player>; 2] = [
            Box::new(NeuralPlayer::new(net.clone(), cpu(), 1.0, seed)),
            Box::new(RandomPlayer::new(seed)),
        ];
        let mut decisions = 0;
        while let Some(view) = PlayerView::new(&g) {
            let a = players[view.player().index()].choose(&view);
            g.apply(&a).expect("the neural bot picks a legal action");
            decisions += 1;
        }
        assert!(g.result().is_some() && decisions > 20);
    }
}

#[test]
fn a_padded_batch_gives_what_one_decision_at_a_time_gives() {
    let device = cpu();
    let net = Net::new(&device);
    // Decisions of different sizes: cards, actions and lingering effects.
    let batch: Vec<Encoded> = decisions(5, 60).into_iter().step_by(7).collect();
    assert!(batch.len() >= 8);
    let (logits, values) = net.forward(&Inputs::new(&batch, &device));
    let width = logits.dims()[1];
    let (logits, values) = (floats(logits), floats(values));
    for (i, e) in batch.iter().enumerate() {
        let (one, value) = net.forward(&Inputs::new(std::slice::from_ref(e), &device));
        let one = floats(one);
        for (j, x) in one.iter().enumerate().take(e.action_count()) {
            let y = logits[i * width + j];
            assert!((x - y).abs() < 1e-4, "decision {i} action {j}: {x} vs {y}");
        }
        let v = floats(value)[0];
        assert!(
            (v - values[i]).abs() < 1e-4,
            "decision {i} value: {v} vs {}",
            values[i]
        );
    }
}

#[test]
fn a_checkpoint_round_trips() {
    let device = cpu();
    let net = Net::new(&device);
    let path = std::env::temp_dir().join(format!("ygo-neural-{}.bpk", std::process::id()));
    net.save(&path).unwrap();
    let loaded = Net::load(&path, &device).unwrap();
    std::fs::remove_file(&path).ok();
    let batch = decisions(9, 12);
    let x = Inputs::new(&batch, &device);
    let (a, b) = (net.forward(&x), loaded.forward(&x));
    assert_eq!(floats(a.0), floats(b.0));
    assert_eq!(floats(a.1), floats(b.1));
    assert!(Net::load(std::path::Path::new("/no/such/checkpoint.bpk"), &device).is_err());
}

#[test]
fn training_lowers_the_loss() {
    let device = cpu().autodiff();
    let mut net = Net::new(&device);
    let batch = decisions(11, 32);
    let x = Inputs::new(&batch, &device);
    // A fixed target: always the last legal action, and a win.
    let targets: Vec<i64> = batch.iter().map(|e| e.action_count() as i64 - 1).collect();
    let n = batch.len();
    let target = Tensor::<1, Int>::from_data(TensorData::new(targets, vec![n]), &device);
    let wins = Tensor::<1>::ones([n], &device);
    let ce = CrossEntropyLossConfig::new().init(&device);
    let loss = |net: &Net| {
        let (logits, value) = net.forward(&x);
        ce.forward(logits, target.clone()) + (value - wins.clone()).powf_scalar(2.0).mean()
    };
    let before = loss(&net).into_scalar::<f32>();
    let mut opt = AdamConfig::new().init();
    for _ in 0..30 {
        let grads = GradientsParams::from_grads(loss(&net).backward(), &net);
        net = opt.step(1e-3, net, grads);
    }
    let after = loss(&net).into_scalar::<f32>();
    assert!(after < before * 0.5, "loss {before} -> {after}");
}

#[test]
fn the_network_search_is_repeatable_legal_and_counts_its_visits() {
    use ygo_ai::mcts::{MctsPlayer, MctsSettings};
    let net = Net::new(&cpu());
    let settings = MctsSettings {
        iterations: 12,
        ..MctsSettings::default()
    };
    let search = |seed| {
        MctsPlayer::new(
            NeuralPlayer::new(net.clone(), cpu(), 0.0, seed),
            settings.clone(),
            seed,
        )
    };
    // A decision with a choice.
    let mut g = new_game(6);
    let mut random = RandomPlayer::new(6);
    let view = loop {
        let view = PlayerView::new(&g).unwrap();
        if view.actions().len() > 2 {
            break view;
        }
        let a = random.choose(&view);
        g.apply(&a).unwrap();
    };
    let a = search(1).analyse(&view);
    assert_eq!(
        a,
        search(1).analyse(&view),
        "the same seed gives the same analysis"
    );
    assert_eq!(a.len(), view.actions().len());
    // The first iteration judges the root; each later one visits one move.
    assert_eq!(a.iter().map(|x| x.1).sum::<u32>(), 11);
    // Whole games with legal actions.
    let mut g = new_game(7);
    let mut players: [Box<dyn Player>; 2] = [Box::new(search(7)), Box::new(RandomPlayer::new(7))];
    while let Some(view) = PlayerView::new(&g) {
        let a = players[view.player().index()].choose(&view);
        g.apply(&a).expect("the search picks a legal action");
    }
    assert!(g.result().is_some());
}
