//! M1 step 6: hidden information, serialisation and replay.

mod common;

use common::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, normal_deck, play,
    shared, stacked, summon,
};
use ygo_engine::{Action, Event, Game, GameSetup, InstanceId, PlayerId, Position, VisibleCard};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

#[test]
fn a_player_sees_their_own_hand_and_set_cards_but_not_the_opponents() {
    let mut g = Game::new(
        shared(),
        &stacked([&[MYSTICAL_ELF], &[CELTIC_GUARDIAN]], SEVEN_COLORED_FISH),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: Vec::new(),
            face_down: true,
        },
    );
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));

    let from_p0 = g.observe(P0);
    assert_eq!(from_p0.viewer, P0);
    assert_eq!(from_p0.turn, 2);
    assert!(from_p0.me().hand.iter().all(|c| c.card().is_some()));
    assert!(from_p0.opponent().hand.iter().all(|c| c.card().is_none()));
    assert_eq!(from_p0.opponent().hand.len(), 5);
    assert_eq!(from_p0.opponent().deck_count, 34);
    assert_eq!(
        from_p0.me().monster_zones[0],
        Some(VisibleCard::Known {
            instance: elf,
            card: MYSTICAL_ELF,
            position: Position::FaceDownDefense
        }),
        "my own Set monster is known to me"
    );
    assert_eq!(
        from_p0.opponent().monster_zones[0],
        Some(VisibleCard::Known {
            instance: celtic,
            card: CELTIC_GUARDIAN,
            position: Position::FaceUpAttack
        })
    );

    let from_p1 = g.observe(P1);
    assert_eq!(
        from_p1.opponent().monster_zones[0],
        Some(VisibleCard::Hidden {
            instance: elf,
            position: Position::FaceDownDefense
        }),
        "the opponent's Set monster is hidden"
    );
    assert_eq!(from_p1.decision, g.pending_decision());
}

#[test]
fn instance_numbers_do_not_follow_the_deck_list() {
    // Player 0's list is Elf first, then 39 Fish. If instance 0 were always
    // the first listed card, the opponent could identify it face-down.
    let elf_is_first: Vec<bool> = (0..16)
        .map(|seed| {
            let mut setup = GameSetup::new(
                [
                    common::stacked([&[MYSTICAL_ELF], &[]], SEVEN_COLORED_FISH).decks[0].clone(),
                    normal_deck(40),
                ],
                seed,
            );
            setup.shuffle = true;
            let g = Game::new(shared(), &setup).unwrap();
            g.stats(InstanceId(0)).id == MYSTICAL_ELF
        })
        .collect();
    assert!(elf_is_first.iter().any(|&b| !b), "{elf_is_first:?}");
}

#[test]
fn actions_events_setup_and_observations_round_trip_through_json() {
    let mut g = Game::new(
        shared(),
        &stacked([&[SEVEN_COLORED_FISH], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    let action = summon(fish);
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), action);

    let events = play(&mut g, action);
    let json = serde_json::to_string(&events).unwrap();
    assert_eq!(serde_json::from_str::<Vec<Event>>(&json).unwrap(), events);

    let setup = stacked([&[SEVEN_COLORED_FISH], &[]], MYSTICAL_ELF);
    let json = serde_json::to_string(&setup).unwrap();
    assert_eq!(serde_json::from_str::<GameSetup>(&json).unwrap(), setup);
    let without_shuffle_key = json.replace(",\"shuffle\":false", "");
    assert!(
        serde_json::from_str::<GameSetup>(&without_shuffle_key)
            .unwrap()
            .shuffle,
        "defaults to true"
    );

    let obs = g.observe(P0);
    let json = serde_json::to_string(&obs).unwrap();
    assert_eq!(
        serde_json::from_str::<ygo_engine::Observation>(&json).unwrap(),
        obs
    );
}

#[test]
fn replaying_the_recorded_actions_rebuilds_the_same_game() {
    let setup = stacked([&[SEVEN_COLORED_FISH], &[CELTIC_GUARDIAN]], MYSTICAL_ELF);
    let mut g = Game::new(shared(), &setup).unwrap();
    let mut record: Vec<Action> = Vec::new();
    let mut play_rec = |g: &mut Game, a: Action| {
        record.push(a.clone());
        play(g, a)
    };
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play_rec(&mut g, summon(fish));
    play_rec(&mut g, Action::EndTurn);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play_rec(&mut g, summon(celtic));
    play_rec(&mut g, Action::EndTurn);
    play_rec(&mut g, Action::EnterBattlePhase);
    play_rec(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(celtic),
        },
    );
    play_rec(&mut g, Action::EndBattlePhase);
    play_rec(&mut g, Action::EndTurn);
    assert_eq!(g.player(P1).lp, 7600);

    let mut replay = Game::new(shared(), &setup).unwrap();
    for a in &record {
        replay.apply(a).unwrap();
    }
    assert_eq!(replay, g);
    assert_eq!(replay.observe(P1), g.observe(P1));
}

#[test]
fn a_shuffled_replay_is_deterministic_too() {
    let setup = GameSetup::new([normal_deck(40), normal_deck(40)], 99);
    let mut a = Game::new(shared(), &setup).unwrap();
    let mut b = Game::new(shared(), &setup).unwrap();
    for _ in 0..30 {
        let actions = a.legal_actions();
        let action = actions[actions.len() / 2].clone();
        a.apply(&action).unwrap();
        b.apply(&action).unwrap();
        assert_eq!(a, b);
    }
}
