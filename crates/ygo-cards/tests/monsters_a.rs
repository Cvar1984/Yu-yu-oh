//! Rulings tests for Effect Monsters batch A.

use std::sync::Arc;

use ygo_cards::continuous::MALEVOLENT_NUZZLER;
use ygo_cards::monsters_a::*;
use ygo_cards::oneshot::THE_CHEERFUL_COFFIN;
use ygo_cards::reveal::CONFISCATION;
use ygo_cards::ritual::{CRAB_TURTLE, TURTLE_OATH};
use ygo_cards::spells::CHANGE_OF_HEART;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, Location, PlayerId, Position, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const SKULL_RED_BIRD: CardId = CardId(10202894); // WIND, 1550/1200, level 4
const PREVENT_RAT: CardId = CardId(549481); // EARTH, 500/2000, level 4

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.hand.max_size = 12;
        r.summon.normal_or_tribute_per_turn = 3;
    })
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

fn decision_is(g: &Game, player: PlayerId, kind: DecisionKind) -> bool {
    g.pending_decision()
        .is_some_and(|d| d.player == player && d.kind == kind)
}

#[test]
fn direct_attackers_may_attack_over_monsters() {
    for id in DIRECT_ATTACKERS {
        let mut g = Game::new(
            shared(),
            &stacked([&[id], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
        )
        .unwrap();
        let m = in_hand(&g, P0, id);
        play(&mut g, summon(m));
        next_turn(&mut g);
        let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
        play(&mut g, summon(celtic));
        next_turn(&mut g);
        play(&mut g, Action::EnterBattlePhase);
        let attacks: Vec<_> = g
            .legal_actions()
            .into_iter()
            .filter(|a| matches!(a, Action::Attack { .. }))
            .collect();
        assert!(
            attacks.contains(&Action::Attack {
                attacker: m,
                target: None
            }),
            "{id}: {attacks:?}"
        );
        assert!(
            attacks.contains(&Action::Attack {
                attacker: m,
                target: Some(celtic)
            }),
            "{id}"
        );
        play(
            &mut g,
            Action::Attack {
                attacker: m,
                target: None,
            },
        );
        pass_all(&mut g);
        assert_eq!(g.player(P1).lp, 8000 - g.stats(m).atk as u32, "{id}");
    }
}

#[test]
fn conditional_stats_follow_the_graveyard_the_hand_and_the_equips() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[MUKA_MUKA, FLASH_ASSAILANT, MAHA_VAILO, MALEVOLENT_NUZZLER],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let muka = in_hand(&g, P0, MUKA_MUKA);
    let flash = in_hand(&g, P0, FLASH_ASSAILANT);
    let maha = in_hand(&g, P0, MAHA_VAILO);
    play(&mut g, summon(muka));
    assert_eq!(g.player(P0).hand.len(), 4);
    assert_eq!((g.atk(muka), g.def(muka)), (600 + 1200, 300 + 1200));
    play(&mut g, summon(flash));
    assert_eq!(
        (g.atk(flash), g.def(flash)),
        (2000 - 1200, 2000 - 1200),
        "three cards in hand"
    );
    assert_eq!(g.atk(muka), 600 + 900);
    play(&mut g, summon(maha));
    assert_eq!(g.atk(maha), 1550);
    let on_maha = activations(&g, MALEVOLENT_NUZZLER)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![maha]))
        .unwrap();
    play(&mut g, on_maha);
    pass_all(&mut g);
    assert_eq!(
        g.atk(maha),
        1550 + 700 + 500,
        "the Nuzzler and Maha Vailo's own bonus"
    );
}

#[test]
fn shadow_ghoul_counts_monsters_in_the_graveyard() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    MYSTICAL_ELF,
                    THE_CHEERFUL_COFFIN,
                    MYSTICAL_ELF,
                    SHADOW_GHOUL,
                    MYSTICAL_ELF,
                ],
                &[],
            ],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let ghoul = in_hand(&g, P0, SHADOW_GHOUL);
    let a = activations(&g, THE_CHEERFUL_COFFIN)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets.len() == 3 && !targets.contains(&ghoul)))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(
        g.graveyard(P0)
            .iter()
            .filter(|&&c| g.stats(c).is_monster())
            .count(),
        3
    );
    next_turn(&mut g);
    next_turn(&mut g);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    play(
        &mut g,
        Action::Summon {
            card: ghoul,
            tributes: vec![celtic],
            face_down: false,
        },
    );
    assert_eq!(
        g.atk(ghoul),
        1600 + 400,
        "three Elves and the tributed Celtic Guardian"
    );
}

