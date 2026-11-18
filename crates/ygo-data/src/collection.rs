use std::collections::{BTreeMap, HashSet};
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_from: Option<String>,
    /// Per-set completion as the game shows it, e.g. `"LOB": "92/126"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totals: Option<BTreeMap<String, String>>,
    pub cards: Vec<OwnedCard>,
}

impl Collection {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        Self::parse(&crate::read_text(path)?, path)
    }

    /// Reads `collection.json` text; `origin` names it in errors.
    pub fn parse(text: &str, origin: &Path) -> Result<Self, DataError> {
        let collection: Self = crate::parse_json(text, origin)?;
        crate::check_schema("collection", collection.schema_version)?;
        Ok(collection)
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
