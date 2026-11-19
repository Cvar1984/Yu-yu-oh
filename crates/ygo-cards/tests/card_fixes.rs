//! Card scripts corrected to their modern text (docs/card-audit.md, batch
//! 3), each shown with the case the old script got wrong.

use std::sync::Arc;

use ygo_cards::continuous::UMI;
use ygo_cards::monsters::SANGAN;
use ygo_cards::remaining::SNATCH_STEAL;
use ygo_cards::spells::{DARK_HOLE, FISSURE, RAIGEKI};
use ygo_cards::traps::MYSTICAL_SPACE_TYPHOON as MST;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, in_hand, next_turn, pass_all, play, shared_from, stacked,
    summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, InstanceId, Location, PlayerId, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const BOOK_OF_SECRET_ARTS: CardId = CardId(91595718);

/// Two Normal Summons a turn, so one turn can set up a board.
fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| r.summon.normal_or_tribute_per_turn = 2)
}

fn game(top: [&[CardId]; 2]) -> Game {
    Game::new(shared(), &stacked(top, CELTIC_GUARDIAN)).unwrap()
}

/// The activation of card `id` whose targets include `target`.
fn activate_on(g: &mut Game, id: CardId, target: InstanceId) {
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| {
            matches!(a, Action::Activate { card, targets, .. }
                if g.stats(*card).id == id && targets.contains(&target))
        })
        .unwrap_or_else(|| panic!("{id} not offered on {target:?}"));
    play(g, a);
    pass_all(g);
}

/// Answers every pending card choice with its first option.
fn choose_first(g: &mut Game) -> usize {
    let mut choices = 0;
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Choose)
    {
        let a = g.legal_actions()[0].clone();
        play(g, a);
        choices += 1;
    }
    pass_all(g);
    choices
}

#[test]
fn sangan_searches_once_per_turn_even_when_two_leave_together() {
    let mut g = game([&[SANGAN, SANGAN], &[RAIGEKI]]);
    for _ in 0..2 {
        let s = in_hand(&g, P0, SANGAN);
        play(&mut g, summon(s));
        pass_all(&mut g);
    }
    assert_eq!(g.player(P0).monsters().count(), 2);
    next_turn(&mut g);
    let hand_before = g.player(P0).hand.len();
    let raigeki = in_hand(&g, P1, RAIGEKI);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == raigeki))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    // Both Sangans trigger; once one is activated, the other is gone.
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.kind == DecisionKind::Triggers)
    );
    let first = g.legal_actions()[0].clone();
    play(&mut g, first);
    assert!(
        g.legal_actions()
            .iter()
            .all(|a| !matches!(a, Action::Activate { card, .. } if g.stats(*card).id == SANGAN)),
        "{:?}",
        g.legal_actions()
    );
    pass_all(&mut g);
    assert_eq!(choose_first(&mut g), 1, "only one search");
    assert_eq!(g.graveyard(P0).len(), 2, "both Sangans were destroyed");
    assert_eq!(g.player(P0).hand.len(), hand_before + 1);
}

#[test]
fn fissure_destroys_the_lowest_current_atk() {
    // Player 1: Mystical Elf with Book of Secret Arts (800 + 300 = 1100)
    // and Sangan (1000). By current ATK, Sangan is the lowest.
    let mut g = game([&[FISSURE], &[MYSTICAL_ELF, SANGAN, BOOK_OF_SECRET_ARTS]]);
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    let sangan = in_hand(&g, P1, SANGAN);
    play(&mut g, summon(elf));
    pass_all(&mut g);
    play(&mut g, summon(sangan));
    pass_all(&mut g);
    activate_on(&mut g, BOOK_OF_SECRET_ARTS, elf);
    assert_eq!(g.atk(elf), 1100);
    next_turn(&mut g);
    let fissure = in_hand(&g, P0, FISSURE);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == fissure))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    choose_first(&mut g); // Sangan's search
    assert_eq!(g.instance(sangan).location, Location::Graveyard);
    assert!(g.on_field(elf), "the Elf has 1100 ATK on the field");
}

#[test]
fn a_stolen_monster_goes_to_its_owners_graveyard_and_returns_when_snatch_steal_leaves() {
    let mut g = game([&[SNATCH_STEAL, MST, SNATCH_STEAL, DARK_HOLE], &[]]);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    next_turn(&mut g);

    activate_on(&mut g, SNATCH_STEAL, celtic);
    assert!(g.monsters_controlled(P0).contains(&celtic));
    let steal = g
        .player(P0)
        .spell_trap_zones
        .iter()
        .flatten()
        .copied()
        .find(|&c| g.stats(c).id == SNATCH_STEAL)
        .unwrap();
    activate_on(&mut g, MST, steal);
    assert_eq!(g.instance(steal).location, Location::Graveyard);
    assert!(
        g.monsters_controlled(P1).contains(&celtic),
        "control returns when Snatch Steal leaves the field"
    );

    activate_on(&mut g, SNATCH_STEAL, celtic);
    assert!(g.monsters_controlled(P0).contains(&celtic));
    let dark_hole = in_hand(&g, P0, DARK_HOLE);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == dark_hole))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    assert!(
        g.graveyard(P1).contains(&celtic),
        "a destroyed monster goes to its owner's Graveyard"
    );
    assert!(g.player(P0).monsters().next().is_none());
    assert!(g.player(P1).monsters().next().is_none());
}

#[test]
fn mystical_space_typhoon_can_target_a_field_spell() {
    let mut g = game([&[MST], &[UMI]]);
    next_turn(&mut g);
    let umi = in_hand(&g, P1, UMI);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == umi))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(g.instance(umi).location, Location::FieldZone);
    next_turn(&mut g);
    activate_on(&mut g, MST, umi);
    assert_eq!(g.instance(umi).location, Location::Graveyard);
}
