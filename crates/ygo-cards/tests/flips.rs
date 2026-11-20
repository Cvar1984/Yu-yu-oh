//! Rulings tests for the FLIP monsters and Dragon Capture Jar.

use std::sync::Arc;

use ygo_cards::continuous::SWORDS_OF_REVEALING_LIGHT;
use ygo_cards::flips::*;
use ygo_cards::oneshot::SPARKS;
use ygo_cards::spells::DARK_HOLE;
use ygo_cards::traps::MIRROR_FORCE;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, MYSTICAL_ELF, SEVEN_COLORED_FISH, SUMMONED_SKULL, answer,
    in_hand, next_turn, pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{
    Action, DecisionKind, Game, InstanceId, Location, Phase, PlayerId, Position, Shared,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const ONE_EYED_SHIELD_DRAGON: CardId = CardId(33064647); // Dragon, level 3
const THE_13TH_GRAVE: CardId = CardId(32864); // Zombie, 1200/900

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.summon.normal_or_tribute_per_turn = 5;
        r.summon.tributes_level_5_6 = 0;
        r.hand.max_size = 30;
    })
}

fn game(top: [&[CardId]; 2]) -> Game {
    Game::new(shared(), &stacked(top, MYSTICAL_ELF)).unwrap()
}

fn set_monster(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(
        g,
        Action::Summon {
            card,
            tributes: vec![],
            face_down: true,
        },
    );
    pass_all(g);
    card
}

fn put(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

fn set_spell_trap(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    pass_all(g);
    card
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

/// Flip Summons `card`; if its FLIP effect offers a choice of targets,
/// takes the one on `target`.
fn flip(g: &mut Game, card: InstanceId, target: Option<InstanceId>) {
    play(g, Action::FlipSummon { card });
    pass_all(g);
    if g.pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Triggers)
    {
        let a = g
            .legal_actions()
            .into_iter()
            .find(|a| match (a, target) {
                (Action::Activate { targets, .. }, Some(t)) => targets == &vec![t],
                (Action::Activate { .. }, None) => true,
                _ => false,
            })
            .unwrap_or_else(|| panic!("no FLIP activation on {target:?}: {:?}", g.legal_actions()));
        play(g, a);
        pass_all(g);
    }
}

/// Answers every pending choice and question with its first option.
fn first_choices(g: &mut Game) -> usize {
    let mut n = 0;
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Choose)
    {
        let a = g.legal_actions()[0].clone();
        play(g, a);
        pass_all(g);
        n += 1;
    }
    n
}

#[test]
fn reaper_of_the_cards_destroys_a_trap_and_spares_a_set_spell() {
    for (pick_trap, destroyed) in [(true, true), (false, false)] {
        let mut g = game([&[REAPER_OF_THE_CARDS], &[MIRROR_FORCE, SPARKS]]);
        let reaper = set_monster(&mut g, P0, REAPER_OF_THE_CARDS);
        next_turn(&mut g);
        let mirror = set_spell_trap(&mut g, P1, MIRROR_FORCE);
        let sparks = set_spell_trap(&mut g, P1, SPARKS);
        next_turn(&mut g);
        let target = if pick_trap { mirror } else { sparks };
        flip(&mut g, reaper, Some(target));
        assert_eq!(
            g.instance(target).location == Location::Graveyard,
            destroyed
        );
        assert_eq!(
            g.instance(target).position,
            if destroyed {
                Position::FaceUp
            } else {
                Position::FaceDown
            }
        );
    }
}

#[test]
fn armed_ninja_destroys_a_spell() {
    let mut g = game([&[ARMED_NINJA], &[SPARKS]]);
    let ninja = set_monster(&mut g, P0, ARMED_NINJA);
    next_turn(&mut g);
    let sparks = set_spell_trap(&mut g, P1, SPARKS);
    next_turn(&mut g);
    flip(&mut g, ninja, Some(sparks));
    assert_eq!(g.instance(sparks).location, Location::Graveyard);
}