#[test]
fn insect_soldiers_gain_1000_only_while_attacking_a_wind_monster() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[INSECT_SOLDIERS_OF_THE_SKY], &[SKULL_RED_BIRD]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let soldiers = in_hand(&g, P0, INSECT_SOLDIERS_OF_THE_SKY);
    play(&mut g, summon(soldiers));
    assert_eq!(g.atk(soldiers), 1000);
    next_turn(&mut g);
    let bird = in_hand(&g, P1, SKULL_RED_BIRD);
    play(&mut g, summon(bird));
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: soldiers,
            target: Some(bird),
        },
    );
    pass_all(&mut g);
    assert_eq!(
        g.instance(bird).location,
        Location::Graveyard,
        "2000 beats 1550"
    );
    assert_eq!(g.player(P1).lp, 8000 - 450);
    assert_eq!(g.atk(soldiers), 1000, "only during the Damage Step");
}

#[test]
fn every_recruiter_may_special_summon_its_attribute_when_destroyed_by_battle() {
    for (recruiter, attribute) in RECRUITERS {
        let shared = shared();
        let recruit = (0..shared.pool.len() as u16)
            .map(ygo_engine::CardIndex)
            .map(|i| shared.pool.get(i))
            .find(|c| {
                c.kind == ygo_data::CardKind::Normal
                    && c.attribute.as_deref() == Some(attribute)
                    && c.atk <= 1500
            })
            .map(|c| c.id)
            .unwrap();
        let mut g = Game::new(
            shared,
            &stacked(
                [
                    &[
                        recruiter,
                        MYSTICAL_ELF,
                        MYSTICAL_ELF,
                        MYSTICAL_ELF,
                        MYSTICAL_ELF,
                        recruit,
                    ],
                    &[SEVEN_COLORED_FISH],
                ],
                MYSTICAL_ELF,
            ),
        )
        .unwrap();
        let r = in_hand(&g, P0, recruiter);
        play(&mut g, summon(r));
        next_turn(&mut g);
        let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
        play(&mut g, summon(fish));
        play(&mut g, Action::EnterBattlePhase);
        play(
            &mut g,
            Action::Attack {
                attacker: fish,
                target: Some(r),
            },
        );
        assert!(
            decision_is(&g, P0, DecisionKind::Triggers),
            "{recruiter}: optional trigger offered: {:?}",
            g.pending_decision()
        );
        let activate = g
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::Activate { .. }))
            .unwrap();
        play(&mut g, activate);
        pass_all(&mut g);
        assert!(decision_is(&g, P0, DecisionKind::Choose), "{recruiter}");
        let pick = g
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == recruit))
            .unwrap();
        play(&mut g, pick);
        pass_all(&mut g);
        let summoned = g.player(P0).monsters().find(|&m| g.stats(m).id == recruit);
        assert!(summoned.is_some(), "{recruiter} summoned {recruit}");
        assert_eq!(
            g.instance(summoned.unwrap()).position,
            Position::FaceUpAttack
        );
    }
}

#[test]
fn giant_germ_burns_and_nimble_momonga_heals_then_copies_may_follow() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    GIANT_GERM,
                    NIMBLE_MOMONGA,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    GIANT_GERM,
                    NIMBLE_MOMONGA,
                ],
                &[SEVEN_COLORED_FISH, SEVEN_COLORED_FISH],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let germ = in_hand(&g, P0, GIANT_GERM);
    let momonga = in_hand(&g, P0, NIMBLE_MOMONGA);
    play(&mut g, summon(germ));
    play(&mut g, summon(momonga));
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    let fish2 = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish2));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(germ),
        },
    );
    pass_all(&mut g);
    assert_eq!(g.player(P1).lp, 7500, "500 burn");
    assert_eq!(
        g.observe(P0).question.len(),
        2,
        "a second Giant Germ waits in the Deck"
    );
    play(&mut g, Action::Answer { option: 0 });
    pass_all(&mut g);
    let germ2 = g
        .player(P0)
        .monsters()
        .find(|&m| g.stats(m).id == GIANT_GERM)
        .expect("the copy");
    assert_eq!(g.instance(germ2).position, Position::FaceUpAttack);
    play(
        &mut g,
        Action::Attack {
            attacker: fish2,
            target: Some(momonga),
        },
    );
    pass_all(&mut g);
    assert_eq!(
        g.player(P0).lp,
        8000 - 800 - 800 + 1000,
        "two Fish attacks into 1000 ATK monsters, then Momonga's 1000"
    );
    play(&mut g, Action::Answer { option: 0 });
    pass_all(&mut g);
    let momonga2 = g
        .player(P0)
        .monsters()
        .find(|&m| g.stats(m).id == NIMBLE_MOMONGA)
        .expect("the copy");
    assert_eq!(g.instance(momonga2).position, Position::FaceDownDefense);
}

