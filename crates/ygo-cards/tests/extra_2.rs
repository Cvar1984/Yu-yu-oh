//! Rulings tests for the last EXTRA cards (pool-440 plan, batch 2), on the
//! modern text. The board helpers are the PSV tests' (`psv_common`).

use ygo_cards::continuous::LEGENDARY_SWORD;
use ygo_cards::extra_spells::{SCAPEGOAT, SHEEP_TOKEN, SOUL_EXCHANGE};
use ygo_cards::extra_traps::*;
use ygo_cards::oneshot::{DARKNESS_APPROACHES, RUSH_RECKLESSLY, SHIELD_AND_SWORD};
use ygo_cards::spells::{CHANGE_OF_HEART, DARK_HOLE};
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SUMMONED_SKULL, check_invariants, in_hand, next_turn, play,
    stacked,
};
use ygo_engine::{Action, DecisionKind, Game, Location, Position, SetupError};

mod psv_common;
use psv_common::*;

// D. Tribe: the face-up monsters you control at resolution are Dragons
// until the End Phase, so Dragon cards (Burst Breath) see them.
#[test]
fn d_tribe_makes_your_face_up_monsters_dragons_this_turn() {
    let mut g = game(&[CELTIC_GUARDIAN, D_TRIBE, BURST_BREATH, CYBER_FALCON], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, D_TRIBE);
    set_card(&mut g, P0, BURST_BREATH);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.race(celtic), "Warrior");
    assert!(
        activations(&g, BURST_BREATH).is_empty(),
        "no Dragon to Tribute"
    );
    activate(&mut g, D_TRIBE);
    assert_eq!(g.race(celtic), "Dragon");
    assert!(!activations(&g, BURST_BREATH).is_empty());
    let falcon = summon_now(&mut g, P0, CYBER_FALCON);
    assert_eq!(g.race(falcon), "Machine", "summoned after the resolution");
    next_turn(&mut g);
    assert_eq!(g.race(celtic), "Warrior", "ends in the End Phase");
}

// Riryoku Field: a Spell that targets exactly 1 monster on the field is
// negated and destroyed.
#[test]
fn riryoku_field_negates_a_spell_that_targets_one_monster() {
    let mut g = game(&[CELTIC_GUARDIAN, RUSH_RECKLESSLY], &[RIRYOKU_FIELD]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let riryoku = set_card(&mut g, P1, RIRYOKU_FIELD);
    next_turn(&mut g);
    let rush = in_hand(&g, P0, RUSH_RECKLESSLY);
    let a = activations(&g, RUSH_RECKLESSLY).remove(0);
    play(&mut g, a);
    respond(&mut g, P1, RIRYOKU_FIELD, None);
    assert_eq!(g.atk(celtic), 1400, "Rush Recklessly was negated");
    assert_eq!(g.instance(rush).location, Location::Graveyard);
    assert_eq!(g.instance(riryoku).location, Location::Graveyard);
}

#[test]
fn riryoku_field_ignores_a_spell_without_a_single_monster_target() {
    let mut g = game(&[CELTIC_GUARDIAN, DARK_HOLE], &[RIRYOKU_FIELD]);
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    set_card(&mut g, P1, RIRYOKU_FIELD);
    next_turn(&mut g);
    let a = activations(&g, DARK_HOLE).remove(0);
    play(&mut g, a);
    while let Some(d) = g.pending_decision() {
        if d.kind != DecisionKind::Respond {
            break;
        }
        assert!(activations(&g, RIRYOKU_FIELD).is_empty());
        play(&mut g, Action::Pass);
    }
}

// Shift, attack: the attack moves to another monster the attacker could
// attack, with no replay.
#[test]
fn shift_moves_an_attack_to_another_of_your_monsters() {
    let mut g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF, SHIFT], &[KOUMORI_DRAGON]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let elf = set_monster(&mut g, P0, MYSTICAL_ELF);
    set_card(&mut g, P0, SHIFT);
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    attack(&mut g, koumori, Some(celtic));
    respond(&mut g, P0, SHIFT, Some(&[elf]));
    assert_eq!(g.instance(celtic).location, Location::MonsterZone(0));
    assert_eq!(lp(&g, P1), 7500, "1500 ATK into the Elf's 2000 DEF");
    assert!(g.on_field(elf) && g.on_field(koumori));
}

// Shift, Spell: the opponent's single-target Spell takes the new target.
#[test]
fn shift_moves_a_spell_to_another_of_your_monsters() {
    let mut g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF, SHIFT], &[CHANGE_OF_HEART]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let elf = summon_now(&mut g, P0, MYSTICAL_ELF);
    set_card(&mut g, P0, SHIFT);
    next_turn(&mut g);
    let a = activations(&g, CHANGE_OF_HEART)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, SHIFT, None);
    assert_eq!(
        g.instance(elf).controller,
        P1,
        "Change of Heart took the Elf"
    );
    assert_eq!(g.instance(celtic).controller, P0);
}

