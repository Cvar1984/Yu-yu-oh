//! `ygo play`: builds a `PlaySession` from command-line choices, runs the
//! terminal UI and saves the game record.

use std::path::{Path, PathBuf};
use ygo_data::{DataError, Deck};
use ygo_engine::{DeckList, GameSetup, PlayerId};

use crate::bots::Bots;
use crate::session::PlaySession;

/// The human always plays as player 0 with their own deck.
pub const HUMAN: PlayerId = PlayerId(0);

/// Who goes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum First {
    Me,
    Bot,
    Random,
}

pub struct PlayOptions<'a> {
    pub deck: &'a Path,
    /// The bot's deck; `None` mirrors the human's deck.
    pub bot_deck: Option<&'a Path>,
    pub bot: &'a str,
    pub seed: u64,
    pub first: First,
    pub max_turns: u32,
}

/// Loads the decks and starts the session; the bot has played up to the
/// human's first decision when this returns.
pub fn start(bots: &Bots, opts: &PlayOptions) -> Result<PlaySession, Box<dyn std::error::Error>> {
    let player = bots.make(opts.bot, opts.seed ^ 0xB0)?;
    let mine = Deck::load(opts.deck)?;
    let theirs = match opts.bot_deck {
        Some(path) => Deck::load(path)?,
        None => mine.clone(),
    };
    let mut setup = GameSetup::new([DeckList::from(&mine), DeckList::from(&theirs)], opts.seed);
    setup.max_turns = opts.max_turns;
    setup.first_player = match opts.first {
        First::Me => Some(HUMAN),
        First::Bot => Some(HUMAN.opponent()),
        First::Random => None,
    };
    let session = PlaySession::new(
        bots.shared.clone(),
        bots.cards.clone(),
        &setup,
        HUMAN,
        player,
        opts.bot,
    )?;
    Ok(session)
}

/// Where a record goes when the player names no file.
pub fn default_record_path(seed: u64, unix_secs: u64) -> PathBuf {
    PathBuf::from("records/human").join(format!("{unix_secs}-{seed}.jsonl"))
}

/// Writes the record, creating its directory.
pub fn save_record(session: &PlaySession, path: &Path) -> Result<(), DataError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|source| DataError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, session.record().to_jsonl()).map_err(|source| DataError::Io {
        path: path.to_path_buf(),
        source,
    })
}
