//! `ygo audit`: how well a heuristic bot plays its cards. Two bots play
//! each other; at every decision with an activation, an answer or a card
//! choice in it, each option is tried on the real game and played on to
//! the next open decision (`HeuristicPlayer::review`). The gap between the
//! best option and the one the bot took is its regret, in points of card
//! value (1,000 LP is 20 points). Regret is booked to the card involved:
//! the one activated, the one it should have activated, or the one whose
//! question or choice it answered.

use std::collections::BTreeMap;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_ai::Player;
use ygo_data::{CardId, Deck};
use ygo_engine::{Action, CardIndex, DeckList, Game, GameSetup, PlayerView};

use crate::bots::Bots;

/// A regret above this many points (500 LP, or a card worth 10) counts as
/// a mistake.
pub const MISTAKE: f64 = 10.0;
/// A regret this large means the option won or lost the game outright (a
/// finished game scores a million points either way): a decisive mistake.
/// Its regret is counted as `DECISIVE_REGRET` in the sums.
pub const DECISIVE: f64 = 1e5;
pub const DECISIVE_REGRET: f64 = 100.0;

pub struct AuditOptions<'a> {
    /// Decks to draw both sides from; random pool decks when empty.
    pub decks: &'a [Deck],
    pub games: u32,
    pub seed: u64,
    pub style: &'a str,
    /// Audit the lookahead bot instead of the plain one.
    pub lookahead: bool,
    pub max_turns: u32,
    pub threads: usize,
}

/// What the bot did with a card when it went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// It activated the card.
    Activated,
    /// It did something else while activating the card was better.
    Held,
    /// It answered the card's question or choice.
    Answered,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Line {
    pub decisions: u32,
    pub mistakes: u32,
    /// Mistakes that threw away a win or walked into a loss.
    pub decisive: u32,
    pub regret: f64,
}

impl Line {
    fn note(&mut self, regret: f64) {
        self.decisions += 1;
        self.mistakes += u32::from(regret > MISTAKE);
        self.decisive += u32::from(regret > DECISIVE);
        self.regret += regret.min(DECISIVE_REGRET);
    }