#[test]
fn hane_hane_returns_a_monster_to_its_owners_hand() {
    let mut g = game([&[HANE_HANE], &[CELTIC_GUARDIAN]]);
    let hane = set_monster(&mut g, P0, HANE_HANE);
    next_turn(&mut g);
    let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    flip(&mut g, hane, Some(celtic));
    assert_eq!(g.instance(celtic).location, Location::Hand);
    assert!(g.player(P1).hand.contains(&celtic));
}

#[test]
fn mask_of_darkness_returns_a_trap_from_the_graveyard() {
    let base = shared();
    let tight = with_rules(&base, |r| {
        r.hand.max_size = 4;
        r.hand.hand_limit_check = ygo_data::HandLimitCheck::AfterDraw;
    });
    let mut g = Game::new(
        tight,
        &stacked([&[MIRROR_FORCE, MASK_OF_DARKNESS], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    // The after-draw hand limit discards Mirror Force on turn 1.
    let mirror = in_hand(&g, P0, MIRROR_FORCE);
    play(&mut g, Action::Discard { card: mirror });
    let mask = set_monster(&mut g, P0, MASK_OF_DARKNESS);
    next_turn(&mut g);
    next_turn(&mut g);
    flip(&mut g, mask, Some(mirror));
    assert_eq!(g.instance(mirror).location, Location::Hand);
}

#[test]
fn dragon_capture_jar_locks_dragons_in_defense_until_dragon_piper_breaks_it() {
    let mut g = game([
        &[DRAGON_CAPTURE_JAR],
        &[ONE_EYED_SHIELD_DRAGON, DRAGON_PIPER, ONE_EYED_SHIELD_DRAGON],
    ]);
    let jar = set_spell_trap(&mut g, P0, DRAGON_CAPTURE_JAR);
    next_turn(&mut g);
    let dragon = in_hand(&g, P1, ONE_EYED_SHIELD_DRAGON);
    play(&mut g, summon(dragon));
    // Player 0 answers the summon with the Jar.
    while g
        .pending_decision()
        .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond)
        && activations(&g, DRAGON_CAPTURE_JAR).is_empty()
    {
        play(&mut g, Action::Pass);
    }
    let a = activations(&g, DRAGON_CAPTURE_JAR)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(
        g.instance(jar).position,
        Position::FaceUp,
        "a Continuous Trap stays"
    );
    assert_eq!(g.instance(dragon).position, Position::FaceUpDefense);
    let dragon2 = put(&mut g, P1, ONE_EYED_SHIELD_DRAGON);
    assert_eq!(
        g.instance(dragon2).position,
        Position::FaceUpDefense,
        "a newcomer is moved too"
    );
    let piper = set_monster(&mut g, P1, DRAGON_PIPER);
    next_turn(&mut g);
    next_turn(&mut g);
    assert!(
        !g.legal_actions()
            .contains(&Action::ChangePosition { card: dragon }),
        "locked"
    );
    flip(&mut g, piper, None);
    assert_eq!(g.instance(jar).location, Location::Graveyard);
    assert_eq!(g.instance(dragon).position, Position::FaceUpAttack);
    assert_eq!(g.instance(dragon2).position, Position::FaceUpAttack);
}

#[test]
fn castle_of_dark_illusions_powers_up_zombies_for_four_standby_phases() {
    let mut g = game([&[THE_13TH_GRAVE, CASTLE_OF_DARK_ILLUSIONS], &[]]);
    let grave = put(&mut g, P0, THE_13TH_GRAVE);
    let castle = set_monster(&mut g, P0, CASTLE_OF_DARK_ILLUSIONS);
    next_turn(&mut g);
    next_turn(&mut g);
    flip(&mut g, castle, None);
    assert_eq!(g.atk(grave), 1400);
    let mut expected = 1400;
    for _ in 0..5 {
        next_turn(&mut g);
        next_turn(&mut g);
        expected = (expected + 200).min(2200);
        assert_eq!(g.atk(grave), expected, "turn {}", g.turn());
    }
    assert_eq!(g.def(grave), 900 + 1000);
}

#[test]
fn princess_of_tsurugi_burns_for_each_opposing_spell_and_trap() {
    let mut g = game([&[PRINCESS_OF_TSURUGI], &[SPARKS, MIRROR_FORCE]]);
    let princess = set_monster(&mut g, P0, PRINCESS_OF_TSURUGI);
    next_turn(&mut g);
    set_spell_trap(&mut g, P1, SPARKS);
    set_spell_trap(&mut g, P1, MIRROR_FORCE);
    next_turn(&mut g);
    flip(&mut g, princess, None);
    assert_eq!(g.player(P1).lp, 7000);
}

#[test]
fn the_immortal_of_thunder_gives_3000_and_takes_5000_when_it_dies() {
    let mut g = game([&[THE_IMMORTAL_OF_THUNDER, DARK_HOLE], &[]]);
    let immortal = set_monster(&mut g, P0, THE_IMMORTAL_OF_THUNDER);
    next_turn(&mut g);
    next_turn(&mut g);
    flip(&mut g, immortal, None);
    assert_eq!(g.player(P0).lp, 11000);
    let a = activations(&g, DARK_HOLE)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(g.player(P0).lp, 6000);
}

#[test]
fn weather_report_breaks_swords_and_grants_a_second_battle_phase() {
    let mut g = game([
        &[WEATHER_REPORT, CELTIC_GUARDIAN],
        &[SWORDS_OF_REVEALING_LIGHT],
    ]);
    let report = set_monster(&mut g, P0, WEATHER_REPORT);
    next_turn(&mut g);
    let swords = in_hand(&g, P1, SWORDS_OF_REVEALING_LIGHT);
    let a = activations(&g, SWORDS_OF_REVEALING_LIGHT)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    assert!(
        g.instance(report).position.is_face_up(),
        "Swords flipped it"
    );
    assert_eq!(
        g.instance(swords).location,
        Location::Graveyard,
        "its FLIP effect destroyed Swords"
    );
    next_turn(&mut g);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    pass_all(&mut g);
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    assert_eq!(g.phase(), Phase::Main2);
    assert!(
        g.legal_actions().contains(&Action::EnterBattlePhase),
        "a second Battle Phase"
    );
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.player(P1).lp, 8000 - 2800);
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    assert!(
        !g.legal_actions().contains(&Action::EnterBattlePhase),
        "only one extra"
    );
}

#[test]
fn invader_of_the_throne_swaps_control_with_an_opposing_monster() {
    let mut g = game([&[INVADER_OF_THE_THRONE], &[CELTIC_GUARDIAN]]);
    let invader = set_monster(&mut g, P0, INVADER_OF_THE_THRONE);
    next_turn(&mut g);
    let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    flip(&mut g, invader, Some(celtic));
    assert_eq!(g.instance(celtic).controller, P0);
    assert_eq!(g.instance(invader).controller, P1);
}

#[test]
fn jigen_bakudan_blows_up_its_own_side_in_the_next_standby_phase() {
    let mut g = game([&[JIGEN_BAKUDAN, CELTIC_GUARDIAN, SEVEN_COLORED_FISH], &[]]);
    let jigen = set_monster(&mut g, P0, JIGEN_BAKUDAN);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let fish = put(&mut g, P0, SEVEN_COLORED_FISH);
    next_turn(&mut g);
    next_turn(&mut g);
    flip(&mut g, jigen, None);
    next_turn(&mut g);
    play(&mut g, Action::EndTurn);
    assert_eq!(g.phase(), Phase::Standby);
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond)
    );
    let a = activations(&g, JIGEN_BAKUDAN)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    for c in [jigen, celtic, fish] {
        assert_eq!(g.instance(c).location, Location::Graveyard);
    }
    assert_eq!(g.player(P1).lp, 8000 - (1400 + 1800) / 2);
}