#[test]
fn battle_damage_effects_discard_draw_and_make_the_opponent_draw() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[WHITE_MAGICAL_HAT, MASKED_SORCERER, THE_BISTRO_BUTCHER],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let hat = in_hand(&g, P0, WHITE_MAGICAL_HAT);
    let sorcerer = in_hand(&g, P0, MASKED_SORCERER);
    let butcher = in_hand(&g, P0, THE_BISTRO_BUTCHER);
    play(&mut g, summon(hat));
    play(&mut g, summon(sorcerer));
    play(&mut g, summon(butcher));
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    let opp_hand = g.player(P1).hand.len();
    let my_hand = g.player(P0).hand.len();
    play(
        &mut g,
        Action::Attack {
            attacker: hat,
            target: None,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.player(P1).hand.len(), opp_hand - 1, "White Magical Hat");
    play(
        &mut g,
        Action::Attack {
            attacker: sorcerer,
            target: None,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.player(P0).hand.len(), my_hand + 1, "Masked Sorcerer");
    play(
        &mut g,
        Action::Attack {
            attacker: butcher,
            target: None,
        },
    );
    pass_all(&mut g);
    assert_eq!(
        g.player(P1).hand.len(),
        opp_hand - 1 + 2,
        "The Bistro Butcher"
    );
}

#[test]
fn senju_and_sonic_bird_search_on_summon() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    SENJU_OF_THE_THOUSAND_HANDS,
                    SONIC_BIRD,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    CRAB_TURTLE,
                    TURTLE_OATH,
                ],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let senju = in_hand(&g, P0, SENJU_OF_THE_THOUSAND_HANDS);
    play(&mut g, summon(senju));
    assert!(decision_is(&g, P0, DecisionKind::Triggers));
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { .. }))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    let pick = g.legal_actions();
    assert!(
        pick.iter()
            .all(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == CRAB_TURTLE))
    );
    play(&mut g, pick[0].clone());
    pass_all(&mut g);
    assert!(
        g.player(P0)
            .hand
            .iter()
            .any(|&c| g.stats(c).id == CRAB_TURTLE)
    );
    let bird = in_hand(&g, P0, SONIC_BIRD);
    play(&mut g, summon(bird));
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { .. }))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    let pick = g.legal_actions();
    assert!(
        pick.iter()
            .all(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == TURTLE_OATH))
    );
}

#[test]
fn electric_snake_minar_and_penguin_knight_react_to_the_opponents_effects() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[CONFISCATION, CONFISCATION], &[ELECTRIC_SNAKE, MINAR]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let snake = in_hand(&g, P1, ELECTRIC_SNAKE);
    let minar = in_hand(&g, P1, MINAR);
    let a = activations(&g, CONFISCATION)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    play(&mut g, Action::Choose { card: snake });
    pass_all(&mut g);
    assert_eq!(g.player(P1).hand.len(), 4 + 2, "Electric Snake drew two");
    let a = activations(&g, CONFISCATION)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    play(&mut g, Action::Choose { card: minar });
    pass_all(&mut g);
    assert_eq!(
        g.player(P0).lp,
        8000 - 2000 - 1000,
        "two Confiscations and Minar's 1000"
    );

    // Penguin Knight milled by the opponent's Hiro's Shadow Scout? That draws.
    // Use the opponent's... no mill effect targets the opponent's Deck in
    // these sets except Gravekeeper's Servant (step 11). Covered there.
}

#[test]
fn ameba_and_griggle_reward_their_old_controller_once() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[CHANGE_OF_HEART, CHANGE_OF_HEART], &[AMEBA, GRIGGLE]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    next_turn(&mut g);
    let ameba = in_hand(&g, P1, AMEBA);
    let griggle = in_hand(&g, P1, GRIGGLE);
    play(&mut g, summon(ameba));
    play(&mut g, summon(griggle));
    next_turn(&mut g);
    let on_ameba = activations(&g, CHANGE_OF_HEART)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![ameba]))
        .unwrap();
    play(&mut g, on_ameba);
    pass_all(&mut g);
    assert_eq!(g.player(P0).lp, 6000, "the thief takes 2000");
    let on_griggle = activations(&g, CHANGE_OF_HEART)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![griggle]))
        .unwrap();
    play(&mut g, on_griggle);
    pass_all(&mut g);
    assert_eq!(g.player(P1).lp, 11000, "the old controller gains 3000");
    // They go back at the End Phase: control shifts again, but the effects are spent.
    next_turn(&mut g);
    assert_eq!(g.instance(ameba).controller, P1);
    assert_eq!((g.player(P0).lp, g.player(P1).lp), (6000, 11000));
    let _ = PREVENT_RAT;
}
