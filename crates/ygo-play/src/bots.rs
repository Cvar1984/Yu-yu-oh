//! Builds players from their names on the command line: `random`, and with
//! the `neural` feature the trained network, alone or guiding a search.
//!
//! - `network`: the shipped network (`data/networks/best.bpk`), playing its
//!   most likely action; the default opponent.
//! - `network-search`: the same network guiding the search
//!   (`ygo_ai::mcts`): stronger, slower.
//! - `neural:<checkpoint>`, `neural-search:<checkpoint>`: the same with
//!   another saved network.

use std::path::PathBuf;
use std::sync::Arc;

use ygo_ai::{Player, RandomPlayer};
use ygo_data::{Bundle, CardDb};
use ygo_engine::Shared;

/// The opponent when none is named: the trained network (owner,
/// 2026-11-25: the network replaces the heuristic and search bots).
pub const DEFAULT_BOT: &str = "network";

pub struct Bots {
    pub shared: Arc<Shared>,
    pub cards: Arc<CardDb>,
    /// The shipped network file: what `network` and `network-search` play
    /// with.
    pub network: Option<PathBuf>,
    /// How hard search bots think.
    #[cfg(feature = "neural")]
    pub mcts: ygo_ai::mcts::MctsSettings,
}

impl Bots {
    pub fn new(bundle: &Bundle) -> Self {
        Self {
            shared: Arc::new(Shared::new(
                bundle.rules.clone(),
                &bundle.cards,
                ygo_cards::registry(),
            )),
            cards: Arc::new(bundle.cards.clone()),
            network: None,
            #[cfg(feature = "neural")]
            mcts: ygo_ai::mcts::MctsSettings::default(),
        }
    }

    /// The bots to offer a player.
    pub fn names(&self) -> Vec<String> {
        let mut names = vec!["random".to_string()];
        if cfg!(feature = "neural") && self.network.is_some() {
            names.splice(0..0, ["network".to_string(), "network-search".to_string()]);
        }
        names
    }

    pub fn make(&self, name: &str, seed: u64) -> Result<Box<dyn Player>, String> {
        if name == "random" {
            return Ok(Box::new(RandomPlayer::new(seed)));
        }
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
        #[cfg(feature = "neural")]
        {
            use std::path::Path;
            use ygo_ai::mcts::MctsPlayer;
            use ygo_ai::neural::{Net, NeuralPlayer, cpu};
            // A saved network, playing its most likely action.
            if let Some(path) = name.strip_prefix("neural:") {
                let net = Net::load(Path::new(path), &cpu())?;
                return Ok(Box::new(NeuralPlayer::new(net, cpu(), 0.0, seed)));
            }
            // A saved network guiding the search.
            if let Some(path) = name.strip_prefix("neural-search:") {
                let net = Net::load(Path::new(path), &cpu())?;
                let judge = NeuralPlayer::new(net, cpu(), 0.0, seed);
                return Ok(Box::new(MctsPlayer::new(judge, self.mcts.clone(), seed)));
            }
        }
        Err(format!(
            "unknown bot `{name}`; available: {}, neural:<checkpoint>, neural-search:<checkpoint>",
            self.names().join(", ")
        ))
    }
}
