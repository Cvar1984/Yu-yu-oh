//! `ygo sim`: bot versus bot, with win rates and games per second.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use ygo_ai::{Player, RandomPlayer, play_out};
use ygo_data::{Bundle, DataError, Deck};
use ygo_engine::{DeckList, Game, GameResult, GameSetup, PlayerId, Shared};

pub struct SimOptions<'a> {
    pub deck_a: &'a Path,
    pub deck_b: &'a Path,
    pub games: u32,
    pub seed: u64,
    pub max_turns: u32,
}

/// Outcome counts, split by who went first.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    /// Indexed by first player, then by [wins for A, wins for B, draws].
    pub by_first: [[u32; 3]; 2],
    pub turns: u64,
}

impl Tally {
    fn record(&mut self, first: PlayerId, result: GameResult, turns: u32) {
        let slot = match result {
            GameResult::Win { winner, .. } => winner.index(),
            GameResult::Draw { .. } => 2,
        };
        self.by_first[first.index()][slot] += 1;
        self.turns += u64::from(turns);
    }

    fn total(&self, slot: usize) -> u32 {
        self.by_first[0][slot] + self.by_first[1][slot]
    }
}

pub fn run(bundle: &Bundle, opts: &SimOptions) -> Result<(), DataError> {
    let shared = Arc::new(Shared::new(bundle.rules.clone(), &bundle.cards));
    let deck_a = Deck::load(opts.deck_a)?;
    let deck_b = Deck::load(opts.deck_b)?;
    let decks = [DeckList::from(&deck_a), DeckList::from(&deck_b)];

    let mut tally = Tally::default();
    let mut actions = 0u64;
    let started = Instant::now();
    for i in 0..opts.games {
        let mut setup = GameSetup::new(decks.clone(), opts.seed.wrapping_add(u64::from(i)));
        setup.first_player = Some(PlayerId((i % 2) as u8));
        setup.max_turns = opts.max_turns;
        let mut game = match Game::new(shared.clone(), &setup) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(2);
            }
        };
        let mut a = RandomPlayer::new(setup.seed ^ 0xA);
        let mut b = RandomPlayer::new(setup.seed ^ 0xB);
        let mut players: [&mut dyn Player; 2] = [&mut a, &mut b];
        let (result, record) = play_out(&mut game, &mut players);
        actions += record.len() as u64;
        tally.record(setup.first_player.unwrap(), result, game.turn());
    }
    let elapsed = started.elapsed().as_secs_f64();

    println!("A: {}", deck_a.name);
    println!("B: {}", deck_b.name);
    println!(
        "games: {}  random vs random  seed {}",
        opts.games, opts.seed
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
    Ok(())
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
