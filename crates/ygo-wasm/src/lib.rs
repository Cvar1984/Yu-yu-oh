//! The game for a web page. The page sends one JSON command at a time to
//! `Host::handle` and gets JSON back; everything the page shows comes from
//! the human's observation, so no hidden card ever reaches the browser.
//!
//! Commands (`cmd` names the variant):
//! - `init`: the data files as text; returns the decks and bots on offer.
//! - `start`: deck and bot choices, a seed, who goes first, and optionally
//!   the human's earlier choices to replay (resuming a game); returns a view.
//! - `play`: the index of one of the offered actions; returns a view.
//! - `view`, `card` (a card's text by instance), `record` (JSON Lines).

mod view;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};
use ygo_data::{Banlist, Bundle, CardDb, CardValues, Deck, Rules, Styles};
use ygo_engine::{DeckList, GameSetup, InstanceId, PlayerId};
use ygo_play::bots::{Bots, DEFAULT_BOT};
use ygo_play::decks;
use ygo_play::session::PlaySession;

/// The human is always player 0.
pub const HUMAN: PlayerId = PlayerId(0);

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Init {
        rules: String,
        cards: String,
        banlist: String,
        styles: String,
        card_values: String,
        /// Deck files: name (for errors and choosing) and JSON text.
        decks: Vec<DeckFile>,
    },
    Start {
        deck: String,
        bot_deck: String,
        bot: String,
        seed: u64,
        first: First,
        #[serde(default)]
        choices: Vec<usize>,
    },
    Play {
        index: usize,
    },
    View,
    Card {
        instance: u8,
    },
    Record,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeckFile {
    file: String,
    text: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum First {
    Me,
    Bot,
    Random,
}

struct Loaded {
    bots: Bots,
    /// Legal decks by file name.
    decks: Vec<(String, Deck)>,
}

/// Everything the page talks to: the loaded data and the game in play.
#[derive(Default)]
pub struct Host {
    loaded: Option<Loaded>,
    session: Option<PlaySession>,
    /// The human's choices so far, so the page can resume the game.
    choices: Vec<usize>,
}

impl Host {
    /// Runs one JSON command; the answer is `{ "ok": ... }` or
    /// `{ "error": "..." }`.
    pub fn handle(&mut self, request: &str) -> String {
        let answer = serde_json::from_str::<Command>(request)
            .map_err(|e| format!("bad command: {e}"))
            .and_then(|c| self.run(c));
        match answer {
            Ok(v) => json!({ "ok": v }).to_string(),
            Err(e) => json!({ "error": e }).to_string(),
        }
    }

    fn run(&mut self, command: Command) -> Result<Value, String> {
        match command {
            Command::Init {
                rules,
                cards,
                banlist,
                styles,
                card_values,
                decks,
            } => self.init(&rules, &cards, &banlist, &styles, &card_values, decks),
            Command::Start {
                deck,
                bot_deck,
                bot,
                seed,
                first,
                choices,
            } => self.start(&deck, &bot_deck, &bot, seed, first, &choices),
            Command::Play { index } => {
                let session = self.session.as_mut().ok_or("no game in progress")?;
                if !session.play(index) {
                    return Err(format!("action {index} is not offered"));
                }
                self.choices.push(index);
                self.view()
            }
            Command::View => self.view(),
            Command::Card { instance } => {
                let session = self.session.as_ref().ok_or("no game in progress")?;
                Ok(json!(session.card_text(InstanceId(instance))))
            }
            Command::Record => {
                let session = self.session.as_ref().ok_or("no game in progress")?;
                Ok(json!(session.record().to_jsonl()))
            }
        }
    }

    fn init(
        &mut self,
        rules: &str,
        cards: &str,
        banlist: &str,
        styles: &str,
        card_values: &str,
        files: Vec<DeckFile>,
    ) -> Result<Value, String> {
        let err = |e: ygo_data::DataError| e.to_string();
        let rules = Rules::parse(rules, Path::new("rules.toml")).map_err(err)?;
        let cards = CardDb::parse(cards, Path::new("cards.json")).map_err(err)?;
        let banlist = Banlist::parse(banlist, Path::new("banlist.json")).map_err(err)?;
        let bundle = Bundle::from_parts(rules, cards, banlist).map_err(err)?;
        let values = CardValues::parse(card_values, Path::new("card_values.toml"), &bundle.cards)
            .map_err(err)?;
        let styles = Styles::parse(styles, Path::new("styles.toml"), &bundle.cards).map_err(err)?;
        let bots = Bots::new(&bundle, values, styles);

        let mut offered = Vec::new();
        let mut legal = Vec::new();
        for f in files {
            let path = Path::new(&f.file);
            let checked = Deck::parse(&f.text, path)
                .map_err(|e| e.to_string())
                .and_then(|d| decks::check_legal(d, path, &bundle).map_err(|e| e.to_string()));
            match checked {
                Ok(deck) => {
                    offered.push(json!({ "file": f.file, "name": deck.name, "legal": true }));
                    legal.push((f.file, deck));
                }
                Err(problem) => {
                    offered.push(json!({ "file": f.file, "legal": false, "problem": problem }))
                }
            }
        }
        let answer = json!({ "decks": offered, "bots": bots.names(), "default_bot": DEFAULT_BOT });
        self.loaded = Some(Loaded { bots, decks: legal });
        self.session = None;
        Ok(answer)
    }

    fn start(
        &mut self,
        deck: &str,
        bot_deck: &str,
        bot: &str,
        seed: u64,
        first: First,
        choices: &[usize],
    ) -> Result<Value, String> {
        let loaded = self.loaded.as_ref().ok_or("data not loaded")?;
        let find = |file: &str| {
            loaded
                .decks
                .iter()
                .find(|(f, _)| f == file)
                .map(|(_, d)| d)
                .ok_or_else(|| format!("no legal deck named {file}"))
        };
        let (mine, theirs) = (find(deck)?, find(bot_deck)?);
        let player = loaded.bots.make(bot, seed ^ 0xB0)?;
        let mut setup = GameSetup::new([DeckList::from(mine), DeckList::from(theirs)], seed);
        setup.first_player = match first {
            First::Me => Some(HUMAN),
            First::Bot => Some(HUMAN.opponent()),
            First::Random => None,
        };
        let mut session = PlaySession::new(
            loaded.bots.shared.clone(),
            loaded.bots.cards.clone(),
            &setup,
            HUMAN,
            player,
            bot,
        )
        .map_err(|e| e.to_string())?;
        for (n, &index) in choices.iter().enumerate() {
            if !session.play(index) {
                return Err(format!("saved choice {n} ({index}) no longer applies"));
            }
        }
        self.session = Some(session);
        self.choices = choices.to_vec();
        self.view()
    }

    /// The game in progress, if any.
    pub fn session(&self) -> Option<&PlaySession> {
        self.session.as_ref()
    }

    fn view(&self) -> Result<Value, String> {
        let session = self.session.as_ref().ok_or("no game in progress")?;
        Ok(json!({
            "view": view::build(session),
            "choices": self.choices,
        }))
    }
}
