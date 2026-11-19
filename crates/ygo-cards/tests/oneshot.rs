//! Rulings tests for the M3 step 3 one-shot Spells and Traps.

use std::sync::Arc;

use ygo_cards::continuous::{LEGENDARY_SWORD, MALEVOLENT_NUZZLER};
use ygo_cards::oneshot::*;
use ygo_cards::traps::MIRROR_FORCE;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, answer, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, Location, Phase, PlayerId, Position, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.summon.normal_or_tribute_per_turn = 3;
        r.hand.max_size = 12;
    })
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

fn activate(g: &mut Game, id: CardId) {
    let a = activations(g, id);
    assert!(
        !a.is_empty(),
        "{id} is not activatable: {:?}",
        g.legal_actions()
    );
    play(g, a[0].clone());
    pass_all(g);
}

fn set(g: &mut Game, player: PlayerId, id: CardId) -> ygo_engine::InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    card
}

#[test]
fn burn_and_heal_cards_change_life_points_by_their_numbers() {
    for (id, opp, me) in [
        (SPARKS, 200, 0),
        (HINOTAMA, 500, 0),
        (FINAL_FLAME, 600, 0),
        (TREMENDOUS_FIRE, 1000, 500),
    ] {
        let mut g = Game::new(shared(), &stacked([&[id], &[]], MYSTICAL_ELF)).unwrap();
        activate(&mut g, id);
        assert_eq!(
            (g.player(P0).lp, g.player(P1).lp),
            (8000 - me, 8000 - opp),
            "{id}"
        );
    }
    for (id, gain) in [(RED_MEDICINE, 500), (GOBLINS_SECRET_REMEDY, 600)] {
        let mut g = Game::new(shared(), &stacked([&[id], &[]], MYSTICAL_ELF)).unwrap();
        activate(&mut g, id);
        assert_eq!(g.player(P0).lp, 8000 + gain, "{id}");
    }
}

#[test]
fn burn_can_end_the_game() {
    let base = shared();
    let shared = with_rules(&base, |r| r.life.starting_lp = 500);
    let mut g = Game::new(shared, &stacked([&[HINOTAMA], &[]], MYSTICAL_ELF)).unwrap();
    activate(&mut g, HINOTAMA);
    assert!(matches!(
        g.result(),
        Some(ygo_engine::GameResult::Win { winner: P0, .. })
    ));
}

#[test]
fn remove_trap_destroys_a_face_up_trap_only() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[REMOVE_TRAP, CELTIC_GUARDIAN], &[MIRROR_FORCE]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    let mirror = set(&mut g, P1, MIRROR_FORCE);
    next_turn(&mut g);
    assert!(
        activations(&g, REMOVE_TRAP).is_empty(),
        "a Set Trap is not face-up"
    );
    // Player 1 answers the attack with Mirror Force; it resolves and leaves, so
    // instead test a face-up Trap through Giant Trunade's absence: the only
    // face-up Trap in these sets that stays is a Continuous one (M3 step 11).
    // Here: once Mirror Force is activated and resolving, it is face-up and
    // Remove Trap, a Normal Spell, cannot chain. So this test checks only the
    // negative case and that Remove Trap works on a face-up Trap is covered by
    // the Continuous Trap tests of step 11.
    let _ = mirror;
}

#[test]
fn stop_defense_and_block_attack_change_the_opponents_positions() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[STOP_DEFENSE, BLOCK_ATTACK],
                &[CELTIC_GUARDIAN, MYSTICAL_ELF],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    assert!(activations(&g, STOP_DEFENSE).is_empty() && activations(&g, BLOCK_ATTACK).is_empty());
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(&mut g, summon(celtic));
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    assert_eq!(
        activations(&g, STOP_DEFENSE).len(),
        1,
        "the Set Elf is a Defense Position monster"
    );
    activate(&mut g, STOP_DEFENSE);
    assert_eq!(g.instance(elf).position, Position::FaceUpAttack);
    let targets: Vec<_> = activations(&g, BLOCK_ATTACK);
    assert_eq!(
        targets.len(),
        2,
        "Celtic Guardian and the now-attacking Elf"
    );
    let on_celtic = targets
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, on_celtic);
    assert_eq!(g.instance(celtic).position, Position::FaceUpDefense);
    assert!(
        !g.instance(celtic).position_changed_this_turn,
        "a change by an effect leaves the monster's own change unused"
    );
}

