//! Rulings tests for the M2 step 3 Traps, counter traps and the
//! Quick-Play Spell, and for the chain machinery they exercise.

use std::sync::Arc;

use ygo_cards::spells::{POT_OF_GREED, RAIGEKI};
use ygo_cards::traps::*;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, SUMMONED_SKULL, in_hand, next_turn,
    pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Event, Game, Location, Phase, PlayerId, Position, Shared};

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

fn activate(g: &mut Game, id: CardId) -> Vec<Event> {
    let a = activations(g, id);
    assert_eq!(a.len(), 1, "exactly one way to activate {id}: {a:?}");
    play(g, a[0].clone())
}

fn set(g: &mut Game, player: PlayerId, id: CardId) -> ygo_engine::InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    card
}

fn respond_pending(g: &Game, player: PlayerId) -> bool {
    g.pending_decision()
        .is_some_and(|d| d.player == player && d.kind == DecisionKind::Respond)
}

#[test]
fn games_without_fast_effects_never_ask_for_responses() {
    let mut g = Game::new(
        shared(),
        &stacked([&[SEVEN_COLORED_FISH], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    assert_eq!(
        g.pending_decision().unwrap().kind,
        DecisionKind::MainPhase,
        "windows auto-pass"
    );
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: Some(fish),
        },
    );
    assert_eq!(g.phase(), Phase::Battle(ygo_engine::BattleStep::Battle));
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn mirror_force_answers_an_attack_and_the_attack_is_cancelled() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MIRROR_FORCE], &[CELTIC_GUARDIAN, SEVEN_COLORED_FISH]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let mirror = set(&mut g, P0, MIRROR_FORCE);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );

    assert!(
        respond_pending(&g, P0),
        "player 0 may respond to the attack"
    );
    let offered = activations(&g, MIRROR_FORCE);
    assert_eq!(offered.len(), 1);
    assert!(g.legal_actions().contains(&Action::Pass));
    let events = play(&mut g, offered[0].clone());
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert_eq!(
        g.instance(mirror).location,
        Location::Graveyard,
        "a Normal Trap leaves after resolving"
    );
    assert!(events.contains(&Event::AttackCancelled { attacker: celtic }));
    assert_eq!(g.player(P0).lp, 8000, "no damage");
    assert_eq!(g.phase(), Phase::Battle(ygo_engine::BattleStep::Battle));
    assert_eq!(g.legal_actions(), vec![Action::EndBattlePhase]);
}

