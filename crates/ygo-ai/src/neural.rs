//! The neural policy and value (M6 step 3, `docs/m6-plan.md`), with burn.
//!
//! The network reads `encode::Encoded`:
//! - each card vector goes through a per-card net; the results are
//!   averaged in two groups, the cards in play (hands, field, Graveyards,
//!   banished, Extra Decks, choices, chain) and the viewer's own Deck, so
//!   the Deck's thirty-odd cards do not drown the board;
//! - each lingering effect goes through its own small net, averaged;
//! - the averages and the game vector make a 256-number state;
//! - each legal action is scored against the state (one logit each, so
//!   any number of legal actions works), and the value head gives the
//!   expected result for the decider, -1 to 1.
//!
//! Averages, not sums: a sum over many cards saturated the value head in
//! the library trial.

use std::path::Path;

use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;
use burn::tensor::activation::relu;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_engine::{Action, PlayerView};

use crate::Player;
use crate::encode::{ACTION_F, CARD_F, DECK_FLAG, Encoded, GLOBAL_F, LINGER_F, encode};

/// The sizes of the network's hidden layers. A checkpoint does not record
/// them: `Net::load` tries each of `Widths::ALL` until one fits the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Widths {
    pub card: usize,
    pub linger: usize,
    pub state: usize,
    pub action: usize,
}

impl Widths {
    /// The network of M6 steps 3 to 5 (`owner-2`, the first self-play run).
    pub const BASE: Widths = Widths {
        card: 128,
        linger: 64,
        state: 256,
        action: 128,
    };
    /// Twice as wide (owner, 2026-11-25: self-play stalled at the base size).
    pub const WIDE: Widths = Widths {
        card: 256,
        linger: 128,
        state: 512,
        action: 256,
    };
    pub const ALL: [Widths; 2] = [Widths::BASE, Widths::WIDE];
}

#[derive(Module, Debug)]
pub struct Net {
    card1: Linear,
    card2: Linear,
    linger1: Linear,
    linger2: Linear,
    state1: Linear,
    state2: Linear,
    act: Linear,
    act_state: Linear,
    act_out: Linear,
    value: Linear,
}

/// A batch of decisions as tensors, padded to the largest one; masks mark
/// what is real.
#[derive(Debug, Clone)]
pub struct Inputs {
    /// [B, cards, CARD_F]
    pub cards: Tensor<3>,
    /// [B, cards]: 1 for a card in play.
    pub table_mask: Tensor<2>,
    /// [B, cards]: 1 for a card of the viewer's own Deck.
    pub deck_mask: Tensor<2>,
    /// [B, lingering, LINGER_F]
    pub lingering: Tensor<3>,
    /// [B, lingering]
    pub linger_mask: Tensor<2>,
    /// [B, GLOBAL_F]
    pub globals: Tensor<2>,
    /// [B, actions, ACTION_F]
    pub actions: Tensor<3>,
    /// [B, actions]
    pub action_mask: Tensor<2>,
}

