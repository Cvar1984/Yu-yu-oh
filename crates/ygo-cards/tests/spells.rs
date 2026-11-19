//! Rulings tests for the M2 step 2 Spells.

use std::sync::Arc;
use ygo_cards::spells::*;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, SUMMONED_SKULL, answer, in_hand, next_turn,
    play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, Event, Game, Location, Phase, PlayerId, Position, Shared};

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

#[test]
fn pot_of_greed_draws_two_and_goes_to_the_graveyard() {
    let mut g = Game::new(shared(), &stacked([&[POT_OF_GREED], &[]], MYSTICAL_ELF)).unwrap();
    let pot = in_hand(&g, P0, POT_OF_GREED);
    let events = activate(&mut g, POT_OF_GREED);
    assert_eq!(g.player(P0).hand.len(), 6, "5 - Pot + 2");
    assert_eq!(g.instance(pot).location, Location::Graveyard);
    assert!(g.player(P0).spell_trap_zones.iter().all(|z| z.is_none()));
    assert!(g.chain().is_empty());
    assert!(events.contains(&Event::Activated {
        card: pot,
        controller: P0,
        effect: 0
    }));
    assert!(events.contains(&Event::Resolved {
        card: pot,
        effect: 0
    }));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Drew { .. }))
            .count(),
        2
    );
}

#[test]
fn pot_of_greed_needs_two_cards_in_the_deck() {
    let base = shared();
    let shared = with_rules(&base, |r| r.hand.max_size = 60);
    let mut g = Game::new(shared, &stacked([&[POT_OF_GREED], &[]], MYSTICAL_ELF)).unwrap();
    // Draw the deck down to one card; nothing is ever discarded.
    let mut turns = 0;
    while g.deck_size(P0) > 1 {
        next_turn(&mut g);
        next_turn(&mut g);
        turns += 2;
        assert!(turns < 200);
    }
    assert_eq!(g.deck_size(P0), 1);
    assert!(
        g.player(P0)
            .hand
            .iter()
            .any(|&c| g.stats(c).id == POT_OF_GREED)
    );
    assert!(activations(&g, POT_OF_GREED).is_empty());
}

#[test]
fn a_spell_set_face_down_can_be_activated_later() {
    let mut g = Game::new(shared(), &stacked([&[POT_OF_GREED], &[]], MYSTICAL_ELF)).unwrap();
    let pot = in_hand(&g, P0, POT_OF_GREED);
    let events = play(&mut g, Action::SetSpellTrap { card: pot });
    assert_eq!(
        events[0],
        Event::SetCard {
            player: P0,
            card: pot
        }
    );
    assert_eq!(g.instance(pot).location, Location::SpellTrapZone(0));
    assert_eq!(g.instance(pot).position, Position::FaceDown);
    assert_eq!(
        activations(&g, POT_OF_GREED).len(),
        1,
        "a Set Normal Spell may be activated this turn"
    );
    next_turn(&mut g);
    assert!(
        activations(&g, POT_OF_GREED).is_empty(),
        "not on the opponent's turn"
    );
    next_turn(&mut g);
    activate(&mut g, POT_OF_GREED);
    assert_eq!(g.instance(pot).location, Location::Graveyard);
}

#[test]
fn spells_need_a_free_zone_from_the_hand_and_cannot_start_in_the_battle_phase() {
    let mut g = Game::new(shared(), &stacked([&[POT_OF_GREED; 6], &[]], MYSTICAL_ELF)).unwrap();
    for _ in 0..5 {
        let pot = in_hand(&g, P0, POT_OF_GREED);
        play(&mut g, Action::SetSpellTrap { card: pot });
    }
    assert!(
        g.legal_actions()
            .iter()
            .all(|a| !matches!(a, Action::SetSpellTrap { .. }))
    );
    // The sixth Pot is in the hand but all zones are full: only the Set ones can go.
    let offered = activations(&g, POT_OF_GREED);
    assert_eq!(offered.len(), 5);
    assert!(offered.iter().all(|a| matches!(a, Action::Activate { card, .. } if g.instance(*card).location != Location::Hand)));
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert!(
        g.legal_actions()
            .iter()
            .all(|a| !matches!(a, Action::Activate { .. }))
    );
}

#[test]
fn raigeki_destroys_every_opposing_monster_and_needs_one() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[RAIGEKI], &[MYSTICAL_ELF, CELTIC_GUARDIAN]],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    assert!(activations(&g, RAIGEKI).is_empty(), "no monster to destroy");
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    let events = activate(&mut g, RAIGEKI);
    assert_eq!(
        g.instance(elf).location,
        Location::Graveyard,
        "face-down monsters too"
    );
    assert!(g.on_field(fish), "own monsters stay");
    let raigeki = events
        .iter()
        .find_map(|e| match e {
            Event::Activated { card, .. } => Some(*card),
            _ => None,
        })
        .unwrap();
    assert!(events.contains(&Event::DestroyedByEffect {
        card: elf,
        source: raigeki
    }));
}

