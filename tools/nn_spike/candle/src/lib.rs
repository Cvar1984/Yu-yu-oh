//! The trial model in candle: per-card encoder, summed; state MLP; one
//! logit per legal action; a value head. Shapes are in `tools/nn_spike/README.md`.

use candle_core::{Result, Tensor};
use candle_nn::{Linear, Module, VarBuilder, linear};

pub const CARDS: usize = 80;
pub const CARD_F: usize = 48;
pub const GLOBAL_F: usize = 32;
pub const ACTIONS: usize = 32;
pub const ACTION_F: usize = 64;

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
pub struct Batch {
    pub cards: Tensor,
    pub card_mask: Tensor,
    pub globals: Tensor,
    pub actions: Tensor,
    pub action_mask: Tensor,
}

impl Net {
    pub fn new(vb: VarBuilder) -> Result<Self> {
        Ok(Self {
            card1: linear(CARD_F, 128, vb.pp("card1"))?,
            card2: linear(128, 128, vb.pp("card2"))?,
            state1: linear(128 + GLOBAL_F, 256, vb.pp("state1"))?,
            state2: linear(256, 256, vb.pp("state2"))?,
            act: linear(ACTION_F, 128, vb.pp("act"))?,
            act_state: linear(256, 128, vb.pp("act_state"))?,
            act_out: linear(128, 1, vb.pp("act_out"))?,
            value: linear(256, 1, vb.pp("value"))?,
        })
    }

    /// Masked action logits [B, ACTIONS] and values [B].
    pub fn forward(&self, b: &Batch) -> Result<(Tensor, Tensor)> {
        let c = self.card2.forward(&self.card1.forward(&b.cards)?.relu()?)?.relu()?;
        // The mean of the cards present: a sum over up to 80 cards would
        // saturate the value head's tanh.
        let count = b.card_mask.sum_keepdim(1)?.clamp(1.0, f64::MAX)?;
        let pooled = c
            .broadcast_mul(&b.card_mask.unsqueeze(2)?)?
            .sum(1)?
            .broadcast_div(&count)?;
        let s = Tensor::cat(&[pooled, b.globals.clone()], 1)?;
        let s = self.state2.forward(&self.state1.forward(&s)?.relu()?)?.relu()?;
        let a = self.act.forward(&b.actions)?;
        let h = a.broadcast_add(&self.act_state.forward(&s)?.unsqueeze(1)?)?.relu()?;
        let logits = self.act_out.forward(&h)?.squeeze(2)?;
        // Padding actions get a very low logit.
        let logits = (logits + ((&b.action_mask - 1.0)? * 1e9)?)?;
        let value = self.value.forward(&s)?.tanh()?.squeeze(1)?;
        Ok((logits, value))
    }
}
