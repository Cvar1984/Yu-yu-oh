//! Builds players from their names on the command line: `random` or
//! `heuristic:<style>`, with the styles and card values loaded once.

use std::path::Path;
use std::sync::Arc;

use ygo_ai::{HeuristicPlayer, Player, RandomPlayer};
use ygo_data::{Bundle, CardDb, CardValues, DataError, Styles};
use ygo_engine::Shared;

/// The opponent when none is named.
pub const DEFAULT_BOT: &str = "heuristic:beatdown";

pub struct Bots {
    pub shared: Arc<Shared>,
    pub cards: Arc<CardDb>,
    values: CardValues,
    styles: Styles,
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
        }
    }

    /// Every name `make` accepts.
    pub fn names(&self) -> Vec<String> {
        std::iter::once("random".to_string())
            .chain(self.styles.names().map(|s| format!("heuristic:{s}")))
            .collect()
    }

    pub fn make(&self, name: &str, seed: u64) -> Result<Box<dyn Player>, String> {
        if name == "random" {
            return Ok(Box::new(RandomPlayer::new(seed)));
        }
        let style = name
            .strip_prefix("heuristic:")
            .and_then(|s| self.styles.get(s))
            .ok_or_else(|| {
                format!(
                    "unknown bot `{name}`; available: {}",
                    self.names().join(", ")
                )
            })?;
        Ok(Box::new(HeuristicPlayer::new(
            self.shared.clone(),
            &self.cards,
            &self.values,
            style,
            seed,
        )))
    }
}
