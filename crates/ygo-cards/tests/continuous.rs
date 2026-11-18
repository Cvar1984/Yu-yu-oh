//! Rulings tests for the M2 step 5 continuous and lingering effects.

use std::sync::Arc;

use ygo_cards::continuous::*;
use ygo_cards::monsters::MAN_EATER_BUG;
use ygo_cards::traps::MYSTICAL_SPACE_TYPHOON;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, Location, PlayerId, Position, Shared, VisibleCard};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
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

fn attacks_offered(g: &Game) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Attack { .. }))
        .collect()
}

#[test]
fn malevolent_nuzzler_adds_700_atk_until_the_monster_leaves_then_may_return_to_the_deck() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[CELTIC_GUARDIAN, MALEVOLENT_NUZZLER],
                &[SEVEN_COLORED_FISH, SEVEN_COLORED_FISH],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert!(
        activations(&g, MALEVOLENT_NUZZLER).is_empty(),
        "no face-up monster"
    );
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    let nuzzler = in_hand(&g, P0, MALEVOLENT_NUZZLER);
    let offered = activations(&g, MALEVOLENT_NUZZLER);
    assert_eq!(offered.len(), 1);
    play(&mut g, offered[0].clone());
    assert_eq!(g.atk(celtic), 2100);
    assert_eq!(g.def(celtic), 1200);
    assert_eq!(
        g.instance(nuzzler).location,
        Location::SpellTrapZone(0),
        "an Equip stays"
    );
    assert_eq!(g.equip_target(nuzzler), Some(celtic));
    let seen = g.observe(P1).opponent().monster_zones[0].clone();
    assert!(
        matches!(
            seen,
            Some(VisibleCard::Known {
                stats: Some((2100, 1200)),
                ..
            })
        ),
        "{seen:?}"
    );

    // Turn 2: the opponent summons a Fish. Turn 3: 2100 beats 1800.
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: Some(fish),
        },
    );
    pass_all(&mut g);
    assert_eq!(g.instance(fish).location, Location::Graveyard);
    assert_eq!(g.player(P1).lp, 8000 - 300);
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);

    // Turn 4: a second Fish. Turn 5: Celtic goes to Defense (1200 DEF).
    // Turn 6: the Fish destroys it; the Equip goes with it.
    let fish2 = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish2));
    next_turn(&mut g);
    play(&mut g, Action::ChangePosition { card: celtic });
    assert_eq!(g.instance(celtic).position, Position::FaceUpDefense);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish2,
            target: Some(celtic),
        },
    );
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert_eq!(
        g.instance(nuzzler).location,
        Location::Graveyard,
        "the Equip went with it"
    );
    // Optional trigger: pay 500 LP to put the Nuzzler on top of the Deck.
    assert!(
        decision_is(&g, P0, DecisionKind::Triggers),
        "{:?}",
        g.pending_decision()
    );
    let offered = g.legal_actions();
    assert!(offered.contains(&Action::Pass), "optional");
    let pay = offered
        .iter()
        .find(|a| matches!(a, Action::Activate { .. }))
        .unwrap()
        .clone();
    play(&mut g, pay);
    pass_all(&mut g);
    assert_eq!(g.player(P0).lp, 7500);
    assert_eq!(g.instance(nuzzler).location, Location::Deck);
    assert_eq!(g.deck(P0).last(), Some(&nuzzler), "on top");
}

#[test]
fn an_equip_destroyed_by_the_typhoon_takes_its_bonus_with_it() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[CELTIC_GUARDIAN, MALEVOLENT_NUZZLER],
                &[MYSTICAL_SPACE_TYPHOON],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    let nuzzler_action = activations(&g, MALEVOLENT_NUZZLER)[0].clone();
    play(&mut g, nuzzler_action);
    next_turn(&mut g);
    let typhoon = activations(&g, MYSTICAL_SPACE_TYPHOON)[0].clone();
    play(&mut g, typhoon);
    // Player 0 may pay 500 to recover the Nuzzler; decline.
    assert!(decision_is(&g, P0, DecisionKind::Triggers));
    play(&mut g, Action::Pass);
    assert_eq!(g.atk(celtic), 1400);
    assert_eq!(g.player(P0).lp, 8000);
}