/// Player 0's deck: Cyber Jar, four Elves, a draw on turn 3, then the
/// five cards Cyber Jar reveals. Player 1 (five Sparks in hand, no draw
/// on their first turn) reveals two Elves and three Sparks.
fn cyber_jar_game(rules: impl FnOnce(&mut ygo_data::Rules)) -> (Game, InstanceId) {
    let base = shared();
    let shared = with_rules(&base, rules);
    let p0 = [
        CYBER_JAR,
        MYSTICAL_ELF,
        MYSTICAL_ELF,
        MYSTICAL_ELF,
        MYSTICAL_ELF,
        MYSTICAL_ELF,
        CELTIC_GUARDIAN,
        SUMMONED_SKULL,
        SPARKS,
        SEVEN_COLORED_FISH,
        MAMMOTH_GRAVEYARD,
    ];
    let p1 = [
        SPARKS,
        SPARKS,
        SPARKS,
        SPARKS,
        SPARKS,
        MYSTICAL_ELF,
        MYSTICAL_ELF,
        SPARKS,
        SPARKS,
        SPARKS,
    ];
    let mut g = Game::new(shared, &stacked([&p0, &p1], CELTIC_GUARDIAN)).unwrap();
    let jar = set_monster(&mut g, P0, CYBER_JAR);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: jar });
    pass_all(&mut g);
    (g, jar)
}

