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
use ygo_engine::{Action, DecisionKind, Game, GameResult, PlayerId, PlayerView};

use crate::Player;
use crate::encode::{ACTION_F, CARD_F, DECK_FLAG, Encoded, GLOBAL_F, LINGER_F, encode};

const CARD_H: usize = 128;
const LINGER_H: usize = 64;
const STATE_H: usize = 256;
const ACTION_H: usize = 128;

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
    /// A new network with random weights on `device`.
    pub fn new(device: &Device) -> Self {
        let l = |i, o| LinearConfig::new(i, o).init(device);
        Self {
            card1: l(CARD_F, CARD_H),
            card2: l(CARD_H, CARD_H),
            linger1: l(LINGER_F, LINGER_H),
            linger2: l(LINGER_H, LINGER_H),
            state1: l(2 * CARD_H + LINGER_H + GLOBAL_F, STATE_H),
            state2: l(STATE_H, STATE_H),
            act: l(ACTION_F, ACTION_H),
            act_state: l(STATE_H, ACTION_H),
            act_out: l(ACTION_H, 1),
            value: l(STATE_H, 1),
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
        let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
        let record =
            burn::store::ModuleRecord::from_bytes(burn::tensor::Bytes::from_bytes_vec(bytes))
                .map_err(|e| fail(&e))?;
        Net::new(device)
            .try_load_record(record)
            .map_err(|e| fail(&e))
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
    lookahead: Option<Lookahead>,
}

/// Thinking ahead with the network (`NeuralPlayer::with_lookahead`): the
/// policy's `top` actions are each tried on `samples` samples of the
/// hidden cards, played on with the network's own most likely actions to
/// the next open decision (at most `cap` actions), and scored by the
/// value head; the best average wins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lookahead {
    pub top: usize,
    pub samples: usize,
    pub cap: usize,
}

impl Default for Lookahead {
    fn default() -> Self {
        Self {
            top: 4,
            samples: 2,
            cap: 40,
        }
    }
}

impl NeuralPlayer {
    pub fn new(net: Net, device: Device, temperature: f32, seed: u64) -> Self {
        Self {
            net,
            device,
            temperature,
            rng: ChaCha8Rng::seed_from_u64(seed),
            lookahead: None,
        }
    }

    pub fn with_lookahead(self, lookahead: Lookahead) -> Self {
        Self {
            lookahead: Some(lookahead),
            ..self
        }
    }

    /// The most likely legal action of `view` by the policy alone.
    fn greedy(&self, view: &PlayerView) -> Action {
        let (logits, _) = self.evaluate(view);
        let best = (0..logits.len())
            .max_by(|&a, &b| logits[a].total_cmp(&logits[b]))
            .expect("a decision has legal actions");
        view.actions()[best].clone()
    }

    /// Plays `game` on with the policy to the next open decision (a Main or
    /// Battle Phase action, a hand-limit discard) or the end, then values
    /// it for `me`: 1 a win, -1 a loss, else the value head.
    fn value_after(&self, game: &mut Game, me: PlayerId, cap: usize) -> f32 {
        for _ in 0..cap {
            let Some(d) = game.pending_decision() else {
                break;
            };
            if matches!(
                d.kind,
                DecisionKind::MainPhase | DecisionKind::BattlePhase | DecisionKind::HandLimit
            ) {
                break;
            }
            let action = match game.actions() {
                [only] => only.clone(),
                _ => self.greedy(&PlayerView::new(game).expect("a pending decision")),
            };
            game.apply(&action).expect("the policy picks legal actions");
        }
        match game.result() {
            Some(GameResult::Win { winner, .. }) => {
                if winner == me {
                    1.0
                } else {
                    -1.0
                }
            }
            Some(GameResult::Draw { .. }) => 0.0,
            None => {
                let view = PlayerView::new(game).expect("an unfinished game decides");
                let (_, v) = self.evaluate(&view);
                if view.player() == me { v } else { -v }
            }
        }
    }

    /// The index of the action `lookahead` prefers among `logits`.
    fn look(&mut self, view: &PlayerView, logits: &[f32], l: Lookahead) -> usize {
        let mut order: Vec<usize> = (0..logits.len()).collect();
        order.sort_by(|&a, &b| logits[b].total_cmp(&logits[a]));
        order.truncate(l.top.max(1));
        let me = view.player();
        let mut best = (order[0], f32::MIN);
        for &i in &order {
            let (mut total, mut n) = (0.0, 0);
            for _ in 0..l.samples.max(1) {
                let mut g = view.sample(self.rng.next_u64());
                if g.apply(&view.actions()[i]).is_err() {
                    continue;
                }
                total += self.value_after(&mut g, me, l.cap);
                n += 1;
            }
            let score = if n == 0 { f32::MIN } else { total / n as f32 };
            if score > best.1 {
                best = (i, score);
            }
        }
        best.0
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
        if let Some(l) = self.lookahead
            && logits.len() > 1
        {
            let best = self.look(view, &logits, l);
            return view.actions()[best].clone();
        }
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
