//! What one player is allowed to see.

use serde::{Deserialize, Serialize};
use ygo_data::CardId;

use crate::{
    Decision, Game, InstanceId, Lingering, Location, PendingSummon, Phase, PlayerId, PlayerState,
    Position, WindowKind,
};

/// A card as seen from one side of the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisibleCard {
    Known {
        instance: InstanceId,
        card: CardId,
        position: Position,
        /// Current ATK and DEF for a monster on the field.
        stats: Option<(i32, i32)>,
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

    pub fn position(&self) -> Position {
        match self {
            VisibleCard::Known { position, .. } | VisibleCard::Hidden { position, .. } => *position,
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
    /// The Extra Deck: known to its owner, hidden from the opponent.
    pub extra: Vec<VisibleCard>,
    pub graveyard: Vec<VisibleCard>,
    pub banished: Vec<VisibleCard>,
    pub monster_zones: Vec<Option<VisibleCard>>,
    pub spell_trap_zones: Vec<Option<VisibleCard>>,
    pub field_zone: Option<VisibleCard>,
}

/// A chain link as everyone sees it: activations are public.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainView {
    pub instance: InstanceId,
    pub card: CardId,
    pub controller: PlayerId,
    pub effect: u8,
    pub targets: Vec<InstanceId>,
    pub negated: bool,
}

/// Where a card sits, as the viewer sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Hand,
    Monster(usize),
    SpellTrap(usize),
    Field,
    Graveyard,
    Banished,
    Extra,
    /// Shown for a choice (a Deck search, a revealed hand).
    Revealed,
    Chain,
}

/// A card found in the observation: how it is seen, and where.
#[derive(Debug, Clone, Copy)]
pub struct Found<'a> {
    pub card: &'a VisibleCard,
    pub owner: Option<PlayerId>,
    pub place: Place,
}

impl Observation {
    /// Finds instance `id` in what the viewer can see.
    pub fn find(&self, id: InstanceId) -> Option<Found<'_>> {
        let obs = self;
        for p in PlayerId::BOTH {
            let side = &obs.sides[p.index()];
            let zones: [(&[VisibleCard], Place); 4] = [
                (&side.hand, Place::Hand),
                (&side.graveyard, Place::Graveyard),
                (&side.banished, Place::Banished),
                (&side.extra, Place::Extra),
            ];
            for (cards, place) in zones.iter() {
                if let Some(card) = cards.iter().find(|c| c.instance() == id) {
                    return Some(Found {
                        card,
                        owner: Some(p),
                        place: *place,
                    });
                }
            }
            for (i, z) in side.monster_zones.iter().enumerate() {
                if let Some(card) = z.as_ref().filter(|c| c.instance() == id) {
                    return Some(Found {
                        card,
                        owner: Some(p),
                        place: Place::Monster(i),
                    });
                }
            }
            for (i, z) in side.spell_trap_zones.iter().enumerate() {
                if let Some(card) = z.as_ref().filter(|c| c.instance() == id) {
                    return Some(Found {
                        card,
                        owner: Some(p),
                        place: Place::SpellTrap(i),
                    });
                }
            }
            if let Some(card) = side.field_zone.as_ref().filter(|c| c.instance() == id) {
                return Some(Found {
                    card,
                    owner: Some(p),
                    place: Place::Field,
                });
            }
        }
        obs.revealed
            .iter()
            .find(|c| c.instance() == id)
            .map(|card| Found {
                card,
                owner: None,
                place: Place::Revealed,
            })
    }
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
    /// First link first.
    pub chain: Vec<ChainView>,
    /// The open response window and who holds priority, if any.
    pub window: Option<(WindowKind, PlayerId)>,
    pub pending_summon: Option<PendingSummon>,
    /// The options of a pending `Choose` decision when the viewer is the
    /// one choosing, with their identities revealed; empty otherwise.
    pub revealed: Vec<VisibleCard>,
    /// The labelled options of a pending `Ask` decision for the viewer.
    pub question: Vec<String>,
    pub lingering: Vec<Lingering>,
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
        let modifiers = self.modifiers();
        let hand_revealed = [
            self.hand_revealed(PlayerId(0)),
            self.hand_revealed(PlayerId(1)),
        ];
        // A monster being Normal, Tribute or Flip Summoned face-up is public
        // from its declaration, while the opponent decides on a response.
        let declared = self
            .pending_summon()
            .filter(|p| !p.face_down)
            .map(|p| p.card);
        let side = |player: PlayerId| {
            let state: &PlayerState = &self.players[player.index()];
            let own = player == viewer;
            let see = |id: InstanceId, in_hand: bool| {
                let inst = &self.instances[id.index()];
                let revealed = match inst.location {
                    Location::Hand => own || hand_revealed[player.index()] || declared == Some(id),
                    Location::Graveyard => true,
                    Location::Banished => inst.position.is_face_up() || own,
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
                        stats: matches!(inst.location, Location::MonsterZone(_))
                            .then(|| self.stats_with(&modifiers, id)),
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
                extra: state.extra.iter().map(|&id| see(id, false)).collect(),
                graveyard: state.graveyard.iter().map(|&id| see(id, false)).collect(),
                banished: state.banished.iter().map(|&id| see(id, false)).collect(),
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
                field_zone: state.field_zone.map(|id| see(id, false)),
            }
        };
        let chain = self
            .chain
            .iter()
            .map(|l| ChainView {
                instance: l.card,
                card: self.shared.pool.get(self.instances[l.card.index()].card).id,
                controller: l.controller,
                effect: l.effect,
                targets: l.targets.clone(),
                negated: l.negated,
            })
            .collect();
        let (revealed, question) = match &self.pending_choice {
            Some(pc) if pc.decider() == viewer => (
                pc.card_options()
                    .iter()
                    .map(|&id| VisibleCard::Known {
                        instance: id,
                        card: self.shared.pool.get(self.instances[id.index()].card).id,
                        position: self.instances[id.index()].position,
                        stats: None,
                    })
                    .collect(),
                pc.question().iter().map(|q| q.to_string()).collect(),
            ),
            _ => (Vec::new(), Vec::new()),
        };
        Observation {
            viewer,
            turn: self.turn,
            turn_player: self.turn_player,
            phase: self.phase,
            decision: self.pending_decision(),
            sides: [side(PlayerId(0)), side(PlayerId(1))],
            chain,
            window: self.window().map(|(k, _)| {
                let priority = match self.pending.last() {
                    Some(crate::chain::Pending::Window { priority, .. }) => *priority,
                    _ => unreachable!(),
                };
                (k, priority)
            }),
            pending_summon: self.pending_summon(),
            revealed,
            question,
            lingering: self.lingering.clone(),
        }
    }
}
