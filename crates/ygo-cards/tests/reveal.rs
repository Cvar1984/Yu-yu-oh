//! Rulings tests for the M3 step 4 hand-reveal and opponent-choice cards.

use std::sync::Arc;

use ygo_cards::oneshot::SPARKS;
use ygo_cards::reveal::*;
use ygo_data::{CardId, CardKind};
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, pass_all, play,
    shared_from, stacked, with_rules,
};
use ygo_engine::{Action, DecisionKind, Event, Game, Location, PlayerId, Shared, VisibleCard};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| r.hand.max_size = 12)
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

fn decision_is(g: &Game, player: PlayerId, kind: DecisionKind) -> bool {
    g.pending_decision()
        .is_some_and(|d| d.player == player && d.kind == kind)
}

#[test]
fn confiscation_pays_1000_shows_the_hand_and_discards_the_chosen_card() {
    let mut g = Game::new(
        shared(),
        &stacked([&[CONFISCATION], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
    )
    .unwrap();
    activate(&mut g, CONFISCATION);
    assert_eq!(g.player(P0).lp, 7000);
    assert!(decision_is(&g, P0, DecisionKind::Choose));
    let seen = g.observe(P0).revealed;
    assert_eq!(seen.len(), 5, "the whole hand is shown");
    assert!(seen.iter().all(|c| matches!(c, VisibleCard::Known { .. })));
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, Action::Choose { card: celtic });
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert_eq!(g.player(P1).hand.len(), 4);
}

#[test]
fn the_forceful_sentry_puts_the_chosen_card_back_into_a_shuffled_deck() {
    let mut g = Game::new(
        shared(),
        &stacked([&[THE_FORCEFUL_SENTRY], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
    )
    .unwrap();
    activate(&mut g, THE_FORCEFUL_SENTRY);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    let events = play(&mut g, Action::Choose { card: celtic });
    assert_eq!(g.instance(celtic).location, Location::Deck);
    assert_eq!(g.deck_size(P1), 36);
    assert!(events.contains(&Event::DeckShuffled { player: P1 }));
}

#[test]
fn delinquent_duo_discards_one_at_random_then_one_of_the_opponents_choice() {
    let mut g = Game::new(shared(), &stacked([&[DELINQUENT_DUO], &[]], MYSTICAL_ELF)).unwrap();
    activate(&mut g, DELINQUENT_DUO);
    assert_eq!(g.player(P0).lp, 7000);
    assert_eq!(g.player(P1).hand.len(), 4, "one random discard");
    assert!(
        decision_is(&g, P1, DecisionKind::Choose),
        "{:?}",
        g.pending_decision()
    );
    let pick = g.legal_actions()[0].clone();
    play(&mut g, pick);
    assert_eq!(g.player(P1).hand.len(), 3);
    assert_eq!(g.graveyard(P1).len(), 2);
}

#[test]
fn painful_choice_shows_five_the_opponent_picks_one_the_rest_are_discarded() {
    let mut g = Game::new(shared(), &stacked([&[PAINFUL_CHOICE], &[]], MYSTICAL_ELF)).unwrap();
    activate(&mut g, PAINFUL_CHOICE);
    let mut picked = Vec::new();
    for _ in 0..5 {
        assert!(
            decision_is(&g, P0, DecisionKind::Choose),
            "{:?}",
            g.pending_decision()
        );
        let a = g.legal_actions()[0].clone();
        let Action::Choose { card } = a else {
            unreachable!()
        };
        picked.push(card);
        play(&mut g, a);
    }
    assert!(
        decision_is(&g, P1, DecisionKind::Choose),
        "the opponent now chooses"
    );
    assert_eq!(g.observe(P1).revealed.len(), 5);
    play(&mut g, Action::Choose { card: picked[2] });
    assert_eq!(g.instance(picked[2]).location, Location::Hand);
    for &c in [picked[0], picked[1], picked[3], picked[4]].iter() {
        assert_eq!(g.instance(c).location, Location::Graveyard);
    }
    assert_eq!(g.deck_size(P0), 35 - 5);
}

#[test]
fn ceremonial_bell_reveals_both_hands_while_face_up() {
    let mut g = Game::new(shared(), &stacked([&[CEREMONIAL_BELL], &[]], MYSTICAL_ELF)).unwrap();
    let bell = in_hand(&g, P0, CEREMONIAL_BELL);
    assert!(
        g.observe(P1)
            .opponent()
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Hidden { .. }))
    );
    play(
        &mut g,
        Action::Summon {
            card: bell,
            tributes: vec![],
            face_down: true,
        },
    );
    assert!(!g.hands_revealed(), "a Set Bell reveals nothing");
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: bell });
    pass_all(&mut g);
    assert!(g.hands_revealed());
    assert!(
        g.observe(P1)
            .opponent()
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Known { .. }))
    );
    assert!(
        g.observe(P0)
            .opponent()
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Known { .. }))
    );
}

#[test]
fn hiros_shadow_scout_makes_the_opponent_draw_three_and_discard_the_spells() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[HIROS_SHADOW_SCOUT],
                &[
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    SPARKS,
                    CELTIC_GUARDIAN,
                    SPARKS,
                ],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let scout = in_hand(&g, P0, HIROS_SHADOW_SCOUT);
    play(
        &mut g,
        Action::Summon {
            card: scout,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    next_turn(&mut g);
    // Player 1 has drawn one Elf on turn 2; the next three are Sparks, Celtic, Sparks.
    let events = play(&mut g, Action::FlipSummon { card: scout });
    pass_all(&mut g);
    let revealed = events
        .iter()
        .find_map(|e| match e {
            Event::Revealed { cards, .. } => Some(cards.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(revealed.len(), 3);
    let spells = revealed
        .iter()
        .filter(|&&c| g.stats(c).kind == CardKind::Spell)
        .count();
    assert_eq!(spells, 2);
    assert!(revealed.iter().all(|&c| {
        let loc = g.instance(c).location;
        if g.stats(c).kind == CardKind::Spell {
            loc == Location::Graveyard
        } else {
            loc == Location::Hand
        }
    }));
}

#[test]
fn big_eye_reorders_the_top_five() {
    let mut g = Game::new(shared(), &stacked([&[BIG_EYE], &[]], MYSTICAL_ELF)).unwrap();
    let eye = in_hand(&g, P0, BIG_EYE);
    play(
        &mut g,
        Action::Summon {
            card: eye,
            tributes: vec![],
            face_down: true,
        },
    );
    next_turn(&mut g);
    next_turn(&mut g);
    let top5 = g.top_of_deck(P0, 5);
    play(&mut g, Action::FlipSummon { card: eye });
    pass_all(&mut g);
    // Choose the five in reverse order: the former fifth goes on top.
    let mut order = Vec::new();
    for _ in 0..5 {
        assert!(
            decision_is(&g, P0, DecisionKind::Choose),
            "{:?}",
            g.pending_decision()
        );
        let options: Vec<_> = g.legal_actions();
        let last = options.last().unwrap().clone();
        let Action::Choose { card } = last else {
            unreachable!()
        };
        order.push(card);
        play(&mut g, last);
    }
    assert_eq!(g.top_of_deck(P0, 5), order);
    assert_eq!(g.top_of_deck(P0, 5)[0], top5[4]);
    let _ = SEVEN_COLORED_FISH;
}
