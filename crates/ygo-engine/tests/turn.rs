//! M1 step 2: phases, draws, hand limit, end of turn, deck-out, turn cap.

mod common;

use common::{end_turns_until_over, normal_deck, play, shared, shared_with};
use ygo_data::HandLimitCheck;
use ygo_engine::{
    Action, DecisionKind, DrawReason, Event, Game, GameResult, GameSetup, Location, Phase,
    PlayerId, RuleError, WinReason,
};

fn setup(seed: u64) -> GameSetup {
    let mut s = GameSetup::new([normal_deck(40), normal_deck(40)], seed);
    s.first_player = Some(PlayerId(0));
    s
}

fn game() -> Game {
    Game::new(shared(), &setup(1)).unwrap()
}

#[test]
fn turn_one_starts_in_main_phase_one_with_end_turn_as_the_only_action() {
    let g = game();
    let d = g.pending_decision().unwrap();
    assert_eq!(d.player, PlayerId(0));
    assert_eq!(d.kind, DecisionKind::MainPhase);
    assert_eq!(g.legal_actions().last(), Some(&Action::EndTurn));
}

#[test]
fn ending_the_turn_passes_to_the_opponent_who_draws() {
    let mut g = game();
    let events = play(&mut g, Action::EndTurn);
    assert_eq!(g.turn(), 2);
    assert_eq!(g.turn_player(), PlayerId(1));
    assert_eq!(g.phase(), Phase::Main1);
    assert_eq!(g.player(PlayerId(1)).hand.len(), 6);
    assert_eq!(g.player(PlayerId(1)).deck.len(), 34);
    let drawn = g.player(PlayerId(1)).hand[5];
    assert_eq!(g.instance(drawn).location, Location::Hand);
    assert_eq!(
        events,
        vec![
            Event::PhaseEntered { phase: Phase::End },
            Event::TurnStarted {
                turn: 2,
                player: PlayerId(1)
            },
            Event::PhaseEntered { phase: Phase::Draw },
            Event::Drew {
                player: PlayerId(1),
                card: drawn
            },
            Event::PhaseEntered {
                phase: Phase::Standby
            },
            Event::PhaseEntered {
                phase: Phase::Main1
            },
        ]
    );
}

#[test]
fn doing_nothing_ends_by_deck_out_on_turn_72() {
    // 35 cards after the opening hand. Player 1 draws on even turns and
    // runs out on turn 72; player 0, who skipped turn 1, still has one.
    let mut g = game();
    end_turns_until_over(&mut g);
    assert_eq!(g.turn(), 72);
    assert_eq!(
        g.result(),
        Some(GameResult::Win {
            winner: PlayerId(0),
            reason: WinReason::DeckOut
        })
    );
    assert_eq!(g.player(PlayerId(0)).deck.len(), 0);
    assert!(g.legal_actions().is_empty());
    assert_eq!(g.apply(&Action::EndTurn), Err(RuleError::GameOver));
}

#[test]
fn flip_first_player_skips_draw() {
    let shared = shared_with(|r| r.turn.first_player_skips_draw = false);
    let mut g = Game::new(shared, &setup(1)).unwrap();
    assert_eq!(g.player(PlayerId(0)).hand.len(), 6, "drew on turn 1");
    end_turns_until_over(&mut g);
    assert_eq!(g.turn(), 71);
    assert_eq!(
        g.result(),
        Some(GameResult::Win {
            winner: PlayerId(1),
            reason: WinReason::DeckOut
        })
    );
}

