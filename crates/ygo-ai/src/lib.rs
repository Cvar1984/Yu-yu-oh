//! Players that answer the engine's decisions. M1 ships only the uniform
//! random player, the floor of the evaluation ladder and the engine's
//! fuzzing driver. Heuristic bots, search and learned policies follow in
//! later milestones.

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_engine::{Action, Game, GameResult, Observation};

/// Anything that can answer a decision with one of the legal actions.
pub trait Player {
    /// `actions` is never empty. The observation is from this player's
    /// point of view.
    fn choose(&mut self, observation: &Observation, actions: &[Action]) -> Action;
}

/// Picks uniformly among the legal actions.
#[derive(Debug, Clone)]
pub struct RandomPlayer {
    rng: ChaCha8Rng,
}

impl RandomPlayer {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl Player for RandomPlayer {
    fn choose(&mut self, _observation: &Observation, actions: &[Action]) -> Action {
        let i = (self.rng.next_u64() % actions.len() as u64) as usize;
        actions[i].clone()
    }
}

/// Plays `game` to its end with `players[0]` and `players[1]` answering
/// for player 0 and player 1. Returns the result and the actions played.
pub fn play_out(game: &mut Game, players: &mut [&mut dyn Player; 2]) -> (GameResult, Vec<Action>) {
    let mut record = Vec::new();
    while game.result().is_none() {
        let decision = game
            .pending_decision()
            .expect("a running game always has a decision");
        let actions = game.legal_actions();
        let observation = game.observe(decision.player);
        let action = players[decision.player.index()].choose(&observation, &actions);
        game.apply(&action)
            .expect("players choose only among legal actions");
        record.push(action);
    }
    (
        game.result().expect("loop ends when the game is over"),
        record,
    )
}
