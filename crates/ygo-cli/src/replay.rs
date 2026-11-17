//! `ygo replay`: steps through a game record from one player's view or
//! with everything shown. The whole record is checked when it loads.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Wrap};
use ygo_engine::{Event, Game, GameRecord, Observation, PlayerId, RecordError, Shared};

use crate::describe;
use crate::tui::{Screen, side_lines};

/// Whose eyes the replay is seen through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum View {
    P0,
    P1,
    All,
}

impl View {
    fn next(self) -> Self {
        match self {
            View::P0 => View::P1,
            View::P1 => View::All,
            View::All => View::P0,
        }
    }
}

pub struct ReplaySession {
    record: GameRecord,
    /// `states[i]` is the game before action `i`; the last is the end.
    states: Vec<Game>,
    /// The events each action caused.
    events: Vec<Vec<Event>>,
    index: usize,
    pub view: View,
}

impl ReplaySession {
    /// Replays the whole record under its own rules, refusing it if any
    /// action is illegal or the result differs.
    pub fn new(shared: &Shared, record: GameRecord, view: View) -> Result<Self, RecordError> {
        let mut game = record.start(shared)?;
        let mut states = vec![game.clone()];
        let mut events = Vec::with_capacity(record.actions.len());
        for (i, a) in record.actions.iter().enumerate() {
            events.push(GameRecord::step(&mut game, i, a)?);
            states.push(game.clone());
        }
        if game.result() != record.result {
            return Err(RecordError::ResultMismatch {
                recorded: record.result,
                replayed: game.result(),
            });
        }
        Ok(Self {
            record,
            states,
            events,
            index: 0,
            view,
        })
    }

    pub fn index(&self) -> usize {
        self.index
    }

    /// Number of recorded actions.
    pub fn len(&self) -> usize {
        self.record.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.record.actions.is_empty()
    }

    /// The game as it stands at the current position.
    pub fn game(&self) -> &Game {
        &self.states[self.index]
    }

    pub fn forward(&mut self) {
        self.index = (self.index + 1).min(self.len());
    }

    pub fn back(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn to_start(&mut self) {
        self.index = 0;
    }

    pub fn to_end(&mut self) {
        self.index = self.len();
    }

    /// Moves to the first decision of `turn`, or the end if the game never
    /// reached it.
    pub fn jump_to_turn(&mut self, turn: u32) {
        self.index = (0..=self.len())
            .find(|&i| self.states[i].turn() >= turn)
            .unwrap_or(self.len());
    }

    pub fn next_turn(&mut self) {
        let turn = self.game().turn() + 1;
        self.jump_to_turn(turn);
    }

    pub fn previous_turn(&mut self) {
        let turn = self.game().turn();
        let start = (0..=self.index)
            .find(|&i| self.states[i].turn() >= turn)
            .unwrap_or(0);
        if start < self.index {
            self.index = start;
        } else {
            self.jump_to_turn(turn.saturating_sub(1).max(1));
        }
    }

    /// What the view shows of state `i`. `All` combines each player's view
    /// of their own side, so both hands and all Set cards are shown.
    fn observe(&self, i: usize, actor: PlayerId) -> Observation {
        let g = &self.states[i];
        match self.view {
            View::P0 => g.observe(PlayerId(0)),
            View::P1 => g.observe(PlayerId(1)),
            View::All => {
                let mut obs = g.observe(actor);
                let other = actor.opponent();
                obs.sides[other.index()] = g.observe(other).sides[other.index()].clone();
                obs
            }
        }
    }

    pub fn observation(&self) -> Observation {
        self.observe(self.index, PlayerId(0))
    }

    pub fn labels(&self) -> &[String; 2] {
        &self.record.header.players
    }

    /// Every action played so far with what it caused, as the view saw it.
    pub fn log(&self) -> Vec<String> {
        let pool = &self.states[0].shared().pool;
        let mut lines = Vec::new();
        for i in 0..self.index {
            let a = &self.record.actions[i];
            let before = self.observe(i, a.player);
            let label = &self.labels()[a.player.index()];
            lines.push(format!(
                "T{} P{} {label}: {}",
                a.turn,
                a.player.0,
                describe::action(&before, pool, &a.action)
            ));
            let after = self.observe(i + 1, a.player);
            for e in &self.events[i] {
                if let Some(line) = describe::event(&after, pool, e) {
                    lines.push(format!("  {line}"));
                }
            }
        }
        lines
    }
}

/// The replay screen and its keys.
pub struct ReplayApp {
    pub session: ReplaySession,
    pub quit: bool,
}

impl Screen for ReplayApp {
    fn key(&mut self, key: KeyCode) {
        let s = &mut self.session;
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Right | KeyCode::Char(' ') | KeyCode::Char('n') => s.forward(),
            KeyCode::Left | KeyCode::Backspace | KeyCode::Char('p') => s.back(),
            KeyCode::Char(']') | KeyCode::PageDown => s.next_turn(),
            KeyCode::Char('[') | KeyCode::PageUp => s.previous_turn(),
            KeyCode::Home => s.to_start(),
            KeyCode::End => s.to_end(),
            KeyCode::Char('v') => s.view = s.view.next(),
            _ => {}
        }
    }

    fn done(&self) -> bool {
        self.quit
    }

    fn draw(&self, frame: &mut Frame) {
        let s = &self.session;
        let obs = s.observation();
        let pool = &s.game().shared().pool;
        let [top, body, help] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let result = match s.game().result() {
            Some(r) if s.index() == s.len() => {
                format!(
                    "   {}",
                    describe::result_sentence(&s.observe(s.index(), PlayerId(0)), r)
                )
            }
            _ => String::new(),
        };
        let status = format!(
            " Replay {}/{}  Turn {} - {}  View: {:?}  P0 {} LP  P1 {} LP{result}",
            s.index(),
            s.len(),
            obs.turn,
            describe::phase_name(obs.phase),
            s.view,
            obs.sides[0].lp,
            obs.sides[1].lp,
        );
        frame.render_widget(
            Paragraph::new(status).style(Style::new().add_modifier(Modifier::BOLD)),
            top,
        );
        let [left, log_area] =
            Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .areas(body);
        let [p1, chain, p0] = Layout::vertical([
            Constraint::Length(7),
            Constraint::Min(3),
            Constraint::Length(7),
        ])
        .areas(left);
        for (player, area) in [(PlayerId(1), p1), (PlayerId(0), p0)] {
            let side = &obs.sides[player.index()];
            let mut lines = side_lines(&obs, pool, side);
            let hand: Vec<String> = side
                .hand
                .iter()
                .map(|c| describe::card(&obs, pool, c.instance()))
                .collect();
            lines.push(Line::from(format!("Hand: {}", hand.join(", "))));
            let title = format!(" P{} {} ", player.0, s.labels()[player.index()]);
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: true })
                    .block(Block::bordered().title(title)),
                area,
            );
        }
        let chain_text = obs
            .chain
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}. {}", i + 1, describe::card_name(pool, l.card)))
            .collect::<Vec<_>>()
            .join("  ");
        frame.render_widget(
            Paragraph::new(chain_text).block(Block::bordered().title(" Chain ")),
            chain,
        );
        let log = s.log();
        let visible = log_area.height.saturating_sub(2) as usize;
        let lines: Vec<Line> = log[log.len().saturating_sub(visible)..]
            .iter()
            .map(|l| Line::from(l.clone()))
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title(" Log ")),
            log_area,
        );
        frame.render_widget(
            Paragraph::new(
                " Right/Left: step   ]/[: next/previous turn   Home/End   v: view   q: quit",
            ),
            help,
        );
    }
}
