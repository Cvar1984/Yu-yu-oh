use serde::{Deserialize, Serialize};
use ygo_data::{CardId, Deck};

use crate::PlayerId;

/// A deck as the engine takes it: one id per physical card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckList {
    pub main: Vec<CardId>,
    pub extra: Vec<CardId>,
}

impl From<&Deck> for DeckList {
    fn from(deck: &Deck) -> Self {
        let expand = |entries: &[ygo_data::DeckEntry]| {
            entries
                .iter()
                .flat_map(|e| std::iter::repeat_n(e.id, e.count))
                .collect()
        };
        Self {
            main: expand(&deck.main),
            extra: expand(&deck.extra),
        }
    }
}

/// Everything that, together with the rules, determines a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSetup {
    pub decks: [DeckList; 2],
    pub seed: u64,
    /// `None`: the seed decides.
    pub first_player: Option<PlayerId>,
    /// A game still running after this many turns is a draw.
    pub max_turns: u32,
    /// `false` keeps each deck in listed order, first card drawn first.
    /// For scenario tests and stacked-deck replays only.
    #[serde(default = "default_true")]
    pub shuffle: bool,
}

fn default_true() -> bool {
    true
}

impl GameSetup {
    pub const DEFAULT_MAX_TURNS: u32 = 200;

    pub fn new(decks: [DeckList; 2], seed: u64) -> Self {
        Self {
            decks,
            seed,
            first_player: None,
            max_turns: Self::DEFAULT_MAX_TURNS,
            shuffle: true,
        }
    }
}
