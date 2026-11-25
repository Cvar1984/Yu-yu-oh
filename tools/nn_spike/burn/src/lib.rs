//! The trial model in burn: per-card encoder, averaged; state MLP; one
//! logit per legal action; a value head. Shapes are in `tools/nn_spike/README.md`.

use burn::nn::{Linear, LinearConfig};
use burn::prelude::*;
use burn::tensor::activation::relu;

pub const CARDS: usize = 80;
pub const CARD_F: usize = 48;
pub const GLOBAL_F: usize = 32;
pub const ACTIONS: usize = 32;
pub const ACTION_F: usize = 64;

#[derive(Module, Debug)]
pub struct Net {
    card1: Linear,
    card2: Linear,
    state1: Linear,
    state2: Linear,
    act: Linear,
    act_state: Linear,
    act_out: Linear,
    value: Linear,
}

/// One batch: cards [B, CARDS, CARD_F], card_mask [B, CARDS],
/// globals [B, GLOBAL_F], actions [B, ACTIONS, ACTION_F], action_mask [B, ACTIONS].
#[derive(Clone)]
pub struct Batch {
    pub cards: Tensor<3>,
    pub card_mask: Tensor<2>,
    pub globals: Tensor<2>,
    pub actions: Tensor<3>,
    pub action_mask: Tensor<2>,
}

impl Net {
    pub fn new(device: &Device) -> Self {
        let l = |i, o| LinearConfig::new(i, o).init(device);
        Self {
            card1: l(CARD_F, 128),
            card2: l(128, 128),
            state1: l(128 + GLOBAL_F, 256),
            state2: l(256, 256),
            act: l(ACTION_F, 128),
            act_state: l(256, 128),
            act_out: l(128, 1),
            value: l(256, 1),
        }
    }

    /// Masked action logits [B, ACTIONS] and values [B].
    pub fn forward(&self, b: &Batch) -> (Tensor<2>, Tensor<1>) {
        let c = relu(self.card2.forward(relu(self.card1.forward(b.cards.clone()))));
        let mask = b.card_mask.clone();
        let count = mask.clone().sum_dim(1).clamp_min(1.0);
        let pooled: Tensor<2> = (c * mask.unsqueeze_dim(2)).sum_dim(1).squeeze_dim(1);
        let pooled = pooled / count;
        let s = Tensor::cat(vec![pooled, b.globals.clone()], 1);
        let s = relu(self.state2.forward(relu(self.state1.forward(s))));
        let a = self.act.forward(b.actions.clone());
        let h = relu(a + self.act_state.forward(s.clone()).unsqueeze_dim(1));
        let logits: Tensor<2> = self.act_out.forward(h).squeeze_dim(2);
        // Padding actions get a very low logit.
        let logits = logits + (b.action_mask.clone() - 1.0) * 1e9;
        let value: Tensor<1> = self.value.forward(s).tanh().squeeze_dim(1);
        (logits, value)
    }
}
