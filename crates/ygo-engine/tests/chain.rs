//! M2 step 2: the activation and chain machinery, tested with a synthetic
//! script registered on a real Normal Spell's id. Real cards are tested
//! in ygo-cards.

mod common;

use common::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, play, shared_from,
    stacked, summon,
};
use ygo_data::CardId;
use ygo_engine::{
    Action, CardScript, DecisionKind, Effect, Event, Flow, Game, InstanceId, Location, PlayerId,
    Position, Registry, VisibleCard,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
/// Pot of Greed's id, scripted here as a test effect.
const TEST_SPELL: CardId = CardId(55144522);

/// Cost: discard one other hand card. Target: one monster on the field.
/// Resolution: destroy the target, then choose a card in the controller's
/// Graveyard to return to the hand.
fn registry() -> Registry {
    let mut r = Registry::default();
    r.register(
        TEST_SPELL,
        CardScript {
            tags: Vec::new(),
            effects: vec![Effect {
                timing: |g, s, _| g.hand(s.controller).iter().any(|&c| c != s.card),
                cost_choices: Some(|g, s| {
                    g.hand(s.controller)
                        .iter()
                        .copied()
                        .filter(|&c| c != s.card)
                        .map(|c| vec![c])
                        .collect()
                }),
                cost: Some(|g, ctx| g.discard(ctx.cost[0])),
                targets: Some(|g, s| {
                    [s.controller, s.controller.opponent()]
                        .into_iter()
                        .flat_map(|p| g.monsters_controlled(p))
                        .map(|m| vec![m])
                        .collect()
                }),
                ..Effect::activate(1, |g, ctx| {
                    g.destroy(ctx.targets[0], ctx.source);
                    let options: Vec<InstanceId> = g.graveyard(ctx.controller).to_vec();
                    Flow::Choose {
                        options,
                        then: |g, _ctx, picked| {
                            g.add_to_hand(picked);
                            Flow::Done
                        },
                    }
                })
            }],
        },
    );
    r
}

fn activations(g: &Game) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { .. }))
        .collect()
}

#[test]
fn activation_enumerates_costs_times_targets_and_runs_the_whole_flow() {
    let mut g = Game::new(
        shared_from(registry()),
        &stacked(
            [&[TEST_SPELL, SEVEN_COLORED_FISH], &[CELTIC_GUARDIAN]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert!(activations(&g).is_empty(), "no monster to target");
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);

    let offered = activations(&g);
    assert_eq!(
        offered.len(),
        4 * 2,
        "4 other hand cards to discard x 2 targets"
    );
    let spell = in_hand(&g, P0, TEST_SPELL);
    let discard = in_hand(&g, P0, MYSTICAL_ELF);
    let action = Action::Activate {
        card: spell,
        effect: 0,
        targets: vec![celtic],
        cost: vec![discard],
    };
    assert!(offered.contains(&action));

    let events = play(&mut g, action);
    // Activation: the card sits face-up in a zone, the cost is paid, the
    // link is added, then resolution destroys the target and pauses.
    assert_eq!(g.instance(spell).location, Location::SpellTrapZone(0));
    assert_eq!(g.instance(spell).position, Position::FaceUp);
    assert_eq!(g.instance(discard).location, Location::Graveyard);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(events.contains(&Event::Activated {
        card: spell,
        controller: P0,
        effect: 0
    }));
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Choose));
    assert_eq!(
        g.legal_actions(),
        vec![Action::Choose { card: discard }],
        "only the Elf is in player 0's Graveyard"
    );
    assert_eq!(
        g.observe(P0).revealed,
        vec![VisibleCard::Known {
            instance: discard,
            card: MYSTICAL_ELF,
            position: Position::FaceUp,
            stats: None,
        }]
    );
    assert!(
        g.observe(P1).revealed.is_empty(),
        "the chooser's options are theirs alone"
    );
    assert!(
        g.apply(&Action::EndTurn).is_err(),
        "nothing else while a choice is pending"
    );

    let events = play(&mut g, Action::Choose { card: discard });
    assert_eq!(g.instance(discard).location, Location::Hand);
    assert!(events.contains(&Event::AddedToHand {
        player: P0,
        card: discard,
        from: Location::Graveyard
    }));
    assert!(events.contains(&Event::Resolved {
        card: spell,
        effect: 0
    }));
    assert_eq!(
        g.instance(spell).location,
        Location::Graveyard,
        "a Normal Spell leaves after resolving"
    );
    assert!(g.chain().is_empty());
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::MainPhase);
}

#[test]
fn a_set_spell_is_hidden_from_the_opponent_until_activated() {
    let mut g = Game::new(
        shared_from(registry()),
        &stacked([&[TEST_SPELL], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let spell = in_hand(&g, P0, TEST_SPELL);
    play(&mut g, Action::SetSpellTrap { card: spell });
    let seen_by_p1 = g.observe(P1).opponent().spell_trap_zones[0].clone();
    assert_eq!(
        seen_by_p1,
        Some(VisibleCard::Hidden {
            instance: spell,
            position: Position::FaceDown
        })
    );
    let seen_by_p0 = g.observe(P0).me().spell_trap_zones[0].clone();
    assert!(matches!(
        seen_by_p0,
        Some(VisibleCard::Known {
            card: TEST_SPELL,
            ..
        })
    ));
}

#[test]
fn replaying_a_game_with_a_chain_and_a_choice_is_deterministic() {
    let setup = stacked(
        [&[TEST_SPELL, SEVEN_COLORED_FISH], &[CELTIC_GUARDIAN]],
        MYSTICAL_ELF,
    );
    let mut g = Game::new(shared_from(registry()), &setup).unwrap();
    let mut record = Vec::new();
    let mut rec = |g: &mut Game, a: Action| {
        record.push(a.clone());
        play(g, a)
    };
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    rec(&mut g, summon(fish));
    rec(&mut g, Action::EndTurn);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    rec(&mut g, summon(celtic));
    rec(&mut g, Action::EndTurn);
    let spell = in_hand(&g, P0, TEST_SPELL);
    let discard = in_hand(&g, P0, MYSTICAL_ELF);
    rec(
        &mut g,
        Action::Activate {
            card: spell,
            effect: 0,
            targets: vec![celtic],
            cost: vec![discard],
        },
    );
    rec(&mut g, Action::Choose { card: discard });

    let mut replay = Game::new(shared_from(registry()), &setup).unwrap();
    for a in &record {
        replay.apply(a).unwrap();
    }
    assert_eq!(replay, g);
}
