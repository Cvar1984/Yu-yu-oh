//! Game records: everything needed to replay a game exactly, as JSON
//! Lines. Line 1 is the header (rules, setup, player labels), then one
//! line per action, then the result. Encoding and decoding work on
//! strings; reading and writing files is the caller's job.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ygo_data::Rules;

use crate::{Action, Event, Game, GameResult, GameSetup, PlayerId, RuleError, SetupError, Shared};

/// The record format this build reads and writes.
pub const RECORD_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordHeader {
    pub schema_version: u32,
    /// `ygo-engine`'s version when the game was played.
    pub engine_version: String,
    /// The rules the game was played under; replay uses these, not the
    /// current `rules.toml`.
    pub rules: Rules,
    pub setup: GameSetup,
    /// Who played each side: `human`, `random`, `heuristic:beatdown`...
    pub players: [String; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedAction {
    pub turn: u32,
    pub player: PlayerId,
    pub action: Action,
}

/// One line of a record file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "line", rename_all = "snake_case")]
enum Line {
    Header(Box<RecordHeader>),
    Action(RecordedAction),
    Result { result: GameResult },
}

/// A whole game, built action by action while it is played.
#[derive(Debug, Clone, PartialEq)]
pub struct GameRecord {
    pub header: RecordHeader,
    pub actions: Vec<RecordedAction>,
    /// `None` for a game stopped before its end.
    pub result: Option<GameResult>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RecordError {
    #[error("line {line}: {message}")]
    Json { line: usize, message: String },
    #[error("record has schema_version {found}; this build reads {expected}")]
    Schema { found: u32, expected: u32 },
    #[error("line {line}: expected {expected}")]
    Layout { line: usize, expected: &'static str },
    #[error("cannot start the game: {0}")]
    Setup(#[from] SetupError),
    #[error("action {index} ({action:?}) is not legal: {error}")]
    Illegal {
        index: usize,
        action: Action,
        error: RuleError,
    },
    #[error(
        "action {index}: recorded for player {recorded} on turn {turn}, but player {actual} is deciding on turn {actual_turn}"
    )]
    WrongPlayer {
        index: usize,
        recorded: PlayerId,
        turn: u32,
        actual: PlayerId,
        actual_turn: u32,
    },
    #[error("recorded result {recorded:?}, replayed result {replayed:?}")]
    ResultMismatch {
        recorded: Option<GameResult>,
        replayed: Option<GameResult>,
    },
}

impl GameRecord {
    /// Starts a record for a game about to be played under `rules`.
    pub fn new(rules: &Rules, setup: &GameSetup, players: [String; 2]) -> Self {
        Self {
            header: RecordHeader {
                schema_version: RECORD_SCHEMA_VERSION,
                engine_version: env!("CARGO_PKG_VERSION").to_string(),
                rules: rules.clone(),
                setup: setup.clone(),
                players,
            },
            actions: Vec::new(),
            result: None,
        }
    }

    /// Notes the action the deciding player is about to play in `game`.
    /// Call before `Game::apply`.
    pub fn note(&mut self, game: &Game, action: &Action) {
        let player = game
            .pending_decision()
            .map_or(game.turn_player(), |d| d.player);
        self.actions.push(RecordedAction {
            turn: game.turn(),
            player,
            action: action.clone(),
        });
    }

    /// Notes the end of the game.
    pub fn finish(&mut self, game: &Game) {
        self.result = game.result();
    }

    /// The record as JSON Lines, one `\n`-terminated line each.
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        let mut push = |line: &Line| {
            out.push_str(&serde_json::to_string(line).expect("records serialise"));
            out.push('\n');
        };
        push(&Line::Header(Box::new(self.header.clone())));
        for a in &self.actions {
            push(&Line::Action(a.clone()));
        }
        if let Some(result) = self.result {
            push(&Line::Result { result });
        }
        out
    }

    /// Parses JSON Lines written by `to_jsonl`. Blank lines are ignored.
    pub fn from_jsonl(text: &str) -> Result<Self, RecordError> {
        let mut lines = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
            .map(|(i, l)| {
                serde_json::from_str::<Line>(l)
                    .map(|line| (i + 1, line))
                    .map_err(|e| RecordError::Json {
                        line: i + 1,
                        message: e.to_string(),
                    })
            });
        let header = match lines.next() {
            Some(Ok((_, Line::Header(h)))) => *h,
            Some(Ok((line, _))) => {
                return Err(RecordError::Layout {
                    line,
                    expected: "the header",
                });
            }
            Some(Err(e)) => return Err(e),
            None => {
                return Err(RecordError::Layout {
                    line: 1,
                    expected: "the header",
                });
            }
        };
        if header.schema_version != RECORD_SCHEMA_VERSION {
            return Err(RecordError::Schema {
                found: header.schema_version,
                expected: RECORD_SCHEMA_VERSION,
            });
        }
        let mut record = Self {
            header,
            actions: Vec::new(),
            result: None,
        };
        for item in lines {
            let (line, parsed) = item?;
            if record.result.is_some() {
                return Err(RecordError::Layout {
                    line,
                    expected: "nothing after the result",
                });
            }
            match parsed {
                Line::Action(a) => record.actions.push(a),
                Line::Result { result } => record.result = Some(result),
                Line::Header(_) => {
                    return Err(RecordError::Layout {
                        line,
                        expected: "an action or the result",
                    });
                }
            }
        }
        Ok(record)
    }

    /// Replays the record under its own rules with `shared`'s cards and
    /// scripts, checking every action and the result. Returns the final game.
    pub fn replay(&self, shared: &Shared) -> Result<Game, RecordError> {
        let mut game = self.start(shared)?;
        for (index, a) in self.actions.iter().enumerate() {
            Self::step(&mut game, index, a)?;
        }
        if game.result() != self.result {
            return Err(RecordError::ResultMismatch {
                recorded: self.result,
                replayed: game.result(),
            });
        }
        Ok(game)
    }

    /// The game as it stood before any action, under the record's rules.
    pub fn start(&self, shared: &Shared) -> Result<Game, RecordError> {
        let shared = Arc::new(Shared {
            rules: self.header.rules.clone(),
            pool: shared.pool.clone(),
            scripts: shared.scripts.clone(),
        });
        Ok(Game::new(shared, &self.header.setup)?)
    }

    /// Applies recorded action number `index` to `game`, checking it, and
    /// returns the events it caused.
    pub fn step(
        game: &mut Game,
        index: usize,
        a: &RecordedAction,
    ) -> Result<Vec<Event>, RecordError> {
        let deciding = game.pending_decision().map(|d| d.player);
        if deciding != Some(a.player) || game.turn() != a.turn {
            return Err(RecordError::WrongPlayer {
                index,
                recorded: a.player,
                turn: a.turn,
                actual: deciding.unwrap_or(game.turn_player()),
                actual_turn: game.turn(),
            });
        }
        game.apply(&a.action).map_err(|error| RecordError::Illegal {
            index,
            action: a.action.clone(),
            error,
        })
    }
}
