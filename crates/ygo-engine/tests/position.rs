//! M1 step 4: Flip Summon and position change.

mod common;

use common::{
    MYSTICAL_ELF, in_hand, next_turn, play, shared, shared_with, stacked, summon, summons_offered,
};
use ygo_engine::{Action, Event, Game, PlayerId, Position};

const P0: PlayerId = PlayerId(0);

fn position_actions(g: &Game) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::FlipSummon { .. } | Action::ChangePosition { .. }))
        .collect()
}

#[test]
fn a_summoned_monster_changes_position_from_the_next_own_turn_and_once_per_turn() {
    let mut g = Game::new(shared(), &stacked([&[], &[]], MYSTICAL_ELF)).unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        position_actions(&g).is_empty(),
        "not in the turn it was summoned"
    );

    next_turn(&mut g);
    assert!(
        position_actions(&g).is_empty(),
        "the opponent cannot touch it"
    );
    next_turn(&mut g);
    assert_eq!(g.turn(), 3);
    assert_eq!(
        position_actions(&g),
        vec![Action::ChangePosition { card: elf }]
    );

    let events = play(&mut g, Action::ChangePosition { card: elf });
    assert_eq!(
        events[0],
        Event::PositionChanged {
            card: elf,
            position: Position::FaceUpDefense
        }
    );
    assert_eq!(g.instance(elf).position, Position::FaceUpDefense);
    assert!(position_actions(&g).is_empty(), "once per turn");

    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::ChangePosition { card: elf });
    assert_eq!(g.instance(elf).position, Position::FaceUpAttack);
}

#[test]
fn a_set_monster_flip_summons_from_the_next_own_turn() {
    let mut g = Game::new(shared(), &stacked([&[], &[]], MYSTICAL_ELF)).unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: Vec::new(),
            face_down: true,
        },
    );
    assert!(
        position_actions(&g).is_empty(),
        "not in the turn it was Set"
    );
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(position_actions(&g), vec![Action::FlipSummon { card: elf }]);
    assert!(
        !summons_offered(&g).is_empty(),
        "the Normal Summon is still free"
    );

    let events = play(&mut g, Action::FlipSummon { card: elf });
    assert!(events.contains(&Event::FlipSummoned {
        player: P0,
        card: elf
    }));
    assert_eq!(g.instance(elf).position, Position::FaceUpAttack);
    assert!(
        position_actions(&g).is_empty(),
        "a Flip Summon is this turn's position change"
    );
    assert!(
        !summons_offered(&g).is_empty(),
        "and does not use the Normal Summon by default"
    );
}

#[test]
fn flip_summon_counts_toward_limit_when_the_rules_say_so() {
    let shared = shared_with(|r| r.summon.flip_summon_counts_toward_limit = true);
    let mut g = Game::new(shared, &stacked([&[], &[]], MYSTICAL_ELF)).unwrap();
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
    next_turn(&mut g);

    // Using the Normal Summon first removes the Flip Summon.
    let mut used_first = g.clone();
    let elf2 = in_hand(&used_first, P0, MYSTICAL_ELF);
    play(&mut used_first, summon(elf2));
    assert!(position_actions(&used_first).is_empty());

    // Flip Summoning first removes the Normal Summon.
    play(&mut g, Action::FlipSummon { card: elf });
    assert_eq!(g.player(P0).summons_used, 1);
    assert!(summons_offered(&g).is_empty());
}
