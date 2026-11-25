//! `tokens.json`: the Tokens that cards create (Scapegoat's Sheep Token).
//! A Token is a monster outside the card pool: it can never be put in a
//! deck or a collection, and it exists only while it is on the field.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{CardDb, CardId, DataError};

/// One Token, with the stats the text of the card that makes it gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Token {
    /// The Token card's own password.
    pub id: CardId,
    pub name: String,
    /// The card whose effect creates it.
    pub made_by: CardId,
    pub race: String,
    pub attribute: String,
    pub level: u8,
    pub atk: i32,
    pub def: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenFile {
    pub schema_version: u32,
    pub source: String,
    pub count: usize,
    pub note: String,
    pub tokens: Vec<Token>,
}

impl TokenFile {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        Self::parse(&crate::read_text(path)?, path)
    }

    /// Reads `tokens.json` text; `origin` names it in errors.
    pub fn parse(text: &str, origin: &Path) -> Result<Self, DataError> {
        let file: TokenFile = crate::parse_json(text, origin)?;
        crate::check_schema("tokens", file.schema_version)?;
        if file.count != file.tokens.len() {
            return Err(DataError::CountMismatch {
                file: "tokens",
                declared: file.count,
                actual: file.tokens.len(),
            });
        }
        Ok(file)
    }
}

impl CardDb {
    /// Adds the Tokens; each needs its own id and name, and must be made by
    /// a card of the pool.
    pub fn with_tokens(mut self, tokens: Vec<Token>) -> Result<Self, DataError> {
        for t in &tokens {
            if self.contains(t.id) || self.token(t.id).is_some() {
                return Err(DataError::DuplicateCardId(t.id));
            }
            if self.iter().any(|c| c.name == t.name) || self.tokens().any(|o| o.name == t.name) {
                return Err(DataError::DuplicateCardName(t.name.clone()));
            }
            if !self.contains(t.made_by) {
                return Err(DataError::UnknownCard {
                    file: "tokens",
                    id: t.made_by,
                });
            }
            self.tokens.push(t.clone());
        }
        Ok(self)
    }

    pub fn token(&self, id: CardId) -> Option<&Token> {
        self.tokens.iter().find(|t| t.id == id)
    }

    pub fn tokens(&self) -> impl Iterator<Item = &Token> {
        self.tokens.iter()
    }
}
