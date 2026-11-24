//! Imitation (M6 step 4): the network learns to choose what a teacher bot
//! chose and to foresee who won, from game records (`ygo sim --record`).
//!
//! Records are small (the actions), encoded decisions are not (about
//! 80 KB each), so each epoch replays the records and encodes the
//! teacher's decisions as it goes, mixing them in a pool before batching.
//! Every `holdout`-th record is kept out of training to measure how often
//! the network agrees with the teacher on games it never saw.

use burn::nn::loss::CrossEntropyLossConfig;
use burn::optim::{AdamConfig, GradientsParams};
use burn::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_ai::encode::{Encoded, encode};
use ygo_ai::neural::{Inputs, Net};
use ygo_engine::{GameRecord, GameResult, PlayerView, RecordError, Shared};

/// One decision to learn from.
#[derive(Debug, Clone)]
pub struct Sample {
    pub encoded: Encoded,
    /// The teacher's action, as an index into the legal actions.
    pub action: usize,
    /// The game's result for the decider: 1 won, -1 lost, 0 drawn.
    pub value: f32,
}

/// The decisions of `record` made by a player whose label `teacher`
/// accepts, with more than one legal action.
pub fn samples(
    record: &GameRecord,
    shared: &Shared,
    teacher: &dyn Fn(&str) -> bool,
) -> Result<Vec<Sample>, RecordError> {
    let mut game = record.start(shared)?;
    let mut out = Vec::new();
    for (index, a) in record.actions.iter().enumerate() {
        let view = PlayerView::new(&game).expect("a recorded action has a decision");
        let decider = view.player();
        if view.actions().len() > 1 && teacher(&record.header.players[decider.index()]) {
            let action = view
                .actions()
                .iter()
                .position(|x| *x == a.action)
                .expect("a recorded action is legal");
            let value = match record.result {
                Some(GameResult::Win { winner, .. }) if winner == decider => 1.0,
                Some(GameResult::Win { .. }) => -1.0,
                _ => 0.0,
            };
            out.push(Sample {
                encoded: encode(&view),
                action,
                value,
            });
        }
        GameRecord::step(&mut game, index, a)?;
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct Options {
    pub epochs: usize,
    pub batch: usize,
    pub learning_rate: f64,
    /// Every this many records, one is held out for evaluation.
    pub holdout: usize,
    /// Samples mixed before batching.
    pub pool: usize,
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            epochs: 3,
            batch: 128,
            learning_rate: 1e-3,
            holdout: 10,
            pool: 2048,
            seed: 1,
        }
    }
}

/// What one epoch did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Epoch {
    pub epoch: usize,
    pub samples: usize,
    /// Mean training loss (policy cross-entropy plus value error).
    pub loss: f32,
    /// Held-out decisions where the network's most likely action is the
    /// teacher's.
    pub agreement: f32,
    /// Mean squared value error on held-out decisions.
    pub value_error: f32,
    pub held_out: usize,
    pub seconds: f64,
}

/// Held-out agreement and value error of `net` on `samples`.
fn evaluate(net: &Net, samples: &[Sample], batch: usize, device: &Device) -> (f32, f32) {
    let (mut agree, mut error) = (0usize, 0.0f32);
    for chunk in samples.chunks(batch) {
        let encoded: Vec<Encoded> = chunk.iter().map(|s| s.encoded.clone()).collect();
        let (logits, values) = net.forward(&Inputs::new(&encoded, device));
        let width = logits.dims()[1];
        let logits: Vec<f32> = logits.into_data().try_to_vec::<f32>().expect("f32");
        let values: Vec<f32> = values.into_data().try_to_vec::<f32>().expect("f32");
        for (i, s) in chunk.iter().enumerate() {
            let row = &logits[i * width..i * width + s.encoded.action_count()];
            let best = (0..row.len())
                .max_by(|&a, &b| row[a].total_cmp(&row[b]))
                .unwrap_or(0);
            agree += usize::from(best == s.action);
            error += (values[i] - s.value).powi(2);
        }
    }
    let n = samples.len().max(1) as f32;
    (agree as f32 / n, error / n)
}