#[test]
fn legendary_sword_only_targets_warriors() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MYSTICAL_ELF, LEGENDARY_SWORD, CELTIC_GUARDIAN], &[]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        activations(&g, LEGENDARY_SWORD).is_empty(),
        "a Spellcaster is not a Warrior"
    );
    next_turn(&mut g);
    next_turn(&mut g);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    let offered = activations(&g, LEGENDARY_SWORD);
    assert_eq!(offered.len(), 1);
    play(&mut g, offered[0].clone());
    assert_eq!((g.atk(celtic), g.def(celtic)), (1700, 1500));
    assert_eq!((g.atk(elf), g.def(elf)), (800, 2000));
}

#[test]
fn umi_changes_stats_by_type_and_a_new_field_spell_replaces_it_with_one_on_the_field() {
    // With one Field Spell on the whole field (modern is one per player; see
    // flip_field_spells_on_field_to_the_official_two).
    let mut g = Game::new(
        with_rules(&shared(), |r| r.field.field_spells_on_field = 1),
        &stacked(
            [&[SEVEN_COLORED_FISH, UMI], &[UMI, CELTIC_GUARDIAN]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    let umi = in_hand(&g, P0, UMI);
    let offered = activations(&g, UMI);
    assert_eq!(offered.len(), 1, "a Field Spell needs no Spell/Trap zone");
    play(&mut g, offered[0].clone());
    assert_eq!(g.instance(umi).location, Location::FieldZone);
    assert_eq!(g.field_spell(P0), Some(umi));
    assert_eq!(
        (g.atk(fish), g.def(fish)),
        (2000, 1000),
        "7 Colored Fish is a Fish"
    );
    assert!(g.observe(P1).opponent().field_zone.is_some());

    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    assert_eq!(g.atk(celtic), 1400, "a Warrior is unaffected");
    let umi2 = in_hand(&g, P1, UMI);
    let a = activations(&g, UMI)[0].clone();
    play(&mut g, a);
    assert_eq!(
        g.instance(umi).location,
        Location::Graveyard,
        "one Field Spell on the whole field"
    );
    assert_eq!(g.field_spell(P1), Some(umi2));
    assert_eq!(g.field_spell(P0), None);
    assert_eq!(g.atk(fish), 2000, "the new Umi still boosts it");
}

#[test]
fn flip_field_spells_on_field_to_the_official_two() {
    let base = shared();
    let shared = with_rules(&base, |r| r.field.field_spells_on_field = 2);
    let mut g = Game::new(shared, &stacked([&[UMI, UMI], &[UMI]], MYSTICAL_ELF)).unwrap();
    let umi_a = in_hand(&g, P0, UMI);
    let a = activations(&g, UMI)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == umi_a))
        .unwrap();
    play(&mut g, a);
    next_turn(&mut g);
    let umi_b = in_hand(&g, P1, UMI);
    let a = activations(&g, UMI)[0].clone();
    play(&mut g, a);
    assert_eq!(g.field_spell(P0), Some(umi_a), "both stay");
    assert_eq!(g.field_spell(P1), Some(umi_b));
    next_turn(&mut g);
    let a = activations(&g, UMI)[0].clone();
    play(&mut g, a);
    assert_eq!(
        g.instance(umi_a).location,
        Location::Graveyard,
        "replaced by its owner's new one"
    );
    assert_eq!(g.field_spell(P1), Some(umi_b));
}

#[test]
fn messenger_of_peace_stops_big_attackers_and_costs_100_each_standby_phase() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MESSENGER_OF_PEACE, MYSTICAL_ELF], &[SEVEN_COLORED_FISH]],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let a = activations(&g, MESSENGER_OF_PEACE)[0].clone();
    play(&mut g, a);
    let messenger = g.player(P0).spell_trap_zones[0].unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    assert!(attacks_offered(&g).is_empty(), "1800 ATK may not attack");
    play(&mut g, Action::EndBattlePhase);
    play(&mut g, Action::EndTurn);
    pass_all(&mut g);
    // Player 0's Standby Phase: pay or destroy.
    assert!(
        decision_is(&g, P0, DecisionKind::Choose),
        "{:?}",
        g.pending_decision()
    );
    assert_eq!(
        g.observe(P0).question,
        vec!["Pay 100 LP", "Destroy Messenger of Peace"]
    );
    assert_eq!(
        g.legal_actions(),
        vec![Action::Answer { option: 0 }, Action::Answer { option: 1 }]
    );
    play(&mut g, Action::Answer { option: 0 });
    assert_eq!(g.player(P0).lp, 7900);
    assert!(g.on_field(messenger));
    // The Elf (800) may still attack.
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(attacks_offered(&g).len(), 1);
    play(&mut g, Action::EndBattlePhase);
    play(&mut g, Action::EndTurn);
    pass_all(&mut g);
    play(&mut g, Action::EndTurn);
    pass_all(&mut g);
    play(&mut g, Action::Answer { option: 1 });
    assert_eq!(g.instance(messenger).location, Location::Graveyard);
    assert_eq!(g.player(P0).lp, 7900);
}

