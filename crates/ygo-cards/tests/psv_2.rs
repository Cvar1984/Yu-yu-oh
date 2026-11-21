//! Rulings tests for Pharaoh's Servant, batch 2 (docs/psv-plan.md): turn
//! and battle features.

use ygo_cards::extra_monsters::PENGUIN_SOLDIER;
use ygo_cards::fusion::POLYMERIZATION;
use ygo_cards::oneshot::SPARKS;
use ygo_cards::psv_monsters::*;
use ygo_cards::psv_spells::*;
use ygo_cards::psv_traps::*;
use ygo_cards::ritual::RELINQUISHED;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, in_hand, next_turn, pass_all, play, stacked,
};
use ygo_engine::{Action, DecisionKind, Event, Game, Location, Position};

mod psv_common;
use psv_common::*;

const SKULL_SERVANT: CardId = CardId(32274490); // Level 1, 300/200
const THOUSAND_EYES_IDOL: CardId = CardId(27125110);

fn attacks_offered(g: &Game, attacker: ygo_engine::InstanceId) -> usize {
    g.legal_actions()
        .iter()
        .filter(|a| matches!(a, Action::Attack { attacker: x, .. } if *x == attacker))
        .count()
}

#[test]
fn time_seal_skips_the_opponents_next_draw() {
    let mut g = game(&[TIME_SEAL], &[]);
    set_card(&mut g, P0, TIME_SEAL);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, TIME_SEAL);
    let hand = g.hand(P1).len();
    next_turn(&mut g);
    assert_eq!(g.hand(P1).len(), hand, "no draw on turn 4");
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.hand(P1).len(), hand + 1, "turn 6 draws again");
}

#[test]
fn solomons_lawbook_skips_your_next_standby_phase() {
    let mut g = game(&[SOLOMONS_LAWBOOK, BURNING_LAND], &[]);
    set_card(&mut g, P0, SOLOMONS_LAWBOOK);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, BURNING_LAND);
    activate(&mut g, SOLOMONS_LAWBOOK);
    next_turn(&mut g);
    assert_eq!(lp(&g, P1), 7500, "the opponent's Standby Phase still burns");
    next_turn(&mut g);
    assert_eq!(lp(&g, P0), 8000, "your Standby Phase is skipped");
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(lp(&g, P0), 7500, "the next one is not");
}

#[test]
fn gamble_draws_to_five_on_a_right_call_and_skips_your_turn_on_a_wrong_one() {
    let mut seen = [false, false];
    for seed in 0..20 {
        let mut setup = stacked(
            [
                &[
                    GAMBLE, SPARKS, SPARKS, SPARKS, SPARKS, SPARKS, SPARKS, SPARKS,
                ],
                &[],
            ],
            PAD,
        );
        setup.seed = seed;
        let mut g = Game::new(shared(), &setup).unwrap();
        set_card(&mut g, P0, GAMBLE);
        for _ in 0..7 {
            activate(&mut g, SPARKS);
        }
        assert_eq!(g.hand(P0).len(), 2);
        next_turn(&mut g);
        let pad = in_hand(&g, P1, PAD);
        play(&mut g, ygo_engine::testing::summon(pad));
        respond(&mut g, P0, GAMBLE, None);
        let events = play(&mut g, Action::Answer { option: 0 });
        pass_all(&mut g);
        let heads = events
            .iter()
            .find_map(|e| match e {
                Event::CoinTossed { heads, .. } => Some(*heads),
                _ => None,
            })
            .expect("a coin toss");
        if heads {
            assert_eq!(g.hand(P0).len(), 5);
            seen[0] = true;
        } else {
            next_turn(&mut g);
            assert_eq!((g.turn(), g.turn_player()), (4, P1), "turn 3 is skipped");
            seen[1] = true;
        }
    }
    assert_eq!(seen, [true, true], "both outcomes in 20 seeds");
}

#[test]
fn respect_play_shows_each_turn_players_hand() {
    let mut g = game(&[RESPECT_PLAY], &[]);
    set_card(&mut g, P0, RESPECT_PLAY);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, RESPECT_PLAY);
    assert!(g.hand_revealed(P0) && !g.hand_revealed(P1));
    next_turn(&mut g);
    assert!(g.hand_revealed(P1) && !g.hand_revealed(P0));
}

#[test]
fn cold_wave_stops_both_players_spells_and_traps_until_your_next_turn() {
    let mut g = game(&[COLD_WAVE, SPARKS], &[SPARKS]);
    activate(&mut g, COLD_WAVE);
    let plays = |g: &Game| {
        g.legal_actions()
            .iter()
            .filter(|a| matches!(a, Action::SetSpellTrap { .. } | Action::Activate { .. }))
            .count()
    };
    assert_eq!(plays(&g), 0);
    next_turn(&mut g);
    assert_eq!(plays(&g), 0, "the opponent's turn too");
    next_turn(&mut g);
    assert!(plays(&g) > 0, "over at your next turn");
}

#[test]
fn major_riot_returns_every_monster_and_each_player_sets_as_many() {
    let mut g = game(&[CELTIC_GUARDIAN, MAJOR_RIOT], &[PENGUIN_SOLDIER]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, MAJOR_RIOT);
    next_turn(&mut g);
    let penguin = set_monster(&mut g, P1, PENGUIN_SOLDIER);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: penguin });
    pass_all(&mut g);
    let bounce = activations(&g, PENGUIN_SOLDIER)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &[celtic]))
        .expect("Penguin Soldier on Celtic Guardian");
    play(&mut g, bounce);
    respond(&mut g, P0, MAJOR_RIOT, None);
    assert_eq!(g.instance(celtic).location, Location::Hand);
    assert_eq!(g.instance(penguin).location, Location::Hand);
    let d = g.pending_decision().unwrap();
    assert_eq!(
        (d.player, d.kind),
        (P1, DecisionKind::Choose),
        "P1 had one monster"
    );
    let a = g.legal_actions()[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    let set = g.monsters_controlled(P1);
    assert_eq!(set.len(), 1);
    assert_eq!(g.instance(set[0]).position, Position::FaceDownDefense);
    assert!(g.monsters_controlled(P0).is_empty(), "P0 had none left");
}

