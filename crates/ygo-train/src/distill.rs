//! Distillation with the student's own games (DAgger, Ross, Gordon and
//! Bagnell, 2011): the network (the student) plays, and at each of its
//! decisions with a choice the network-guided search (the teacher) says
//! how it would split its visits; the network then learns those shares.
//! Learning on the positions the student itself reaches fixes the mistakes
//! that plain imitation leaves, where the student drifts into positions
//! its teacher never showed it. With probability `mix` the teacher's move
//! is played instead, so early students do not wander too far.

use std::sync::Arc;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_ai::encode::encode;
use ygo_ai::{Player, SearchPlayer};
use ygo_engine::{
    GameRecord, GameResult, GameSetup, PlayerId, PlayerView, RecordError, SetupError, Shared,
};

use crate::imitation::Sample;

/// A game with the teacher's targets for the student's decisions: the
/// index of the action in the record and the visit shares over the legal
/// actions there.
#[derive(Debug, Clone)]
pub struct Labelled {
    pub record: GameRecord,
    pub student: PlayerId,
    pub targets: Vec<(usize, Vec<f32>)>,
}

/// Plays one game, `student` on side `side` against `opponent`, labelling
/// each student decision with more than one action by `teacher`.
#[allow(clippy::too_many_arguments)]
pub fn play_labelled(
    shared: Arc<Shared>,
    setup: &GameSetup,
    student: &mut dyn Player,
    teacher: &mut SearchPlayer,
    opponent: &mut dyn Player,
    side: PlayerId,
    labels: [String; 2],
    mix: f64,
) -> Result<Labelled, SetupError> {
    let mut rng = ChaCha8Rng::seed_from_u64(setup.seed ^ 0xD1);
    let mut record = GameRecord::new(&shared.rules, setup, labels);
    let mut game = ygo_engine::Game::new(shared, setup)?;
    let mut targets = Vec::new();
    while let Some(view) = PlayerView::new(&game) {
        let action = if view.player() != side {
            opponent.choose(&view)
        } else if view.actions().len() == 1 {
            view.actions()[0].clone()
        } else {
            let analysis = teacher.analyse(&view);
            let total: u32 = analysis.iter().map(|a| a.1).sum();
            let shares: Vec<f32> = analysis
                .iter()
                .map(|a| a.1 as f32 / total.max(1) as f32)
                .collect();
            targets.push((record.actions.len(), shares));
            let pick = (rng.next_u64() as f64 / u64::MAX as f64) < mix;
            if pick {
                analysis
                    .iter()
                    .max_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)))
                    .expect("actions are never empty")
                    .0
                    .clone()
            } else {
                student.choose(&view)
            }
        };
        record.note(&game, &action);
        game.apply(&action)
            .expect("players choose only among legal actions");
    }
    record.finish(&game);
    Ok(Labelled {
        record,
        student: side,
        targets,
    })
}

/// The training samples of a labelled game: each labelled decision,
/// encoded, with the visit shares as the target and the game's result for
/// the student as the value.
pub fn samples(game: &Labelled, shared: &Shared) -> Result<Vec<Sample>, RecordError> {
    let value = match game.record.result {
        Some(GameResult::Win { winner, .. }) if winner == game.student => 1.0,
        Some(GameResult::Win { .. }) => -1.0,
        _ => 0.0,
    };
    let mut state = game.record.start(shared)?;
    let mut out = Vec::with_capacity(game.targets.len());
    let mut next = game.targets.iter().peekable();
    for (index, a) in game.record.actions.iter().enumerate() {
        if let Some((_, shares)) = next.next_if(|(i, _)| *i == index) {
            let view = PlayerView::new(&state).expect("a recorded action has a decision");
            let action = (0..shares.len())
                .max_by(|&x, &y| shares[x].total_cmp(&shares[y]))
                .expect("a labelled decision has actions");
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
