//! `ygo replay` without a terminal: stepping, turn jumps, views, and the
//! screen never naming a card the chosen player could not see.

mod common;

use std::sync::Arc;

use common::assert_no_hidden_names;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;
use ygo_ai::{Player, RandomPlayer, play_game};
use ygo_cli::replay::{ReplayApp, ReplaySession, View};
use ygo_cli::tui::Screen;
use ygo_engine::testing::{sample_deck, shared_from};
use ygo_engine::{
    Action, CardIndex, GameRecord, GameSetup, PlayerId, RecordError, Shared, VisibleCard,
};

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn record(seed: u64) -> GameRecord {
    let setup = GameSetup::new(
        [
            sample_deck("save_a_owner_current_40.json"),
            sample_deck("save_b_dark_plasma_40.json"),
        ],
        seed,
    );
    let mut a = RandomPlayer::new(seed);
    let mut b = RandomPlayer::new(seed + 1);
    let mut players: [&mut dyn Player; 2] = [&mut a, &mut b];
    let labels = ["random".to_string(), "random".to_string()];
    let (_, record) = play_game(shared(), &setup, &mut players, labels).unwrap();
    GameRecord::from_jsonl(&record.to_jsonl()).unwrap()
}

fn screen(app: &ReplayApp) -> String {
    let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();
    terminal.draw(|f| app.draw(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn steps_through_the_whole_game_both_ways() {
    let shared = shared();
    let rec = record(3);
    let end = rec.replay(&shared).unwrap();
    let start = rec.start(&shared).unwrap();
    let mut s = ReplaySession::new(&shared, rec.clone(), View::All).unwrap();
    assert_eq!(s.len(), rec.actions.len());
    assert_eq!(s.game(), &start);
    for _ in 0..s.len() + 5 {
        s.forward();
    }
    assert_eq!(s.index(), s.len(), "stops at the end");
    assert_eq!(s.game(), &end);
    assert!(s.log().len() >= s.len());
    for _ in 0..s.len() + 5 {
        s.back();
    }
    assert_eq!(s.game(), &start);
    s.to_end();
    assert_eq!(s.game(), &end);
    s.to_start();
    assert_eq!(s.index(), 0);
}

#[test]
fn jumps_to_the_start_of_a_turn() {
    let shared = shared();
    let mut s = ReplaySession::new(&shared, record(4), View::All).unwrap();
    s.jump_to_turn(5);
    assert_eq!(s.game().turn(), 5);
    let at = s.index();
    s.back();
    assert_eq!(s.game().turn(), 4, "the position before is still turn 4");
    s.jump_to_turn(5);
    assert_eq!(s.index(), at);
    s.next_turn();
    assert_eq!(s.game().turn(), 6);
    s.forward();
    s.previous_turn();
    assert_eq!(
        s.game().turn(),
        6,
        "back to the start of the current turn first"
    );
    s.previous_turn();
    assert_eq!(s.game().turn(), 5);
    s.jump_to_turn(10_000);
    assert_eq!(s.index(), s.len(), "a turn never reached means the end");
}

#[test]
fn one_players_view_never_names_what_they_could_not_see() {
    let shared = shared();
    for (seed, view, viewer) in [(5, View::P0, PlayerId(0)), (6, View::P1, PlayerId(1))] {
        let session = ReplaySession::new(&shared, record(seed), view).unwrap();
        let mut app = ReplayApp {
            session,
            quit: false,
        };
        while app.session.index() < app.session.len().min(150) {
            let text = screen(&app);
            assert!(text.contains(&format!("View: {view:?}")), "{text}");
            // The log was written from the same player's view at the time
            // (describe tests); a card it names may have been hidden since.
            let logged = app.session.log().join("\n");
            let mut names: Vec<&str> = (0..shared.pool.len() as u16)
                .map(|i| shared.pool.get(CardIndex(i)).name.as_str())
                .filter(|n| logged.contains(n))
                .collect();
            names.sort_by_key(|n| std::cmp::Reverse(n.len()));
            let mut rest = text;
            for n in names {
                rest = rest.replace(n, "#");
            }
            let obs = app.session.game().observe(viewer);
            assert_no_hidden_names(&rest, &obs, &shared);
            app.key(KeyCode::Right);
        }
    }
}

#[test]
fn the_all_view_shows_both_hands() {
    let shared = shared();
    let s = ReplaySession::new(&shared, record(7), View::All).unwrap();
    let obs = s.observation();
    for side in &obs.sides {
        assert!(!side.hand.is_empty());
        assert!(
            side.hand
                .iter()
                .all(|c| matches!(c, VisibleCard::Known { .. }))
        );
    }
    let p0_view = ReplaySession::new(&shared, record(7), View::P0).unwrap();
    assert!(
        p0_view.observation().sides[1]
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Hidden { .. }))
    );
}

#[test]
fn a_tampered_record_is_refused_when_loaded() {
    let shared = shared();
    let mut rec = record(8);
    let i = rec
        .actions
        .iter()
        .position(|a| a.action == Action::EndTurn)
        .unwrap();
    rec.actions[i].action = Action::EnterBattlePhase;
    let err = ReplaySession::new(&shared, rec, View::All)
        .err()
        .expect("refused");
    assert!(
        matches!(
            err,
            RecordError::Illegal { .. } | RecordError::WrongPlayer { .. }
        ),
        "{err:?}"
    );
}

#[test]
fn keys_step_switch_views_and_quit() {
    let shared = shared();
    let mut app = ReplayApp {
        session: ReplaySession::new(&shared, record(9), View::P0).unwrap(),
        quit: false,
    };
    app.key(KeyCode::Right);
    app.key(KeyCode::Char(' '));
    assert_eq!(app.session.index(), 2);
    app.key(KeyCode::Left);
    assert_eq!(app.session.index(), 1);
    app.key(KeyCode::End);
    assert_eq!(app.session.index(), app.session.len());
    let status = screen(&app).lines().next().unwrap().to_string();
    assert!(
        status.contains("win") || status.contains("draw"),
        "{status}"
    );
    app.key(KeyCode::Home);
    assert_eq!(app.session.index(), 0);
    app.key(KeyCode::Char(']'));
    assert_eq!(app.session.game().turn(), 2);
    for expected in [View::P1, View::All, View::P0] {
        app.key(KeyCode::Char('v'));
        assert_eq!(app.session.view, expected);
    }
    assert!(!app.done());
    app.key(KeyCode::Char('q'));
    assert!(app.done());
}
