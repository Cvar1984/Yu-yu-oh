use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ygo_data::{CardDb, CardId, CardKind, Rules};

/// Dense index of a card in the [`CardPool`]: what game state refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardIndex(pub u16);

/// The numbers the engine needs about a card. Spells and Traps have zero
/// level, ATK and DEF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardStats {
    pub id: CardId,
    pub name: String,
    pub kind: CardKind,
    pub level: u8,
    pub atk: i32,
    pub def: i32,
}

impl CardStats {
    pub fn is_monster(&self) -> bool {
        !matches!(self.kind, CardKind::Spell | CardKind::Trap)
    }

    /// Lives in the Extra Deck rather than the Main Deck.
    pub fn is_extra_deck(&self) -> bool {
        self.kind == CardKind::Fusion
    }
}

/// Every card the engine may see, in a flat table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardPool {
    cards: Vec<CardStats>,
    by_id: HashMap<CardId, CardIndex>,
}

impl CardPool {
    pub fn from_db(db: &CardDb) -> Self {
        let mut cards: Vec<CardStats> = db
            .iter()
            .map(|c| CardStats {
                id: c.id,
                name: c.name.clone(),
                kind: c.kind(),
                level: c.level.unwrap_or(0),
                atk: c.atk.unwrap_or(0),
                def: c.def.unwrap_or(0),
            })
            .collect();
        cards.sort_by_key(|c| c.id);
        let by_id = cards
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, CardIndex(i as u16)))
            .collect();
        Self { cards, by_id }
    }

    pub fn index_of(&self, id: CardId) -> Option<CardIndex> {
        self.by_id.get(&id).copied()
    }

    pub fn get(&self, index: CardIndex) -> &CardStats {
        &self.cards[index.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

/// Immutable data every game shares: built once, held by `Arc`, so a
/// game copy never clones it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shared {
    pub rules: Rules,
    pub pool: CardPool,
}

impl Shared {
    pub fn new(rules: Rules, db: &CardDb) -> Self {
        Self {
            rules,
            pool: CardPool::from_db(db),
        }
    }
}
