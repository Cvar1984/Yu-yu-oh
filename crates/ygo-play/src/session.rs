//! A human playing against a bot, independent of any screen: the game,
//! the bot, the record and a log written from the human's point of view.

use std::sync::Arc;

use ygo_ai::Player;
use ygo_data::CardDb;
use ygo_engine::{
    Action, Game, GameRecord, GameResult, GameSetup, InstanceId, Observation, PlayerId, SetupError,
    Shared, VisibleCard,
};

use crate::describe;

pub struct PlaySession {
    game: Game,
    record: GameRecord,
    human: PlayerId,
    bot: Box<dyn Player>,
    cards: Arc<CardDb>,
    log: Vec<String>,
}

impl PlaySession {
    /// Starts a game in which `human` is one player and `bot` the other,
    /// then lets the bot play until the human has a decision.
    pub fn new(
        shared: Arc<Shared>,
        cards: Arc<CardDb>,
        setup: &GameSetup,
        human: PlayerId,
        bot: Box<dyn Player>,
        bot_label: &str,
    ) -> Result<Self, SetupError> {
        let mut labels = [String::new(), String::new()];
        labels[human.index()] = "human".to_string();
        labels[human.opponent().index()] = bot_label.to_string();
        let record = GameRecord::new(&shared.rules, setup, labels);
        let game = Game::new(shared, setup)?;
        let mut session = Self {
            game,
            record,
            human,
            bot,
            cards,
            log: Vec::new(),
        };
        let first = session.game.turn_player();
        session.log.push(format!(
            "Game start: {} go first (seed {})",
            if first == human {
                "you"
            } else {
                "your opponent"
            },
            setup.seed
        ));
        session.run_bot();
        Ok(session)
    }

    pub fn observation(&self) -> Observation {
        self.game.observe(self.human)
    }

    pub fn human(&self) -> PlayerId {
        self.human
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn record(&self) -> &GameRecord {
        &self.record
    }

    pub fn log(&self) -> &[String] {
        &self.log
    }

    pub fn result(&self) -> Option<GameResult> {
        self.game.result()
    }

    /// The human's legal actions, or nothing while the game is over.
    pub fn actions(&self) -> &[Action] {
        if self
            .game
            .pending_decision()
            .is_some_and(|d| d.player == self.human)
        {
            self.game.actions()
        } else {
            &[]
        }
    }

    /// The human's legal actions as sentences.
    pub fn action_texts(&self) -> Vec<String> {
        let obs = self.observation();
        let pool = &self.game.shared().pool;
        self.actions()
            .iter()
            .map(|a| describe::action(&obs, pool, a))
            .collect()
    }

    /// Plays the human's action number `index`, then the bot's turn until
    /// the human decides again. Returns `false` if `index` is out of range.
    pub fn play(&mut self, index: usize) -> bool {
        let Some(action) = self.actions().get(index).cloned() else {
            return false;
        };
        self.apply(action, true);
        self.run_bot();
        true
    }

    fn run_bot(&mut self) {
        while let Some(d) = self.game.pending_decision() {
            if d.player == self.human {
                break;
            }
            let obs = self.game.observe(d.player);
            let action = self.bot.choose(&obs, self.game.actions());
            self.apply(action, false);
        }
    }

    fn apply(&mut self, action: Action, by_human: bool) {
        let before = self.observation();
        let pool = &self.game.shared().pool;
        let sentence = describe::action(&before, pool, &action);
        let quiet = matches!(action, Action::Pass) && !by_human;
        if !quiet {
            self.log.push(format!(
                "{}: {sentence}",
                if by_human { "You" } else { "Opponent" }
            ));
        }
        self.record.note(&self.game, &action);
        let events = self
            .game
            .apply(&action)
            .expect("only legal actions are offered");
        let after = self.observation();
        let pool = &self.game.shared().pool;
        for line in describe::events(&before, &after, pool, &events) {
            self.log.push(format!("  {line}"));
        }
        self.record.finish(&self.game);
    }

    /// The card an action is about, for showing its text.
    pub fn subject(action: &Action) -> Option<InstanceId> {
        match action {
            Action::Discard { card }
            | Action::Summon { card, .. }
            | Action::ProcedureSummon { card, .. }
            | Action::FlipSummon { card }
            | Action::ChangePosition { card }
            | Action::SetSpellTrap { card }
            | Action::Activate { card, .. }
            | Action::Choose { card } => Some(*card),
            Action::Attack { attacker, .. } => Some(*attacker),
            _ => None,
        }
    }

    /// Full text of a card the human can see: name, type, stats and effect.
    pub fn card_text(&self, id: InstanceId) -> Option<String> {
        let obs = self.observation();
        let known = match obs.find(id).map(|f| f.card) {
            Some(VisibleCard::Known { card, .. }) => Some(*card),
            _ => obs.chain.iter().find(|l| l.instance == id).map(|l| l.card),
        }?;
        let c = self.cards.get(known)?;
        // Name, then a line such as "Normal Monster · Level 4 · WATER
        // Spellcaster · 1200/2000" or "Trap Card · Counter", then the text.
        let kind = c.card_type.label();
        let line = match (c.level, c.atk, c.def) {
            (Some(level), Some(atk), Some(def)) => format!(
                "{kind} · Level {level} · {} {} · {atk}/{def}",
                c.attribute.as_deref().unwrap_or(""),
                c.race
            ),
            _ => format!("{kind} · {}", c.race),
        };
        Some(format!("{}\n{line}\n\n{}", c.name, c.text))
    }
}
