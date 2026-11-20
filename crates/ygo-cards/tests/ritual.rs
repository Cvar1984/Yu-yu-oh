//! Rulings tests for the Ritual Spells, Ritual Monsters and Relinquished.

use std::sync::Arc;

use ygo_cards::ritual::*;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, SUMMONED_SKULL, answer, in_hand, next_turn,
    pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, Game, Location, PlayerId, Shared};

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

#[test]
fn turtle_oath_tributes_levels_totalling_eight_from_hand_or_field() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    TURTLE_OATH,
                    CRAB_TURTLE,
                    SUMMONED_SKULL,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                ],
                &[],
            ],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    // Hand: Skull (6), Elf (4), Elf (4). Sets reaching 8: Skull+Elf x2 and
    // Elf+Elf. Skull+Elf+Elf is not offered: a monster beyond the ones
    // needed may not be Tributed (Skull+Elf already reach 8).
    let offered = activations(&g, TURTLE_OATH);
    assert_eq!(offered.len(), 3, "{offered:?}");
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    // Now one Elf is on the field: the same three sets.
    let offered = activations(&g, TURTLE_OATH);
    assert_eq!(offered.len(), 3);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let turtle = in_hand(&g, P0, CRAB_TURTLE);
    let skull_and_field_elf = offered
        .iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![turtle, skull, elf]))
        .expect("Skull from the hand and the Elf from the field")
        .clone();
    play(&mut g, skull_and_field_elf);
    pass_all(&mut g);
    answer(&mut g, 0);
    assert!(g.on_field(turtle));
    assert_eq!(g.atk(turtle), 2550);
    assert_eq!(g.instance(skull).location, Location::Graveyard);
    assert_eq!(g.instance(elf).location, Location::Graveyard);
    assert_eq!(g.player(P0).monsters_entered, 2);
}

#[test]
fn ritual_spells_need_their_monster_the_levels_and_the_rule() {
    let g = Game::new(
        shared(),
        &stacked(
            [&[HAMBURGER_RECIPE, SUMMONED_SKULL, MYSTICAL_ELF], &[]],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    assert!(
        activations(&g, HAMBURGER_RECIPE).is_empty(),
        "no Hungry Burger in hand"
    );
    let g = Game::new(
        shared(),
        &stacked(
            [
                &[COMMENCEMENT_DANCE, PERFORMANCE_OF_SWORD, MYSTICAL_ELF],
                &[],
            ],
            ygo_cards::monsters::KURIBOH,
        ),
    )
    .unwrap();
    assert_eq!(
        activations(&g, COMMENCEMENT_DANCE).len(),
        1,
        "Elf (4) plus two Kuribohs (1 each) reach 6 exactly: one set of three"
    );
    let _ = g;
    let base = shared();
    let off = with_rules(&base, |r| r.summon.ritual_summons = false);
    let g = Game::new(
        off,
        &stacked(
            [
                &[TURTLE_OATH, CRAB_TURTLE, SUMMONED_SKULL, MYSTICAL_ELF],
                &[],
            ],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    assert!(
        activations(&g, TURTLE_OATH).is_empty(),
        "ritual_summons = false"
    );
}

#[test]
fn relinquished_absorbs_a_monster_and_fights_with_its_stats() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[BLACK_ILLUSION_RITUAL, RELINQUISHED, MYSTICAL_ELF],
                &[CELTIC_GUARDIAN, SEVEN_COLORED_FISH],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let offered = activations(&g, BLACK_ILLUSION_RITUAL);
    // Three Elves: one Elf alone reaches Level 1, so only the 3 singles; a
    // second Elf would be an excess Tribute.
    assert_eq!(offered.len(), 3, "{offered:?}");
    let one_elf = offered
        .iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets.len() == 2))
        .unwrap()
        .clone();
    play(&mut g, one_elf);
    pass_all(&mut g);
    answer(&mut g, 0);
    let relinquished = g.player(P0).monsters().next().unwrap();
    assert_eq!(g.stats(relinquished).id, RELINQUISHED);
    assert_eq!((g.atk(relinquished), g.def(relinquished)), (0, 0));
    assert!(
        activations(&g, RELINQUISHED).is_empty(),
        "nothing to absorb yet"
    );

    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    let on_celtic = activations(&g, RELINQUISHED)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, on_celtic);
    pass_all(&mut g);
    assert!(
        matches!(g.instance(celtic).location, Location::SpellTrapZone(_)),
        "absorbed as an Equip"
    );
    assert_eq!(g.instance(celtic).controller, P0);
    assert_eq!((g.atk(relinquished), g.def(relinquished)), (1400, 1200));
    assert!(
        activations(&g, RELINQUISHED).is_empty(),
        "once per turn, and max one"
    );

    // The Fish (1800) attacks Relinquished (1400): the absorbed Celtic
    // Guardian is destroyed instead, player 0 takes 400 and so does player 1.
    next_turn(&mut g);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(relinquished),
        },
    );
    pass_all(&mut g);
    assert!(g.on_field(relinquished));
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert_eq!(g.player(P0).lp, 7600);
    assert_eq!(g.player(P1).lp, 7600, "the damage was reflected");
    assert_eq!((g.atk(relinquished), g.def(relinquished)), (0, 0));
}

#[test]
fn relinquished_loses_its_absorbed_monster_when_it_leaves() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    BLACK_ILLUSION_RITUAL,
                    RELINQUISHED,
                    MYSTICAL_ELF,
                    ygo_cards::spells::DARK_HOLE,
                ],
                &[CELTIC_GUARDIAN],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let a = activations(&g, BLACK_ILLUSION_RITUAL)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    answer(&mut g, 0);
    let relinquished = g.player(P0).monsters().next().unwrap();
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    let a = activations(&g, RELINQUISHED)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    let a = activations(&g, ygo_cards::spells::DARK_HOLE)[0].clone();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(g.instance(relinquished).location, Location::Graveyard);
    assert_eq!(
        g.instance(celtic).location,
        Location::Graveyard,
        "the Equip went with it"
    );
}

#[test]
fn every_ritual_spell_summons_its_monster() {
    // Paladin of White Dragon is not in the pool (see extra_spells tests).
    for (spell, monster, _) in RITUALS
        .into_iter()
        .filter(|&(_, m, _)| m != ygo_cards::ritual::PALADIN_OF_WHITE_DRAGON)
    {
        let mut g = Game::new(
            shared(),
            &stacked(
                [
                    &[spell, monster, SUMMONED_SKULL, MYSTICAL_ELF, MYSTICAL_ELF],
                    &[],
                ],
                CELTIC_GUARDIAN,
            ),
        )
        .unwrap();
        let offered = activations(&g, spell);
        assert!(!offered.is_empty(), "{spell}");
        play(&mut g, offered[0].clone());
        pass_all(&mut g);
        answer(&mut g, 0);
        assert!(
            g.player(P0).monsters().any(|m| g.stats(m).id == monster),
            "{spell} summoned {monster}"
        );
    }
}
