//! Rulings tests for the last 22 cards, and Penguin Knight, which needs
//! Gravekeeper's Servant to be milled by an opponent's effect.

use std::sync::Arc;

use ygo_cards::monsters_a::PENGUIN_KNIGHT;
use ygo_cards::oneshot::SPARKS;
use ygo_cards::remaining::*;
use ygo_cards::spells::{CHANGE_OF_HEART, DARK_HOLE};
use ygo_cards::traps::{MIRROR_FORCE, MYSTICAL_SPACE_TYPHOON};
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, InstanceId, Location, PlayerId, Position, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const PETIT_MOTH: CardId = CardId(58192742); // 300/200

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.summon.normal_or_tribute_per_turn = 5;
        r.hand.max_size = 30;
    })
}

fn game(top: [&[CardId]; 2]) -> Game {
    Game::new(shared(), &stacked(top, MYSTICAL_ELF)).unwrap()
}

fn put(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

fn set_card(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    pass_all(g);
    card
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

fn activate_on(g: &mut Game, id: CardId, target: Option<InstanceId>) {
    let a = activations(g, id)
        .into_iter()
        .find(|a| match (a, target) {
            (Action::Activate { targets, .. }, Some(t)) => targets.contains(&t),
            _ => true,
        })
        .unwrap_or_else(|| panic!("{id} on {target:?}: {:?}", g.legal_actions()));
    play(g, a);
    pass_all(g);
}

fn activate(g: &mut Game, id: CardId) {
    activate_on(g, id, None);
}

/// Takes the pending optional trigger, if any.
fn take_trigger(g: &mut Game) {
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.kind == DecisionKind::Triggers),
        "{:?}",
        g.pending_decision()
    );
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { .. }))
        .unwrap();
    play(g, a);
    pass_all(g);
}

fn attack(g: &mut Game, attacker: InstanceId, target: Option<InstanceId>) {
    play(g, Action::EnterBattlePhase);
    play(g, Action::Attack { attacker, target });
    pass_all(g);
}

