use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CardId, DataError};

/// One card in a deck list and how many copies of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckEntry {
    pub id: CardId,
    /// For humans only; the loader checks ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub count: usize,
}

/// A deck as stored in `data/decks/*.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub schema_version: u32,
    pub name: String,
    /// Free-form: match results, the style it plays, where it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<serde_json::Value>,
    #[serde(default)]
    pub main: Vec<DeckEntry>,
    #[serde(default)]
    pub extra: Vec<DeckEntry>,
}

impl Deck {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        let deck: Self = crate::read_json(path)?;
        crate::check_schema("deck", deck.schema_version)?;
        Ok(deck)
    }

    pub fn main_size(&self) -> usize {
        self.main.iter().map(|e| e.count).sum()
    }

    pub fn extra_size(&self) -> usize {
        self.extra.iter().map(|e| e.count).sum()
    }

    pub fn in_main(&self, id: CardId) -> bool {
        self.main.iter().any(|e| e.id == id && e.count > 0)
    }

    pub fn in_extra(&self, id: CardId) -> bool {
        self.extra.iter().any(|e| e.id == id && e.count > 0)
    }

    /// Copies of each distinct card across Main and Extra Deck, sorted by id.
    pub fn copies(&self) -> BTreeMap<CardId, usize> {
        let mut counts = BTreeMap::new();
        for e in self.main.iter().chain(&self.extra) {
            *counts.entry(e.id).or_insert(0) += e.count;
        }
        counts
    }
}