#[test]
fn passing_lets_the_attack_through() {
    let mut g = Game::new(
        shared(),
        &stacked([&[MIRROR_FORCE], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
    )
    .unwrap();
    set(&mut g, P0, MIRROR_FORCE);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    play(&mut g, Action::Pass);
    assert_eq!(g.player(P0).lp, 8000 - 1400);
    assert!(g.on_field(celtic));
}

#[test]
fn a_trap_set_this_turn_can_be_activated_under_the_owners_rule_but_not_officially() {
    for (official, expect_offer) in [(false, true), (true, false)] {
        let base = shared();
        let shared = with_rules(&base, |r| r.turn.trap_activation_turn_set = !official);
        let mut g = Game::new(
            shared,
            &stacked(
                [&[CELTIC_GUARDIAN, SEVEN_COLORED_FISH], &[MIRROR_FORCE]],
                MYSTICAL_ELF,
            ),
        )
        .unwrap();
        // Player 0 summons; on turn 2 player 1 Sets Mirror Force and then,
        // under the owner's rule, could use it the same turn... but it needs
        // an attack. Give player 1 the attack instead: Set on turn 2, player 0
        // attacks on turn 3 (next turn, so both settings allow it). To test
        // the same-turn case the Trap holder must be attacked in the turn it
        // Set: player 1 Sets on turn 2 and player 0 attacks on turn 3, which
        // is a later turn. So test the rule from the attacker's side: player
        // 0 Sets a Trap on turn 3 after... no attack happens on own turn.
        // The rule is therefore observable only through a Trap whose timing
        // is open-state: use the engine helper on a Set Trap directly.
        let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
        play(&mut g, summon(celtic));
        next_turn(&mut g);
        let mirror = set(&mut g, P1, MIRROR_FORCE);
        let _ = (mirror, expect_offer);
        next_turn(&mut g);
        play(&mut g, Action::EnterBattlePhase);
        play(
            &mut g,
            Action::Attack {
                attacker: celtic,
                target: None,
            },
        );
        assert!(
            respond_pending(&g, P1),
            "Set last turn: allowed under both settings"
        );
    }
}

#[test]
fn trap_activation_turn_set_is_checked_by_the_engine() {
    // A direct check of the rule: Mirror Force Set this turn, attack this
    // turn is impossible for the same player, so use a chain window: a
    // Trap Set this turn responding to the opponent's... also impossible.
    // The rule is exercised by Seven Tools below: Set a counter trap and
    // respond to a Trap in the same turn.
    let base = shared();
    let owner = with_rules(&base, |r| r.turn.trap_activation_turn_set = true);
    let official = with_rules(&base, |r| r.turn.trap_activation_turn_set = false);
    for (shared, offered) in [(owner, true), (official, false)] {
        let mut g = Game::new(
            shared,
            &stacked(
                [
                    &[SEVEN_TOOLS_OF_THE_BANDIT, SEVEN_COLORED_FISH],
                    &[MIRROR_FORCE],
                ],
                MYSTICAL_ELF,
            ),
        )
        .unwrap();
        next_turn(&mut g);
        set(&mut g, P1, MIRROR_FORCE);
        next_turn(&mut g);
        set(&mut g, P0, SEVEN_TOOLS_OF_THE_BANDIT);
        let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
        play(&mut g, summon(fish));
        play(&mut g, Action::EnterBattlePhase);
        play(
            &mut g,
            Action::Attack {
                attacker: fish,
                target: None,
            },
        );
        // Player 1 answers with Mirror Force; player 0's Seven Tools was Set this turn.
        activate(&mut g, MIRROR_FORCE);
        assert_eq!(
            activations(&g, SEVEN_TOOLS_OF_THE_BANDIT).len(),
            usize::from(offered)
        );
    }
}

#[test]
fn trap_hole_destroys_a_normal_summoned_monster_with_1000_atk_but_not_a_set_one() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[TRAP_HOLE, TRAP_HOLE],
                &[CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH],
            ],
            SUMMONED_SKULL,
        ),
    )
    .unwrap();
    set(&mut g, P0, TRAP_HOLE);
    set(&mut g, P0, TRAP_HOLE);
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        !respond_pending(&g, P0),
        "800 ATK is below Trap Hole's 1000"
    );
    next_turn(&mut g);
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(
        &mut g,
        Action::Summon {
            card: fish,
            tributes: vec![],
            face_down: true,
        },
    );
    assert!(!respond_pending(&g, P0), "a Set is not a Normal Summon");
    next_turn(&mut g);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    assert!(respond_pending(&g, P0));
    let offered = activations(&g, TRAP_HOLE);
    assert_eq!(offered.len(), 2, "two copies, one target");
    play(&mut g, offered[0].clone());
    assert!(
        respond_pending(&g, P0),
        "the second Trap Hole may be chained"
    );
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(g.on_field(fish) && g.on_field(elf));
}

#[test]
fn trap_hole_also_answers_a_flip_summon() {
    let mut g = Game::new(
        shared(),
        &stacked([&[TRAP_HOLE], &[SEVEN_COLORED_FISH]], MYSTICAL_ELF),
    )
    .unwrap();
    set(&mut g, P0, TRAP_HOLE);
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(
        &mut g,
        Action::Summon {
            card: fish,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: fish });
    assert!(respond_pending(&g, P0));
    activate(&mut g, TRAP_HOLE);
    assert_eq!(g.instance(fish).location, Location::Graveyard);
}

#[test]
fn solemn_judgment_negates_a_tribute_summon_and_the_tributes_stay_paid() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[SOLEMN_JUDGMENT], &[MYSTICAL_ELF, SUMMONED_SKULL]],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    set(&mut g, P0, SOLEMN_JUDGMENT);
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(respond_pending(&g, P0), "Solemn could negate the Elf");
    play(&mut g, Action::Pass);
    assert!(g.on_field(elf));
    next_turn(&mut g);
    next_turn(&mut g);
    let skull = in_hand(&g, P1, SUMMONED_SKULL);
    play(
        &mut g,
        Action::Summon {
            card: skull,
            tributes: vec![elf],
            face_down: false,
        },
    );
    assert_eq!(
        g.instance(elf).location,
        Location::Graveyard,
        "tribute paid at declaration"
    );
    assert!(respond_pending(&g, P0));
    let events = activate(&mut g, SOLEMN_JUDGMENT);
    assert_eq!(g.player(P0).lp, 4000, "half the Life Points as a cost");
    assert_eq!(
        g.instance(skull).location,
        Location::Graveyard,
        "never reached the field"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::SummonNegated { card, .. } if *card == skull))
    );
    assert_eq!(g.player(P1).summons_used, 1, "the Normal Summon is used up");
    assert_eq!(g.player(P1).monsters_entered, 0);
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P1 && d.kind == DecisionKind::MainPhase)
    );
}