impl Inputs {
    /// Pads and stacks `batch` on `device`.
    pub fn new(batch: &[Encoded], device: &Device) -> Self {
        let b = batch.len();
        // At least one row everywhere, so no tensor has a zero dimension.
        let n = batch
            .iter()
            .map(Encoded::card_count)
            .max()
            .unwrap_or(0)
            .max(1);
        let l = batch
            .iter()
            .map(Encoded::lingering_count)
            .max()
            .unwrap_or(0)
            .max(1);
        let a = batch
            .iter()
            .map(Encoded::action_count)
            .max()
            .unwrap_or(0)
            .max(1);
        let mut cards = vec![0.0; b * n * CARD_F];
        let mut table = vec![0.0; b * n];
        let mut deck = vec![0.0; b * n];
        let mut lingering = vec![0.0; b * l * LINGER_F];
        let mut lmask = vec![0.0; b * l];
        let mut globals = vec![0.0; b * GLOBAL_F];
        let mut actions = vec![0.0; b * a * ACTION_F];
        let mut amask = vec![0.0; b * a];
        for (i, e) in batch.iter().enumerate() {
            for (j, v) in e.cards.chunks(CARD_F).enumerate() {
                cards[(i * n + j) * CARD_F..(i * n + j + 1) * CARD_F].copy_from_slice(v);
                if v[DECK_FLAG] == 1.0 {
                    deck[i * n + j] = 1.0;
                } else {
                    table[i * n + j] = 1.0;
                }
            }
            for (j, v) in e.lingering.chunks(LINGER_F).enumerate() {
                lingering[(i * l + j) * LINGER_F..(i * l + j + 1) * LINGER_F].copy_from_slice(v);
                lmask[i * l + j] = 1.0;
            }
            globals[i * GLOBAL_F..(i + 1) * GLOBAL_F].copy_from_slice(&e.globals);
            for (j, v) in e.actions.chunks(ACTION_F).enumerate() {
                actions[(i * a + j) * ACTION_F..(i * a + j + 1) * ACTION_F].copy_from_slice(v);
                amask[i * a + j] = 1.0;
            }
        }
        let t2 = |v: Vec<f32>, d: usize| Tensor::from_data(TensorData::new(v, vec![b, d]), device);
        let t3 = |v: Vec<f32>, d: usize, f: usize| {
            Tensor::from_data(TensorData::new(v, vec![b, d, f]), device)
        };
        Self {
            cards: t3(cards, n, CARD_F),
            table_mask: t2(table, n),
            deck_mask: t2(deck, n),
            lingering: t3(lingering, l, LINGER_F),
            linger_mask: t2(lmask, l),
            globals: t2(globals, GLOBAL_F),
            actions: t3(actions, a, ACTION_F),
            action_mask: t2(amask, a),
        }
    }
}

/// The mean of `x` [B, N, H] over the rows `mask` [B, N] marks; zeros
/// where none is marked.
fn masked_mean(x: Tensor<3>, mask: Tensor<2>) -> Tensor<2> {
    let count = mask.clone().sum_dim(1).clamp_min(1.0);
    let sum: Tensor<2> = (x * mask.unsqueeze_dim(2)).sum_dim(1).squeeze_dim(1);
    sum / count
}

impl Net {
    /// A new network of the base widths with random weights on `device`.
    pub fn new(device: &Device) -> Self {
        Self::with_widths(device, Widths::BASE)
    }

    /// A new network of widths `w` with random weights on `device`.
    pub fn with_widths(device: &Device, w: Widths) -> Self {
        let l = |i, o| LinearConfig::new(i, o).init(device);
        Self {
            card1: l(CARD_F, w.card),
            card2: l(w.card, w.card),
            linger1: l(LINGER_F, w.linger),
            linger2: l(w.linger, w.linger),
            state1: l(2 * w.card + w.linger + GLOBAL_F, w.state),
            state2: l(w.state, w.state),
            act: l(ACTION_F, w.action),
            act_state: l(w.state, w.action),
            act_out: l(w.action, 1),
            value: l(w.state, 1),
        }
    }

    /// The widths of this network's hidden layers.
    pub fn widths(&self) -> Widths {
        let [_, card] = self.card1.weight.dims();
        let [_, linger] = self.linger1.weight.dims();
        let [_, state] = self.state1.weight.dims();
        let [_, action] = self.act.weight.dims();
        Widths {
            card,
            linger,
            state,
            action,
        }
    }

    /// Action logits [B, actions] (padding far below any real one) and
    /// values [B] in -1..1.
    pub fn forward(&self, x: &Inputs) -> (Tensor<2>, Tensor<1>) {
        let c = relu(
            self.card2
                .forward(relu(self.card1.forward(x.cards.clone()))),
        );
        let table = masked_mean(c.clone(), x.table_mask.clone());
        let deck = masked_mean(c, x.deck_mask.clone());
        let l = relu(
            self.linger2
                .forward(relu(self.linger1.forward(x.lingering.clone()))),
        );
        let lingering = masked_mean(l, x.linger_mask.clone());
        let s = Tensor::cat(vec![table, deck, lingering, x.globals.clone()], 1);
        let s = relu(self.state2.forward(relu(self.state1.forward(s))));
        let a = self.act.forward(x.actions.clone());
        let h = relu(a + self.act_state.forward(s.clone()).unsqueeze_dim(1));
        let logits: Tensor<2> = self.act_out.forward(h).squeeze_dim(2);
        let logits = logits + (x.action_mask.clone() - 1.0) * 1e9;
        let value: Tensor<1> = self.value.forward(s).tanh().squeeze_dim(1);
        (logits, value)
    }