#[test]
fn flip_deck_out_loses_then_the_turn_cap_draws_the_game() {
    let shared = shared_with(|r| r.win.deck_out_loses = false);
    let mut s = setup(1);
    s.max_turns = 80;
    let mut g = Game::new(shared, &s).unwrap();
    end_turns_until_over(&mut g);
    assert_eq!(g.turn(), 80);
    assert_eq!(
        g.result(),
        Some(GameResult::Draw {
            reason: DrawReason::TurnLimit
        })
    );
    for p in PlayerId::BOTH {
        let s = g.player(p);
        assert!(s.deck.is_empty(), "empty decks are not a loss here");
        assert_eq!(
            s.hand.len() + s.graveyard.len(),
            40,
            "drawn, discarded, never lost"
        );
        assert!(s.hand.len() <= 6);
    }
}

#[test]
fn hand_limit_after_draw_when_the_rules_say_so() {
    // Max hand 4: the opening 5 cards already exceed it at turn 1, and the
    // check happens in the Draw Phase before any Main Phase action.
    let shared = shared_with(|r| {
        r.hand.max_size = 4;
        r.hand.hand_limit_check = HandLimitCheck::AfterDraw;
    });
    let mut g = Game::new(shared, &setup(1)).unwrap();
    assert_eq!(g.phase(), Phase::Draw);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (PlayerId(0), DecisionKind::HandLimit));
    let actions = g.legal_actions();
    assert_eq!(actions.len(), 5);
    assert!(actions.iter().all(|a| matches!(a, Action::Discard { .. })));
    assert_eq!(
        g.apply(&Action::EndTurn),
        Err(RuleError::IllegalAction(Action::EndTurn))
    );

    let Action::Discard { card } = actions[0].clone() else {
        unreachable!()
    };
    let events = play(&mut g, actions[0].clone());
    assert_eq!(
        events[1],
        Event::Discarded {
            player: PlayerId(0),
            card
        }
    );
    assert_eq!(g.instance(card).location, Location::Graveyard);
    assert_eq!(g.player(PlayerId(0)).graveyard, vec![card]);
    assert_eq!(g.player(PlayerId(0)).hand.len(), 4);
    assert_eq!(g.phase(), Phase::Main1);

    // Player 1 draws to 6 on turn 2 and must discard twice.
    play(&mut g, Action::EndTurn);
    assert_eq!(g.turn_player(), PlayerId(1));
    assert_eq!(g.phase(), Phase::Draw);
    assert_eq!(g.legal_actions().len(), 6);
    let a = g.legal_actions()[0].clone();
    play(&mut g, a);
    assert_eq!(g.phase(), Phase::Draw);
    let a = g.legal_actions()[0].clone();
    play(&mut g, a);
    assert_eq!(g.phase(), Phase::Main1);
    assert_eq!(g.player(PlayerId(1)).hand.len(), 4);
}

#[test]
fn flip_hand_limit_check_to_the_official_end_phase() {
    let shared = shared_with(|r| {
        r.hand.max_size = 4;
        r.hand.hand_limit_check = HandLimitCheck::EndPhase;
    });
    let mut g = Game::new(shared, &setup(1)).unwrap();
    assert_eq!(g.phase(), Phase::Main1, "no discard in the Draw Phase");
    assert_eq!(g.player(PlayerId(0)).hand.len(), 5);

    play(&mut g, Action::EndTurn);
    assert_eq!(g.phase(), Phase::End);
    assert_eq!(g.turn(), 1);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (PlayerId(0), DecisionKind::HandLimit));
    let a = g.legal_actions()[0].clone();
    play(&mut g, a);
    assert_eq!(g.turn(), 2);
    assert_eq!(g.turn_player(), PlayerId(1));
    assert_eq!(g.player(PlayerId(0)).hand.len(), 4);
}

#[test]
fn hand_limit_tests_cover_both_modes() {
    // Guards the engine against a third mode being added without a test.
    for mode in [HandLimitCheck::AfterDraw, HandLimitCheck::EndPhase] {
        let shared = shared_with(|r| r.hand.hand_limit_check = mode);
        let g = Game::new(shared, &setup(1)).unwrap();
        assert_eq!(g.player(PlayerId(0)).hand.len(), 5);
    }
}