#[test]
fn shift_needs_a_second_monster() {
    let mut g = game(&[CELTIC_GUARDIAN, SHIFT], &[KOUMORI_DRAGON]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, SHIFT);
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    attack(&mut g, koumori, Some(celtic));
    while let Some(d) = g.pending_decision() {
        if d.kind != DecisionKind::Respond {
            break;
        }
        assert!(activations(&g, SHIFT).is_empty());
        play(&mut g, Action::Pass);
    }
}

// Reverse Trap: additions subtract and subtractions add until the End
// Phase, whether the change came before or after it.
#[test]
fn reverse_trap_reverses_additions_until_the_end_phase() {
    let mut g = game(
        &[
            CELTIC_GUARDIAN,
            REVERSE_TRAP,
            LEGENDARY_SWORD,
            RUSH_RECKLESSLY,
        ],
        &[],
    );
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, REVERSE_TRAP);
    next_turn(&mut g);
    next_turn(&mut g);
    activate_on(&mut g, LEGENDARY_SWORD, &[celtic]);
    assert_eq!(g.atk(celtic), 1700);
    activate(&mut g, REVERSE_TRAP);
    assert_eq!((g.atk(celtic), g.def(celtic)), (1100, 900));
    activate_on(&mut g, RUSH_RECKLESSLY, &[celtic]);
    assert_eq!(g.atk(celtic), 400, "1400 - 300 - 700");
    next_turn(&mut g);
    assert_eq!(g.atk(celtic), 1700, "only the Sword's 300 is left");
}

#[test]
fn reverse_trap_leaves_swaps_and_multiplications_alone() {
    let mut g = game(&[CELTIC_GUARDIAN, REVERSE_TRAP, SHIELD_AND_SWORD], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, REVERSE_TRAP);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, SHIELD_AND_SWORD);
    assert_eq!((g.atk(celtic), g.def(celtic)), (1200, 1400));
    activate(&mut g, REVERSE_TRAP);
    assert_eq!((g.atk(celtic), g.def(celtic)), (1200, 1400));
}

// Light of Intervention: a Set is a face-up Defense Normal Summon for both
// players, and nothing turns a monster face-down.
#[test]
fn light_of_intervention_turns_sets_into_face_up_defense_summons() {
    let mut g = game(
        &[
            CELTIC_GUARDIAN,
            LIGHT_OF_INTERVENTION,
            MYSTICAL_ELF,
            DARKNESS_APPROACHES,
        ],
        &[KOUMORI_DRAGON],
    );
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, LIGHT_OF_INTERVENTION);
    next_turn(&mut g);
    let koumori = set_monster(&mut g, P1, KOUMORI_DRAGON);
    assert_eq!(g.instance(koumori).position, Position::FaceDownDefense);
    next_turn(&mut g);
    activate(&mut g, LIGHT_OF_INTERVENTION);
    let elf = set_monster(&mut g, P0, MYSTICAL_ELF);
    assert_eq!(g.instance(elf).position, Position::FaceUpDefense);
    assert!(
        activations(&g, DARKNESS_APPROACHES).is_empty(),
        "nothing can be turned face-down"
    );
    next_turn(&mut g);
    let other = in_hand(&g, P1, PAD);
    play(
        &mut g,
        Action::Summon {
            card: other,
            tributes: vec![],
            face_down: true,
        },
    );
    assert_eq!(g.instance(other).position, Position::FaceUpDefense);
}

// Soul Exchange: this turn the Tribute Summon uses the opponent's monster,
// and the Battle Phase is gone.
#[test]
fn soul_exchange_tributes_the_opponents_monster_and_ends_the_battle_phase() {
    let mut g = game(
        &[CELTIC_GUARDIAN, SOUL_EXCHANGE, SUMMONED_SKULL],
        &[KOUMORI_DRAGON],
    );
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    next_turn(&mut g);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let tributes = |g: &ygo_engine::Game| -> Vec<Vec<ygo_engine::InstanceId>> {
        g.legal_actions()
            .into_iter()
            .filter_map(|a| match a {
                Action::Summon { card, tributes, .. } if card == skull => Some(tributes),
                _ => None,
            })
            .collect()
    };
    assert!(tributes(&g).iter().all(|t| t == &vec![celtic]));
    activate_on(&mut g, SOUL_EXCHANGE, &[koumori]);
    assert!(!g.legal_actions().contains(&Action::EnterBattlePhase));
    let offered = tributes(&g);
    assert!(!offered.is_empty() && offered.iter().all(|t| t == &vec![koumori]));
    play(
        &mut g,
        Action::Summon {
            card: skull,
            tributes: vec![koumori],
            face_down: false,
        },
    );
    assert_eq!(g.instance(koumori).location, Location::Graveyard);
    assert!(g.graveyard(P1).contains(&koumori));
    assert!(g.on_field(celtic));
}

fn sheep(g: &Game, p: ygo_engine::PlayerId) -> Vec<ygo_engine::InstanceId> {
    g.monsters_controlled(p)
        .into_iter()
        .filter(|&m| g.stats(m).id == SHEEP_TOKEN)
        .collect()
}

