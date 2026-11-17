//! Loading and validation of everything the engine reads from disk: the card
//! pool, the ban list, the owner's collection, decks, the house rules and
//! the heuristic bots' card values and styles.
//!
//! Every file refers to cards by their id (the official password). Formats
//! are strict: unknown fields are errors, so a changed data bundle fails
//! loudly instead of silently dropping information.

mod banlist;
mod bot;
mod bundle;
mod card;
mod check;
mod collection;
mod deck;
mod error;
mod rules;

pub use banlist::{Banlist, CardLimit};
pub use bot::{CardValues, MAX_CARD_VALUE, Style, Styles, ValueDefaults};
pub use bundle::Bundle;
pub use card::{Card, CardDb, CardFile, CardId, CardKind, CardType, Images, Printing};
pub use check::{DeckProblem, check_deck};
pub use collection::{Collection, OwnedCard};
pub use deck::{Deck, DeckEntry};
pub use error::DataError;
pub use rules::{
    CardRules, DeckRules, FieldRules, HandLimitCheck, HandRules, LifeRules, Rules, SummonRules,
    TextVersion, TurnRules, WinRules,
};

use std::path::Path;

/// The one JSON schema version this build understands.
pub const SCHEMA_VERSION: u32 = 1;

pub(crate) fn check_schema(file: &'static str, found: u32) -> Result<(), DataError> {
    if found == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(DataError::UnsupportedSchema {
            file,
            found,
            expected: SCHEMA_VERSION,
        })
    }
}

/// Reads and deserializes one JSON file, naming the file in any error.
pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| DataError::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads and deserializes one TOML file, naming the file in any error.
pub(crate) fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| DataError::Toml {
        path: path.to_path_buf(),
        source,
    })
}
