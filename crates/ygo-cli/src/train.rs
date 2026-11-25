//! `ygo train`: AlphaZero-style generations (M6 steps 5 and 6). Each
//! generation the best network's search plays itself on the decks (mirror
//! games, each deck equally often, each side first in half), a candidate
//! trains from the best on the last `buffer` generations' games, and the
//! candidate plays the best, both alone (most likely action), over
//! `gate_games`: it becomes the best only if its 95% interval lies above
//! 50% (the M6 gate). Every candidate is saved as `gen-<g>.bpk` and the
//! best as `best.bpk` in `out`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use ygo_ai::mcts::MctsSettings;
use ygo_ai::neural::Net;
use ygo_data::Deck;
use ygo_engine::{DeckList, GameSetup, PlayerId};
use ygo_train::imitation::{self, Options};
use ygo_train::selfplay::{self, SelfPlayGame};

use crate::bots::Bots;
use crate::sim::{self, SimOptions};

pub struct TrainOptions<'a> {
    pub decks: &'a [Deck],
    pub out: &'a Path,
    pub generations: usize,
    /// Self-play games per generation.
    pub games: u32,
    /// Generations of games trained on.
    pub buffer: usize,
    pub explore_turns: u32,
    pub gate_games: u32,
    pub search: MctsSettings,
    pub train: Options,
    pub seed: u64,
    pub threads: usize,
}

/// What the gate measured: the candidate's score (draws count half) and
/// the half-width of its 95% interval.
pub struct Gate {
    pub score: f64,
    pub half: f64,
    pub games: u32,
}

impl Gate {
    pub fn passed(&self) -> bool {
        self.score - self.half > 0.5
    }
}

fn save(net: &Net, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    net.save(path)
}

/// Plays `games` self-play games with `net`, worker `w` playing games
/// w, w + threads, ...
fn self_play(
    bots: &Bots,
    net: &Net,
    opts: &TrainOptions,
    generation: usize,
) -> Result<Vec<SelfPlayGame>, String> {
    let n = opts.decks.len() as u32;
    let play = |i: u32| -> Result<SelfPlayGame, String> {
        let deck = DeckList::from(&opts.decks[(i % n) as usize]);
        let seed = opts
            .seed
            .wrapping_add(generation as u64 * 1_000_000)
            .wrapping_add(u64::from(i));
        let mut setup = GameSetup::new([deck.clone(), deck], seed);
        setup.first_player = Some(PlayerId(((i / n) % 2) as u8));
        selfplay::play(
            bots.shared.clone(),
            &setup,
            net,
            &opts.search,
            opts.explore_turns,
        )
        .map_err(|e| e.to_string())
    };
    let threads = opts.threads.clamp(1, opts.games.max(1) as usize);
    let played: Vec<Result<Vec<SelfPlayGame>, String>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|w| {
                let play = &play;
                scope.spawn(move || {
                    (w as u32..opts.games)
                        .step_by(threads)
                        .map(play)
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|h| h.join().expect("a self-play game panicked"))
            .collect()
    });
    let mut games = Vec::new();
    for p in played {
        games.extend(p?);
    }
    Ok(games)
}

/// The candidate alone against the best alone, the games split over the
/// decks.
pub fn gate(
    bots: &Bots,
    candidate: &Path,
    best: &Path,
    decks: &[Deck],
    games: u32,
    seed: u64,
    threads: usize,
) -> Result<Gate, String> {
    let (a, b) = (
        format!("neural:{}", candidate.display()),
        format!("neural:{}", best.display()),
    );
    let per_deck = games.div_ceil(decks.len() as u32);
    let (mut won, mut drawn, mut total) = (0u32, 0u32, 0u32);
    for (i, deck) in decks.iter().enumerate() {
        let tally = sim::run(
            bots,
            &SimOptions {
                deck_a: deck,
                deck_b: deck,
                bot_a: &a,
                bot_b: &b,
                games: per_deck,
                seed: seed.wrapping_add(i as u64 * 100_000),
                max_turns: GameSetup::DEFAULT_MAX_TURNS,
                record_dir: None,
                threads,
            },
        )
        .map_err(|e| e.to_string())?;
        won += tally.total(0);
        drawn += tally.total(2);
        total += tally.total(0) + tally.total(1) + tally.total(2);
    }
    let score = (f64::from(won) + f64::from(drawn) / 2.0) / f64::from(total.max(1));
    let half = 1.96 * (score * (1.0 - score) / f64::from(total.max(1))).sqrt();
    Ok(Gate {
        score,
        half,
        games: total,
    })
}

pub fn run(bots: &Bots, start: Net, opts: &TrainOptions) -> Result<Net, String> {
    let best_path: PathBuf = opts.out.join("best.bpk");
    let mut best = start;
    save(&best, &best_path)?;
    let mut buffer: VecDeque<Vec<SelfPlayGame>> = VecDeque::new();
    for generation in 1..=opts.generations {
        let started = std::time::Instant::now();
        let games = self_play(bots, &best, opts, generation)?;
        let decisions: usize = games.iter().map(|g| g.targets.len()).sum();
        println!(
            "generation {generation}: {} self-play games, {decisions} decisions, {:.0}s",
            games.len(),
            started.elapsed().as_secs_f64()
        );
        buffer.push_back(games);
        while buffer.len() > opts.buffer.max(1) {
            buffer.pop_front();
        }
        let all: Vec<&SelfPlayGame> = buffer.iter().flatten().collect();
        let shared = bots.shared.clone();
        let candidate = imitation::train_on(
            best.clone(),
            all.len(),
            &|i| selfplay::samples(all[i], &shared),
            &opts.train,
            &mut |e| {
                println!(
                    "  epoch {}: {} samples, loss {:.3}, held-out agreement {:.1}% and value error {:.3} ({} decisions), {:.0}s",
                    e.epoch,
                    e.samples,
                    e.loss,
                    100.0 * e.agreement,
                    e.value_error,
                    e.held_out,
                    e.seconds
                );
            },
        )
        .map_err(|e| e.to_string())?;
        let path = opts.out.join(format!("gen-{generation}.bpk"));
        save(&candidate, &path)?;
        let g = gate(
            bots,
            &path,
            &best_path,
            opts.decks,
            opts.gate_games,
            opts.seed.wrapping_add(generation as u64 * 7_919),
            opts.threads,
        )?;
        let verdict = if g.passed() {
            "promoted"
        } else {
            "kept the best"
        };
        println!(
            "  gate: candidate {:.1}% ±{:.1} against the best over {} games: {verdict}",
            100.0 * g.score,
            100.0 * g.half,
            g.games
        );
        if g.passed() {
            best = candidate;
            save(&best, &best_path)?;
        }
    }
    Ok(best)
}
