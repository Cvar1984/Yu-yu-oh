//! Builds players from their names on the command line: `random`,
//! `heuristic:<style>` or `search[:<style>]`, with the styles and card
//! values loaded once.

use std::path::Path;
use std::sync::Arc;

use ygo_ai::{HeuristicPlayer, Player, RandomPlayer, SearchPlayer, SearchSettings};
use ygo_data::{Bundle, CardDb, CardValues, DataError, Styles};
use ygo_engine::Shared;

/// The opponent when none is named: the Beatdown style looking one step
/// ahead (owner, 2026-11-22). `heuristic:<style>` is the plain bot, kept
/// for comparisons; search bots play out in the plain Beatdown style.
pub const DEFAULT_BOT: &str = "lookahead:beatdown";

/// The style search bots play out in when none is named.
const DEFAULT_ROLLOUT_STYLE: &str = "beatdown";

pub struct Bots {
    pub shared: Arc<Shared>,
    pub cards: Arc<CardDb>,
    values: CardValues,
    styles: Styles,
    /// How hard `search` bots think.
    pub search: SearchSettings,
    /// `search` bots play their playouts with the lookahead heuristic bot.
    pub rollout_lookahead: bool,
    /// The shipped network (`data/networks/best.bpk`): the bot names
    /// `network` and `network-search` stand for `neural:<it>` and
    /// `neural-search:<it>`.
    pub network: Option<std::path::PathBuf>,
    /// How hard `neural-search` bots think.
    #[cfg(feature = "neural")]
    pub mcts: ygo_ai::mcts::MctsSettings,
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
            rollout_lookahead: false,
            network: None,
            #[cfg(feature = "neural")]
            mcts: ygo_ai::mcts::MctsSettings::default(),
        }
    }

    /// The bots to offer: `random`, every style looking ahead, and `search`.
    /// `make` also takes `search:<style>`, a search bot whose playouts use
    /// another style.
    pub fn names(&self) -> Vec<String> {
        std::iter::once("random".to_string())
            .chain(self.styles.names().map(|s| format!("lookahead:{s}")))
            .chain(std::iter::once("search".to_string()))
            .collect()
    }

    /// A heuristic bot of this style, looking one step ahead or not.
    pub fn heuristic(
        &self,
        style: &str,
        lookahead: bool,
        seed: u64,
    ) -> Result<HeuristicPlayer, String> {
        let style = self
            .styles
            .get(style)
            .ok_or_else(|| format!("unknown style `{style}`"))?;
        Ok(
            HeuristicPlayer::new(self.shared.clone(), &self.cards, &self.values, style, seed)
                .with_lookahead(lookahead),
        )
    }

    pub fn make(&self, name: &str, seed: u64) -> Result<Box<dyn Player>, String> {
        if let Some(kind) = match name {
            "network" => Some("neural"),
            "network-search" => Some("neural-search"),
            _ => None,
        } {
            let path = self
                .network
                .as_ref()
                .ok_or_else(|| format!("`{name}`: no network file is set"))?;
            return self.make(&format!("{kind}:{}", path.display()), seed);
        }
        if name == "random" {
            return Ok(Box::new(RandomPlayer::new(seed)));
        }
        if let Some(style) = name.strip_prefix("lookahead:") {
            return Ok(Box::new(self.heuristic(style, true, seed)?));
        }
        // A trained network, playing its most likely action.
        #[cfg(feature = "neural")]
        if let Some(path) = name.strip_prefix("neural:") {
            use ygo_ai::neural::{Net, NeuralPlayer, cpu};
            let net = Net::load(Path::new(path), &cpu())?;
            return Ok(Box::new(NeuralPlayer::new(net, cpu(), 0.0, seed)));
        }
        // The same, thinking ahead with its value head.
        #[cfg(feature = "neural")]
        if let Some(path) = name.strip_prefix("neural-lookahead:") {
            use ygo_ai::neural::{Lookahead, Net, NeuralPlayer, cpu};
            let net = Net::load(Path::new(path), &cpu())?;
            let player =
                NeuralPlayer::new(net, cpu(), 0.0, seed).with_lookahead(Lookahead::default());
            return Ok(Box::new(player));
        }
        // The network-guided search (AlphaZero): the network's priors and
        // values, no playouts.
        #[cfg(feature = "neural")]
        if let Some(path) = name.strip_prefix("neural-search:") {
            use ygo_ai::mcts::MctsPlayer;
            use ygo_ai::neural::{Net, NeuralPlayer, cpu};
            let net = Net::load(Path::new(path), &cpu())?;
            let judge = NeuralPlayer::new(net, cpu(), 0.0, seed);
            return Ok(Box::new(MctsPlayer::new(judge, self.mcts.clone(), seed)));
        }
        // `search` plays out its samples with the default heuristic style.
        let (search, style_name) = match name {
            "search" => (true, Some(DEFAULT_ROLLOUT_STYLE)),
            _ => match name.strip_prefix("search:") {
                Some(style) => (true, Some(style)),
                None => (false, name.strip_prefix("heuristic:")),
            },
        };
        let style = style_name.and_then(|s| self.styles.get(s)).ok_or_else(|| {
            format!(
                "unknown bot `{name}`; available: {}, search:<style>, heuristic:<style> (no lookahead), neural:<checkpoint>, neural-lookahead:<checkpoint>, neural-search:<checkpoint>",
                self.names().join(", ")
            )
        })?;
        let heuristic =
            HeuristicPlayer::new(self.shared.clone(), &self.cards, &self.values, style, seed);
        Ok(if search {
            let rollout = heuristic.with_lookahead(self.rollout_lookahead);
            Box::new(SearchPlayer::new(rollout, self.search.clone(), seed))
        } else {
            Box::new(heuristic)
        })
    }
}
