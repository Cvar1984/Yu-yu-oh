//! Players that answer the engine's decisions, and the loop that plays a
//! recorded game between two of them. The uniform random player is the
//! floor of the evaluation ladder and the engine's fuzzing driver.

pub mod encode;
mod eval;
mod heuristic;
mod search;

pub use eval::Worth;

pub use heuristic::HeuristicPlayer;
pub use search::{SearchPlayer, SearchSettings};

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use std::sync::Arc;

use ygo_engine::{Action, Game, GameRecord, GameResult, GameSetup, PlayerView, SetupError, Shared};

/// Anything that can answer a decision with one of the legal actions.
pub trait Player {
    /// Returns one of `view.actions()`. The view holds this player's
    /// observation; search bots also play forward in its samples.
    fn choose(&mut self, view: &PlayerView) -> Action;
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
    fn choose(&mut self, view: &PlayerView) -> Action {
        let actions = view.actions();
        let i = (self.rng.next_u64() % actions.len() as u64) as usize;
        actions[i].clone()
    }
}

/// Plays `game` to its end with `players[0]` and `players[1]` answering
/// for player 0 and player 1, noting every action in `record`.
pub fn play_out(
    game: &mut Game,
    players: &mut [&mut dyn Player; 2],
    record: &mut GameRecord,
) -> GameResult {
    while let Some(view) = PlayerView::new(game) {
        let action = players[view.player().index()].choose(&view);
        record.note(game, &action);
        game.apply(&action)
            .expect("players choose only among legal actions");
    }
    record.finish(game);
    game.result().expect("loop ends when the game is over")
}

/// Starts a game from `setup` and plays it to its end, returning the
/// final game and its record. `labels` name the two players in the record.
pub fn play_game(
    shared: Arc<Shared>,
    setup: &GameSetup,
    players: &mut [&mut dyn Player; 2],
    labels: [String; 2],
) -> Result<(Game, GameRecord), SetupError> {
    let mut record = GameRecord::new(&shared.rules, setup, labels);
    let mut game = Game::new(shared, setup)?;
    play_out(&mut game, players, &mut record);
    Ok((game, record))
}
