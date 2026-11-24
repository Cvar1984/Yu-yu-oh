use ygo_data::CardId;

use crate::PlayerId;

/// Why a game could not be created from a [`crate::GameSetup`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetupError {
    #[error("player {player} has an unknown card id {id}")]
    UnknownCard { player: PlayerId, id: CardId },
    #[error("player {player}: {id} is a Token, which no deck may hold")]
    TokenInDeck { player: PlayerId, id: CardId },
    #[error("player {player}: {name} ({id}) is in the wrong deck list")]
    WrongDeckList {
        player: PlayerId,
        id: CardId,
        name: String,
    },
    #[error("player {player}: Main Deck has {size} cards; rules allow {min} to {max}")]
    MainDeckSize {
        player: PlayerId,
        size: usize,
        min: usize,
        max: usize,
    },
    #[error("player {player}: Extra Deck has {size} cards; rules allow at most {max}")]
    ExtraDeckSize {
        player: PlayerId,
        size: usize,
        max: usize,
    },
    #[error("a game holds at most {max} cards; the decks hold {total}")]
    TooManyCards { total: usize, max: usize },
    #[error("opening hand of {opening} cards exceeds the Main Deck minimum of {min}")]
    OpeningHandTooLarge { opening: usize, min: usize },
}
