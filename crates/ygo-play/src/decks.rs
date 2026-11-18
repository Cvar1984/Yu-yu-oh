//! Loading a deck for a game: `play` and `sim` refuse a deck that breaks
//! the ban list or the deck rules, with the same report as `check-deck`.

use std::fmt;
use std::path::{Path, PathBuf};

use ygo_data::{Bundle, Deck, DeckProblem, check_deck};

/// A deck that may not be played, with every reason.
#[derive(Debug)]
pub struct IllegalDeck {
    pub path: PathBuf,
    pub name: String,
    pub problems: Vec<DeckProblem>,
}

impl fmt::Display for IllegalDeck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) is not legal:", self.path.display(), self.name)?;
        for p in &self.problems {
            write!(f, "\n  - {p}")?;
        }
        Ok(())
    }
}

impl std::error::Error for IllegalDeck {}

/// Loads `path` and checks it against the bundle's cards, ban list and
/// rules (not the collection).
pub fn load_legal(path: &Path, bundle: &Bundle) -> Result<Deck, Box<dyn std::error::Error>> {
    check_legal(Deck::load(path)?, path, bundle)
}

/// Checks a deck already loaded; `origin` names it in the report.
pub fn check_legal(
    deck: Deck,
    origin: &Path,
    bundle: &Bundle,
) -> Result<Deck, Box<dyn std::error::Error>> {
    let problems = check_deck(&deck, &bundle.cards, &bundle.banlist, &bundle.rules, None);
    if problems.is_empty() {
        Ok(deck)
    } else {
        Err(Box::new(IllegalDeck {
            path: origin.to_path_buf(),
            name: deck.name,
            problems,
        }))
    }
}
