//! Deterministic rules engine. Same rules, decks, seed and actions always
//! give the same game. No I/O, no threads, no system clock: randomness
//! comes only from the seed in [`GameSetup`].
//!
//! Milestone M1: phases, summons, positions and battle for Normal
//! Monsters. Cards that need an effect are drawn and held but offer no
//! actions until M2 adds chains and effect scripts.

mod action;
mod error;
mod game;
mod pool;
mod position;
mod setup;
mod state;
mod summon;
mod turn;

pub use action::{Action, Decision, DecisionKind, Event, RuleError};
pub use error::SetupError;
pub use game::Game;
pub use pool::{CardIndex, CardPool, CardStats, Shared};
pub use setup::{DeckList, GameSetup};
pub use state::{
    BattleStep, CardInstance, DrawReason, GameResult, InstanceId, Location, Phase, PlayerId,
    PlayerState, Position, WinReason,
};