#[test]
fn hayabusa_knight_attacks_twice() {
    let mut g = game(&[HAYABUSA_KNIGHT], &[]);
    let knight = summon_now(&mut g, P0, HAYABUSA_KNIGHT);
    next_turn(&mut g);
    next_turn(&mut g);
    attack(&mut g, knight, None);
    pass_all(&mut g);
    assert_eq!(attacks_offered(&g, knight), 1, "a second attack");
    play(
        &mut g,
        Action::Attack {
            attacker: knight,
            target: None,
        },
    );
    pass_all(&mut g);
    assert_eq!(lp(&g, P1), 8000 - 2000);
    assert_eq!(attacks_offered(&g, knight), 0, "no third");
}

#[test]
fn sword_hunter_equips_the_monsters_it_destroyed_at_the_end_of_the_battle_phase() {
    let mut g = game(&[PAD, PAD, SWORD_HUNTER], &[CELTIC_GUARDIAN]);
    let a = summon_now(&mut g, P0, PAD);
    let b = summon_now(&mut g, P0, PAD);
    let hunter = in_hand(&g, P0, SWORD_HUNTER);
    play(
        &mut g,
        Action::Summon {
            card: hunter,
            tributes: vec![a, b],
            face_down: false,
        },
    );
    pass_all(&mut g);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    attack(&mut g, hunter, Some(celtic));
    pass_all(&mut g);
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    assert_eq!(g.equip_target(celtic), Some(hunter));
    assert_eq!(g.atk(hunter), 2450 + 200);
}

#[test]
fn vampire_baby_may_summon_the_monster_it_destroyed() {
    let mut g = game(&[VAMPIRE_BABY], &[SKULL_SERVANT]);
    let baby = summon_now(&mut g, P0, VAMPIRE_BABY);
    next_turn(&mut g);
    let servant = summon_now(&mut g, P1, SKULL_SERVANT);
    next_turn(&mut g);
    attack(&mut g, baby, Some(servant));
    pass_all(&mut g);
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Triggers));
    activate(&mut g, VAMPIRE_BABY);
    answer(&mut g, 0);
    assert_eq!(g.instance(servant).controller, P0);
    assert_eq!(g.instance(servant).position, Position::FaceUpAttack);
}

#[test]
fn goblin_attack_force_turns_to_defense_and_stays_until_your_next_end_phase() {
    let mut g = game(&[GOBLIN_ATTACK_FORCE], &[]);
    let goblin = summon_now(&mut g, P0, GOBLIN_ATTACK_FORCE);
    next_turn(&mut g);
    next_turn(&mut g);
    attack(&mut g, goblin, None);
    pass_all(&mut g);
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    assert_eq!(g.instance(goblin).position, Position::FaceUpDefense);
    let change = Action::ChangePosition { card: goblin };
    next_turn(&mut g);
    next_turn(&mut g);
    assert!(
        !g.legal_actions().contains(&change),
        "locked in your next turn"
    );
    next_turn(&mut g);
    next_turn(&mut g);
    assert!(g.legal_actions().contains(&change), "free again after it");
}

#[test]
fn morphing_jar_2_reshuffles_the_field_and_excavates_as_many_monsters() {
    let mut g = game(&[MORPHING_JAR_2], &[CELTIC_GUARDIAN, MYSTICAL_ELF]);
    let jar = set_monster(&mut g, P0, MORPHING_JAR_2);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let elf = summon_now(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    flip_summon(&mut g, jar);
    for card in [jar, celtic, elf] {
        assert_ne!(g.instance(card).location, Location::MonsterZone(0));
        assert!(!g.on_field(card));
    }
    assert_eq!(g.monsters_controlled(P0).len(), 1, "one monster shuffled");
    assert_eq!(g.monsters_controlled(P1).len(), 2, "two monsters shuffled");
    for m in g.monsters_controlled(P1) {
        assert_eq!(g.instance(m).position, Position::FaceDownDefense);
    }
}

#[test]
fn thousand_eyes_restrict_absorbs_a_monster_and_freezes_the_others() {
    let mut setup = stacked(
        [
            &[POLYMERIZATION, RELINQUISHED, THOUSAND_EYES_IDOL],
            &[CELTIC_GUARDIAN, MYSTICAL_ELF],
        ],
        PAD,
    );
    setup.decks[0].extra = vec![THOUSAND_EYES_RESTRICT];
    let mut g = Game::new(shared(), &setup).unwrap();
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, POLYMERIZATION);
    while let Some(d) = g.pending_decision() {
        match d.kind {
            DecisionKind::Choose => {
                let a = g.legal_actions()[0].clone();
                play(&mut g, a);
                pass_all(&mut g);
            }
            _ => break,
        }
    }
    let restrict = g.monsters_controlled(P0)[0];
    assert_eq!(g.stats(restrict).id, THOUSAND_EYES_RESTRICT);
    activate_on(&mut g, THOUSAND_EYES_RESTRICT, &[celtic]);
    assert_eq!(g.equip_target(celtic), Some(restrict));
    assert_eq!(g.atk(restrict), 1400);
    next_turn(&mut g);
    let elf = summon_now(&mut g, P1, MYSTICAL_ELF);
    assert!(g.attack_restricted(elf));
    assert!(g.position_locked(elf));
    assert!(!g.attack_restricted(restrict));
}
