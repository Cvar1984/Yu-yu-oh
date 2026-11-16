//! Deterministic rules engine. Same rules, decks, seed and actions always
//! give the same game. No I/O, no threads, no system clock: randomness
//! comes only from the seed in [`GameSetup`].
//!
//! Milestone M1: phases, summons, positions and battle for Normal
//! Monsters. Cards that need an effect are drawn and held but offer no
//! actions until M2 adds chains and effect scripts.
//!
//! Typical use: build one [`Shared`] from the rules and card pool, then
//! `Game::new(shared, &setup)`, and loop over `pending_decision`,
//! `legal_actions`, `observe` and `apply` until `result` is `Some`.

mod action;
mod battle;
mod chain;
mod effect;
mod error;
mod game;
mod observe;
mod ops;
mod pool;
mod position;
mod setup;
mod state;
mod summon;
#[cfg(feature = "test-support")]
pub mod testing;
mod turn;

pub use action::{Action, Decision, DecisionKind, Event, RuleError};
pub use chain::{ChainLink, PendingSummon, SummonKind, TriggerRef, WindowKind};
pub use effect::{
    CardScript, Effect, EffectCtx, EffectKind, Flow, Registry, Source, Speed, StatMod, Tag,
    TargetSets, Window, always, flipped_this_window, nothing,
};
pub use error::SetupError;
pub use game::Game;
pub use observe::{ChainView, Observation, Side, VisibleCard};
pub use pool::{CardIndex, CardPool, CardStats, Shared, SpellTrapType};
pub use setup::{DeckList, GameSetup};
pub use state::{
    BattleStep, CardInstance, DrawReason, GameResult, InstanceId, Lingering, LingeringKind,
    Location, Phase, PlayerId, PlayerState, Position, WinReason,
};
