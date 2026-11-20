//! Game records: round trip through JSON Lines and exact replay.

use std::sync::Arc;

use ygo_ai::{Player, RandomPlayer, play_game};
use ygo_engine::testing::{sample_deck, shared_from, with_rules};
use ygo_engine::{Action, GameRecord, GameSetup, RecordError, Shared};

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn random_record(shared: Arc<Shared>, seed: u64) -> (ygo_engine::Game, GameRecord) {
    let decks = [
        sample_deck("save_a_owner_current_40.json"),
        sample_deck("save_b_dark_plasma_40.json"),
    ];
    let setup = GameSetup::new(decks, seed);
    let mut a = RandomPlayer::new(seed);
    let mut b = RandomPlayer::new(seed + 1);
    let mut players: [&mut dyn Player; 2] = [&mut a, &mut b];
    play_game(
        shared,
        &setup,
        &mut players,
        ["random".into(), "random".into()],
    )
    .unwrap()
}

#[test]
fn records_round_trip_and_replay_to_the_same_final_game() {
    for seed in 0..40 {
        let (game, record) = random_record(shared(), seed);
        assert!(record.result.is_some());
        let text = record.to_jsonl();
        assert_eq!(
            text.lines().count(),
            record.actions.len() + 2,
            "header, actions, result"
        );
        let parsed = GameRecord::from_jsonl(&text).unwrap();
        assert_eq!(parsed, record);
        let replayed = parsed.replay(&shared()).unwrap();
        assert_eq!(replayed, game, "seed {seed}");
    }
}

#[test]
fn a_record_replays_under_its_own_rules_even_after_the_config_changes() {
    let base = shared();
    let odd = with_rules(&base, |r| {
        r.life.starting_lp = 4000;
        r.hand.max_size = 5;
    });
    let (game, record) = random_record(odd, 3);
    assert_eq!(record.header.rules.life.starting_lp, 4000);
    let replayed = record.replay(&base).unwrap();
    assert_eq!(replayed.result(), game.result());
    assert_eq!(
        replayed.player(ygo_engine::PlayerId(0)).lp,
        game.player(ygo_engine::PlayerId(0)).lp
    );
}

#[test]
fn a_tampered_record_fails_loudly() {
    let (_, mut record) = random_record(shared(), 5);
    let i = record
        .actions
        .iter()
        .position(|a| a.action == Action::EndTurn)
        .unwrap();
    record.actions[i].action = Action::EnterBattlePhase;
    let err = record.replay(&shared()).unwrap_err();
    assert!(
        matches!(
            err,
            RecordError::Illegal { .. }
                | RecordError::WrongPlayer { .. }
                | RecordError::ResultMismatch { .. }
        ),
        "{err}"
    );

    let (_, mut record) = random_record(shared(), 6);
    record.result = None;
    assert!(matches!(
        record.replay(&shared()),
        Err(RecordError::ResultMismatch { .. })
    ));
}

#[test]
fn malformed_files_are_rejected_with_their_line() {
    let (_, record) = random_record(shared(), 7);
    let text = record.to_jsonl();
    let wrong_schema = text.replacen("\"schema_version\":1", "\"schema_version\":2", 1);
    assert!(matches!(
        GameRecord::from_jsonl(&wrong_schema),
        Err(RecordError::Schema { found: 2, .. })
    ));
    let mut lines: Vec<&str> = text.lines().collect();
    lines[2] = "{not json";
    assert!(matches!(
        GameRecord::from_jsonl(&lines.join("\n")),
        Err(RecordError::Json { line: 3, .. })
    ));
    let no_header: String = text.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert!(matches!(
        GameRecord::from_jsonl(&no_header),
        Err(RecordError::Layout { line: 1, .. })
    ));
}
