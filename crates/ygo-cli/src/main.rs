//! `ygo`: quick access to everything.

use ygo_cli::{bots, decks, play, replay, sim, tui};

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
    /// Playstyles for the heuristic bots.
    #[arg(long, global = true, default_value = "config/styles.toml")]
    styles: PathBuf,
    /// Card values for the heuristic bots.
    #[arg(long, global = true, default_value = "config/card_values.toml")]
    card_values: PathBuf,
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
        /// Player A: `random` or `heuristic:<style>`.
        #[arg(long, default_value = "random")]
        bot_a: String,
        /// Player B: `random` or `heuristic:<style>`.
        #[arg(long, default_value = "random")]
        bot_b: String,
        /// Number of games; each player goes first in half of them.
        #[arg(short = 'n', long, default_value_t = 100)]
        games: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// A game still running after this many turns is a draw.
        #[arg(long, default_value_t = ygo_engine::GameSetup::DEFAULT_MAX_TURNS)]
        max_turns: u32,
        /// Write every game's record (JSON Lines) into this directory.
        #[arg(long)]
        record: Option<PathBuf>,
    },
    /// Play against a bot in a full-screen terminal UI.
    Play {
        /// Your deck.
        #[arg(long)]
        deck: PathBuf,
        /// The bot's deck; defaults to a copy of yours.
        #[arg(long)]
        bot_deck: Option<PathBuf>,
        /// The opponent: `random` or `heuristic:<style>` (see config/styles.toml).
        #[arg(long, default_value = bots::DEFAULT_BOT)]
        bot: String,
        /// Shuffle seed; taken from the clock when left out.
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, value_enum, default_value_t = play::First::Random)]
        first: play::First,
        /// A game still running after this many turns is a draw.
        #[arg(long, default_value_t = ygo_engine::GameSetup::DEFAULT_MAX_TURNS)]
        max_turns: u32,
        /// Record file; defaults to records/human/<time>-<seed>.jsonl.
        #[arg(long)]
        record: Option<PathBuf>,
    },
    /// Step through a recorded game in a full-screen terminal UI.
    Replay {
        /// A record written by `ygo play` or `ygo sim --record`.
        record: PathBuf,
        /// Whose cards are shown: one player's view, or everything.
        #[arg(long, value_enum, default_value_t = replay::View::All)]
        view: replay::View,
        /// Start at this turn.
        #[arg(long, default_value_t = 1)]
        turn: u32,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err}");
            let mut source = err.source();
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
fn run(cli: Cli) -> Result<bool, Box<dyn std::error::Error>> {
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
            bot_a,
            bot_b,
            games,
            seed,
            max_turns,
            record,
        } => {
            if let Some(dir) = &record {
                std::fs::create_dir_all(dir).map_err(|source| DataError::Io {
                    path: dir.clone(),
                    source,
                })?;
            }
            let bots = bots::Bots::load(&bundle, &cli.styles, &cli.card_values)?;
            let deck_a = decks::load_legal(&deck_a, &bundle)?;
            let deck_b = decks::load_legal(&deck_b, &bundle)?;
            sim::run(
                &bots,
                &sim::SimOptions {
                    deck_a: &deck_a,
                    deck_b: &deck_b,
                    bot_a: &bot_a,
                    bot_b: &bot_b,
                    games,
                    seed,
                    max_turns,
                    record_dir: record.as_deref(),
                },
            )?;
            Ok(true)
        }
        Command::Play {
            deck,
            bot_deck,
            bot,
            seed,
            first,
            max_turns,
            record,
        } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let seed = seed.unwrap_or(now.as_nanos() as u64);
            let bots = bots::Bots::load(&bundle, &cli.styles, &cli.card_values)?;
            let deck = decks::load_legal(&deck, &bundle)?;
            let bot_deck = bot_deck
                .map(|p| decks::load_legal(&p, &bundle))
                .transpose()?;
            let session = play::start(
                &bots,
                &play::PlayOptions {
                    deck: &deck,
                    bot_deck: bot_deck.as_ref(),
                    bot: &bot,
                    seed,
                    first,
                    max_turns,
                },
            )?;
            let path = record.unwrap_or_else(|| play::default_record_path(seed, now.as_secs()));
            let mut app = tui::App::new(session);
            let shown = tui::run(&mut app);
            play::save_record(&app.session, &path)?;
            shown?;
            println!("record saved to {}", path.display());
            Ok(true)
        }
        Command::Replay { record, view, turn } => {
            let text = std::fs::read_to_string(&record).map_err(|source| DataError::Io {
                path: record.clone(),
                source,
            })?;
            let parsed = ygo_engine::GameRecord::from_jsonl(&text)?;
            let shared =
                ygo_engine::Shared::new(bundle.rules.clone(), &bundle.cards, ygo_cards::registry());
            let mut session = replay::ReplaySession::new(&shared, parsed, view)?;
            session.jump_to_turn(turn);
            tui::run(&mut replay::ReplayApp {
                session,
                quit: false,
            })?;
            Ok(true)
        }
    }
}

fn load_collection(path: &Path, bundle: &Bundle) -> Result<Collection, DataError> {
    let collection = Collection::load(path)?;
    collection.validate(&bundle.cards)?;
    Ok(collection)
}