#[test]
fn dark_hole_destroys_both_sides() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[SEVEN_COLORED_FISH, DARK_HOLE], &[MYSTICAL_ELF]],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    activate(&mut g, DARK_HOLE);
    assert_eq!(g.instance(fish).location, Location::Graveyard);
    assert_eq!(g.player(P0).monsters().count(), 0);
}

#[test]
fn fissure_destroys_the_lowest_atk_face_up_monster_only() {
    let base = shared();
    let shared = with_rules(&base, |r| r.summon.normal_or_tribute_per_turn = 2);
    let mut g = Game::new(
        shared,
        &stacked(
            [&[FISSURE], &[CELTIC_GUARDIAN, MYSTICAL_ELF]],
            SUMMONED_SKULL,
        ),
    )
    .unwrap();
    assert!(activations(&g, FISSURE).is_empty(), "no face-up monster");
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
    // The Set Elf has the lower ATK but is face-down: Celtic Guardian goes.
    activate(&mut g, FISSURE);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(g.on_field(elf));
}

#[test]
fn fissure_tie_is_a_choice() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[FISSURE], &[CELTIC_GUARDIAN, CELTIC_GUARDIAN]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let shared = with_rules(&g.shared().clone(), |r| {
        r.summon.normal_or_tribute_per_turn = 2
    });
    g = Game::new(
        shared,
        &stacked(
            [&[FISSURE], &[CELTIC_GUARDIAN, CELTIC_GUARDIAN]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    next_turn(&mut g);
    let a = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(a));
    let b = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(b));
    next_turn(&mut g);
    activate(&mut g, FISSURE);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, ygo_engine::DecisionKind::Choose));
    assert_eq!(
        g.legal_actions(),
        vec![Action::Choose { card: a }, Action::Choose { card: b }]
    );
    assert_eq!(g.chain().len(), 0, "the link is popped while it resolves");
    assert_eq!(g.observe(P0).revealed.len(), 2);
    assert!(g.observe(P1).revealed.is_empty());
    play(&mut g, Action::Choose { card: b });
    assert_eq!(g.instance(b).location, Location::Graveyard);
    assert!(g.on_field(a));
    assert_eq!(g.phase(), Phase::Main1);
}

#[test]
fn upstart_goblin_draws_one_and_gives_the_opponent_1000() {
    let mut g = Game::new(shared(), &stacked([&[UPSTART_GOBLIN], &[]], MYSTICAL_ELF)).unwrap();
    activate(&mut g, UPSTART_GOBLIN);
    assert_eq!(g.player(P0).hand.len(), 5);
    assert_eq!(g.player(P1).lp, 9000);
}

#[test]
fn monster_reborn_special_summons_from_either_graveyard() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[SEVEN_COLORED_FISH, MONSTER_REBORN, MONSTER_REBORN],
                &[CELTIC_GUARDIAN],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert!(
        activations(&g, MONSTER_REBORN).is_empty(),
        "nothing in a Graveyard"
    );
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
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
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);

    let offered = activations(&g, MONSTER_REBORN);
    assert_eq!(
        offered.len(),
        2,
        "two copies x one target (Celtic Guardian in the opponent's Graveyard)"
    );
    play(&mut g, offered[0].clone());
    // No position in the text: the player picks; Defense here.
    assert_eq!(
        g.pending_decision().map(|d| (d.player, d.kind)),
        Some((P0, ygo_engine::DecisionKind::Choose))
    );
    answer(&mut g, 1);
    assert_eq!(
        g.instance(celtic).location,
        Location::MonsterZone(1),
        "zone 0 holds the Fish"
    );
    assert_eq!(g.instance(celtic).controller, P0);
    assert_eq!(g.instance(celtic).position, Position::FaceUpDefense);
    assert_eq!(g.player(P0).monsters_entered, 1);
    assert_eq!(
        g.player(P0).summons_used,
        0,
        "a Special Summon is not the Normal Summon"
    );
    assert!(!ygo_engine::testing::summons_offered(&g).is_empty());
    assert!(
        activations(&g, MONSTER_REBORN).is_empty(),
        "the Graveyards hold no monster now"
    );
}

