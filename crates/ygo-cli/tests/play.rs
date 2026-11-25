//! `ygo play` without a terminal: the session plays whole games, its record
//! replays, and the screen shows the board without naming hidden cards.

mod common;

use std::path::PathBuf;

use common::assert_no_hidden_names;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;
use ygo_cli::bots::{Bots, DEFAULT_BOT};
use ygo_cli::decks;
use ygo_cli::play::{self, First, HUMAN, PlayOptions};
use ygo_cli::session::PlaySession;
use ygo_cli::tui::{self, App};
use ygo_engine::testing::{bundle, root};
use ygo_engine::{CardIndex, GameRecord, GameSetup, VisibleCard};

fn deck(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/decks")
        .join(name)
}

fn bots() -> Bots {
    let mut bots = Bots::new(&bundle());
    let net = ygo_ai::neural::Net::load(
        &root().join("data/networks/best.bpk"),
        &ygo_ai::neural::cpu(),
    );
    bots.network = Some(net.unwrap());
    bots
}

fn start_with(bot: &str, seed: u64, first: First) -> PlaySession {
    let b = bundle();
    let mine = decks::load_legal(&deck("save_a_owner_current_40.json"), &b).unwrap();
    let theirs = decks::load_legal(&deck("save_b_dark_plasma_40.json"), &b).unwrap();
    let mut bots = bots();
    // A light search keeps the test quick.
    bots.mcts.iterations = 10;
    play::start(
        &bots,
        &PlayOptions {
            deck: &mine,
            bot_deck: Some(&theirs),
            bot,
            seed,
            first,
            max_turns: GameSetup::DEFAULT_MAX_TURNS,
        },
    )
    .expect("session starts")
}

fn start(seed: u64, first: First) -> PlaySession {
    start_with("random", seed, first)
}

/// The text of every cell on the screen, row by row.
fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).unwrap();
    terminal.draw(|f| tui::draw(f, app)).unwrap();
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

fn lcg(state: &mut u64) -> usize {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 33) as usize
}

#[test]
fn a_human_plays_whole_games_and_the_record_replays() {
    let shared = ygo_engine::testing::shared_from(ygo_cards::registry());
    for seed in 0..8u64 {
        let bot = match seed {
            0..3 => "random",
            3..6 => DEFAULT_BOT,
            _ => "network-search",
        };
        let mut s = start_with(
            bot,
            seed,
            if seed % 2 == 0 { First::Me } else { First::Bot },
        );
        let mut rng = seed;
        while s.result().is_none() {
            let n = s.actions().len();
            assert!(n > 0, "the human has a decision until the game ends");
            assert_eq!(s.action_texts().len(), n);
            assert!(s.play(lcg(&mut rng) % n));
        }
        assert!(s.actions().is_empty());
        assert!(!s.play(0), "nothing to play once the game is over");
        assert!(s.log().len() > 2, "{:?}", s.log());
        let record = s.record();
        assert_eq!(record.header.players[HUMAN.index()], "human");
        assert_eq!(record.header.players[HUMAN.opponent().index()], bot);
        assert_eq!(record.result, s.result());
        let parsed = GameRecord::from_jsonl(&record.to_jsonl()).unwrap();
        let replayed = parsed.replay(&shared).unwrap();
        assert_eq!(&replayed, s.game(), "seed {seed}");
    }
}

#[test]
fn who_goes_first_follows_the_choice() {
    let s = start(3, First::Me);
    assert_eq!(s.game().turn_player(), HUMAN);
    assert!(s.log()[0].contains("you go first"), "{:?}", s.log());
    let s = start(3, First::Bot);
    assert!(
        s.log()[0].contains("your opponent go first"),
        "{:?}",
        s.log()
    );
    let err = bots()
        .make("nonsense", 1)
        .err()
        .expect("unknown bot refused");
    assert!(
        err.contains("network-search") && err.contains("random"),
        "{err}"
    );
    for gone in ["heuristic:beatdown", "lookahead:beatdown", "search"] {
        assert!(bots().make(gone, 1).is_err(), "{gone}");
    }
    assert_eq!(bots().names(), ["network", "network-search", "random"]);
    for name in bots().names() {
        assert!(bots().make(&name, 1).is_ok(), "{name}");
    }
}