#[test]
fn swords_of_revealing_light_flips_stops_attacks_for_three_end_phases_and_goes() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[SWORDS_OF_REVEALING_LIGHT, MYSTICAL_ELF],
                &[
                    MAN_EATER_BUG,
                    SEVEN_COLORED_FISH,
                    SEVEN_COLORED_FISH,
                    SEVEN_COLORED_FISH,
                ],
            ],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    next_turn(&mut g);
    let bug = in_hand(&g, P1, MAN_EATER_BUG);
    play(
        &mut g,
        Action::Summon {
            card: bug,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    let swords = in_hand(&g, P0, SWORDS_OF_REVEALING_LIGHT);
    let a = activations(&g, SWORDS_OF_REVEALING_LIGHT)[0].clone();
    play(&mut g, a);
    // The Bug was flipped face-up by Swords: its FLIP effect fires for its
    // controller (player 1), who must target a monster.
    assert_eq!(g.instance(bug).position, Position::FaceUpDefense);
    assert!(
        decision_is(&g, P1, DecisionKind::Triggers),
        "{:?}",
        g.pending_decision()
    );
    let on_elf = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![elf]))
        .unwrap();
    play(&mut g, on_elf);
    pass_all(&mut g);
    assert_eq!(g.instance(elf).location, Location::Graveyard);
    assert_eq!(
        g.instance(swords).location,
        Location::SpellTrapZone(0),
        "a Normal Spell that stays"
    );

    // Three of player 1's turns without attacks, then Swords leaves.
    for end_phases in 1..=3 {
        next_turn(&mut g);
        assert_eq!(g.turn_player(), P1);
        let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
        if g.player(P1).monsters().count() < 5 {
            play(&mut g, summon(fish));
        }
        play(&mut g, Action::EnterBattlePhase);
        assert!(
            attacks_offered(&g).is_empty(),
            "turn {}: no attacks under Swords",
            g.turn()
        );
        play(&mut g, Action::EndBattlePhase);
        next_turn(&mut g);
        if end_phases < 3 {
            assert!(g.on_field(swords), "after End Phase {end_phases}");
            assert_eq!(g.counter(swords), end_phases);
        }
    }
    assert_eq!(
        g.instance(swords).location,
        Location::Graveyard,
        "destroyed in the 3rd End Phase"
    );
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert!(!attacks_offered(&g).is_empty(), "attacks are back");
}

#[test]
fn the_typhoon_ends_swords_early() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[SWORDS_OF_REVEALING_LIGHT],
                &[SEVEN_COLORED_FISH, MYSTICAL_SPACE_TYPHOON],
            ],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let a = activations(&g, SWORDS_OF_REVEALING_LIGHT)[0].clone();
    play(&mut g, a);
    let swords = g.player(P0).spell_trap_zones[0].unwrap();
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    let typhoon = activations(&g, MYSTICAL_SPACE_TYPHOON)[0].clone();
    play(&mut g, typhoon);
    assert_eq!(g.instance(swords).location, Location::Graveyard);
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(
        attacks_offered(&g).len(),
        1,
        "the restriction ended with the card"
    );
}
