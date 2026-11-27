//! Rulings tests for Pharaoh's Servant, batch 3 (docs/psv-plan.md):
//! negation and current Types.

use ygo_cards::continuous::{LEGENDARY_SWORD, UMI};
use ygo_cards::psv_monsters::*;
use ygo_cards::psv_spells::*;
use ygo_cards::psv_traps::*;
use ygo_cards::spells::DARK_HOLE;
use ygo_cards::traps::TRAP_HOLE;
use ygo_data::CardId;
use ygo_engine::testing::{CELTIC_GUARDIAN, in_hand, next_turn, pass_all, play, summon};
use ygo_engine::{Action, DecisionKind, Game, InstanceId, Location, Position};

mod psv_common;
use psv_common::*;

const ISLAND_TURTLE: CardId = CardId(4042268); // Level 4 WATER Aqua, 1100 ATK

/// Tribute Summons `id` (Level 5 or 6) with the given monsters.
fn tribute_summon(
    g: &mut Game,
    player: ygo_engine::PlayerId,
    id: CardId,
    tributes: Vec<InstanceId>,
) -> InstanceId {
    let card = in_hand(g, player, id);
    play(
        g,
        Action::Summon {
            card,
            tributes,
            face_down: false,
        },
    );
    pass_all(g);
    card
}

fn trap_offered(g: &Game, id: CardId) -> bool {
    !activations(g, id).is_empty()
}

#[test]
fn jinzo_stops_trap_activations_and_negates_face_up_traps() {
    let mut g = game(&[GRAVITY_BIND, TRAP_HOLE], &[CELTIC_GUARDIAN, JINZO]);
    let bind = set_card(&mut g, P0, GRAVITY_BIND);
    set_card(&mut g, P0, TRAP_HOLE);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, GRAVITY_BIND);
    assert!(g.attack_restricted(celtic));
    next_turn(&mut g);
    let jinzo = in_hand(&g, P1, JINZO);
    play(
        &mut g,
        Action::Summon {
            card: jinzo,
            tributes: vec![celtic],
            face_down: false,
        },
    );
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Respond)
    {
        assert!(!trap_offered(&g, TRAP_HOLE), "no Trap Hole on Jinzo");
        play(&mut g, Action::Pass);
    }
    assert!(g.on_field(jinzo));
    assert!(g.negated(bind));
    let pad = summon_now(&mut g, P1, PAD);
    let _ = pad;
    assert!(!g.attack_restricted(jinzo), "Gravity Bind is negated");
}

#[test]
fn imperial_order_negates_spells_and_costs_700_each_standby() {
    let mut g = game(
        &[IMPERIAL_ORDER],
        &[CELTIC_GUARDIAN, LEGENDARY_SWORD, DARK_HOLE],
    );
    set_card(&mut g, P0, IMPERIAL_ORDER);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    activate_on(&mut g, LEGENDARY_SWORD, &[celtic]);
    assert_eq!(g.atk(celtic), 1700);
    next_turn(&mut g);
    activate(&mut g, IMPERIAL_ORDER);
    assert_eq!(g.atk(celtic), 1400, "the Equip Spell is negated");
    // "Once per turn, during the Standby Phase": every Standby Phase,
    // the opponent's too.
    next_turn(&mut g);
    assert_eq!(lp(&g, P0), 7300, "paid in the opponent's Standby Phase");
    // TCG rulings: Spells may still be activated; their effects are negated.
    let dark_hole = in_hand(&g, P1, DARK_HOLE);
    activate(&mut g, DARK_HOLE);
    assert!(g.on_field(celtic), "Dark Hole resolved without effect");
    assert_eq!(g.instance(dark_hole).location, Location::Graveyard);
    next_turn(&mut g);
    assert_eq!(lp(&g, P0), 6600, "and again in its controller's");
}

#[test]
fn armored_glass_negates_equip_cards_for_the_turn() {
    let mut g = game(&[ARMORED_GLASS], &[CELTIC_GUARDIAN, LEGENDARY_SWORD]);
    set_card(&mut g, P0, ARMORED_GLASS);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let sword = activations(&g, LEGENDARY_SWORD)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &[celtic]))
        .unwrap();
    play(&mut g, sword);
    respond(&mut g, P0, ARMORED_GLASS, None);
    assert_eq!(g.atk(celtic), 1400);
    next_turn(&mut g);
    assert_eq!(g.atk(celtic), 1700, "only this turn");
}

#[test]
fn world_suppression_negates_a_field_spell_as_it_is_activated() {
    let mut g = game(&[WORLD_SUPPRESSION], &[ISLAND_TURTLE, UMI]);
    set_card(&mut g, P0, WORLD_SUPPRESSION);
    next_turn(&mut g);
    let turtle = summon_now(&mut g, P1, ISLAND_TURTLE);
    let umi = activations(&g, UMI).remove(0);
    play(&mut g, umi);
    respond(&mut g, P0, WORLD_SUPPRESSION, None);
    assert_eq!(g.atk(turtle), 1100);
    next_turn(&mut g);
    assert_eq!(g.atk(turtle), 1300, "only this turn");
}

