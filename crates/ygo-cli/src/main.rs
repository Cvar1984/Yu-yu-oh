//! `ygo`: quick access to everything.

use ygo_cli::{audit, bots, decks, distill, play, replay, sim, tui};

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
        /// Player A: `random`, `lookahead:<style>`, `heuristic:<style>`, `search[:<style>]`, `neural:<checkpoint>`, `neural-lookahead:<checkpoint>` or `neural-search:<checkpoint>`.
        #[arg(long, default_value = "random")]
        bot_a: String,
        /// Player B: `random`, `lookahead:<style>`, `heuristic:<style>`, `search[:<style>]`, `neural:<checkpoint>`, `neural-lookahead:<checkpoint>` or `neural-search:<checkpoint>`.
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
    /// How well a heuristic bot plays its cards: at each activation,
    /// answer or card choice, every option is tried on the real game and
    /// the gap to the best is booked to the card (see `audit.rs`).
    Audit {
        /// Decks for both sides; random 40-card pool decks when none.
        decks: Vec<PathBuf>,
        #[arg(short = 'n', long, default_value_t = 200)]
        games: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Heuristic style of both bots.
        #[arg(long, default_value = "beatdown")]
        style: String,
        /// Audit the lookahead bot instead of the plain heuristic one.
        #[arg(long)]
        lookahead: bool,
        /// A game still running after this many turns is a draw.
        #[arg(long, default_value_t = ygo_engine::GameSetup::DEFAULT_MAX_TURNS)]
        max_turns: u32,
        #[arg(long, default_value_t = 1)]
        threads: usize,
        /// Cards listed in the report.
        #[arg(long, default_value_t = 40)]
        top: usize,
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
    /// Train the network on its own games against the opponents, the
    /// network-guided search labelling its decisions (DAgger, M6 step 5).
    Distill {
        /// The deck both sides play.
        deck: PathBuf,
        /// The network to start from.
        #[arg(long)]
        from: PathBuf,
        /// Where to save the network (and `<stem>-r<round>.bpk` each round).
        #[arg(long)]
        out: PathBuf,
        /// Opponents, comma-separated; every reference bot when empty.
        #[arg(long, default_value = "")]
        opponents: String,
        #[arg(long, default_value_t = 4)]
        rounds: usize,
        /// Games per round.
        #[arg(long, default_value_t = 1400)]
        games: u32,
        /// Chance of playing the search's move instead of the network's,
        /// per round, comma-separated (the last repeats).
        #[arg(long, default_value = "0.5,0.25,0")]
        mix: String,
        /// Epochs over all labelled games after each round.
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
        /// The opponent: `random`, `heuristic:<style>` (see config/styles.toml)
        /// or `search[:<style>]`.
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

/// How hard `search` bots think.
#[derive(clap::Args)]
struct SearchArgs {
    /// Playouts per decision for `search` bots [default: 200, or no
    /// limit with --think].
    #[arg(long)]
    iterations: Option<u32>,
    /// Seconds a `search` bot thinks per decision (less if --iterations
    /// runs out first).
    #[arg(long)]
    think: Option<f64>,
    /// Turns a `search` bot plays ahead before scoring; 0 plays to the end.
    #[arg(long, default_value_t = ygo_ai::SearchSettings::default().horizon_turns)]
    horizon: u32,
    /// `search` bots play out their samples with the lookahead heuristic bot.
    #[arg(long)]
    rollout_lookahead: bool,
    /// Weight of the network's prior in `neural-search` bots (PUCT).
    #[arg(long, default_value_t = ygo_ai::SearchSettings::default().prior_weight)]
    prior_weight: f64,
}

impl SearchArgs {
    fn apply(&self, bots: &mut bots::Bots) -> Result<(), String> {
        bots.search.iterations = match (self.iterations, self.think) {
            (Some(n), _) => n,
            (None, Some(_)) => u32::MAX,
            (None, None) => bots.search.iterations,
        };
        bots.search.horizon_turns = self.horizon;
        bots.rollout_lookahead = self.rollout_lookahead;
        bots.search.prior_weight = self.prior_weight;
        if let Some(secs) = self.think {
            let limit = std::time::Duration::try_from_secs_f64(secs)
                .map_err(|_| format!("--think {secs}: not a number of seconds"))?;
            bots.search.time_limit = Some(limit);
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
            let mut bots = bots::Bots::load(&bundle, &cli.styles, &cli.card_values)?;
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
        Command::Audit {
            decks,
            games,
            seed,
            style,
            lookahead,
            max_turns,
            threads,
            top,
        } => {
            let bots = bots::Bots::load(&bundle, &cli.styles, &cli.card_values)?;
            let decks = decks
                .iter()
                .map(|p| decks::load_legal(p, &bundle))
                .collect::<Result<Vec<_>, _>>()?;
            let report = audit::run(
                &bots,
                &audit::AuditOptions {
                    decks: &decks,
                    games,
                    seed,
                    style: &style,
                    lookahead,
                    max_turns,
                    threads,
                },
            )?;
            audit::print(&bots, &report, top);
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
        Command::Distill {
            deck,
            from,
            out,
            opponents,
            rounds,
            games,
            mix,
            epochs,
            batch,
            learning_rate,
            holdout,
            seed,
            threads,
            search,
        } => {
            let mut bots = bots::Bots::load(&bundle, &cli.styles, &cli.card_values)?;
            search.apply(&mut bots)?;
            let deck = decks::load_legal(&deck, &bundle)?;
            let opponents: Vec<String> = if opponents.is_empty() {
                let styles = ["beatdown", "control", "stall", "burn", "exodia", "patient"];
                std::iter::once("random".to_string())
                    .chain(styles.iter().map(|s| format!("heuristic:{s}")))
                    .chain(styles.iter().map(|s| format!("lookahead:{s}")))
                    .chain(std::iter::once("search".to_string()))
                    .collect()
            } else {
                opponents.split(',').map(str::to_string).collect()
            };
            let mix = mix
                .split(',')
                .map(|m| {
                    m.trim()
                        .parse::<f64>()
                        .map_err(|e| format!("--mix {m}: {e}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let net = ygo_ai::neural::Net::load(&from, &ygo_ai::neural::cpu())?;
            distill::run(
                &bots,
                net,
                &distill::DistillOptions {
                    deck: &deck,
                    opponents: &opponents,
                    rounds,
                    games,
                    mix: &mix,
                    train: ygo_train::imitation::Options {
                        epochs,
                        batch,
                        learning_rate,
                        holdout,
                        seed,
                        ..Default::default()
                    },
                    out: &out,
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
            let mut bots = bots::Bots::load(&bundle, &cli.styles, &cli.card_values)?;
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
