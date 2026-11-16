use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CardId, DataError};

/// A deck as stored in `data/decks/*.json`. Copies are repeated ids, as in
/// the `.ydk` format.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub name: String,
    #[serde(default)]
    pub main: Vec<CardId>,
    #[serde(default)]
    pub extra: Vec<CardId>,
    /// Free-form: match results, the style it plays, where it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<serde_json::Value>,
}

impl Deck {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        crate::read_json(path)
    }

    /// Copies of each distinct card across Main and Extra Deck, sorted by id.
    pub fn copies(&self) -> BTreeMap<CardId, usize> {
        let mut counts = BTreeMap::new();
        for id in self.main.iter().chain(&self.extra) {
            *counts.entry(*id).or_insert(0) += 1;
        }
        counts
    }
}