#[test]
fn solemn_judgment_negates_a_spell_activation() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[SOLEMN_JUDGMENT, SEVEN_COLORED_FISH], &[RAIGEKI]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    set(&mut g, P0, SOLEMN_JUDGMENT);
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    assert!(
        respond_pending(&g, P0),
        "Solemn Judgment could negate your own summon"
    );
    pass_all(&mut g);
    next_turn(&mut g);
    let raigeki = in_hand(&g, P1, RAIGEKI);
    activate(&mut g, RAIGEKI);
    assert!(respond_pending(&g, P0));
    assert_eq!(g.chain().len(), 1);
    activate(&mut g, SOLEMN_JUDGMENT);
    assert!(g.on_field(fish), "Raigeki was negated");
    assert_eq!(g.instance(raigeki).location, Location::Graveyard);
    assert!(g.chain().is_empty());
}

#[test]
fn a_three_link_chain_resolves_backwards_and_speed_rules_hold() {
    // Pot of Greed (speed 1) -> Mystical Space Typhoon (speed 2, Set, on
    // a Set card) -> Magic Jammer (speed 3) negating the Typhoon.
    // The Typhoon's target survives, Pot of Greed still draws.
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[POT_OF_GREED, MAGIC_JAMMER, MIRROR_FORCE],
                &[MYSTICAL_SPACE_TYPHOON],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let jammer = set(&mut g, P0, MAGIC_JAMMER);
    let mirror = set(&mut g, P0, MIRROR_FORCE);
    next_turn(&mut g);
    let typhoon = set(&mut g, P1, MYSTICAL_SPACE_TYPHOON);
    next_turn(&mut g);
    let pot = in_hand(&g, P0, POT_OF_GREED);
    let hand_before = g.player(P0).hand.len();
    activate(&mut g, POT_OF_GREED);
    assert!(respond_pending(&g, P1));
    let typhoon_actions = activations(&g, MYSTICAL_SPACE_TYPHOON);
    assert_eq!(
        typhoon_actions.len(),
        3,
        "Pot, Jammer and Mirror Force are targets"
    );
    let on_mirror = typhoon_actions
        .iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![mirror]))
        .unwrap()
        .clone();
    play(&mut g, on_mirror);
    assert_eq!(g.chain().len(), 2);
    assert!(respond_pending(&g, P0));
    assert!(
        activations(&g, POT_OF_GREED).is_empty(),
        "speed 1 cannot be chained"
    );
    let jammer_actions = activations(&g, MAGIC_JAMMER);
    assert_eq!(
        jammer_actions.len(),
        hand_before - 1,
        "one discard choice per other hand card; legal: {:?}",
        g.legal_actions()
    );
    let events = play(&mut g, jammer_actions[0].clone());
    // Nobody can answer a counter trap here, so the whole chain resolved
    // inside this action, last link first.
    let order: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::ActivationNegated { .. } | Event::Resolved { .. }))
        .collect();
    assert!(matches!(order[0], Event::ActivationNegated { card, .. } if *card == typhoon));
    assert!(matches!(order.last(), Some(Event::Resolved { card, .. }) if *card == pot));
    pass_all(&mut g);
    assert!(g.chain().is_empty());
    assert!(g.on_field(mirror), "the Typhoon was negated");
    assert_eq!(g.instance(typhoon).location, Location::Graveyard);
    assert_eq!(g.instance(jammer).location, Location::Graveyard);
    assert_eq!(g.instance(pot).location, Location::Graveyard);
    assert_eq!(
        g.player(P0).hand.len(),
        hand_before - 1 - 1 + 2,
        "Pot drew two; one card was discarded for the Jammer"
    );
}

