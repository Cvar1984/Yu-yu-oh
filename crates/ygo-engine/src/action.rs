use serde::{Deserialize, Serialize};

use crate::{GameResult, InstanceId, Location, Phase, PlayerId, Position};

/// Everything a player can do. `Game::legal_actions` lists the ones
/// allowed right now; `Game::apply` refuses any other.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    /// Send a card from the hand to the Graveyard for the hand limit.
    Discard { card: InstanceId },
    /// Normal Summon (face-up Attack) or Set (face-down Defense) a monster
    /// from the hand, tributing the listed monsters first.
    Summon {
        card: InstanceId,
        tributes: Vec<InstanceId>,
        face_down: bool,
    },
    /// Turn a face-down Defense Position monster to face-up Attack.
    FlipSummon { card: InstanceId },
    /// Switch a face-up monster between Attack and Defense Position.
    ChangePosition { card: InstanceId },
    /// Place a Spell or Trap from the hand face-down in a Spell/Trap zone.
    SetSpellTrap { card: InstanceId },
    /// Activate effect number `effect` of `card`, with the targets and
    /// the cost cards chosen now.
    Activate {
        card: InstanceId,
        effect: u8,
        targets: Vec<InstanceId>,
        cost: Vec<InstanceId>,
    },
    /// Answer a `Choose` decision during an effect's resolution.
    Choose { card: InstanceId },
    /// Answer an `Ask` question during an effect's resolution.
    Answer { option: u8 },
    /// Decline to respond in a window.
    Pass,
    /// Main Phase 1 to the Battle Phase.
    EnterBattlePhase,
    /// Declare an attack; `None` attacks the opponent directly.
    Attack {
        attacker: InstanceId,
        target: Option<InstanceId>,
    },
    /// Battle Phase to Main Phase 2.
    EndBattlePhase,
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
    /// Attack or leave the Battle Phase.
    BattlePhase,
    /// Pick one card for a resolving effect.
    Choose,
    /// Activate a fast effect in a response window, or pass.
    Respond,
    /// Order (and for optional ones, choose) the triggers that fired.
    Triggers,
    /// The attack target left the field: attack another monster, or stop.
    Replay,
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
    Tributed {
        player: PlayerId,
        card: InstanceId,
    },
    Summoned {
        player: PlayerId,
        card: InstanceId,
        position: Position,
    },
    SentToGraveyard {
        card: InstanceId,
        from: Location,
    },
    FlipSummoned {
        player: PlayerId,
        card: InstanceId,
    },
    PositionChanged {
        card: InstanceId,
        position: Position,
    },
    AttackDeclared {
        attacker: InstanceId,
        target: Option<InstanceId>,
    },
    /// A face-down defender was turned face up by the attack.
    Flipped {
        card: InstanceId,
    },
    DestroyedByBattle {
        card: InstanceId,
    },
    DamageTaken {
        player: PlayerId,
        amount: u32,
        lp: u32,
    },
    SetCard {
        player: PlayerId,
        card: InstanceId,
    },
    Activated {
        card: InstanceId,
        controller: PlayerId,
        effect: u8,
    },
    Resolved {
        card: InstanceId,
        effect: u8,
    },
    DestroyedByEffect {
        card: InstanceId,
        source: InstanceId,
    },
    SpecialSummoned {
        player: PlayerId,
        card: InstanceId,
        position: Position,
    },
    ControlChanged {
        card: InstanceId,
        to: PlayerId,
    },
    LifePointsGained {
        player: PlayerId,
        amount: u32,
        lp: u32,
    },
    LifePointsPaid {
        player: PlayerId,
        amount: u32,
        lp: u32,
    },
    AddedToHand {
        player: PlayerId,
        card: InstanceId,
        from: Location,
    },
    SummonDeclared {
        player: PlayerId,
        card: InstanceId,
        face_down: bool,
    },
    SummonNegated {
        card: InstanceId,
        by: InstanceId,
    },
    ActivationNegated {
        card: InstanceId,
        by: InstanceId,
    },
    AttackCancelled {
        attacker: InstanceId,
    },
    BattleDamagePrevented {
        player: PlayerId,
    },
    DeckShuffled {
        player: PlayerId,
    },
    Equipped {
        card: InstanceId,
        to: InstanceId,
    },
    PlacedOnTopOfDeck {
        player: PlayerId,
        card: InstanceId,
    },
    Banished {
        card: InstanceId,
        from: Location,
    },
    CoinTossed {
        player: PlayerId,
        heads: bool,
    },
    FusionSummoned {
        player: PlayerId,
        card: InstanceId,
        materials: Vec<InstanceId>,
    },
    RitualSummoned {
        player: PlayerId,
        card: InstanceId,
        tributes: Vec<InstanceId>,
    },
    AttackReplay {
        attacker: InstanceId,
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