#[test]
fn sword_of_deep_seated_adds_500_and_goes_back_on_top_of_the_deck() {
    let mut g = game([&[CELTIC_GUARDIAN, SWORD_OF_DEEP_SEATED, DARK_HOLE], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let sword = in_hand(&g, P0, SWORD_OF_DEEP_SEATED);
    activate_on(&mut g, SWORD_OF_DEEP_SEATED, Some(celtic));
    assert_eq!((g.atk(celtic), g.def(celtic)), (1900, 1700));
    activate(&mut g, DARK_HOLE);
    assert_eq!(g.instance(sword).location, Location::Deck);
    assert_eq!(g.deck(P0).last(), Some(&sword));
}

#[test]
fn germ_infection_weakens_its_monster_each_of_its_controllers_standby_phases() {
    let mut g = game([&[GERM_INFECTION], &[CELTIC_GUARDIAN]]);
    next_turn(&mut g);
    let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate_on(&mut g, GERM_INFECTION, Some(celtic));
    assert_eq!(g.atk(celtic), 1400);
    next_turn(&mut g);
    assert_eq!(g.atk(celtic), 1100, "player 1's Standby Phase");
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.atk(celtic), 800);
}

#[test]
fn paralyzing_potion_stops_a_non_machine_from_attacking() {
    let mut g = game([&[CELTIC_GUARDIAN, PARALYZING_POTION], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    activate_on(&mut g, PARALYZING_POTION, Some(celtic));
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(g.legal_actions(), vec![Action::EndBattlePhase]);
}

#[test]
fn ring_of_magnetism_weakens_its_monster_and_draws_every_attack() {
    let mut g = game([
        &[MYSTICAL_ELF, CELTIC_GUARDIAN, RING_OF_MAGNETISM],
        &[SEVEN_COLORED_FISH],
    ]);
    let elf = put(&mut g, P0, MYSTICAL_ELF);
    put(&mut g, P0, CELTIC_GUARDIAN);
    activate_on(&mut g, RING_OF_MAGNETISM, Some(elf));
    assert_eq!((g.atk(elf), g.def(elf)), (300, 1500));
    next_turn(&mut g);
    let fish = put(&mut g, P1, SEVEN_COLORED_FISH);
    play(&mut g, Action::EnterBattlePhase);
    let attacks: Vec<_> = g
        .legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Attack { .. }))
        .collect();
    assert_eq!(
        attacks,
        vec![Action::Attack {
            attacker: fish,
            target: Some(elf)
        }]
    );
}

#[test]
fn stim_pack_starts_at_700_and_fades_by_200_each_standby_phase() {
    let mut g = game([&[CELTIC_GUARDIAN, STIM_PACK], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    activate_on(&mut g, STIM_PACK, Some(celtic));
    assert_eq!(g.atk(celtic), 2100);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.atk(celtic), 1900);
}

#[test]
fn axe_of_despair_returns_to_the_deck_for_a_tribute() {
    let mut g = game([
        &[CELTIC_GUARDIAN, AXE_OF_DESPAIR, MYSTICAL_ELF],
        &[MYSTICAL_SPACE_TYPHOON],
    ]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let elf = put(&mut g, P0, MYSTICAL_ELF);
    let axe = in_hand(&g, P0, AXE_OF_DESPAIR);
    activate_on(&mut g, AXE_OF_DESPAIR, Some(celtic));
    assert_eq!(g.atk(celtic), 2400);
    next_turn(&mut g);
    activate_on(&mut g, MYSTICAL_SPACE_TYPHOON, Some(axe));
    take_trigger(&mut g);
    assert_eq!(g.deck(P0).last(), Some(&axe));
    assert_eq!(g.player(P0).monsters().count(), 1, "one monster tributed");
    let _ = elf;
}

#[test]
fn black_pendant_burns_500_when_it_leaves() {
    let mut g = game([&[CELTIC_GUARDIAN, BLACK_PENDANT, DARK_HOLE], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    activate_on(&mut g, BLACK_PENDANT, Some(celtic));
    assert_eq!(g.atk(celtic), 1900);
    activate(&mut g, DARK_HOLE);
    assert_eq!(g.player(P1).lp, 7500);
}

#[test]
fn horn_of_light_and_horn_of_the_unicorn_come_back_to_the_deck() {
    let mut g = game([&[CELTIC_GUARDIAN, HORN_OF_LIGHT, DARK_HOLE], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let horn = in_hand(&g, P0, HORN_OF_LIGHT);
    activate_on(&mut g, HORN_OF_LIGHT, Some(celtic));
    assert_eq!(g.def(celtic), 2000);
    activate(&mut g, DARK_HOLE);
    take_trigger(&mut g);
    assert_eq!(g.player(P0).lp, 7500);
    assert_eq!(g.deck(P0).last(), Some(&horn));

    let mut g = game([&[CELTIC_GUARDIAN, HORN_OF_THE_UNICORN, DARK_HOLE], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let horn = in_hand(&g, P0, HORN_OF_THE_UNICORN);
    activate_on(&mut g, HORN_OF_THE_UNICORN, Some(celtic));
    assert_eq!((g.atk(celtic), g.def(celtic)), (2100, 1900));
    activate(&mut g, DARK_HOLE);
    assert_eq!(g.deck(P0).last(), Some(&horn), "mandatory");
}

#[test]
fn megamorph_doubles_when_behind_and_halves_when_ahead() {
    let mut g = game([&[CELTIC_GUARDIAN, MEGAMORPH, SPARKS], &[SPARKS, SPARKS]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    activate_on(&mut g, MEGAMORPH, Some(celtic));
    assert_eq!(g.atk(celtic), 1400, "equal Life Points");
    activate(&mut g, SPARKS);
    assert_eq!(g.atk(celtic), 700, "ahead");
    next_turn(&mut g);
    activate(&mut g, SPARKS);
    activate(&mut g, SPARKS);
    assert_eq!(g.atk(celtic), 2800, "behind");
}

#[test]
fn snatch_steal_takes_a_monster_heals_the_opponent_and_gives_it_back_when_it_goes() {
    let mut g = game([&[SNATCH_STEAL], &[CELTIC_GUARDIAN, MYSTICAL_SPACE_TYPHOON]]);
    next_turn(&mut g);
    let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let snatch = in_hand(&g, P0, SNATCH_STEAL);
    activate_on(&mut g, SNATCH_STEAL, Some(celtic));
    assert_eq!(g.instance(celtic).controller, P0);
    next_turn(&mut g);
    assert_eq!(g.player(P1).lp, 9000, "player 1's Standby Phase");
    activate_on(&mut g, MYSTICAL_SPACE_TYPHOON, Some(snatch));
    assert_eq!(g.instance(celtic).controller, P1, "control returns");
}

#[test]
fn spellbinding_circle_freezes_its_monster_and_dies_with_it() {
    let mut g = game([&[SPELLBINDING_CIRCLE, DARK_HOLE], &[CELTIC_GUARDIAN]]);
    let circle = set_card(&mut g, P0, SPELLBINDING_CIRCLE);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    while !g
        .pending_decision()
        .is_some_and(|d| d.player == P0 && !activations(&g, SPELLBINDING_CIRCLE).is_empty())
    {
        play(&mut g, Action::Pass);
    }
    activate_on(&mut g, SPELLBINDING_CIRCLE, Some(celtic));
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(g.legal_actions(), vec![Action::EndBattlePhase]);
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);
    assert!(
        !g.legal_actions()
            .contains(&Action::ChangePosition { card: celtic })
    );
    next_turn(&mut g);
    activate(&mut g, DARK_HOLE);
    assert_eq!(g.instance(circle).location, Location::Graveyard);
}

#[test]
fn robbin_goblin_makes_the_opponent_discard_on_battle_damage() {
    let mut g = game([&[ROBBIN_GOBLIN, CELTIC_GUARDIAN], &[]]);
    set_card(&mut g, P0, ROBBIN_GOBLIN);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    // Modern rule: a Trap waits a turn after being Set.
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, ROBBIN_GOBLIN);
    next_turn(&mut g);
    next_turn(&mut g);
    let hand = g.player(P1).hand.len();
    attack(&mut g, celtic, None);
    assert_eq!(g.player(P1).hand.len(), hand - 1);
}

#[test]
fn gravekeepers_servant_mills_each_attacker_and_penguin_knight_reshuffles() {
    let mut g = game([
        &[GRAVEKEEPERS_SERVANT],
        &[
            SEVEN_COLORED_FISH,
            SPARKS,
            SPARKS,
            SPARKS,
            SPARKS,
            MYSTICAL_ELF,
            PENGUIN_KNIGHT,
        ],
    ]);
    activate(&mut g, GRAVEKEEPERS_SERVANT);
    next_turn(&mut g);
    let fish = put(&mut g, P1, SEVEN_COLORED_FISH);
    assert_eq!(
        g.stats(*g.deck(P1).last().unwrap()).id,
        PENGUIN_KNIGHT,
        "Penguin Knight is on top"
    );
    let deck = g.deck_size(P1);
    let graveyard = g.graveyard(P1).len();
    attack(&mut g, fish, None);
    // Milled by player 0's effect: the Graveyard (with Penguin Knight) and
    // the Deck are shuffled together.
    assert!(g.graveyard(P1).is_empty(), "{:?}", g.graveyard(P1));
    assert_eq!(g.deck_size(P1), deck + graveyard);
    assert_eq!(g.player(P0).lp, 8000 - 1800);
}

#[test]
fn toll_charges_500_per_attack() {
    let mut g = game([&[TOLL, CELTIC_GUARDIAN], &[]]);
    activate(&mut g, TOLL);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    next_turn(&mut g);
    attack(&mut g, celtic, None);
    assert_eq!(g.player(P0).lp, 7500);
}

#[test]
fn chain_energy_charges_500_for_every_card_played_from_the_hand() {
    let mut g = game([&[CHAIN_ENERGY, CELTIC_GUARDIAN, SPARKS], &[MYSTICAL_ELF]]);
    activate(&mut g, CHAIN_ENERGY);
    assert_eq!(
        g.player(P0).lp,
        8000,
        "Chain Energy itself is played before it is active"
    );
    put(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, SPARKS);
    assert_eq!(g.player(P0).lp, 7000);
    next_turn(&mut g);
    put(&mut g, P1, MYSTICAL_ELF);
    assert_eq!(g.player(P1).lp, 7500);

    let base = shared();
    let poor = with_rules(&base, |r| r.life.starting_lp = 500);
    let mut g = Game::new(
        poor,
        &stacked([&[CHAIN_ENERGY, CELTIC_GUARDIAN], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    activate(&mut g, CHAIN_ENERGY);
    assert!(
        !g.legal_actions()
            .iter()
            .any(|a| matches!(a, Action::Summon { .. })),
        "500 LP cannot pay 500"
    );
}

#[test]
fn chorus_of_sanctuary_adds_500_def_to_defense_position_monsters() {
    let mut g = game([&[CELTIC_GUARDIAN, MYSTICAL_ELF, CHORUS_OF_SANCTUARY], &[]]);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(
        &mut g,
        Action::Summon {
            card: elf,
            tributes: vec![],
            face_down: true,
        },
    );
    pass_all(&mut g);
    activate(&mut g, CHORUS_OF_SANCTUARY);
    assert_eq!(g.def(celtic), 1200, "Attack Position");
    assert_eq!(g.def(elf), 2500, "face-down Defense Position");
}

#[test]
fn fairys_hand_mirror_moves_a_spells_target() {
    let mut g = game([&[FAIRYS_HAND_MIRROR, CELTIC_GUARDIAN], &[CHANGE_OF_HEART]]);
    set_card(&mut g, P0, FAIRYS_HAND_MIRROR);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let elf = put(&mut g, P0, MYSTICAL_ELF);
    next_turn(&mut g);
    let a = activations(&g, CHANGE_OF_HEART)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, a);
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond)
    );
    activate_on(&mut g, FAIRYS_HAND_MIRROR, Some(elf));
    assert_eq!(g.instance(elf).controller, P1, "the Elf was taken instead");
    assert_eq!(g.instance(celtic).controller, P0);
}

#[test]
fn fake_trap_takes_the_hit_for_another_trap() {
    let mut g = game([&[FAKE_TRAP, MIRROR_FORCE], &[MYSTICAL_SPACE_TYPHOON]]);
    let fake = set_card(&mut g, P0, FAKE_TRAP);
    let mirror = set_card(&mut g, P0, MIRROR_FORCE);
    next_turn(&mut g);
    let a = activations(&g, MYSTICAL_SPACE_TYPHOON)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![mirror]))
        .unwrap();
    play(&mut g, a);
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond)
    );
    activate(&mut g, FAKE_TRAP);
    assert_eq!(g.instance(fake).location, Location::Graveyard);
    assert!(g.on_field(mirror), "Mirror Force survived the Typhoon");
}

#[test]
fn horn_of_heaven_tributes_to_negate_a_summon() {
    let mut g = game([&[HORN_OF_HEAVEN, MYSTICAL_ELF], &[CELTIC_GUARDIAN]]);
    set_card(&mut g, P0, HORN_OF_HEAVEN);
    let elf = put(&mut g, P0, MYSTICAL_ELF);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    assert!(
        g.pending_decision()
            .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond)
    );
    activate(&mut g, HORN_OF_HEAVEN);
    assert_eq!(g.instance(elf).location, Location::Graveyard, "the tribute");
    assert_eq!(
        g.instance(celtic).location,
        Location::Graveyard,
        "the negated summon"
    );
}

#[test]
fn house_of_adhesive_tape_and_eatgaboon_catch_weak_summons() {
    for trap in [HOUSE_OF_ADHESIVE_TAPE, EATGABOON] {
        let mut g = game([&[trap], &[CELTIC_GUARDIAN, PETIT_MOTH]]);
        set_card(&mut g, P0, trap);
        next_turn(&mut g);
        let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
        assert!(g.on_field(celtic), "{trap}: 1400/1200 is not weak");
        let moth = in_hand(&g, P1, PETIT_MOTH);
        play(&mut g, summon(moth));
        assert!(
            g.pending_decision()
                .is_some_and(|d| d.player == P0 && d.kind == DecisionKind::Respond),
            "{trap}"
        );
        activate(&mut g, trap);
        assert_eq!(g.instance(moth).location, Location::Graveyard, "{trap}");
    }
    let _ = Position::FaceUp;
}
