//! Builds players from their names on the command line: `random`,
//! `heuristic:<style>` or `search[:<style>]`, with the styles and card
//! values loaded once.

use std::path::Path;
use std::sync::Arc;

use ygo_ai::{HeuristicPlayer, Player, RandomPlayer, SearchPlayer, SearchSettings};
use ygo_data::{Bundle, CardDb, CardValues, DataError, Styles};
use ygo_engine::Shared;

/// The opponent when none is named.
pub const DEFAULT_BOT: &str = "heuristic:beatdown";

pub struct Bots {
    pub shared: Arc<Shared>,
    pub cards: Arc<CardDb>,
    values: CardValues,
    styles: Styles,
    /// How hard `search` bots think.
    pub search: SearchSettings,
}

impl Bots {
    pub fn load(bundle: &Bundle, styles: &Path, card_values: &Path) -> Result<Self, DataError> {
        Ok(Self::new(
            bundle,
            CardValues::load(card_values, &bundle.cards)?,
            Styles::load(styles, &bundle.cards)?,
        ))
    }

    /// From configuration already loaded (from text, in a browser).
    pub fn new(bundle: &Bundle, values: CardValues, styles: Styles) -> Self {
        Self {
            shared: Arc::new(Shared::new(
                bundle.rules.clone(),
                &bundle.cards,
                ygo_cards::registry(),
            )),
            cards: Arc::new(bundle.cards.clone()),
            values,
            styles,
            search: SearchSettings::default(),
        }
    }

    /// The bots to offer: `random`, every heuristic style and `search`.
    /// `make` also takes `search:<style>`, a search bot whose playouts use
    /// another style.
    pub fn names(&self) -> Vec<String> {
        std::iter::once("random".to_string())
            .chain(self.styles.names().map(|s| format!("heuristic:{s}")))
            .chain(std::iter::once("search".to_string()))
            .collect()
    }

    pub fn make(&self, name: &str, seed: u64) -> Result<Box<dyn Player>, String> {
        if name == "random" {
            return Ok(Box::new(RandomPlayer::new(seed)));
        }
        // `search` plays out its samples with the default heuristic style.
        let (search, style_name) = match name {
            "search" => (true, DEFAULT_BOT.strip_prefix("heuristic:")),
            _ => match name.strip_prefix("search:") {
                Some(style) => (true, Some(style)),
                None => (false, name.strip_prefix("heuristic:")),
            },
        };
        let style = style_name.and_then(|s| self.styles.get(s)).ok_or_else(|| {
            format!(
                "unknown bot `{name}`; available: {}, search:<style>",
                self.names().join(", ")
            )
        })?;
        let heuristic =
            HeuristicPlayer::new(self.shared.clone(), &self.cards, &self.values, style, seed);
        Ok(if search {
            Box::new(SearchPlayer::new(heuristic, self.search.clone(), seed))
        } else {
            Box::new(heuristic)
        })
    }
}