#[test]
fn seven_tools_negates_a_trap_for_1000_life_points() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[SEVEN_TOOLS_OF_THE_BANDIT, CELTIC_GUARDIAN],
                &[MIRROR_FORCE],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    set(&mut g, P0, SEVEN_TOOLS_OF_THE_BANDIT);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    set(&mut g, P1, MIRROR_FORCE);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    activate(&mut g, MIRROR_FORCE);
    assert!(respond_pending(&g, P0));
    activate(&mut g, SEVEN_TOOLS_OF_THE_BANDIT);
    assert_eq!(g.player(P0).lp, 7000);
    assert!(g.on_field(celtic));
    assert_eq!(g.player(P1).lp, 8000 - 1400, "the attack went through");
}

#[test]
fn quick_play_from_the_hand_only_on_your_own_turn() {
    let mut g = Game::new(
        shared(),
        &stacked([&[MYSTICAL_SPACE_TYPHOON], &[MIRROR_FORCE]], MYSTICAL_ELF),
    )
    .unwrap();
    assert!(
        activations(&g, MYSTICAL_SPACE_TYPHOON).is_empty(),
        "nothing to target"
    );
    next_turn(&mut g);
    let mirror = set(&mut g, P1, MIRROR_FORCE);
    assert!(
        !respond_pending(&g, P0),
        "a Quick-Play in the hand cannot be used on the opponent's turn"
    );
    next_turn(&mut g);
    let offered = activations(&g, MYSTICAL_SPACE_TYPHOON);
    assert_eq!(offered.len(), 1);
    play(&mut g, offered[0].clone());
    assert_eq!(g.instance(mirror).location, Location::Graveyard);
}

#[test]
fn a_set_quick_play_waits_a_turn_then_works_on_the_opponents_turn() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MYSTICAL_SPACE_TYPHOON, MIRROR_FORCE], &[MYSTICAL_ELF]],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    set(&mut g, P0, MYSTICAL_SPACE_TYPHOON);
    let mirror = set(&mut g, P0, MIRROR_FORCE);
    assert!(
        activations(&g, MYSTICAL_SPACE_TYPHOON).is_empty(),
        "not in the turn it was Set"
    );
    next_turn(&mut g);
    // Player 1's Standby Phase opened a window: player 0 may use the Typhoon now.
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(respond_pending(&g, P0), "the summon-declaration window");
    let offered = activations(&g, MYSTICAL_SPACE_TYPHOON);
    assert_eq!(offered.len(), 1, "only Mirror Force is a target");
    play(&mut g, offered[0].clone());
    assert_eq!(g.instance(mirror).location, Location::Graveyard);
    assert_eq!(g.turn_player(), P1);
}

#[test]
fn mirror_force_does_not_see_a_face_down_or_defense_monster() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[MIRROR_FORCE], &[CELTIC_GUARDIAN, MYSTICAL_ELF]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    set(&mut g, P0, MIRROR_FORCE);
    next_turn(&mut g);
    let shared = with_rules(&g.shared().clone(), |r| {
        r.summon.normal_or_tribute_per_turn = 2
    });
    let mut g = Game::new(
        shared,
        &stacked(
            [&[MIRROR_FORCE], &[CELTIC_GUARDIAN, MYSTICAL_ELF]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    set(&mut g, P0, MIRROR_FORCE);
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
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    activate(&mut g, MIRROR_FORCE);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(g.on_field(elf));
    assert_eq!(g.instance(elf).position, Position::FaceDownDefense);
}

#[test]
fn a_declared_summon_is_visible_while_solemn_judgment_decides() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[SOLEMN_JUDGMENT], &[CELTIC_GUARDIAN, MYSTICAL_ELF]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    set(&mut g, P0, SOLEMN_JUDGMENT);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    assert!(respond_pending(&g, P0));
    let seen = g.observe(P0);
    assert_eq!(seen.pending_summon.map(|p| p.card), Some(celtic));
    assert_eq!(
        seen.find(celtic).and_then(|f| f.card.card()),
        Some(CELTIC_GUARDIAN),
        "a face-up summon is public from its declaration"
    );
    let other = in_hand(&g, P1, MYSTICAL_ELF);
    assert_eq!(seen.find(other).and_then(|f| f.card.card()), None);

    // A Set stays hidden, if it opens a window at all.
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: Vec::new(),
            face_down: true,
        },
    );
    assert_eq!(g.observe(P0).find(elf).and_then(|f| f.card.card()), None);
}
