//! M1 step 5: the Battle Phase and the damage calculation rulings.

mod common;

use common::{
    CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, play,
    shared, shared_with, stacked, summon, summons_offered,
};
use std::sync::Arc;
use ygo_data::CardId;
use ygo_engine::{
    Action, BattleStep, DecisionKind, Event, Game, GameResult, InstanceId, Location, Phase,
    PlayerId, Position, Shared, WinReason,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

#[test]
fn battle_test_cards_are_what_we_think() {
    let shared = shared();
    let stats = |id| shared.pool.get(shared.pool.index_of(id).unwrap()).clone();
    assert_eq!(
        (stats(SEVEN_COLORED_FISH).atk, stats(SEVEN_COLORED_FISH).def),
        (1800, 800)
    );
    assert_eq!(
        (stats(CELTIC_GUARDIAN).atk, stats(CELTIC_GUARDIAN).def),
        (1400, 1200)
    );
    assert_eq!(
        (stats(MAMMOTH_GRAVEYARD).atk, stats(MAMMOTH_GRAVEYARD).def),
        (1200, 800)
    );
    assert_eq!(
        (stats(MYSTICAL_ELF).atk, stats(MYSTICAL_ELF).def),
        (800, 2000)
    );
}

/// Player 0 summons `attacker` on turn 1, player 1 puts `defender` on
/// the field in `position` by turn 4, and the game stands at player 0's
/// Main Phase 1 of turn 5 with both players at 8000.
fn board(
    shared: Arc<Shared>,
    attacker: CardId,
    defender: CardId,
    position: Position,
) -> (Game, InstanceId, InstanceId) {
    let mut g = Game::new(shared, &stacked([&[attacker], &[defender]], MYSTICAL_ELF)).unwrap();
    let a = in_hand(&g, P0, attacker);
    play(&mut g, summon(a));
    next_turn(&mut g);
    let d = in_hand(&g, P1, defender);
    play(
        &mut g,
        Action::Summon {
            card: d,
            tributes: Vec::new(),
            face_down: position == Position::FaceDownDefense,
        },
    );
    next_turn(&mut g);
    next_turn(&mut g);
    if position == Position::FaceUpDefense {
        play(&mut g, Action::ChangePosition { card: d });
    }
    next_turn(&mut g);
    assert_eq!(
        (g.turn(), g.turn_player(), g.phase()),
        (5, P0, Phase::Main1)
    );
    assert_eq!(g.instance(d).position, position);
    (g, a, d)
}

fn attack(g: &mut Game, attacker: InstanceId, target: Option<InstanceId>) -> Vec<Event> {
    play(g, Action::EnterBattlePhase);
    assert_eq!(g.phase(), Phase::Battle(BattleStep::Battle));
    assert_eq!(
        g.pending_decision().unwrap().kind,
        DecisionKind::BattlePhase
    );
    play(g, Action::Attack { attacker, target })
}

fn lp(g: &Game) -> (u32, u32) {
    (g.player(P0).lp, g.player(P1).lp)
}

fn in_graveyard(g: &Game, card: InstanceId) -> bool {
    g.instance(card).location == Location::Graveyard
}

#[test]
fn attack_position_higher_atk_destroys_and_damages_by_the_difference() {
    let (mut g, fish, celtic) = board(
        shared(),
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    let events = attack(&mut g, fish, Some(celtic));
    assert!(in_graveyard(&g, celtic) && !in_graveyard(&g, fish));
    assert_eq!(lp(&g), (8000, 7600));
    assert!(events.contains(&Event::DestroyedByBattle { card: celtic }));
    assert!(events.contains(&Event::DamageTaken {
        player: P1,
        amount: 400,
        lp: 7600
    }));
    assert_eq!(
        g.phase(),
        Phase::Battle(BattleStep::Battle),
        "back to the Battle Step"
    );
    assert!(g.instance(fish).attacked_this_turn);
}

#[test]
fn attack_position_lower_atk_loses_the_attacker() {
    let (mut g, celtic, fish) = board(
        shared(),
        CELTIC_GUARDIAN,
        SEVEN_COLORED_FISH,
        Position::FaceUpAttack,
    );
    attack(&mut g, celtic, Some(fish));
    assert!(in_graveyard(&g, celtic) && !in_graveyard(&g, fish));
    assert_eq!(lp(&g), (7600, 8000));
}

#[test]
fn equal_atk_destroys_both_with_no_damage() {
    let (mut g, a, b) = board(
        shared(),
        CELTIC_GUARDIAN,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    attack(&mut g, a, Some(b));
    assert!(in_graveyard(&g, a) && in_graveyard(&g, b));
    assert_eq!(lp(&g), (8000, 8000));
}

#[test]
fn atk_above_def_destroys_without_damage() {
    let (mut g, fish, celtic) = board(
        shared(),
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpDefense,
    );
    attack(&mut g, fish, Some(celtic));
    assert!(in_graveyard(&g, celtic) && !in_graveyard(&g, fish));
    assert_eq!(lp(&g), (8000, 8000));
}

#[test]
fn atk_below_def_damages_the_attacker_and_destroys_nothing() {
    let (mut g, fish, elf) = board(
        shared(),
        SEVEN_COLORED_FISH,
        MYSTICAL_ELF,
        Position::FaceUpDefense,
    );
    attack(&mut g, fish, Some(elf));
    assert!(!in_graveyard(&g, fish) && !in_graveyard(&g, elf));
    assert_eq!(lp(&g), (7800, 8000));
}

#[test]
fn atk_equal_to_def_does_nothing() {
    let (mut g, mammoth, celtic) = board(
        shared(),
        MAMMOTH_GRAVEYARD,
        CELTIC_GUARDIAN,
        Position::FaceUpDefense,
    );
    let events = attack(&mut g, mammoth, Some(celtic));
    assert!(!in_graveyard(&g, mammoth) && !in_graveyard(&g, celtic));
    assert_eq!(lp(&g), (8000, 8000));
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::DamageTaken { .. } | Event::DestroyedByBattle { .. }
    )));
}