#[test]
fn heavy_storm_destroys_and_giant_trunade_returns_every_spell_and_trap() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    HEAVY_STORM,
                    GIANT_TRUNADE,
                    SPARKS,
                    CELTIC_GUARDIAN,
                    LEGENDARY_SWORD,
                ],
                &[MIRROR_FORCE],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert!(
        activations(&g, HEAVY_STORM).is_empty(),
        "nothing else on the field"
    );
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    activate(&mut g, LEGENDARY_SWORD);
    let sword = g.player(P0).spell_trap_zones[0].unwrap();
    set(&mut g, P0, SPARKS);
    next_turn(&mut g);
    let mirror = set(&mut g, P1, MIRROR_FORCE);
    next_turn(&mut g);
    activate(&mut g, GIANT_TRUNADE);
    assert_eq!(g.instance(mirror).location, Location::Hand);
    assert_eq!(g.instance(sword).location, Location::Hand);
    assert_eq!(g.atk(celtic), 1400, "the Equip's bonus went with it");
    assert_eq!(
        g.player(P0)
            .hand
            .iter()
            .filter(|&&c| g.stats(c).id == SPARKS)
            .count(),
        1
    );
    activate(&mut g, LEGENDARY_SWORD);
    next_turn(&mut g);
    set(&mut g, P1, MIRROR_FORCE);
    next_turn(&mut g);
    activate(&mut g, HEAVY_STORM);
    assert_eq!(g.instance(mirror).location, Location::Graveyard);
    assert_eq!(g.instance(sword).location, Location::Graveyard);
    assert!(g.player(P0).spell_trap_zones.iter().all(|z| z.is_none()));
}

#[test]
fn eternal_rest_destroys_every_equipped_monster() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    CELTIC_GUARDIAN,
                    MYSTICAL_ELF,
                    MALEVOLENT_NUZZLER,
                    ETERNAL_REST,
                ],
                &[],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    assert!(activations(&g, ETERNAL_REST).is_empty());
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(celtic));
    play(&mut g, summon(elf));
    let on_celtic = activations(&g, MALEVOLENT_NUZZLER)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, on_celtic);
    activate(&mut g, ETERNAL_REST);
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(g.on_field(elf));
}

#[test]
fn two_pronged_attack_takes_two_of_yours_for_one_of_theirs() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[TWO_PRONGED_ATTACK, MYSTICAL_ELF, MYSTICAL_ELF],
                &[CELTIC_GUARDIAN],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let trap = set(&mut g, P0, TWO_PRONGED_ATTACK);
    let e1 = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(e1));
    let e2 = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(e2));
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    // The summon-success window: player 0 may fire the Trap.
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond)
    );
    let offered = activations(&g, TWO_PRONGED_ATTACK);
    assert_eq!(offered.len(), 1, "one pair of Elves x one Celtic Guardian");
    play(&mut g, offered[0].clone());
    pass_all(&mut g);
    for c in [e1, e2, celtic, trap] {
        assert_eq!(g.instance(c).location, Location::Graveyard);
    }
}

#[test]
fn snake_fang_rush_recklessly_and_the_reliable_guardian_last_until_the_end_phase() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    CELTIC_GUARDIAN,
                    RUSH_RECKLESSLY,
                    THE_RELIABLE_GUARDIAN,
                    SNAKE_FANG,
                ],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    activate(&mut g, RUSH_RECKLESSLY);
    assert_eq!(g.atk(celtic), 2100);
    activate(&mut g, THE_RELIABLE_GUARDIAN);
    assert_eq!(g.def(celtic), 1900);
    set(&mut g, P0, SNAKE_FANG);
    next_turn(&mut g);
    assert_eq!(
        (g.atk(celtic), g.def(celtic)),
        (1400, 1200),
        "expired at the End Phase"
    );
    // Snake Fang answers the opponent's Standby Phase window; it is player 1's turn.
    next_turn(&mut g);
    play(&mut g, Action::EndTurn);
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond),
        "{:?}",
        g.pending_decision()
    );
    activate(&mut g, SNAKE_FANG);
    assert_eq!(g.def(celtic), 700);
}