#[test]
fn cyber_jar_summons_every_revealed_small_monster_and_hands_out_the_rest() {
    let (mut g, jar) = cyber_jar_game(|_| {});
    let decisions = first_choices(&mut g);
    assert_eq!(
        decisions,
        5 * 2,
        "five monsters, a pick and a position each"
    );
    assert_eq!(g.instance(jar).location, Location::Graveyard);
    assert_eq!(
        g.player(P0).monsters().count(),
        3,
        "Celtic Guardian, the Fish, Mammoth Graveyard"
    );
    assert_eq!(g.player(P1).monsters().count(), 2, "two Elves");
    assert!(
        g.player(P0)
            .hand
            .iter()
            .any(|&c| g.stats(c).id == SUMMONED_SKULL),
        "Level 6 to the hand"
    );
    assert_eq!(
        g.player(P1)
            .hand
            .iter()
            .filter(|&&c| g.stats(c).id == SPARKS)
            .count(),
        5 + 3,
        "the opening five and the three revealed"
    );
}

#[test]
fn cyber_jar_respects_a_cap_on_monsters_entering_per_turn() {
    let (mut g, _) = cyber_jar_game(|r| r.summon.max_monsters_entering_per_turn = 1);
    first_choices(&mut g);
    assert_eq!(g.player(P0).monsters().count(), 1);
    assert_eq!(g.player(P1).monsters().count(), 1);
    let p0_monsters_in_hand = g
        .player(P0)
        .hand
        .iter()
        .filter(|&&c| g.stats(c).is_monster())
        .count();
    assert!(
        p0_monsters_in_hand >= 3,
        "the two it could not summon and Summoned Skull"
    );
}

#[test]
fn spear_cretin_lets_each_player_revive_a_monster() {
    let mut g = game([&[SPEAR_CRETIN], &[SEVEN_COLORED_FISH]]);
    let cretin = set_monster(&mut g, P0, SPEAR_CRETIN);
    next_turn(&mut g);
    let fish = put(&mut g, P1, SEVEN_COLORED_FISH);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(cretin),
        },
    );
    pass_all(&mut g);
    // Flipped and destroyed by the same attack. Player 0 picks the Cretin
    // itself; player 1 has nothing in the Graveyard.
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Choose),
        "{:?}",
        g.pending_decision()
    );
    play(&mut g, Action::Choose { card: cretin });
    // Its owner picks the position: face-up Attack here.
    answer(&mut g, 0);
    assert!(g.on_field(cretin));
    assert_eq!(g.instance(cretin).position, Position::FaceUpAttack);
}
