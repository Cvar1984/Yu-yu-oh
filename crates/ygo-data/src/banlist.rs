use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CardDb, CardId, DataError};

/// One line of the ban list: how many copies of a card a deck may hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardLimit {
    pub id: CardId,
    /// For humans only; the loader checks ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub limit: usize,
}

/// The owner's game's copy limits. Cards not listed use the default from
/// `rules.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Banlist {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    pub limits: Vec<CardLimit>,
}

impl Banlist {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        crate::read_json(path)
    }

    /// Every id must exist, appear once, and sit below the default limit
    /// (a limit equal to or above the default would be dead data).
    pub fn validate(&self, cards: &CardDb, default_limit: usize) -> Result<(), DataError> {
        let mut seen = HashSet::new();
        for entry in &self.limits {
            if !cards.contains(entry.id) {
                return Err(DataError::UnknownCard {
                    file: "banlist",
                    id: entry.id,
                });
            }
            if !seen.insert(entry.id) {
                return Err(DataError::DuplicateEntry {
                    file: "banlist",
                    id: entry.id,
                });
            }
            if entry.limit >= default_limit {
                return Err(DataError::LimitNotBelowDefault {
                    id: entry.id,
                    limit: entry.limit,
                    default: default_limit,
                });
            }
        }
        Ok(())
    }

    /// Copies allowed for `id`, falling back to `default_limit`.
    pub fn limit_for(&self, id: CardId, default_limit: usize) -> usize {
        self.limits
            .iter()
            .find(|l| l.id == id)
            .map_or(default_limit, |l| l.limit)
    }
}
