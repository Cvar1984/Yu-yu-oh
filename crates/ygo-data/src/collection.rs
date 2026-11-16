use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CardDb, CardId, DataError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedCard {
    pub id: CardId,
    /// For humans only; the loader checks ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub copies: usize,
}

/// The cards a player owns. Deck building is restricted to these.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub cards: Vec<OwnedCard>,
}

impl Collection {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        crate::read_json(path)
    }

    pub fn validate(&self, cards: &CardDb) -> Result<(), DataError> {
        let mut seen = HashSet::new();
        for owned in &self.cards {
            if !cards.contains(owned.id) {
                return Err(DataError::UnknownCard {
                    file: "collection",
                    id: owned.id,
                });
            }
            if !seen.insert(owned.id) {
                return Err(DataError::DuplicateEntry {
                    file: "collection",
                    id: owned.id,
                });
            }
            if owned.copies == 0 {
                return Err(DataError::ZeroCopies { id: owned.id });
            }
        }
        Ok(())
    }

    /// Copies owned of `id`; zero when the card is not in the collection.
    pub fn owned(&self, id: CardId) -> usize {
        self.cards
            .iter()
            .find(|c| c.id == id)
            .map_or(0, |c| c.copies)
    }

    pub fn unique_cards(&self) -> usize {
        self.cards.len()
    }

    pub fn total_copies(&self) -> usize {
        self.cards.iter().map(|c| c.copies).sum()
    }
}
