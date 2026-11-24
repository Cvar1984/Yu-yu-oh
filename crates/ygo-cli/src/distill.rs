//! `ygo distill`: rounds of DAgger (`ygo_train::distill`). Each round the
//! current network plays the opponents (each one equally often, on both
//! sides, going first and second), the network-guided search with the
//! same network labels its decisions, and the network trains on every
//! labelled game so far. A checkpoint is saved after each round.

use std::path::{Path, PathBuf};

use ygo_ai::SearchPlayer;
use ygo_ai::neural::{Net, NeuralPlayer, cpu};
use ygo_data::Deck;
use ygo_engine::{DeckList, GameResult, GameSetup, PlayerId};
use ygo_train::distill::{self, Labelled};
use ygo_train::imitation::{self, Options};

use crate::bots::Bots;

pub struct DistillOptions<'a> {
    pub deck: &'a Deck,
    pub opponents: &'a [String],
    pub rounds: usize,
    /// Games per round.
    pub games: u32,
    /// Chance of playing the teacher's move, per round (the last repeats).
    pub mix: &'a [f64],
    pub train: Options,
    pub out: &'a Path,
    pub seed: u64,
    pub threads: usize,
}

/// The checkpoint of round `r`, next to `out`.
pub fn round_path(out: &Path, round: usize) -> PathBuf {
    let stem = out.file_stem().and_then(|s| s.to_str()).unwrap_or("net");
    out.with_file_name(format!("{stem}-r{round}.bpk"))
}

pub fn run(bots: &Bots, mut net: Net, opts: &DistillOptions) -> Result<Net, String> {
    for name in opts.opponents {
        bots.make(name, 0)?;
    }
    let n = opts.opponents.len().max(1) as u32;
    let decks = [DeckList::from(opts.deck), DeckList::from(opts.deck)];
    let mut data: Vec<Labelled> = Vec::new();
    for round in 1..=opts.rounds {
        let mix = opts.mix[(round - 1).min(opts.mix.len() - 1)];
        let started = std::time::Instant::now();
        let play = |i: u32| -> Result<Labelled, String> {
            let seed = opts
                .seed
                .wrapping_add(round as u64 * 1_000_000)
                .wrapping_add(u64::from(i));
            let name = &opts.opponents[(i % n) as usize];
            let side = PlayerId(((i / n) % 2) as u8);
            let mut setup = GameSetup::new(decks.clone(), seed);
            setup.first_player = Some(PlayerId(((i / (2 * n)) % 2) as u8));
            let mut student = NeuralPlayer::new(net.clone(), cpu(), 0.0, seed);
            let prior = NeuralPlayer::new(net.clone(), cpu(), 0.0, seed);
            let rollout = bots.heuristic("beatdown", false, seed ^ 0x7)?;
            let mut teacher =
                SearchPlayer::new(rollout, bots.search.clone(), seed).with_prior(Box::new(prior));
            let mut opponent = bots.make(name, seed ^ 0xB)?;
            let mut labels = [name.clone(), name.clone()];
            labels[side.index()] = "student".to_string();
            distill::play_labelled(
                bots.shared.clone(),
                &setup,
                &mut student,
                &mut teacher,
                opponent.as_mut(),
                side,
                labels,
                mix,
            )
            .map_err(|e| e.to_string())
        };
        let threads = opts.threads.clamp(1, opts.games.max(1) as usize);
        let played: Vec<Result<Vec<Labelled>, String>> = std::thread::scope(|scope| {
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
                .map(|h| h.join().expect("a game panicked"))
                .collect()
        });
        let mut games = Vec::new();
        for p in played {
            games.extend(p?);
        }
        // How the student's side did against each opponent (the teacher
        // played a `mix` share of its moves).
        println!(
            "round {round}: {} games, mix {mix}, {:.0}s",
            games.len(),
            started.elapsed().as_secs_f64()
        );
        for name in opts.opponents {
            let (mut won, mut total) = (0u32, 0u32);
            for g in games
                .iter()
                .filter(|g| &g.record.header.players[1 - g.student.index()] == name)
            {
                total += 1;
                if matches!(g.record.result, Some(GameResult::Win { winner, .. }) if winner == g.student)
                {
                    won += 1;
                }
            }
            println!("  vs {name}: {won}/{total}");
        }
        data.extend(games);
        let shared = bots.shared.clone();
        let data = &data;
        net = imitation::train_on(
            net,
            data.len(),
            &|i| distill::samples(&data[i], &shared),
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
        if let Some(dir) = opts.out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let path = round_path(opts.out, round);
        net.save(&path)?;
        net.save(opts.out)?;
        println!("  saved {} and {}", path.display(), opts.out.display());
    }
    Ok(net)
}
