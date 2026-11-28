//! `ygo sim`: bot versus bot, with win rates and games per second.

use std::path::Path;
use std::time::Instant;

use ygo_ai::{Player, play_game};
use ygo_data::{DataError, Deck};
use ygo_engine::{DeckList, GameResult, GameSetup, PlayerId, WinReason};

use crate::bots::Bots;

pub struct SimOptions<'a> {
    pub deck_a: &'a Deck,
    pub deck_b: &'a Deck,
    /// Bot names, as `Bots::make` reads them.
    pub bot_a: &'a str,
    pub bot_b: &'a str,
    pub games: u32,
    pub seed: u64,
    pub max_turns: u32,
    /// Write each game's record to this directory.
    pub record_dir: Option<&'a Path>,
    /// Games played at once. Each game depends only on its own seed, so the
    /// results are the same for any number.
    pub threads: usize,
}

/// What one game left for the report.
pub struct Played {
    pub first: PlayerId,
    pub result: GameResult,
    pub turns: u32,
    pub actions: usize,
}

/// Outcome counts, split by who went first.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    /// Indexed by first player, then by [wins for A, wins for B, draws].
    pub by_first: [[u32; 3]; 2],
    /// Wins by Life Points, deck-out and Exodia, indexed by winner.
    pub by_reason: [[u32; 3]; 2],
    pub turns: u64,
}

impl Tally {
    fn record(&mut self, first: PlayerId, result: GameResult, turns: u32) {
        let slot = match result {
            GameResult::Win { winner, reason } => {
                let r = match reason {
                    WinReason::LifePoints => 0,
                    WinReason::DeckOut => 1,
                    WinReason::Exodia => 2,
                };
                self.by_reason[winner.index()][r] += 1;
                winner.index()
            }
            GameResult::Draw { .. } => 2,
        };
        self.by_first[first.index()][slot] += 1;
        self.turns += u64::from(turns);
    }

    /// Wins for A, wins for B and draws over all games.
    pub fn total(&self, slot: usize) -> u32 {
        self.by_first[0][slot] + self.by_first[1][slot]
    }
}

/// Plays the games, prints the report and returns the tally.
pub fn run(bots: &Bots, opts: &SimOptions) -> Result<Tally, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let games = play_games(bots, opts)?;
    let mut tally = Tally::default();
    let mut actions = 0u64;
    for p in &games {
        tally.record(p.first, p.result, p.turns);
        actions += p.actions as u64;
    }
    let elapsed = started.elapsed().as_secs_f64();

    println!("A: {}", opts.deck_a.name);
    println!("B: {}", opts.deck_b.name);
    println!(
        "games: {}  {} vs {}  seed {}",
        opts.games, opts.bot_a, opts.bot_b, opts.seed
    );
    report("overall", tally.total(0), tally.total(1), tally.total(2));
    report(
        "A first",
        tally.by_first[0][0],
        tally.by_first[0][1],
        tally.by_first[0][2],
    );
    report(
        "B first",
        tally.by_first[1][0],
        tally.by_first[1][1],
        tally.by_first[1][2],
    );
    for (label, r) in [("A", tally.by_reason[0]), ("B", tally.by_reason[1])] {
        println!(
            "{label:>8} wins by: Life Points {}, deck-out {}, Exodia {}",
            r[0], r[1], r[2]
        );
    }
    println!(
        "mean turns: {:.1}   actions/game: {:.0}",
        tally.turns as f64 / f64::from(opts.games).max(1.0),
        actions as f64 / f64::from(opts.games).max(1.0)
    );
    println!(
        "speed: {:.0} games/s, {:.0} actions/s ({elapsed:.2}s)",
        f64::from(opts.games) / elapsed,
        actions as f64 / elapsed
    );
    Ok(tally)
}

/// Plays the games without a report: game `i` uses seed `seed + i`, and A
/// goes first in the even-numbered ones.
pub fn play_games(
    bots: &Bots,
    opts: &SimOptions,
) -> Result<Vec<Played>, Box<dyn std::error::Error>> {
    let shared = bots.shared.clone();
    // Refuse unknown names before the first game.
    bots.make(opts.bot_a, 0)?;
    bots.make(opts.bot_b, 0)?;
    let decks = [DeckList::from(opts.deck_a), DeckList::from(opts.deck_b)];
    let threads = opts.threads.clamp(1, opts.games.max(1) as usize);
    let play = |i: u32| -> Result<Played, String> {
        let mut setup = GameSetup::new(decks.clone(), opts.seed.wrapping_add(u64::from(i)));
        setup.first_player = Some(PlayerId((i % 2) as u8));
        setup.max_turns = opts.max_turns;
        let mut a = bots.make(opts.bot_a, setup.seed ^ 0xA)?;
        let mut b = bots.make(opts.bot_b, setup.seed ^ 0xB)?;
        let mut players: [&mut dyn Player; 2] = [a.as_mut(), b.as_mut()];
        let labels = [opts.bot_a.to_string(), opts.bot_b.to_string()];
        let (game, record) =
            play_game(shared.clone(), &setup, &mut players, labels).map_err(|e| e.to_string())?;
        if let Some(dir) = opts.record_dir {
            let path = dir.join(format!("game-{}.jsonl", setup.seed));
            std::fs::write(&path, record.to_jsonl())
                .map_err(|source| DataError::Io { path, source }.to_string())?;
        }
        Ok(Played {
            first: setup.first_player.unwrap(),
            result: record.result.expect("a played-out game has a result"),
            turns: game.turn(),
            actions: record.actions.len(),
        })
    };
    // Worker `w` plays games w, w + threads, w + 2 * threads...
    let per_worker: Vec<Result<Vec<Played>, String>> = std::thread::scope(|scope| {
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
    let mut games = Vec::with_capacity(opts.games as usize);
    for played in per_worker {
        games.extend(played?);
    }
    Ok(games)
}

/// Win rate for A with a 95% normal-approximation confidence interval.
fn report(label: &str, a: u32, b: u32, draws: u32) {
    let n = a + b + draws;
    if n == 0 {
        println!("{label:>8}: no games");
        return;
    }
    let p = f64::from(a) / f64::from(n);
    let half = 1.96 * (p * (1.0 - p) / f64::from(n)).sqrt();
    println!(
        "{label:>8}: A {:5.1}% ±{:.1}  (A {a}, B {b}, draws {draws}, n {n})",
        p * 100.0,
        half * 100.0
    );
}