// Scapegoat: four Sheep Tokens in face-up Defense Position; no other
// summon that turn, but a Set; the Tokens are not Tribute Summon material.
#[test]
fn scapegoat_summons_four_sheep_and_locks_other_summons() {
    let mut g = game(&[SCAPEGOAT, CELTIC_GUARDIAN, SUMMONED_SKULL], &[]);
    activate(&mut g, SCAPEGOAT);
    let tokens = sheep(&g, P0);
    assert_eq!(tokens.len(), 4);
    for &t in &tokens {
        assert_eq!(g.instance(t).position, Position::FaceUpDefense);
        assert_eq!(g.instance(t).owner, P0);
    }
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let summons: Vec<Action> = g
        .legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Summon { card, .. } if *card == celtic || *card == skull))
        .collect();
    assert_eq!(
        summons,
        vec![Action::Summon {
            card: celtic,
            tributes: vec![],
            face_down: true,
        }],
        "only a Set; the Skull would need a Sheep as its Tribute"
    );
    check_invariants(&g);
}

#[test]
fn scapegoat_cannot_follow_a_summon() {
    let mut g = game(&[CELTIC_GUARDIAN, SCAPEGOAT], &[]);
    pass_all_windows(&mut g);
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    assert!(activations(&g, SCAPEGOAT).is_empty());
}

// A Token that leaves the field is gone: it never reaches the Graveyard,
// and a later Scapegoat reuses its instance.
#[test]
fn sheep_tokens_vanish_when_they_leave_the_field() {
    let mut g = game(&[SCAPEGOAT, SCAPEGOAT], &[DARK_HOLE]);
    activate(&mut g, SCAPEGOAT);
    let first = sheep(&g, P0);
    let instances = g.instances().len();
    next_turn(&mut g);
    activate(&mut g, DARK_HOLE);
    for &t in &first {
        assert_eq!(g.instance(t).location, Location::Gone);
    }
    assert!(
        g.graveyard(P0)
            .iter()
            .all(|&c| g.stats(c).id != SHEEP_TOKEN)
    );
    check_invariants(&g);
    next_turn(&mut g);
    activate(&mut g, SCAPEGOAT);
    assert_eq!(sheep(&g, P0).len(), 4);
    assert_eq!(
        g.instances().len(),
        instances,
        "the gone Tokens were reused"
    );
    check_invariants(&g);
}

#[test]
fn no_deck_may_hold_a_token() {
    let mut deck = vec![PAD; 40];
    deck[0] = SHEEP_TOKEN;
    let err = Game::new(shared(), &stacked([&deck, &[]], PAD)).unwrap_err();
    assert!(matches!(err, SetupError::TokenInDeck { .. }), "{err}");
}

// Ultimate Offering: pay 500 LP, then Normal Summon or Set at once, on top
// of the turn's Normal Summon, Tributes included.
#[test]
fn ultimate_offering_grants_an_extra_tribute_summon() {
    let mut g = game(&[CELTIC_GUARDIAN, ULTIMATE_OFFERING, SUMMONED_SKULL], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, ULTIMATE_OFFERING);
    next_turn(&mut g);
    next_turn(&mut g);
    let offering = in_hand_or_field(&g, ULTIMATE_OFFERING);
    let a = activations(&g, ULTIMATE_OFFERING)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { effect: 0, .. }))
        .unwrap();
    play(&mut g, a);
    pass_all_windows(&mut g);
    assert!(g.instance(offering).position.is_face_up());
    let used = g.player(P0).summons_used;
    activate(&mut g, ULTIMATE_OFFERING);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    choose(&mut g, skull);
    choose(&mut g, celtic);
    answer(&mut g, 0);
    assert_eq!(g.instance(skull).position, Position::FaceUpAttack);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert_eq!(lp(&g, P0), 7500);
    assert_eq!(
        g.player(P0).summons_used,
        used,
        "the turn's summon is untouched"
    );
}

#[test]
fn ultimate_offering_sets_a_monster_in_the_opponents_battle_phase() {
    let mut g = game(&[ULTIMATE_OFFERING, MYSTICAL_ELF], &[KOUMORI_DRAGON]);
    set_card(&mut g, P0, ULTIMATE_OFFERING);
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    play(&mut g, Action::EnterBattlePhase);
    // The Trap Card first, as the Battle Phase starts; its effect later.
    respond(&mut g, P0, ULTIMATE_OFFERING, None);
    attack(&mut g, koumori, None);
    respond(&mut g, P0, ULTIMATE_OFFERING, None);
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    choose(&mut g, elf);
    answer(&mut g, 1);
    assert_eq!(g.instance(elf).position, Position::FaceDownDefense);
    // 500 paid, and the direct attack goes on: the engine replays an
    // attack only when its target leaves, not when a monster joins.
    assert_eq!(lp(&g, P0), 8000 - 500 - 1500);
    check_invariants(&g);
}

fn in_hand_or_field(g: &Game, id: ygo_data::CardId) -> ygo_engine::InstanceId {
    (0..g.instances().len())
        .map(|i| ygo_engine::InstanceId(i as u8))
        .find(|&c| g.stats(c).id == id && g.instance(c).owner == P0)
        .unwrap()
}

fn pass_all_windows(g: &mut Game) {
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Respond)
    {
        play(g, Action::Pass);
    }
}
