use std::path::PathBuf;

use crate::CardId;

/// Anything that stops a data file from loading or being consistent with the
/// others. Deck legality is not an error; see [`crate::DeckProblem`].
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("cannot read {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in {path}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid TOML in {path}")]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("duplicate card id {0}")]
    DuplicateCardId(CardId),
    #[error("duplicate card name {0:?}")]
    DuplicateCardName(String),
    #[error("monster {name:?} ({id}) is missing level, ATK, DEF or attribute")]
    MonsterMissingStats { id: CardId, name: String },
    #[error("{file} refers to unknown card id {id}")]
    UnknownCard { file: &'static str, id: CardId },
    #[error("{file} lists card id {id} more than once")]
    DuplicateEntry { file: &'static str, id: CardId },
    #[error("ban list limit {limit} for card {id} is not below the default copy limit {default}")]
    LimitNotBelowDefault {
        id: CardId,
        limit: usize,
        default: usize,
    },
    #[error("collection lists card {id} with zero copies")]
    ZeroCopies { id: CardId },
    #[error("{file} has schema_version {found}; this build reads version {expected}")]
    UnsupportedSchema {
        file: &'static str,
        found: u32,
        expected: u32,
    },
    #[error("{file} declares {declared} entries but holds {actual}")]
    CountMismatch {
        file: &'static str,
        declared: usize,
        actual: usize,
    },
    #[error("banlist default_limit {banlist} disagrees with rules default_copy_limit {rules}")]
    DefaultLimitMismatch { banlist: usize, rules: usize },
    #[error("invalid rules: {0}")]
    InvalidRules(String),
    #[error("invalid bot configuration: {0}")]
    InvalidBotConfig(String),
}
