//! `ygo`: quick access to everything.

mod sim;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ygo_data::{Bundle, Collection, DataError, Deck, check_deck};

#[derive(Parser)]
#[command(
    name = "ygo",
    version,
    about = "Yu-Gi-Oh! LOB/MRD/SRL engine, bot and deck builder"
)]
struct Cli {
    /// House rules file; it also names the card and ban list files.
    #[arg(long, global = true, default_value = "config/rules.toml")]
    rules: PathBuf,
    /// Directory that relative paths in the rules file resolve against.
    #[arg(long, global = true, default_value = ".")]
    data_root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Load the rules, card pool and ban list and print a summary.
    CheckData {
        /// Also load and validate this collection file.
        #[arg(long)]
        collection: Option<PathBuf>,
    },
    /// Check a deck against the card pool, ban list, rules and optionally a collection.
    CheckDeck {
        deck: PathBuf,
        /// Only allow cards and copies from this collection file.
        #[arg(long)]
        collection: Option<PathBuf>,
    },
    /// Play bot against bot and print win rates and speed.
    Sim {
        deck_a: PathBuf,
        deck_b: PathBuf,
        /// Number of games; each player goes first in half of them.
        #[arg(short = 'n', long, default_value_t = 100)]
        games: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// A game still running after this many turns is a draw.
        #[arg(long, default_value_t = ygo_engine::GameSetup::DEFAULT_MAX_TURNS)]
        max_turns: u32,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err}");
            let mut source = std::error::Error::source(&err);
            while let Some(s) = source {
                eprintln!("  caused by: {s}");
                source = s.source();
            }
            ExitCode::from(2)
        }
    }
}

/// `Ok(true)` when the checked data or deck is clean, `Ok(false)` when problems
/// were printed, `Err` when a file could not be loaded.
fn run(cli: Cli) -> Result<bool, DataError> {
    let bundle = Bundle::load(&cli.rules, &cli.data_root)?;
    match cli.command {
        Command::CheckData { collection } => {
            println!("rules:   {}", cli.rules.display());
            println!("cards:   {} loaded", bundle.cards.len());
            for (kind, count) in bundle.cards.count_by_kind() {
                println!("  {count:>4} {kind}");
            }
            println!("banlist: {} limited cards", bundle.banlist.limits.len());
            if let Some(path) = collection {
                let c = load_collection(&path, &bundle)?;
                println!(
                    "collection: {} cards, {} copies",
                    c.unique_cards(),
                    c.total_copies()
                );
            }
            Ok(true)
        }
        Command::CheckDeck { deck, collection } => {
            let deck = Deck::load(&deck)?;
            let collection = collection
                .map(|path| load_collection(&path, &bundle))
                .transpose()?;
            let problems = check_deck(
                &deck,
                &bundle.cards,
                &bundle.banlist,
                &bundle.rules,
                collection.as_ref(),
            );
            println!(
                "{}: {} main, {} extra",
                deck.name,
                deck.main_size(),
                deck.extra_size()
            );
            if problems.is_empty() {
                println!("legal");
                return Ok(true);
            }
            for p in &problems {
                println!("  - {p}");
            }
            println!("{} problem(s)", problems.len());
            Ok(false)
        }
        Command::Sim {
            deck_a,
            deck_b,
            games,
            seed,
            max_turns,
        } => {
            sim::run(
                &bundle,
                &sim::SimOptions {
                    deck_a: &deck_a,
                    deck_b: &deck_b,
                    games,
                    seed,
                    max_turns,
                },
            )?;
            Ok(true)
        }
    }
}

fn load_collection(path: &Path, bundle: &Bundle) -> Result<Collection, DataError> {
    let collection = Collection::load(path)?;
    collection.validate(&bundle.cards)?;
    Ok(collection)
}