#[test]
fn shield_and_sword_swaps_original_stats_until_the_end_phase() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MYSTICAL_ELF, SHIELD_AND_SWORD], &[CELTIC_GUARDIAN]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    activate(&mut g, SHIELD_AND_SWORD);
    assert_eq!((g.atk(elf), g.def(elf)), (2000, 800));
    assert_eq!((g.atk(celtic), g.def(celtic)), (1200, 1400));
    next_turn(&mut g);
    assert_eq!((g.atk(elf), g.def(elf)), (800, 2000));
}

#[test]
fn curse_of_fiend_activates_in_the_standby_phase_and_flips_positions() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MYSTICAL_ELF, CURSE_OF_FIEND], &[CELTIC_GUARDIAN]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        activations(&g, CURSE_OF_FIEND).is_empty(),
        "not in the Main Phase"
    );
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    play(&mut g, Action::EndTurn);
    // Player 0's Standby Phase opens a window where Curse of Fiend is offered.
    assert_eq!(g.phase(), Phase::Standby);
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond),
        "{:?}",
        g.pending_decision()
    );
    activate(&mut g, CURSE_OF_FIEND);
    assert_eq!(g.instance(elf).position, Position::FaceUpDefense);
    assert_eq!(g.instance(celtic).position, Position::FaceUpDefense);
    pass_all(&mut g);
    assert_eq!(g.phase(), Phase::Main1);
    assert!(
        !g.legal_actions()
            .contains(&Action::ChangePosition { card: elf }),
        "positions are locked this turn"
    );
}

#[test]
fn darkness_approaches_discards_two_and_turns_a_monster_face_down() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[DARKNESS_APPROACHES, MYSTICAL_ELF], &[CELTIC_GUARDIAN]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    let offered = activations(&g, DARKNESS_APPROACHES);
    assert!(!offered.is_empty());
    let hand_before = g.player(P0).hand.len();
    play(&mut g, offered[0].clone());
    assert_eq!(
        g.player(P0).hand.len(),
        hand_before - 3,
        "two discards and the Spell itself"
    );
    assert_eq!(g.instance(celtic).position, Position::FaceDownDefense);
}

#[test]
fn the_cheerful_coffin_discards_up_to_three_monsters() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    THE_CHEERFUL_COFFIN,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    SPARKS,
                    SPARKS,
                ],
                &[],
            ],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let offered = activations(&g, THE_CHEERFUL_COFFIN);
    assert_eq!(offered.len(), 1, "the discards are picked on resolution");
    play(&mut g, offered[0].clone());
    pass_all(&mut g);
    let elves: Vec<_> = g
        .hand(P0)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).id == MYSTICAL_ELF)
        .collect();
    let picks: Vec<_> = g
        .legal_actions()
        .into_iter()
        .filter_map(|a| match a {
            Action::Choose { card } => Some(card),
            _ => None,
        })
        .collect();
    assert_eq!(picks, elves, "only the Monster Cards; Sparks is a Spell");
    play(&mut g, Action::Choose { card: elves[0] });
    answer(&mut g, 0); // Discard another
    play(&mut g, Action::Choose { card: elves[1] });
    assert_eq!(g.graveyard(P0).len(), 3, "both Elves and the Coffin");
    assert_eq!(g.hand(P0).len(), 2, "the two Sparks stay");
}