/// Trains `net` on the teacher decisions of `records`, calling `report`
/// before training (epoch 0) and after each epoch. Returns the trained network, ready to play.
pub fn train(
    mut net: Net,
    records: &[GameRecord],
    shared: &Shared,
    teacher: &dyn Fn(&str) -> bool,
    opts: &Options,
    report: &mut dyn FnMut(&Epoch),
) -> Result<Net, RecordError> {
    let plain = net
        .devices()
        .into_iter()
        .next()
        .unwrap_or_else(ygo_ai::neural::cpu);
    let device = plain.clone().autodiff();
    let mut held_out = Vec::new();
    let mut training = Vec::new();
    for (i, r) in records.iter().enumerate() {
        if opts.holdout > 0 && i % opts.holdout == opts.holdout - 1 {
            held_out.extend(samples(r, shared, teacher)?);
        } else {
            training.push(r);
        }
    }
    // Epoch 0: the network before training, as a baseline.
    let (agreement, value_error) = evaluate(&net, &held_out, opts.batch, &plain);
    report(&Epoch {
        epoch: 0,
        samples: 0,
        loss: f32::NAN,
        agreement,
        value_error,
        held_out: held_out.len(),
        seconds: 0.0,
    });
    let mut trained = net.fork(&device).train();
    let mut optimizer = AdamConfig::new().init();
    let ce = CrossEntropyLossConfig::new().init(&device);
    let mut rng = ChaCha8Rng::seed_from_u64(opts.seed);
    for epoch in 1..=opts.epochs {
        let started = std::time::Instant::now();
        // Records in a new order each epoch.
        let mut order: Vec<usize> = (0..training.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, (rng.next_u64() % (i as u64 + 1)) as usize);
        }
        let (mut seen, mut loss_sum, mut batches) = (0usize, 0.0f32, 0usize);
        let mut pool: Vec<Sample> = Vec::with_capacity(opts.pool + 512);
        let mut step = |batch: Vec<Sample>, trained: Net| -> (Net, f32) {
            let encoded: Vec<Encoded> = batch.iter().map(|s| s.encoded.clone()).collect();
            let x = Inputs::new(&encoded, &device);
            let n = batch.len();
            let target = Tensor::<1, Int>::from_data(
                TensorData::new(
                    batch.iter().map(|s| s.action as i64).collect::<Vec<_>>(),
                    vec![n],
                ),
                &device,
            );
            let value = Tensor::<1>::from_data(
                TensorData::new(batch.iter().map(|s| s.value).collect::<Vec<_>>(), vec![n]),
                &device,
            );
            let (logits, v) = trained.forward(&x);
            let loss = ce.forward(logits, target) + (v - value).powf_scalar(2.0).mean();
            let l = loss.clone().into_scalar::<f32>();
            let grads = GradientsParams::from_grads(loss.backward(), &trained);
            (optimizer.step(opts.learning_rate, trained, grads), l)
        };
        let mut drain =
            |pool: &mut Vec<Sample>, all: bool, trained: &mut Net, rng: &mut ChaCha8Rng| {
                for i in (1..pool.len()).rev() {
                    pool.swap(i, (rng.next_u64() % (i as u64 + 1)) as usize);
                }
                while pool.len() >= opts.batch || (all && !pool.is_empty()) {
                    let take = pool.len().min(opts.batch);
                    let batch: Vec<Sample> = pool.drain(pool.len() - take..).collect();
                    seen += batch.len();
                    let (next, l) = step(batch, trained.clone());
                    *trained = next;
                    loss_sum += l;
                    batches += 1;
                    if !all && pool.len() < opts.pool / 2 {
                        break;
                    }
                }
            };
        for &i in &order {
            pool.extend(samples(training[i], shared, teacher)?);
            if pool.len() >= opts.pool {
                drain(&mut pool, false, &mut trained, &mut rng);
            }
        }
        drain(&mut pool, true, &mut trained, &mut rng);
        let playing = trained.valid().to_device(&plain);
        let (agreement, value_error) = evaluate(&playing, &held_out, opts.batch, &plain);
        report(&Epoch {
            epoch,
            samples: seen,
            loss: loss_sum / batches.max(1) as f32,
            agreement,
            value_error,
            held_out: held_out.len(),
            seconds: started.elapsed().as_secs_f64(),
        });
    }
    net = trained.valid().to_device(&plain);
    Ok(net)
}
