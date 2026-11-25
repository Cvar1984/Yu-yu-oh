//! `ygo`: quick access to everything.

use ygo_cli::{bots, decks, play, replay, sim, train, tui};

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
    /// The trained network the `network` and `network-search` bots play
    /// with.
    #[arg(long, global = true, default_value = "data/networks/best.bpk")]
    network: PathBuf,
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
        /// Player A: `network` (the shipped network), `network-search`, `random`, `neural:<checkpoint>` or `neural-search:<checkpoint>`.
        #[arg(long, default_value = "random")]
        bot_a: String,
        /// Player B: `network` (the shipped network), `network-search`, `random`, `neural:<checkpoint>` or `neural-search:<checkpoint>`.
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
        /// Games played at once; results do not depend on it.
        #[arg(long, default_value_t = 1)]
        threads: usize,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Train the neural bot to imitate the bots of recorded games
    /// (`ygo sim --record`): their choices and who won (M6 step 4).
    Imitate {
        /// Directories of game records (`*.jsonl`).
        #[arg(required = true)]
        records: Vec<PathBuf>,
        /// Where to save the trained network.
        #[arg(long)]
        out: PathBuf,
        /// Start from this network instead of a new one.
        #[arg(long)]
        from: Option<PathBuf>,
        /// Learn from players whose label starts with one of these
        /// (comma-separated); all but `random` when empty.
        #[arg(long, default_value = "")]
        teachers: String,
        #[arg(long, default_value_t = 3)]
        epochs: usize,
        #[arg(long, default_value_t = 128)]
        batch: usize,
        #[arg(long, default_value_t = 1e-3)]
        learning_rate: f64,
        /// One record in this many is held out to measure agreement.
        #[arg(long, default_value_t = 10)]
        holdout: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Self-play training (AlphaZero): the best network's search plays
    /// itself, a candidate learns from the games, and it replaces the best
    /// only if it beats it (the M6 gate).
    Train {
        /// Decks for the mirror games, each played equally often.
        #[arg(required = true)]
        decks: Vec<PathBuf>,
        /// The network to start from [default: the --network file].
        #[arg(long)]
        from: Option<PathBuf>,
        /// Directory for `best.bpk` and each generation's `gen-<g>.bpk`.
        #[arg(long, default_value = "checkpoints/train")]
        out: PathBuf,
        #[arg(long, default_value_t = 5)]
        generations: usize,
        /// Self-play games per generation.
        #[arg(long, default_value_t = 800)]
        games: u32,
        /// Generations of games the candidate trains on.
        #[arg(long, default_value_t = 3)]
        buffer: usize,
        /// Turns in which moves are drawn by the search's visits.
        #[arg(long, default_value_t = 4)]
        explore_turns: u32,
        /// Games of candidate against best (the gate).
        #[arg(long, default_value_t = 2000)]
        gate_games: u32,
        #[arg(long, default_value_t = 2)]
        epochs: usize,
        #[arg(long, default_value_t = 128)]
        batch: usize,
        #[arg(long, default_value_t = 3e-4)]
        learning_rate: f64,
        #[arg(long, default_value_t = 10)]
        holdout: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 4)]
        threads: usize,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Play against a bot in a full-screen terminal UI.
    Play {
        /// Your deck.
        #[arg(long)]
        deck: PathBuf,
        /// The bot's deck; defaults to a copy of yours.
        #[arg(long)]
        bot_deck: Option<PathBuf>,
        /// The opponent: `network` (the trained network, the default),
        /// `network-search`, `random`, or any bot name `ygo sim` takes.
        #[arg(long, default_value = "network")]
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
        #[command(flatten)]
        search: SearchArgs,
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

/// How hard search bots (`network-search`, `neural-search:`) think.
#[derive(clap::Args)]
struct SearchArgs {
    /// Network evaluations per decision [default: 100, or no limit with
    /// --think].
    #[arg(long)]
    iterations: Option<u32>,
    /// Seconds a search bot thinks per decision (less if --iterations runs
    /// out first).
    #[arg(long)]
    think: Option<f64>,
    /// Weight of the network's prior against the average value (PUCT).
    #[arg(long, default_value_t = ygo_ai::mcts::MctsSettings::default().exploration)]
    exploration: f64,
}

impl SearchArgs {
    fn apply(&self, bots: &mut bots::Bots) -> Result<(), String> {
        let m = &mut bots.mcts;
        m.exploration = self.exploration;
        m.iterations = match (self.iterations, self.think) {
            (Some(n), _) => n,
            (None, Some(_)) => u32::MAX,
            (None, None) => m.iterations,
        };
        if let Some(secs) = self.think {
            let limit = std::time::Duration::try_from_secs_f64(secs)
                .map_err(|_| format!("--think {secs}: not a number of seconds"))?;
            m.time_limit = Some(limit);
        }
        Ok(())
    }
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
            threads,
            search,
        } => {
            if let Some(dir) = &record {
                std::fs::create_dir_all(dir).map_err(|source| DataError::Io {
                    path: dir.clone(),
                    source,
                })?;
            }
            let mut bots = bots::Bots::new(&bundle);
            load_network(&mut bots, &cli.network)?;
            search.apply(&mut bots)?;
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
                    threads,
                },
            )?;
            Ok(true)
        }
        Command::Imitate {
            records,
            out,
            from,
            teachers,
            epochs,
            batch,
            learning_rate,
            holdout,
            seed,
        } => {
            imitate(
                &bundle,
                &records,
                &out,
                from.as_deref(),
                &teachers,
                &ygo_train::imitation::Options {
                    epochs,
                    batch,
                    learning_rate,
                    holdout,
                    seed,
                    ..Default::default()
                },
            )?;
            Ok(true)
        }
        Command::Train {
            decks: deck_files,
            from,
            out,
            generations,
            games,
            buffer,
            explore_turns,
            gate_games,
            epochs,
            batch,
            learning_rate,
            holdout,
            seed,
            threads,
            search,
        } => {
            let mut bots = bots::Bots::new(&bundle);
            search.apply(&mut bots)?;
            let decks = deck_files
                .iter()
                .map(|d| decks::load_legal(d, &bundle))
                .collect::<Result<Vec<_>, _>>()?;
            let from = from.unwrap_or_else(|| cli.network.clone());
            let start = ygo_ai::neural::Net::load(&from, &ygo_ai::neural::cpu())?;
            train::run(
                &bots,
                start,
                &train::TrainOptions {
                    decks: &decks,
                    out: &out,
                    generations,
                    games,
                    buffer,
                    explore_turns,
                    gate_games,
                    search: bots.mcts.clone(),
                    train: ygo_train::imitation::Options {
                        epochs,
                        batch,
                        learning_rate,
                        holdout,
                        seed,
                        ..Default::default()
                    },
                    seed,
                    threads,
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
            search,
        } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let seed = seed.unwrap_or(now.as_nanos() as u64);
            let mut bots = bots::Bots::new(&bundle);
            load_network(&mut bots, &cli.network)?;
            search.apply(&mut bots)?;
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

/// The shipped network, when its file exists; the `network` bots refuse
/// to play without it.
fn load_network(bots: &mut bots::Bots, path: &Path) -> Result<(), String> {
    if path.exists() {
        bots.network = Some(ygo_ai::neural::Net::load(path, &ygo_ai::neural::cpu())?);
    }
    Ok(())
}

fn load_collection(path: &Path, bundle: &Bundle) -> Result<Collection, DataError> {
    let collection = Collection::load(path)?;
    collection.validate(&bundle.cards)?;
    Ok(collection)
}

/// `ygo imitate`: reads the records, trains, saves the network.
fn imitate(
    bundle: &ygo_data::Bundle,
    dirs: &[PathBuf],
    out: &std::path::Path,
    from: Option<&std::path::Path>,
    teachers: &str,
    opts: &ygo_train::imitation::Options,
) -> Result<(), Box<dyn std::error::Error>> {
    use ygo_ai::neural::{Net, cpu};
    let mut records = Vec::new();
    for dir in dirs {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f)?;
            records.push(
                ygo_engine::GameRecord::from_jsonl(&text)
                    .map_err(|e| format!("{}: {e}", f.display()))?,
            );
        }
    }
    let prefixes: Vec<&str> = teachers.split(',').filter(|t| !t.is_empty()).collect();
    let teacher = |label: &str| {
        if prefixes.is_empty() {
            label != "random"
        } else {
            prefixes.iter().any(|p| label.starts_with(p))
        }
    };
    let shared =
        ygo_engine::Shared::new(bundle.rules.clone(), &bundle.cards, ygo_cards::registry());
    let net = match from {
        Some(path) => Net::load(path, &cpu())?,
        None => Net::new(&cpu()),
    };
    println!("{} records from {} directories", records.len(), dirs.len());
    let net = ygo_train::imitation::train(net, &records, &shared, &teacher, opts, &mut |e| {
        println!(
            "epoch {}: {} samples, loss {:.3}, held-out agreement {:.1}% and value error {:.3} ({} decisions), {:.0}s",
            e.epoch,
            e.samples,
            e.loss,
            100.0 * e.agreement,
            e.value_error,
            e.held_out,
            e.seconds
        );
    })?;
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    net.save(out)?;
    println!("saved {}", out.display());
    Ok(())
}
