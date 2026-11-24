//! Trains the trial model on the synthetic task and times it.

use std::time::Instant;

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap, loss};
use nn_spike_candle::*;

/// xorshift64*: the same numbers in every trial project.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545F4914F6CDD1D);
        (x >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
    fn below(&mut self, n: usize) -> usize {
        (((self.next() + 1.0) / 2.0) * n as f32) as usize % n
    }
}

/// A batch of `n` decisions with their right action and value.
fn batch(rng: &mut Rng, n: usize, wa: &[f32], wg: &[f32], dev: &Device) -> Result<(Batch, Tensor, Tensor)> {
    let mut cards = vec![0f32; n * CARDS * CARD_F];
    let mut cmask = vec![0f32; n * CARDS];
    let mut globals = vec![0f32; n * GLOBAL_F];
    let mut actions = vec![0f32; n * ACTIONS * ACTION_F];
    let mut amask = vec![0f32; n * ACTIONS];
    let mut target = vec![0u32; n];
    let mut value = vec![0f32; n];
    for i in 0..n {
        let present = 20 + rng.below(CARDS - 20);
        for c in 0..present {
            cmask[i * CARDS + c] = 1.0;
            for f in 0..CARD_F {
                cards[(i * CARDS + c) * CARD_F + f] = rng.next();
            }
        }
        let mut s = 0.0;
        for f in 0..GLOBAL_F {
            let x = rng.next();
            globals[i * GLOBAL_F + f] = x;
            s += x * wg[f];
        }
        value[i] = if s >= 0.0 { 1.0 } else { -1.0 };
        let legal = 8 + rng.below(ACTIONS - 8);
        let (mut best, mut best_score) = (0, f32::MIN);
        for a in 0..legal {
            amask[i * ACTIONS + a] = 1.0;
            let mut score = 0.0;
            for f in 0..ACTION_F {
                let x = rng.next();
                actions[(i * ACTIONS + a) * ACTION_F + f] = x;
                score += x * wa[f];
            }
            if score > best_score {
                (best, best_score) = (a, score);
            }
        }
        target[i] = best as u32;
    }
    let b = Batch {
        cards: Tensor::from_vec(cards, (n, CARDS, CARD_F), dev)?,
        card_mask: Tensor::from_vec(cmask, (n, CARDS), dev)?,
        globals: Tensor::from_vec(globals, (n, GLOBAL_F), dev)?,
        actions: Tensor::from_vec(actions, (n, ACTIONS, ACTION_F), dev)?,
        action_mask: Tensor::from_vec(amask, (n, ACTIONS), dev)?,
    };
    Ok((b, Tensor::from_vec(target, n, dev)?, Tensor::from_vec(value, n, dev)?))
}

fn evaluate(net: &Net, b: &(Batch, Tensor, Tensor)) -> Result<(f32, f32, f32)> {
    let (logits, value) = net.forward(&b.0)?;
    let pl = loss::cross_entropy(&logits, &b.1)?.to_scalar::<f32>()?;
    let vl = (value - &b.2)?.sqr()?.mean_all()?.to_scalar::<f32>()?;
    let hit = logits.argmax(1)?.eq(&b.1)?.to_dtype(DType::F32)?.mean_all()?.to_scalar::<f32>()?;
    Ok((pl, vl, hit))
}

fn main() -> Result<()> {
    let dev = if cfg!(feature = "cuda") { Device::new_cuda(0)? } else { Device::Cpu };
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let wa: Vec<f32> = (0..ACTION_F).map(|_| rng.next()).collect();
    let wg: Vec<f32> = (0..GLOBAL_F).map(|_| rng.next()).collect();
    let vars = VarMap::new();
    let net = Net::new(VarBuilder::from_varmap(&vars, DType::F32, &dev))?;
    let mut opt = AdamW::new(vars.all_vars(), ParamsAdamW { lr: 1e-3, weight_decay: 0.0, ..Default::default() })?;
    let test = batch(&mut rng, 1024, &wa, &wg, &dev)?;
    let (pl, vl, hit) = evaluate(&net, &test)?;
    println!("candle {:?}: before: policy loss {pl:.3}, value loss {vl:.3}, right action {:.1}%", dev, hit * 100.0);
    let (steps, size) = (300, 256);
    let data: Vec<_> = (0..steps).map(|_| batch(&mut rng, size, &wa, &wg, &dev)).collect::<Result<_>>()?;
    let t = Instant::now();
    for (b, target, value) in &data {
        let (logits, v) = net.forward(b)?;
        let l = (loss::cross_entropy(&logits, target)? + (v - value)?.sqr()?.mean_all()?)?;
        opt.backward_step(&l)?;
    }
    let secs = t.elapsed().as_secs_f64();
    let (pl, vl, hit) = evaluate(&net, &test)?;
    println!("after {steps} steps: policy loss {pl:.3}, value loss {vl:.3}, right action {:.1}%", hit * 100.0);
    println!("training: {:.0} samples/s ({secs:.2}s)", (steps * size) as f64 / secs);
    let one = batch(&mut rng, 1, &wa, &wg, &dev)?;
    let calls = 2000;
    let t = Instant::now();
    for _ in 0..calls {
        let (l, v) = net.forward(&one.0)?;
        let _ = (l.to_vec2::<f32>()?, v.to_vec1::<f32>()?);
    }
    println!("one decision: {:.0} µs", t.elapsed().as_secs_f64() * 1e6 / calls as f64);
    Ok(())
}