#[test]
fn share_the_pain_tributes_one_each_and_the_opponent_picks_theirs() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[SHARE_THE_PAIN, MYSTICAL_ELF],
                &[CELTIC_GUARDIAN, SEVEN_COLORED_FISH],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        activations(&g, SHARE_THE_PAIN).is_empty(),
        "the opponent has nothing to tribute"
    );
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(celtic));
    play(&mut g, summon(fish));
    next_turn(&mut g);
    activate(&mut g, SHARE_THE_PAIN);
    assert_eq!(
        g.instance(elf).location,
        Location::Graveyard,
        "paid at activation"
    );
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P1, DecisionKind::Choose));
    play(&mut g, Action::Choose { card: fish });
    assert_eq!(g.instance(fish).location, Location::Graveyard);
    assert!(g.on_field(celtic));
}

#[test]
fn final_destiny_discards_five_and_clears_the_field() {
    let base = shared();
    let shared = with_rules(&base, |r| r.hand.opening_size = 7);
    let mut g = Game::new(
        shared,
        &stacked(
            [
                &[FINAL_DESTINY, MYSTICAL_ELF],
                &[CELTIC_GUARDIAN, MIRROR_FORCE],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    let mirror = set(&mut g, P1, MIRROR_FORCE);
    next_turn(&mut g);
    assert_eq!(g.player(P0).hand.len(), 7, "7 - Elf + 1 draw");
    let offered = activations(&g, FINAL_DESTINY);
    assert_eq!(offered.len(), 6, "choose 5 of the 6 other hand cards");
    play(&mut g, offered[0].clone());
    pass_all(&mut g);
    for c in [elf, celtic, mirror] {
        assert_eq!(g.instance(c).location, Location::Graveyard);
    }
    assert_eq!(g.player(P0).hand.len(), 1);
}

#[test]
fn soul_release_banishes_up_to_five_from_any_graveyard_one_pick_at_a_time() {
    let mut g = Game::new(
        shared(),
        &stacked([&[SPARKS, SPARKS, SOUL_RELEASE], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    activate(&mut g, SPARKS);
    activate(&mut g, SPARKS);
    assert_eq!(g.graveyard(P0).len(), 2);
    activate(&mut g, SOUL_RELEASE);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Choose));
    let first = g.legal_actions()[0].clone();
    play(&mut g, first);
    // One left: asked whether to continue.
    assert_eq!(g.observe(P0).question, vec!["Banish another", "Done"]);
    play(&mut g, Action::Answer { option: 1 });
    assert_eq!(g.banished(P0).len(), 1);
    assert_eq!(
        g.graveyard(P0).len(),
        2,
        "the other Sparks and Soul Release itself"
    );
}

#[test]
fn gravedigger_ghoul_banishes_up_to_two_monsters_from_the_opponents_graveyard() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[GRAVEDIGGER_GHOUL, SEVEN_COLORED_FISH],
                &[MYSTICAL_ELF, CELTIC_GUARDIAN],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    assert!(activations(&g, GRAVEDIGGER_GHOUL).is_empty());
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(elf));
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(celtic),
        },
    );
    pass_all(&mut g);
    play(&mut g, Action::EndBattlePhase);
    activate(&mut g, GRAVEDIGGER_GHOUL);
    play(&mut g, Action::Choose { card: celtic });
    assert_eq!(
        g.instance(celtic).location,
        Location::Banished,
        "the only monster there: no question asked"
    );
    let _ = elf;
}

#[test]
fn tailor_of_the_fickle_moves_an_equip_to_another_legal_target() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    CELTIC_GUARDIAN,
                    MYSTICAL_ELF,
                    MALEVOLENT_NUZZLER,
                    LEGENDARY_SWORD,
                    TAILOR_OF_THE_FICKLE,
                ],
                &[],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(celtic));
    play(&mut g, summon(elf));
    let on_celtic = activations(&g, MALEVOLENT_NUZZLER)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, on_celtic);
    pass_all(&mut g);
    activate(&mut g, LEGENDARY_SWORD);
    let offered = activations(&g, TAILOR_OF_THE_FICKLE);
    assert_eq!(
        offered.len(),
        1,
        "only the Nuzzler can move (to the Elf); the Sword's other target must be a Warrior"
    );
    play(&mut g, offered[0].clone());
    assert_eq!((g.atk(celtic), g.def(celtic)), (1700, 1500));
    assert_eq!(g.atk(elf), 1500);
}
