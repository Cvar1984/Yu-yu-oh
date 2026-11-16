use serde::{Deserialize, Serialize};

use crate::{GameResult, InstanceId, Phase, PlayerId};

/// Everything a player can do. `Game::legal_actions` lists the ones
/// allowed right now; `Game::apply` refuses any other.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    /// Send a card from the hand to the Graveyard for the hand limit.
    Discard { card: InstanceId },
    /// Leave the current Main Phase and go to the End Phase.
    EndTurn,
}

/// What kind of choice is pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecisionKind {
    /// Discard down to the hand limit.
    HandLimit,
    /// Any Main Phase action.
    MainPhase,
}

/// Who must choose, and what kind of choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Decision {
    pub player: PlayerId,
    pub kind: DecisionKind,
}

/// What happened as a result of an action or an automatic phase step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    TurnStarted {
        turn: u32,
        player: PlayerId,
    },
    PhaseEntered {
        phase: Phase,
    },
    Drew {
        player: PlayerId,
        card: InstanceId,
    },
    /// A draw was due but the Deck was empty.
    DeckEmpty {
        player: PlayerId,
    },
    Discarded {
        player: PlayerId,
        card: InstanceId,
    },
    GameOver {
        result: GameResult,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("the game is over")]
    GameOver,
    #[error("no decision is pending")]
    NoDecisionPending,
    #[error("action is not legal now: {0:?}")]
    IllegalAction(Action),
}
