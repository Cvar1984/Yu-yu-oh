//! Trains the trial model on the synthetic task and times it.

use std::time::Instant;

use burn::nn::loss::CrossEntropyLossConfig;
use burn::optim::{AdamConfig, GradientsParams};
use burn::prelude::*;
use burn::tensor::TensorData;
use nn_spike_burn::*;

/// The plain device: CUDA with `--features cuda`, else the pure-Rust CPU one.
fn plain_device() -> Device {
    #[cfg(feature = "cuda")]
    return Device::cuda(0);
    #[cfg(not(feature = "cuda"))]
    Device::flex()
}

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
fn batch(
    rng: &mut Rng,
    n: usize,
    wa: &[f32],
    wg: &[f32],
    dev: &Device,
) -> (Batch, Tensor<1, Int>, Tensor<1>) {
    let mut cards = vec![0f32; n * CARDS * CARD_F];
    let mut cmask = vec![0f32; n * CARDS];
    let mut globals = vec![0f32; n * GLOBAL_F];
    let mut actions = vec![0f32; n * ACTIONS * ACTION_F];
    let mut amask = vec![0f32; n * ACTIONS];
    let mut target = vec![0i64; n];
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
        target[i] = best as i64;
    }
    let t = |v: Vec<f32>, shape: Vec<usize>| TensorData::new(v, shape);
    let b = Batch {
        cards: Tensor::from_data(t(cards, vec![n, CARDS, CARD_F]), dev),
        card_mask: Tensor::from_data(t(cmask, vec![n, CARDS]), dev),
        globals: Tensor::from_data(t(globals, vec![n, GLOBAL_F]), dev),
        actions: Tensor::from_data(t(actions, vec![n, ACTIONS, ACTION_F]), dev),
        action_mask: Tensor::from_data(t(amask, vec![n, ACTIONS]), dev),
    };
    (
        b,
        Tensor::from_data(TensorData::new(target, vec![n]), dev),
        Tensor::from_data(t(value, vec![n]), dev),
    )
}

fn evaluate(net: &Net, b: &(Batch, Tensor<1, Int>, Tensor<1>)) -> (f32, f32, f32) {
    let (logits, value) = net.forward(&b.0);
    let dev = logits.device();
    let pl = CrossEntropyLossConfig::new()
        .init(&dev)
        .forward(logits.clone(), b.1.clone())
        .into_scalar::<f32>();
    let vl = (value - b.2.clone()).powf_scalar(2.0).mean().into_scalar::<f32>();
    let hit = logits
        .argmax(1)
        .squeeze_dim::<1>(1)
        .equal(b.1.clone())
        .float()
        .mean()
        .into_scalar::<f32>();
    (pl, vl, hit)
}

fn main() {
    let plain = plain_device();
    let dev = plain.clone().autodiff();
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let wa: Vec<f32> = (0..ACTION_F).map(|_| rng.next()).collect();
    let wg: Vec<f32> = (0..GLOBAL_F).map(|_| rng.next()).collect();
    let mut net = Net::new(&dev);
    let mut opt = AdamConfig::new().init();
    let test = batch(&mut rng, 1024, &wa, &wg, &dev);
    let (pl, vl, hit) = evaluate(&net, &test);
    println!("burn {plain:?}: before: policy loss {pl:.3}, value loss {vl:.3}, right action {:.1}%", hit * 100.0);
    let (steps, size) = (300, 256);
    let data: Vec<_> = (0..steps).map(|_| batch(&mut rng, size, &wa, &wg, &dev)).collect();
    let ce = CrossEntropyLossConfig::new().init(&dev);
    let t = Instant::now();
    for (b, target, value) in &data {
        let (logits, v) = net.forward(b);
        let loss = ce.forward(logits, target.clone()) + (v - value.clone()).powf_scalar(2.0).mean();
        let grads = GradientsParams::from_grads(loss.backward(), &net);
        net = opt.step(1e-3, net, grads);
    }
    let secs = t.elapsed().as_secs_f64();
    let (pl, vl, hit) = evaluate(&net, &test);
    println!("after {steps} steps: policy loss {pl:.3}, value loss {vl:.3}, right action {:.1}%", hit * 100.0);
    println!("training: {:.0} samples/s ({secs:.2}s)", (steps * size) as f64 / secs);
    // Inference without autodiff, as a bot plays.
    let play = net.valid().to_device(&plain);
    let one = batch(&mut rng, 1, &wa, &wg, &plain);
    let calls = 2000;
    let t = Instant::now();
    for _ in 0..calls {
        let (l, v) = play.forward(&one.0);
        let _ = (l.into_data(), v.into_data());
    }
    println!("one decision: {:.0} µs", t.elapsed().as_secs_f64() * 1e6 / calls as f64);
}
