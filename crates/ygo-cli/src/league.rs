//! `ygo league run` and `ygo league table` (M7, `docs/m7-plan.md`): the
//! league's games played by a bot on both sides, the league file written
//! after every generation, and a Markdown log beside it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ygo_data::{Bundle, Deck};
use ygo_engine::{GameResult, WinReason};
use ygo_train::league::{Arena, League, Note, Results, Settings};

use crate::bots::Bots;
use crate::decks;
use crate::sim::{SimOptions, play_games};

/// The league's games: `bot` plays both decks.
pub struct BotArena<'a> {
    pub bots: &'a Bots,
    pub bot: &'a str,
    pub threads: usize,
    pub max_turns: u32,
}

impl Arena for BotArena<'_> {
    fn play(&self, a: &Deck, b: &Deck, games: u32, seed: u64) -> Result<Results, String> {
        let opts = SimOptions {
            deck_a: a,
            deck_b: b,
            bot_a: self.bot,
            bot_b: self.bot,
            games,
            seed,
            max_turns: self.max_turns,
            record_dir: None,
            threads: self.threads,
        };
        let mut r: Results = [[0; 3]; 2];
        for p in play_games(self.bots, &opts).map_err(|e| e.to_string())? {
            let slot = match p.result {
                // A deck-out win counts a half, like a draw.
                GameResult::Win {
                    reason: WinReason::DeckOut,
                    ..
                }
                | GameResult::Draw { .. } => 2,
                GameResult::Win { winner, .. } => winner.index(),
            };
            r[p.first.index()][slot] += 1;
        }
        Ok(r)
    }
}

pub struct RunOptions {
    pub league: PathBuf,
    pub generations: u32,
    /// For a new league: its settings, seed and the shipped decks.
    pub settings: Settings,
    pub seed: u64,
    pub decks: PathBuf,
}

/// The shipped decks of `dir`, by file name, each checked as `ygo sim`
/// checks a deck.
fn shipped(dir: &Path, bundle: &Bundle) -> Result<Vec<(String, Deck)>, Box<dyn std::error::Error>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let name = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok((name, decks::load_legal(&p, bundle)?))
        })
        .collect()
}

/// One generation in a line: games, leader, the hall, who left and came.
fn summary(league: &League, note: &Note, secs: f64) -> String {
    let name = |id: u32| league.agents[id as usize].name();
    let names = |ids: &[u32]| ids.iter().map(|&i| name(i)).collect::<Vec<_>>().join(", ");
    let mut s = format!(
        "generation {}: {} games + {} exploiter games in {secs:.0} s; leader {}",
        note.generation,
        note.games,
        note.exploiter_games,
        name(note.leader)
    );
    if let Some(h) = note.joined_hall {
        let _ = write!(s, "; joined the hall: {}", name(h));
    }
    let _ = write!(
        s,
        "; left: {}; born: {}",
        names(&note.eliminated),
        names(&note.born)
    );
    match note.exploiter {
        Some(e) => {
            let _ = write!(s, "; exploiter {}", name(e));
        }
        None => s.push_str("; no exploiter beat its parent"),
    }
    s
}

/// Starts the league file or resumes it, and runs `generations` more.
pub fn run(
    arena: &BotArena,
    bundle: &Bundle,
    opts: RunOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut league = if opts.league.exists() {
        League::load(&opts.league)?
    } else {
        if let Some(dir) = opts.league.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let league = League::new(
            shipped(&opts.decks, bundle)?,
            opts.settings,
            opts.seed,
            bundle,
        );
        league.save(&opts.league)?;
        league
    };
    let log = opts.league.with_extension("md");
    for _ in 0..opts.generations {
        let started = Instant::now();
        let note = league.run_generation(arena, bundle)?;
        league.save(&opts.league)?;
        let line = summary(&league, &note, started.elapsed().as_secs_f64());
        println!("{line}");
        let mut text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = write!(
            text,
            "## Generation {}\n\n{line}\n\n{}\n",
            note.generation,
            league.render_table()
        );
        std::fs::write(&log, text).map_err(|e| format!("{}: {e}", log.display()))?;
    }
    Ok(())
}

/// Prints the table; with `export`, writes the best `top` non-fixed decks
/// there in the `data/decks` format.
pub fn table(
    path: &Path,
    export: Option<&Path>,
    top: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let league = League::load(path)?;
    println!(
        "league {} after {} generations\n",
        path.display(),
        league.generation
    );
    print!("{}", league.render_table());
    if let Some(dir) = export {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for row in league
            .table()
            .into_iter()
            .filter(|r| r.kind != ygo_train::league::Kind::Fixed)
            .take(top)
        {
            let deck = &league.agents[row.id as usize].deck;
            let file = dir.join(format!(
                "league_{}_{}.json",
                league.agents[row.id as usize].lineage, row.id
            ));
            let text = serde_json::to_string_pretty(deck).map_err(|e| e.to_string())? + "\n";
            std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
            println!("wrote {}", file.display());
        }
    }
    Ok(())
}
