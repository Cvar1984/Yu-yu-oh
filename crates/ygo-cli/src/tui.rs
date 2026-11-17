//! The full-screen terminal UI for `ygo play`, drawn with ratatui. All
//! game logic lives in `PlaySession`; this file only draws and reads keys.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use ygo_engine::{CardPool, Observation, Side};

use crate::describe;
use crate::session::PlaySession;

/// What the screen shows besides the game itself.
pub struct App {
    pub session: PlaySession,
    pub selected: usize,
    pub show_card: bool,
    pub quit: bool,
}

impl App {
    pub fn new(session: PlaySession) -> Self {
        Self {
            session,
            selected: 0,
            show_card: false,
            quit: false,
        }
    }

    /// Reacts to one key press.
    pub fn handle_key(&mut self, key: KeyCode) {
        let n = self.session.actions().len();
        match key {
            KeyCode::Char('q') | KeyCode::Esc if !self.show_card => self.quit = true,
            KeyCode::Esc | KeyCode::Char('c') if self.show_card => self.show_card = false,
            KeyCode::Char('c') => self.show_card = true,
            KeyCode::Up if n > 0 => self.selected = (self.selected + n - 1) % n,
            KeyCode::Down if n > 0 => self.selected = (self.selected + 1) % n,
            KeyCode::Char(d @ '1'..='9') => {
                let i = d as usize - '1' as usize;
                if i < n {
                    self.selected = i;
                }
            }
            KeyCode::Enter if self.session.result().is_some() => self.quit = true,
            KeyCode::Enter if n > 0 => {
                self.session.play(self.selected);
                self.selected = 0;
                self.show_card = false;
            }
            _ => {}
        }
    }
}

fn side_lines(obs: &Observation, pool: &CardPool, side: &Side) -> Vec<Line<'static>> {
    let monsters: Vec<String> = side
        .monster_zones
        .iter()
        .map(|z| describe::zone_text(pool, z))
        .collect();
    let spells: Vec<String> = side
        .spell_trap_zones
        .iter()
        .map(|z| describe::zone_text(pool, z))
        .collect();
    let top = side
        .graveyard
        .last()
        .map(|c| describe::card(obs, pool, c.instance()))
        .unwrap_or_else(|| "empty".to_string());
    vec![
        Line::from(format!(
            "Hand {}  Deck {}  Extra {}  Graveyard {} (top: {top})  Banished {}",
            side.hand.len(),
            side.deck_count,
            side.extra_count,
            side.graveyard.len(),
            side.banished.len()
        )),
        Line::from(format!("Monsters: {}", monsters.join(" | "))),
        Line::from(format!("Spells/Traps: {}", spells.join(" | "))),
        Line::from(format!(
            "Field: {}",
            describe::zone_text(pool, &side.field_zone)
        )),
    ]
}

/// Draws the whole screen.
pub fn draw(frame: &mut Frame, app: &App) {
    let session = &app.session;
    let obs = session.observation();
    let pool = &session.game().shared().pool;
    let [top, body, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let status = format!(
        " Turn {} - {} turn - {}   You {} LP   Opponent {} LP",
        obs.turn,
        if obs.turn_player == obs.viewer {
            "your"
        } else {
            "your opponent's"
        },
        describe::phase_name(obs.phase),
        obs.me().lp,
        obs.opponent().lp
    );
    frame.render_widget(
        Paragraph::new(status).style(Style::new().add_modifier(Modifier::BOLD)),
        top,
    );

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(body);
    let [opp, chain, me, hand] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(3),
        Constraint::Length(6),
        Constraint::Min(3),
    ])
    .areas(left);
    frame.render_widget(
        Paragraph::new(side_lines(&obs, pool, obs.opponent()))
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(" Opponent ")),
        opp,
    );
    let chain_text = if obs.chain.is_empty() {
        "No chain".to_string()
    } else {
        obs.chain
            .iter()
            .enumerate()
            .map(|(i, l)| {
                format!(
                    "{}. {} ({})",
                    i + 1,
                    describe::card_name(pool, l.card),
                    describe::who(&obs, l.controller)
                )
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    frame.render_widget(
        Paragraph::new(chain_text).block(Block::bordered().title(" Chain ")),
        chain,
    );
    frame.render_widget(
        Paragraph::new(side_lines(&obs, pool, obs.me()))
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title(" You ")),
        me,
    );
    let hand_items: Vec<ListItem> = obs
        .me()
        .hand
        .iter()
        .map(|c| ListItem::new(describe::card(&obs, pool, c.instance())))
        .collect();
    frame.render_widget(
        List::new(hand_items).block(Block::bordered().title(" Your hand ")),
        hand,
    );

    let [actions_area, log_area] =
        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(right);
    let title = match session.result() {
        Some(result) => format!(
            " {} Press Enter to quit ",
            describe::result_sentence(&obs, result)
        ),
        None => " Your move ".to_string(),
    };
    let items: Vec<ListItem> = session
        .action_texts()
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            ListItem::new(if i < 9 {
                format!("{} {t}", i + 1)
            } else {
                format!("  {t}")
            })
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
        actions_area,
        &mut state,
    );
    let log = session.log();
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
        Paragraph::new(" Up/Down or 1-9: choose   Enter: play   c: card text   q: quit"),
        help,
    );

    if app.show_card {
        let text = session
            .actions()
            .get(app.selected)
            .and_then(PlaySession::subject)
            .and_then(|id| session.card_text(id))
            .unwrap_or_else(|| "No card for this action.".to_string());
        let area = centered(frame.area(), 60, 50);
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title(" Card (c to close) ")),
            area,
        );
    }
}

fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let [_, mid, _] = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .areas(area);
    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .areas(mid);
    center
}

/// Runs the UI in the real terminal until the player quits.
pub fn run(app: &mut App) -> std::io::Result<()> {
    let mut terminal: DefaultTerminal = ratatui::init();
    let result = (|| -> std::io::Result<()> {
        while !app.quit {
            terminal.draw(|f| draw(f, app))?;
            if let Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) = event::read()?
            {
                app.handle_key(code);
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}