    fn add(&mut self, other: &Line) {
        self.decisions += other.decisions;
        self.mistakes += other.mistakes;
        self.decisive += other.decisive;
        self.regret += other.regret;
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub games: u32,
    pub total: Line,
    pub by_card: BTreeMap<(CardId, Kind), Line>,
}

impl Report {
    fn add(&mut self, other: Report) {
        self.games += other.games;
        self.total.add(&other.total);
        for (k, l) in other.by_card {
            self.by_card.entry(k).or_default().add(&l);
        }
    }
}

/// A random 40-card Main Deck from the pool, at most 3 copies of a card.
fn random_deck(bots: &Bots, rng: &mut ChaCha8Rng) -> DeckList {
    let pool = &bots.shared.pool;
    let ids: Vec<CardId> = (0..pool.len() as u16)
        .map(|i| pool.get(CardIndex(i)))
        .filter(|s| s.is_main_deck())
        .map(|s| s.id)
        .collect();
    let mut main = Vec::with_capacity(40);
    while main.len() < 40 {
        let id = ids[(rng.next_u64() % ids.len() as u64) as usize];
        if main.iter().filter(|&&c| c == id).count() < 3 {
            main.push(id);
        }
    }
    DeckList {
        main,
        extra: Vec::new(),
    }
}

fn deck_list(deck: &Deck) -> DeckList {
    let expand = |entries: &[ygo_data::DeckEntry]| {
        entries
            .iter()
            .flat_map(|e| std::iter::repeat_n(e.id, e.count))
            .collect()
    };
    DeckList {
        main: expand(&deck.main),
        extra: expand(&deck.extra),
    }
}

/// The card a reviewed decision is about, and how.
fn subject(g: &Game, chosen: &Action, best: &Action) -> Option<(CardId, Kind)> {
    let id = |c| g.stats(c).id;
    match (chosen, best) {
        (Action::Activate { card, .. }, _) => Some((id(*card), Kind::Activated)),
        (_, Action::Activate { card, .. }) => Some((id(*card), Kind::Held)),
        _ => g.resolving_card().map(|c| (id(c), Kind::Answered)),
    }
}

fn audit_game(bots: &Bots, opts: &AuditOptions, index: u32) -> Result<Report, String> {
    let seed = opts
        .seed
        .wrapping_mul(1_000_003)
        .wrapping_add(u64::from(index));
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let decks = if opts.decks.is_empty() {
        [random_deck(bots, &mut rng), random_deck(bots, &mut rng)]
    } else {
        let n = opts.decks.len() as u64;
        [
            deck_list(&opts.decks[(rng.next_u64() % n) as usize]),
            deck_list(&opts.decks[(rng.next_u64() % n) as usize]),
        ]
    };
    let mut setup = GameSetup::new(decks, rng.next_u64());
    setup.max_turns = opts.max_turns;
    let mut game = Game::new(bots.shared.clone(), &setup).map_err(|e| e.to_string())?;
    let mut players = [
        bots.heuristic(opts.style, opts.lookahead, rng.next_u64())?,
        bots.heuristic(opts.style, opts.lookahead, rng.next_u64())?,
    ];
    let mut report = Report {
        games: 1,
        ..Report::default()
    };
    while let Some(view) = PlayerView::new(&game) {
        let p = view.player();
        let action = players[p.index()].choose(&view);
        if let Some((chosen, best_value, best)) =
            players[p.index()].review(&game, &action, rng.next_u64())
        {
            let regret = (best_value - chosen).max(0.0);
            report.total.note(regret);
            if let Some(key) = subject(&game, &action, &best) {
                report.by_card.entry(key).or_default().note(regret);
            }
        }
        game.apply(&action).map_err(|e| e.to_string())?;
    }
    Ok(report)
}

pub fn run(bots: &Bots, opts: &AuditOptions) -> Result<Report, String> {
    let threads = opts.threads.max(1) as u32;
    let parts: Vec<Result<Report, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut report = Report::default();
                    let mut i = t;
                    while i < opts.games {
                        report.add(audit_game(bots, opts, i)?);
                        i += threads;
                    }
                    Ok(report)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("audit thread"))
            .collect()
    });
    let mut total = Report::default();
    for part in parts {
        total.add(part?);
    }
    Ok(total)
}

/// The report as text: totals, then the cards with the most regret
/// (decisive mistakes first).
pub fn print(bots: &Bots, report: &Report, top: usize) {
    let games = f64::from(report.games.max(1));
    let t = &report.total;
    println!(
        "{} games, {} decisions reviewed, {} mistakes ({:.1}%), {} decisive, regret {:.1} points a game",
        report.games,
        t.decisions,
        t.mistakes,
        100.0 * f64::from(t.mistakes) / f64::from(t.decisions.max(1)),
        t.decisive,
        t.regret / games,
    );
    let mut lines: Vec<_> = report.by_card.iter().collect();
    lines.sort_by(|a, b| {
        b.1.decisive
            .cmp(&a.1.decisive)
            .then(b.1.regret.total_cmp(&a.1.regret))
    });
    println!(
        "{:<42} {:<9} {:>9} {:>8} {:>6} {:>8} {:>8}",
        "card", "how", "decisions", "mistakes", "share", "decisive", "regret"
    );
    for ((id, kind), l) in lines.into_iter().take(top) {
        let name = bots
            .shared
            .pool
            .index_of(*id)
            .map_or_else(|| id.to_string(), |i| bots.shared.pool.get(i).name.clone());
        println!(
            "{:<42} {:<9} {:>9} {:>8} {:>5.0}% {:>8} {:>8.2}",
            name,
            format!("{kind:?}"),
            l.decisions,
            l.mistakes,
            100.0 * f64::from(l.mistakes) / f64::from(l.decisions.max(1)),
            l.decisive,
            l.regret / games,
        );
    }
}