#[test]
fn a_face_down_defender_is_flipped_face_up_and_stays_that_way() {
    let (mut g, fish, elf) = board(
        shared(),
        SEVEN_COLORED_FISH,
        MYSTICAL_ELF,
        Position::FaceDownDefense,
    );
    let events = attack(&mut g, fish, Some(elf));
    assert_eq!(events[2], Event::Flipped { card: elf });
    assert_eq!(g.instance(elf).position, Position::FaceUpDefense);
    assert_eq!(lp(&g), (7800, 8000), "Elf's 2000 DEF beats 1800 ATK");
}

#[test]
fn direct_attacks_only_when_the_opponent_controls_no_monsters() {
    let (mut g, fish, celtic) = board(
        shared(),
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(
        g.legal_actions(),
        vec![
            Action::Attack {
                attacker: fish,
                target: Some(celtic)
            },
            Action::EndBattlePhase
        ],
        "a monster on the field must be attacked"
    );
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);

    // Player 1 never attacked; remove Celtic Guardian by battle first.
    attack(&mut g, fish, Some(celtic));
    assert!(
        g.legal_actions() == vec![Action::EndBattlePhase],
        "each monster attacks once"
    );
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(
        g.legal_actions(),
        vec![
            Action::Attack {
                attacker: fish,
                target: None
            },
            Action::EndBattlePhase
        ]
    );
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: None,
        },
    );
    assert_eq!(lp(&g), (8000, 7600 - 1800));
}

#[test]
fn main_phase_two_follows_the_battle_phase() {
    let (mut g, fish, _) = board(
        shared(),
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    play(&mut g, Action::EnterBattlePhase);
    let events = play(&mut g, Action::EndBattlePhase);
    assert_eq!(
        events,
        vec![
            Event::PhaseEntered {
                phase: Phase::Battle(BattleStep::End)
            },
            Event::PhaseEntered {
                phase: Phase::Main2
            },
        ]
    );
    assert_eq!(g.phase(), Phase::Main2);
    let actions = g.legal_actions();
    assert!(
        summons_offered(&g).is_empty(),
        "owner: no summons in Main Phase 2"
    );
    assert!(
        actions.contains(&Action::ChangePosition { card: fish }),
        "a monster that did not attack"
    );
    assert!(!actions.contains(&Action::EnterBattlePhase));
    assert_eq!(actions.last(), Some(&Action::EndTurn));
    play(&mut g, Action::EndTurn);
    assert_eq!(g.turn(), 6);
}

#[test]
fn a_monster_that_attacked_cannot_change_position_in_main_phase_two() {
    let (mut g, fish, celtic) = board(
        shared(),
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    attack(&mut g, fish, Some(celtic));
    play(&mut g, Action::EndBattlePhase);
    assert!(
        !g.legal_actions()
            .contains(&Action::ChangePosition { card: fish })
    );
}

#[test]
fn flip_summon_in_main_phase_2() {
    let shared = shared_with(|r| r.summon.summon_in_main_phase_2 = true);
    let (mut g, _, _) = board(
        shared,
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    play(&mut g, Action::EnterBattlePhase);
    play(&mut g, Action::EndBattlePhase);
    assert!(!summons_offered(&g).is_empty());
}

#[test]
fn the_first_player_cannot_attack_on_turn_one_unless_the_rules_allow_it() {
    let mut g = Game::new(
        shared(),
        &stacked([&[SEVEN_COLORED_FISH], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    assert!(!g.legal_actions().contains(&Action::EnterBattlePhase));
    next_turn(&mut g);
    assert!(
        g.legal_actions().contains(&Action::EnterBattlePhase),
        "player 1 may attack on turn 2"
    );

    let shared = shared_with(|r| r.turn.first_player_can_attack = true);
    let mut g = Game::new(shared, &stacked([&[SEVEN_COLORED_FISH], &[]], MYSTICAL_ELF)).unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    attack(&mut g, fish, None);
    assert_eq!(lp(&g), (8000, 6200));
}

#[test]
fn life_points_reaching_zero_ends_the_game() {
    let shared = shared_with(|r| r.life.starting_lp = 1500);
    let (mut g, fish, celtic) = board(
        shared,
        SEVEN_COLORED_FISH,
        CELTIC_GUARDIAN,
        Position::FaceUpAttack,
    );
    attack(&mut g, fish, Some(celtic));
    assert_eq!(lp(&g), (1500, 1100));
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);
    let events = attack(&mut g, fish, None);
    assert_eq!(lp(&g), (1500, 0));
    assert_eq!(
        g.result(),
        Some(GameResult::Win {
            winner: P0,
            reason: WinReason::LifePoints
        })
    );
    assert!(matches!(events.last(), Some(Event::GameOver { .. })));
    assert!(g.legal_actions().is_empty());
}
