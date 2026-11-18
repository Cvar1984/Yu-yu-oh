use std::path::Path;

use crate::{Banlist, CardDb, DataError, Rules};

/// The rules, card pool and ban list, loaded together and checked against
/// each other. Everything the engine needs before a game starts.
#[derive(Debug, Clone)]
pub struct Bundle {
    pub rules: Rules,
    pub cards: CardDb,
    pub banlist: Banlist,
}

impl Bundle {
    /// Loads `rules_path`, then the card and ban list files it names,
    /// resolving their relative paths against `data_root`.
    pub fn load(rules_path: &Path, data_root: &Path) -> Result<Self, DataError> {
        let rules = Rules::load(rules_path)?;
        let cards = CardDb::load(&data_root.join(&rules.cards.cards))?;
        let banlist = Banlist::load(&data_root.join(&rules.cards.banlist))?;
        Self::from_parts(rules, cards, banlist)
    }

    /// Joins parts loaded elsewhere (from text, in a browser), with the
    /// same cross-checks as `load`.
    pub fn from_parts(rules: Rules, cards: CardDb, banlist: Banlist) -> Result<Self, DataError> {
        banlist.validate(&cards, rules.deck.default_copy_limit)?;
        Ok(Self {
            rules,
            cards,
            banlist,
        })
    }
}