#[test]
fn the_screen_shows_the_board_and_never_a_hidden_card() {
    let shared = ygo_engine::testing::shared_from(ygo_cards::registry());
    for seed in 0..4u64 {
        let mut app = App::new(start(seed, First::Random));
        let mut rng = seed + 100;
        let mut first = true;
        while app.session.result().is_none() && app.session.game().turn() < 30 {
            let text = screen(&app);
            if first {
                let turn = app.session.game().turn();
                assert!(text.contains(&format!("Turn {turn} ")), "{text}");
                assert!(text.contains("Field: "), "{text}");
                assert!(text.contains("Your move"), "{text}");
                assert!(text.contains("Your hand"), "{text}");
                first = false;
            }
            // Log lines were checked when written (describe tests); a card
            // named there may have gone back to a hidden zone since.
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
            assert_no_hidden_names(&rest, &app.session.observation(), &shared);
            let n = app.session.actions().len();
            assert!(app.session.play(lcg(&mut rng) % n));
        }
    }
}

#[test]
fn keys_choose_and_play_actions() {
    let mut app = App::new(start(5, First::Me));
    let actions = app.session.actions().to_vec();
    assert!(actions.len() >= 2);
    app.handle_key(KeyCode::Up);
    assert_eq!(
        app.selected,
        actions.len() - 1,
        "Up wraps to the last action"
    );
    app.handle_key(KeyCode::Down);
    app.handle_key(KeyCode::Down);
    assert_eq!(app.selected, 1);
    app.handle_key(KeyCode::Char('c'));
    assert!(app.show_card);
    assert!(screen(&app).contains("Card (c to close)"));
    app.handle_key(KeyCode::Esc);
    assert!(!app.show_card && !app.quit, "Esc closes the card first");
    let at = app.session.record().actions.len();
    app.handle_key(KeyCode::Enter);
    assert_eq!(app.session.record().actions[at].action, actions[1]);
    assert_eq!(app.selected, 0);
    app.handle_key(KeyCode::Char('q'));
    assert!(app.quit);
}

#[test]
fn card_text_is_shown_only_for_cards_the_human_can_see() {
    let s = start(7, First::Me);
    let obs = s.observation();
    let mine = obs.me().hand[0].instance();
    let text = s.card_text(mine).expect("own hand card has text");
    let VisibleCard::Known { card, .. } = obs.me().hand[0] else {
        panic!("own hand is known")
    };
    let cards = bundle().cards;
    let name = &cards.get(card).unwrap().name;
    assert!(text.starts_with(name.as_str()), "{text}");
    let theirs = obs.opponent().hand[0].instance();
    assert_eq!(s.card_text(theirs), None);
}

#[test]
fn the_record_is_saved_where_asked() {
    let s = start(9, First::Me);
    let dir = std::env::temp_dir().join(format!("ygo-play-test-{}", std::process::id()));
    let path = dir.join("nested/game.jsonl");
    play::save_record(&s, &path).unwrap();
    let back = GameRecord::from_jsonl(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(&back, s.record());
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        play::default_record_path(42, 1000),
        PathBuf::from("records/human/1000-42.jsonl")
    );
}

#[test]
fn a_tokens_card_text_names_the_card_that_made_it() {
    use std::sync::Arc;
    use ygo_engine::{Action, PlayerId, Position};
    let b = bundle();
    let shared = Arc::new(ygo_engine::Shared::new(
        b.rules.clone(),
        &b.cards,
        ygo_cards::registry(),
    ));
    let scapegoat = ygo_data::CardId(73915051);
    let pad = ygo_engine::testing::MAMMOTH_GRAVEYARD;
    let mut main = vec![scapegoat; 5];
    main.resize(40, pad);
    let mut setup = GameSetup::new(
        [
            ygo_engine::DeckList {
                main: main.clone(),
                extra: vec![],
            },
            ygo_engine::DeckList {
                main: vec![pad; 40],
                extra: vec![],
            },
        ],
        1,
    );
    setup.shuffle = false;
    setup.first_player = Some(PlayerId(0));
    let bot = Box::new(ygo_ai::RandomPlayer::new(1));
    let mut s = PlaySession::new(
        shared,
        Arc::new(b.cards),
        &setup,
        PlayerId(0),
        bot,
        "random",
    )
    .unwrap();
    let goat = s
        .actions()
        .iter()
        .position(|a| matches!(a, Action::Activate { .. }))
        .expect("Scapegoat is offered");
    assert!(s.play(goat));
    while !s.actions().iter().any(|a| matches!(a, Action::EndTurn)) {
        let pass = s.actions().iter().position(|a| *a == Action::Pass).unwrap();
        assert!(s.play(pass));
    }
    let obs = s.observation();
    let sheep = obs
        .me()
        .monster_zones
        .iter()
        .flatten()
        .find(|m| m.position() == Position::FaceUpDefense)
        .expect("a Sheep Token")
        .instance();
    let text = s.card_text(sheep).unwrap();
    assert!(
        text.starts_with("Sheep Token\nToken · Level 1 · EARTH Beast · 0/0"),
        "{text}"
    );
    assert!(text.contains("Created by Scapegoat"), "{text}");
}