#[test]
fn mystic_probe_negates_continuous_spells_for_the_turn() {
    let mut g = game(&[MYSTIC_PROBE, GIROCHIN_KUWAGATA], &[INSECT_BARRIER]);
    let insect = summon_now(&mut g, P0, GIROCHIN_KUWAGATA);
    set_card(&mut g, P0, MYSTIC_PROBE);
    next_turn(&mut g);
    let barrier = in_hand(&g, P1, INSECT_BARRIER);
    let a = activations(&g, INSECT_BARRIER).remove(0);
    play(&mut g, a);
    respond(&mut g, P0, MYSTIC_PROBE, None);
    assert!(g.negated(barrier));
    assert!(!g.attack_restricted(insect));
    next_turn(&mut g);
    assert!(!g.negated(barrier));
    assert!(g.attack_restricted(insect));
}

#[test]
fn metal_detector_negates_continuous_traps_for_the_turn() {
    let mut g = game(&[GRAVITY_BIND, METAL_DETECTOR], &[CELTIC_GUARDIAN]);
    let bind = set_card(&mut g, P0, GRAVITY_BIND);
    set_card(&mut g, P0, METAL_DETECTOR);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let a = activations(&g, GRAVITY_BIND).remove(0);
    play(&mut g, a);
    respond(&mut g, P0, METAL_DETECTOR, None);
    assert!(g.negated(bind));
    assert!(!g.attack_restricted(celtic));
    next_turn(&mut g);
    assert!(g.attack_restricted(celtic));
}

#[test]
fn prohibition_stops_a_seen_name_from_being_played_from_the_hand() {
    let mut g = game(&[PROHIBITION], &[CELTIC_GUARDIAN, CELTIC_GUARDIAN]);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, PROHIBITION);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Choose);
    choose(&mut g, celtic);
    next_turn(&mut g);
    let other = in_hand(&g, P1, CELTIC_GUARDIAN);
    assert!(
        !g.legal_actions().contains(&summon(other)),
        "the hand copy is prohibited"
    );
    assert!(
        !g.attack_restricted(celtic),
        "the one on the field is not affected"
    );
}

#[test]
fn dna_surgery_makes_every_face_up_monster_the_declared_type() {
    let mut g = game(&[DNA_SURGERY], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, DNA_SURGERY);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, DNA_SURGERY);
    let dragon = RACES.iter().position(|&r| r == "Dragon").unwrap() as u8;
    answer(&mut g, dragon);
    assert_eq!(g.race(celtic), "Dragon");
}

#[test]
fn the_regulation_of_tribe_stops_the_declared_type_and_needs_a_tribute_each_standby() {
    let mut g = game(&[THE_REGULATION_OF_TRIBE], &[CELTIC_GUARDIAN]);
    let tribe = set_card(&mut g, P0, THE_REGULATION_OF_TRIBE);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let mammoth = summon_now(&mut g, P1, PAD);
    next_turn(&mut g);
    activate(&mut g, THE_REGULATION_OF_TRIBE);
    let warrior = RACES.iter().position(|&r| r == "Warrior").unwrap() as u8;
    answer(&mut g, warrior);
    assert!(g.attack_restricted(celtic));
    assert!(!g.attack_restricted(mammoth));
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(
        g.instance(tribe).location,
        Location::Graveyard,
        "no monster to Tribute in your Standby Phase"
    );
}

#[test]
fn parasite_paracide_summons_itself_when_drawn_and_makes_the_drawers_monsters_insects() {
    let mut g = game(&[PARASITE_PARACIDE], &[CELTIC_GUARDIAN]);
    let paracide = set_monster(&mut g, P0, PARASITE_PARACIDE);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    flip_summon(&mut g, paracide);
    assert_eq!(g.instance(paracide).location, Location::Deck);
    assert!(g.deck(P1).contains(&paracide));
    assert_eq!(g.instance(paracide).position, Position::FaceUp);
    g.reorder_top(P1, &[paracide]);
    next_turn(&mut g);
    assert_eq!(g.instance(paracide).location, Location::MonsterZone(1));
    assert_eq!(g.instance(paracide).position, Position::FaceUpDefense);
    assert_eq!(g.instance(paracide).controller, P1);
    assert_eq!(lp(&g, P1), 7000);
    assert_eq!(g.race(celtic), "Insect");
}

#[test]
fn the_legendary_fisherman_cannot_be_attacked_or_destroyed_by_spells_with_umi() {
    let mut g = game(
        &[UMI, PAD, THE_LEGENDARY_FISHERMAN],
        &[CELTIC_GUARDIAN, DARK_HOLE],
    );
    activate(&mut g, UMI);
    let pad = summon_now(&mut g, P0, PAD);
    let fisherman = tribute_summon(&mut g, P0, THE_LEGENDARY_FISHERMAN, vec![pad]);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    let targets: Vec<_> = g
        .legal_actions()
        .into_iter()
        .filter_map(|a| match a {
            Action::Attack { attacker, target } if attacker == celtic => Some(target),
            _ => None,
        })
        .collect();
    assert_eq!(targets, vec![None], "only a direct attack");
    play(&mut g, Action::EndBattlePhase);
    pass_all(&mut g);
    activate(&mut g, DARK_HOLE);
    assert!(g.on_field(fisherman), "unaffected by Dark Hole");
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn deepsea_warrior_is_unaffected_by_spells_while_umi_is_face_up() {
    let mut g = game(&[UMI, PAD, DEEPSEA_WARRIOR], &[DARK_HOLE]);
    activate(&mut g, UMI);
    let pad = summon_now(&mut g, P0, PAD);
    let warrior = tribute_summon(&mut g, P0, DEEPSEA_WARRIOR, vec![pad]);
    next_turn(&mut g);
    activate(&mut g, DARK_HOLE);
    assert!(g.on_field(warrior));
}
