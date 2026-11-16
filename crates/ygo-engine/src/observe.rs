//! What one player is allowed to see.

use serde::{Deserialize, Serialize};
use ygo_data::CardId;

use crate::{Decision, Game, InstanceId, Location, Phase, PlayerId, PlayerState, Position};

/// A card as seen from one side of the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisibleCard {
    Known {
        instance: InstanceId,
        card: CardId,
        position: Position,
    },
    /// Face-down, or in the other player's hand: the instance can still be
    /// targeted, its identity cannot be read.
    Hidden {
        instance: InstanceId,
        position: Position,
    },
}

impl VisibleCard {
    pub fn instance(&self) -> InstanceId {
        match self {
            VisibleCard::Known { instance, .. } | VisibleCard::Hidden { instance, .. } => *instance,
        }
    }

    pub fn card(&self) -> Option<CardId> {
        match self {
            VisibleCard::Known { card, .. } => Some(*card),
            VisibleCard::Hidden { .. } => None,
        }
    }
}

/// One player's side as the viewer sees it. Deck and Extra Deck contents
/// are never visible, only their sizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Side {
    pub lp: u32,
    pub hand: Vec<VisibleCard>,
    pub deck_count: usize,
    pub extra_count: usize,
    pub graveyard: Vec<VisibleCard>,
    pub monster_zones: Vec<Option<VisibleCard>>,
    pub spell_trap_zones: Vec<Option<VisibleCard>>,
}

/// The game from `viewer`'s point of view. This, not the `Game`, is what
/// bots and the browser get.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub viewer: PlayerId,
    pub turn: u32,
    pub turn_player: PlayerId,
    pub phase: Phase,
    pub decision: Option<Decision>,
    /// Indexed by `PlayerId`.
    pub sides: [Side; 2],
}

impl Observation {
    pub fn me(&self) -> &Side {
        &self.sides[self.viewer.index()]
    }

    pub fn opponent(&self) -> &Side {
        &self.sides[self.viewer.opponent().index()]
    }
}

impl Game {
    /// The state with everything `viewer` may not know removed: the
    /// opponent's hand and every face-down card are `Hidden`.
    pub fn observe(&self, viewer: PlayerId) -> Observation {
        let side = |player: PlayerId| {
            let state: &PlayerState = &self.players[player.index()];
            let own = player == viewer;
            let see = |id: InstanceId, in_hand: bool| {
                let inst = &self.instances[id.index()];
                let revealed = match inst.location {
                    Location::Hand => own,
                    Location::Graveyard => true,
                    _ => inst.position.is_face_up() || own,
                };
                let position = if in_hand {
                    Position::FaceDown
                } else {
                    inst.position
                };
                if revealed {
                    VisibleCard::Known {
                        instance: id,
                        card: self.shared.pool.get(inst.card).id,
                        position,
                    }
                } else {
                    VisibleCard::Hidden {
                        instance: id,
                        position,
                    }
                }
            };
            Side {
                lp: state.lp,
                hand: state.hand.iter().map(|&id| see(id, true)).collect(),
                deck_count: state.deck.len(),
                extra_count: state.extra.len(),
                graveyard: state.graveyard.iter().map(|&id| see(id, false)).collect(),
                monster_zones: state
                    .monster_zones
                    .iter()
                    .map(|z| z.map(|id| see(id, false)))
                    .collect(),
                spell_trap_zones: state
                    .spell_trap_zones
                    .iter()
                    .map(|z| z.map(|id| see(id, false)))
                    .collect(),
            }
        };
        Observation {
            viewer,
            turn: self.turn,
            turn_player: self.turn_player,
            phase: self.phase,
            decision: self.pending_decision(),
            sides: [side(PlayerId(0)), side(PlayerId(1))],
        }
    }
}