    /// Saves the weights to `path` (a burnpack file). The file is written
    /// next to it and renamed into place, so a bot loading `path` while
    /// training saves never reads half a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = self
            .clone()
            .into_record()
            .into_bytes()
            .map_err(|e| e.to_string())?;
        let partial = path.with_extension("partial");
        std::fs::write(&partial, &bytes[..]).map_err(|e| format!("{}: {e}", partial.display()))?;
        std::fs::rename(&partial, path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A network with the weights saved at `path`, read into memory at
    /// once (the file may be replaced afterwards).
    pub fn load(path: &Path, device: &Device) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_bytes(bytes, device).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A network from the bytes of a saved file (the browser has no files).
    /// The file does not record the widths: each of `Widths::ALL` is tried
    /// until the weights fit.
    pub fn from_bytes(bytes: Vec<u8>, device: &Device) -> Result<Self, String> {
        let record =
            burn::store::ModuleRecord::from_bytes(burn::tensor::Bytes::from_bytes_vec(bytes))
                .map_err(|e| e.to_string())?;
        let mut first_error = None;
        for w in Widths::ALL {
            match Net::with_widths(device, w).try_load_record(record.clone()) {
                Ok(net) => return Ok(net),
                Err(e) => {
                    first_error.get_or_insert(e.to_string());
                }
            }
        }
        Err(first_error.unwrap_or_default())
    }
}

/// The CPU device the bots play on (burn's pure-Rust `flex` backend).
pub fn cpu() -> Device {
    Device::flex()
}

/// A bot that plays by the network's policy: the most likely action, or
/// with a `temperature` above 0 one drawn from the softmax of the logits
/// divided by it (self-play explores this way).
#[derive(Debug, Clone)]
pub struct NeuralPlayer {
    net: Net,
    device: Device,
    temperature: f32,
    rng: ChaCha8Rng,
}

impl NeuralPlayer {
    pub fn new(net: Net, device: Device, temperature: f32, seed: u64) -> Self {
        Self {
            net,
            device,
            temperature,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// The logits of `view`'s legal actions and the value of the position
    /// for the decider.
    pub fn evaluate(&self, view: &PlayerView) -> (Vec<f32>, f32) {
        let e = encode(view);
        let k = e.action_count();
        let (logits, value) = self.net.forward(&Inputs::new(&[e], &self.device));
        let logits: Vec<f32> = logits.into_data().try_to_vec::<f32>().expect("f32 logits");
        let value: Vec<f32> = value.into_data().try_to_vec::<f32>().expect("f32 value");
        (logits[..k].to_vec(), value[0])
    }
}

impl Player for NeuralPlayer {
    fn choose(&mut self, view: &PlayerView) -> Action {
        let (logits, _) = self.evaluate(view);
        let best = if self.temperature <= 0.0 {
            (0..logits.len())
                .max_by(|&a, &b| logits[a].total_cmp(&logits[b]))
                .expect("a decision has legal actions")
        } else {
            let top = logits.iter().copied().fold(f32::MIN, f32::max);
            let weights: Vec<f32> = logits
                .iter()
                .map(|x| ((x - top) / self.temperature).exp())
                .collect();
            let total: f32 = weights.iter().sum();
            let mut pick = (self.rng.next_u64() as f64 / u64::MAX as f64) as f32 * total;
            let mut chosen = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                if pick < *w {
                    chosen = i;
                    break;
                }
                pick -= w;
            }
            chosen
        };
        view.actions()[best].clone()
    }
}
