//! Self-play (M6 step 5, AlphaZero): the network-guided search
//! (`ygo_ai::mcts`) plays both sides; at each decision with a choice the
//! search's visit shares become the policy target, and the game's result
//! for the decider the value target. In the first `explore_turns` turns a
//! move is drawn in proportion to its visits, so games differ; after that
//! the most visited move is played.

use std::sync::Arc;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_ai::encode::encode;
use ygo_ai::mcts::{MctsPlayer, MctsSettings};
use ygo_ai::neural::{Net, NeuralPlayer};
use ygo_engine::{GameRecord, GameResult, GameSetup, PlayerView, RecordError, SetupError, Shared};

use crate::imitation::Sample;

/// A self-play game with the search's visit shares at each decision with
/// a choice: the index of the action in the record and the shares over
/// the legal actions there.
#[derive(Debug, Clone)]
pub struct SelfPlayGame {
    pub record: GameRecord,
    pub targets: Vec<(usize, Vec<f32>)>,
}

/// Plays one game of `net`'s search against itself.
pub fn play(
    shared: Arc<Shared>,
    setup: &GameSetup,
    net: &Net,
    settings: &MctsSettings,
    explore_turns: u32,
) -> Result<SelfPlayGame, SetupError> {
    let device = ygo_ai::neural::cpu();
    let mut rng = ChaCha8Rng::seed_from_u64(setup.seed ^ 0x5E1F);
    let mut players = [0x5A, 0x5B].map(|salt| {
        let judge = NeuralPlayer::new(net.clone(), device.clone(), 0.0, setup.seed ^ salt);
        MctsPlayer::new(judge, settings.clone(), setup.seed ^ salt)
    });
    let labels = ["self-play".to_string(), "self-play".to_string()];
    let mut record = GameRecord::new(&shared.rules, setup, labels);
    let mut game = ygo_engine::Game::new(shared, setup)?;
    let mut targets = Vec::new();
    while let Some(view) = PlayerView::new(&game) {
        let action = if view.actions().len() == 1 {
            view.actions()[0].clone()
        } else {
            let analysis = players[view.player().index()].analyse(&view);
            let total: u32 = analysis.iter().map(|a| a.1).sum();
            let shares: Vec<f32> = analysis
                .iter()
                .map(|a| a.1 as f32 / total.max(1) as f32)
                .collect();
            let pick = if game.turn() <= explore_turns && total > 0 {
                // A move drawn in proportion to its visits.
                let mut left = rng.next_u64() % u64::from(total);
                let mut chosen = analysis.len() - 1;
                for (i, a) in analysis.iter().enumerate() {
                    if left < u64::from(a.1) {
                        chosen = i;
                        break;
                    }
                    left -= u64::from(a.1);
                }
                chosen
            } else {
                (0..analysis.len())
                    .max_by(|&x, &y| {
                        analysis[x]
                            .1
                            .cmp(&analysis[y].1)
                            .then(analysis[x].2.total_cmp(&analysis[y].2))
                    })
                    .expect("a choice has actions")
            };
            targets.push((record.actions.len(), shares));
            analysis[pick].0.clone()
        };
        record.note(&game, &action);
        game.apply(&action)
            .expect("the search chooses only among legal actions");
    }
    record.finish(&game);
    Ok(SelfPlayGame { record, targets })
}

/// The training samples of a self-play game: each decision with a choice,
/// encoded, with the visit shares as the policy target and the result for
/// its decider as the value target.
pub fn samples(game: &SelfPlayGame, shared: &Shared) -> Result<Vec<Sample>, RecordError> {
    let mut state = game.record.start(shared)?;
    let mut out = Vec::with_capacity(game.targets.len());
    let mut next = game.targets.iter().peekable();
    for (index, a) in game.record.actions.iter().enumerate() {
        if let Some((_, shares)) = next.next_if(|(i, _)| *i == index) {
            let view = PlayerView::new(&state).expect("a recorded action has a decision");
            let value = match game.record.result {
                Some(GameResult::Win { winner, .. }) if winner == view.player() => 1.0,
                Some(GameResult::Win { .. }) => -1.0,
                _ => 0.0,
            };
            let action = (0..shares.len())
                .max_by(|&x, &y| shares[x].total_cmp(&shares[y]))
                .expect("a decision with a choice has actions");
            out.push(Sample {
                encoded: encode(&view),
                action,
                policy: shares.clone(),
                value,
            });
        }
        GameRecord::step(&mut state, index, a)?;
    }
    Ok(out)
}