#[test]
fn flip_special_summon_counts_toward_limit() {
    let base = shared();
    let shared = with_rules(&base, |r| {
        r.summon.special_summon_counts_toward_limit = true
    });
    let mut g = Game::new(
        shared,
        &stacked(
            [&[SEVEN_COLORED_FISH, MONSTER_REBORN, DARK_HOLE], &[]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    activate(&mut g, DARK_HOLE);
    assert!(
        activations(&g, MONSTER_REBORN).is_empty(),
        "the one summon of the turn is used"
    );
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, MONSTER_REBORN);
    answer(&mut g, 0);
    assert!(g.on_field(fish));
    assert_eq!(g.player(P0).summons_used, 1);
    assert!(ygo_engine::testing::summons_offered(&g).is_empty());
}

#[test]
fn flip_max_monsters_entering_blocks_special_summons() {
    let base = shared();
    let shared = with_rules(&base, |r| r.summon.max_monsters_entering_per_turn = 1);
    let mut g = Game::new(
        shared,
        &stacked(
            [
                &[
                    SEVEN_COLORED_FISH,
                    MONSTER_REBORN,
                    DARK_HOLE,
                    CELTIC_GUARDIAN,
                ],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    activate(&mut g, DARK_HOLE);
    assert!(
        activations(&g, MONSTER_REBORN).is_empty(),
        "one monster already entered this turn"
    );
}

#[test]
fn change_of_heart_takes_a_monster_until_the_end_phase() {
    let mut g = Game::new(
        shared(),
        &stacked([&[CHANGE_OF_HEART], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
    )
    .unwrap();
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    let events = activate(&mut g, CHANGE_OF_HEART);
    assert!(events.contains(&Event::ControlChanged {
        card: celtic,
        to: P0
    }));
    assert_eq!(g.instance(celtic).controller, P0);
    assert_eq!(g.player(P0).monster_zones[0], Some(celtic));
    assert!(g.player(P1).monster_zones.iter().all(|z| z.is_none()));
    assert_eq!(g.lingering().len(), 1);

    // It may attack this turn, straight into the opponent's empty field.
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    assert_eq!(g.player(P1).lp, 8000 - 1400);
    play(&mut g, Action::EndBattlePhase);
    let events = play(&mut g, Action::EndTurn);
    assert!(events.contains(&Event::ControlChanged {
        card: celtic,
        to: P1
    }));
    assert_eq!(g.instance(celtic).controller, P1);
    assert_eq!(g.player(P1).monster_zones[0], Some(celtic));
    assert!(g.lingering().is_empty());
}

#[test]
fn a_monster_set_this_turn_cannot_be_targeted_unless_the_rules_allow_it() {
    // Player 1 Sets on turn 2 and player 0 tries Change of Heart on turn 3:
    // the Set happened last turn, so it is a legal target. The rule bites
    // when the Set and the Spell happen in the same turn, which needs
    // Change of Heart on the opponent's own turn: impossible for a Normal
    // Spell. So test the helper directly.
    let house = with_rules(&shared(), |r| {
        r.turn.spell_target_monster_set_this_turn = false
    });
    let mut g = Game::new(house, &stacked([&[MYSTICAL_ELF], &[]], CELTIC_GUARDIAN)).unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: vec![],
            face_down: true,
        },
    );
    assert!(!g.spell_targetable(elf), "house rule: Set this turn");
    next_turn(&mut g);
    assert!(g.spell_targetable(elf), "from the next turn on");

    let base = shared();
    let official = with_rules(&base, |r| r.turn.spell_target_monster_set_this_turn = true);
    let mut g = Game::new(official, &stacked([&[MYSTICAL_ELF], &[]], CELTIC_GUARDIAN)).unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: vec![],
            face_down: true,
        },
    );
    assert!(g.spell_targetable(elf), "official rules");
}

#[test]
fn tribute_to_the_doomed_discards_as_a_cost_and_destroys_the_target() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[TRIBUTE_TO_THE_DOOMED, SEVEN_COLORED_FISH],
                &[CELTIC_GUARDIAN],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert!(
        activations(&g, TRIBUTE_TO_THE_DOOMED).is_empty(),
        "no monster on the field"
    );
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(
        &mut g,
        Action::Summon {
            card: celtic,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    let offered = activations(&g, TRIBUTE_TO_THE_DOOMED);
    assert_eq!(offered.len(), 5, "5 other hand cards to discard x 1 target");
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    let action = offered
        .iter()
        .find(|a| matches!(a, Action::Activate { cost, .. } if cost == &vec![fish]))
        .unwrap()
        .clone();
    let events = play(&mut g, action);
    assert_eq!(g.instance(fish).location, Location::Graveyard);
    assert_eq!(
        g.instance(celtic).location,
        Location::Graveyard,
        "face-down target destroyed"
    );
    let discarded_before_resolved = events
        .iter()
        .position(|e| matches!(e, Event::Discarded { .. }))
        .unwrap()
        < events
            .iter()
            .position(|e| matches!(e, Event::Resolved { .. }))
            .unwrap();
    assert!(discarded_before_resolved, "costs are paid at activation");
}
