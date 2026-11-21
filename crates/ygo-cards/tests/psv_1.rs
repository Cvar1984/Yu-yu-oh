//! Rulings tests for Pharaoh's Servant, batch 1 (docs/psv-plan.md): one
//! scenario per card, on the modern text.

use std::sync::Arc;

use ygo_cards::continuous::{LEGENDARY_SWORD, UMI};
use ygo_cards::extra_spells::GRACEFUL_CHARITY;
use ygo_cards::monsters::MAN_EATER_BUG;
use ygo_cards::oneshot::{HINOTAMA, SPARKS};
use ygo_cards::psv_monsters::*;
use ygo_cards::psv_spells::*;
use ygo_cards::psv_traps::*;
use ygo_cards::spells::{DARK_HOLE, POT_OF_GREED};
use ygo_cards::traps::{MYSTICAL_SPACE_TYPHOON as MST, TRAP_HOLE};
use ygo_data::{CardId, Rules};
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, MYSTICAL_ELF, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{
    Action, DecisionKind, Event, Game, InstanceId, Location, PlayerId, Position, Shared,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const PAD: CardId = MAMMOTH_GRAVEYARD; // Level 3, 1200/800 EARTH Dinosaur
const KOUMORI_DRAGON: CardId = CardId(67724379); // Level 4 DARK Dragon
const CYBER_FALCON: CardId = CardId(30655537); // Level 4 WIND Machine, 1400 ATK
const GIROCHIN_KUWAGATA: CardId = CardId(84620194); // Level 4 Insect

/// Ten-card opening hands, five Normal Summons a turn and no hand limit
/// to speak of, so one turn can set up a board.
fn rules(r: &mut Rules) {
    r.summon.normal_or_tribute_per_turn = 5;
    r.hand.opening_size = 10;
    r.hand.max_size = 30;
}

fn shared() -> Arc<Shared> {
    with_rules(&shared_from(ygo_cards::registry()), rules)
}

fn game(p0: &[CardId], p1: &[CardId]) -> Game {
    Game::new(shared(), &stacked([p0, p1], PAD)).unwrap()
}

/// `hand` in the opening hand, then the pads drawn in this player's next
/// two Draw Phases, then `deck` in draw order.
fn with_deck(hand: &[CardId], deck: &[CardId]) -> Vec<CardId> {
    let mut cards = hand.to_vec();
    cards.resize(12, PAD);
    cards.extend_from_slice(deck);
    cards
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

/// Activates card `id` (the first activation offered) and passes.
fn activate(g: &mut Game, id: CardId) {
    let a = activations(g, id)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{id} not offered: {:?}", g.legal_actions()));
    play(g, a);
    pass_all(g);
}

/// Activates card `id` with exactly these targets and passes.
fn activate_on(g: &mut Game, id: CardId, targets: &[InstanceId]) {
    let a = activations(g, id)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets: t, .. } if t == targets))
        .unwrap_or_else(|| panic!("{id} not offered on {targets:?}: {:?}", g.legal_actions()));
    play(g, a);
    pass_all(g);
}

/// Passes response windows until `player` may activate `id` (on `targets`
/// if given), then activates it and passes.
fn respond(g: &mut Game, player: PlayerId, id: CardId, targets: Option<&[InstanceId]>) {
    loop {
        let d = g
            .pending_decision()
            .unwrap_or_else(|| panic!("no decision before {id}"));
        assert_eq!(d.kind, DecisionKind::Respond, "waiting for {id}: {d:?}");
        let offer = activations(g, id).into_iter().find(|a| {
            matches!(a, Action::Activate { targets: t, .. } if targets.is_none_or(|x| x == t.as_slice()))
        });
        match offer {
            Some(a) if d.player == player => {
                play(g, a);
                pass_all(g);
                return;
            }
            _ => {
                play(g, Action::Pass);
            }
        }
    }
}

fn set_card(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    pass_all(g);
    card
}

fn summon_now(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

fn set_monster(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(
        g,
        Action::Summon {
            card,
            tributes: vec![],
            face_down: true,
        },
    );
    pass_all(g);
    card
}

fn flip_summon(g: &mut Game, card: InstanceId) {
    play(g, Action::FlipSummon { card });
    pass_all(g);
}

fn attack(g: &mut Game, attacker: InstanceId, target: Option<InstanceId>) {
    if g.legal_actions().contains(&Action::EnterBattlePhase) {
        play(g, Action::EnterBattlePhase);
        pass_all(g);
    }
    play(g, Action::Attack { attacker, target });
}

fn choose(g: &mut Game, card: InstanceId) {
    play(g, Action::Choose { card });
    pass_all(g);
}

fn answer(g: &mut Game, option: u8) {
    play(g, Action::Answer { option });
    pass_all(g);
}

fn lp(g: &Game, p: PlayerId) -> u32 {
    g.player(p).lp
}

fn in_deck(g: &Game, p: PlayerId, id: CardId) -> usize {
    g.deck(p).iter().filter(|&&c| g.stats(c).id == id).count()
}

// ----- monsters -----

#[test]
fn bite_shoes_changes_the_position_of_a_face_up_monster() {
    let mut g = game(&[BITE_SHOES], &[CELTIC_GUARDIAN]);
    let shoes = set_monster(&mut g, P0, BITE_SHOES);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    flip_summon(&mut g, shoes);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::Choose);
    choose(&mut g, celtic);
    assert_eq!(g.instance(celtic).position, Position::FaceUpDefense);
}

#[test]
fn bombardment_beetle_destroys_a_face_down_effect_monster_but_not_a_normal_one() {
    let mut g = game(&[BOMBARDMENT_BEETLE], &[MAD_SWORD_BEAST]);
    let beetle = set_monster(&mut g, P0, BOMBARDMENT_BEETLE);
    next_turn(&mut g);
    let beast = set_monster(&mut g, P1, MAD_SWORD_BEAST);
    let mammoth = set_monster(&mut g, P1, PAD);
    next_turn(&mut g);
    flip_summon(&mut g, beetle);
    choose(&mut g, beast);
    assert_eq!(g.instance(beast).location, Location::Graveyard);
    assert_eq!(g.instance(mammoth).position, Position::FaceDownDefense);
}

#[test]
fn bubonic_vermin_may_summon_another_from_the_deck_face_down() {
    let mut g = game(&with_deck(&[BUBONIC_VERMIN], &[BUBONIC_VERMIN]), &[]);
    let vermin = set_monster(&mut g, P0, BUBONIC_VERMIN);
    next_turn(&mut g);
    next_turn(&mut g);
    flip_summon(&mut g, vermin);
    answer(&mut g, 0);
    let summoned: Vec<_> = g
        .monsters_controlled(P0)
        .into_iter()
        .filter(|&m| m != vermin)
        .collect();
    assert_eq!(summoned.len(), 1);
    assert_eq!(g.stats(summoned[0]).id, BUBONIC_VERMIN);
    assert_eq!(g.instance(summoned[0]).position, Position::FaceDownDefense);
}

#[test]
fn buster_blader_counts_face_up_dragons_the_opponent_controls() {
    let mut g = game(
        &[CELTIC_GUARDIAN, PAD, BUSTER_BLADER],
        &[KOUMORI_DRAGON, KOUMORI_DRAGON],
    );
    let a = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let b = summon_now(&mut g, P0, PAD);
    let blader = in_hand(&g, P0, BUSTER_BLADER);
    play(
        &mut g,
        Action::Summon {
            card: blader,
            tributes: vec![a, b],
            face_down: false,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.atk(blader), 2600);
    next_turn(&mut g);
    summon_now(&mut g, P1, KOUMORI_DRAGON);
    set_monster(&mut g, P1, KOUMORI_DRAGON);
    assert_eq!(g.atk(blader), 3100, "the Set Dragon does not count");
}

#[test]
fn drill_bug_may_put_parasite_paracide_on_top_after_battle_damage() {
    let mut g = game(&with_deck(&[DRILL_BUG], &[PARASITE_PARACIDE]), &[]);
    let bug = summon_now(&mut g, P0, DRILL_BUG);
    next_turn(&mut g);
    next_turn(&mut g);
    attack(&mut g, bug, None);
    pass_all(&mut g);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Triggers));
    activate(&mut g, DRILL_BUG);
    assert_eq!(lp(&g, P1), 8000 - 1100);
    let top = g.deck(P0).last().copied().unwrap();
    assert_eq!(g.stats(top).id, PARASITE_PARACIDE);
}

#[test]
fn four_starred_ladybug_destroys_only_level_4_monsters() {
    let mut g = game(&[FOUR_STARRED_LADYBUG_OF_DOOM], &[CELTIC_GUARDIAN]);
    let ladybug = set_monster(&mut g, P0, FOUR_STARRED_LADYBUG_OF_DOOM);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let mammoth = summon_now(&mut g, P1, PAD);
    next_turn(&mut g);
    flip_summon(&mut g, ladybug);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(g.on_field(mammoth), "Level 3 stays");
}

#[test]
fn gearfried_destroys_an_equip_card_equipped_to_it() {
    let mut g = game(&[GEARFRIED_THE_IRON_KNIGHT, LEGENDARY_SWORD], &[]);
    let gearfried = summon_now(&mut g, P0, GEARFRIED_THE_IRON_KNIGHT);
    let sword = in_hand(&g, P0, LEGENDARY_SWORD);
    activate_on(&mut g, LEGENDARY_SWORD, &[gearfried]);
    assert_eq!(g.instance(sword).location, Location::Graveyard);
    assert_eq!(g.atk(gearfried), 1800);
}

#[test]
fn invitation_to_a_dark_sleep_stops_its_target_attacking_while_face_up() {
    let mut g = game(&[PAD, INVITATION_TO_A_DARK_SLEEP], &[CELTIC_GUARDIAN]);
    let mammoth = summon_now(&mut g, P0, PAD);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let invitation = in_hand(&g, P0, INVITATION_TO_A_DARK_SLEEP);
    play(
        &mut g,
        Action::Summon {
            card: invitation,
            tributes: vec![mammoth],
            face_down: false,
        },
    );
    pass_all(&mut g);
    next_turn(&mut g);
    assert!(g.attack_restricted(celtic));
}

#[test]
fn kiseitai_becomes_an_equip_of_its_attacker_and_drains_its_atk() {
    let mut g = game(&[KISEITAI], &[CELTIC_GUARDIAN]);
    let kiseitai = set_monster(&mut g, P0, KISEITAI);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    attack(&mut g, celtic, Some(kiseitai));
    pass_all(&mut g);
    assert!(matches!(
        g.instance(kiseitai).location,
        Location::SpellTrapZone(_)
    ));
    assert_eq!(g.equip_target(kiseitai), Some(celtic));
    assert_eq!((lp(&g, P0), lp(&g, P1)), (8000, 8000), "no battle");
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(
        lp(&g, P0),
        8700,
        "half of 1400 in the opponent's Standby Phase"
    );
}

#[test]
fn mad_sword_beast_inflicts_piercing_damage() {
    let mut g = game(&[MAD_SWORD_BEAST], &[]);
    let beast = summon_now(&mut g, P0, MAD_SWORD_BEAST);
    next_turn(&mut g);
    let mammoth = set_monster(&mut g, P1, PAD);
    next_turn(&mut g);
    attack(&mut g, beast, Some(mammoth));
    pass_all(&mut g);
    assert_eq!(lp(&g, P1), 8000 - 600);
}

#[test]
fn the_fiend_megacyber_summons_itself_when_outnumbered_by_two() {
    let mut g = game(&[THE_FIEND_MEGACYBER], &[CELTIC_GUARDIAN, MYSTICAL_ELF]);
    let procedure = |g: &Game| {
        g.legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::ProcedureSummon { .. }))
    };
    assert!(procedure(&g).is_none());
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    summon_now(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    let megacyber = in_hand(&g, P0, THE_FIEND_MEGACYBER);
    let a = procedure(&g).expect("offered");
    play(&mut g, a);
    pass_all(&mut g);
    assert!(g.on_field(megacyber));
}

// ----- spells -----

#[test]
fn burning_land_destroys_field_spells_and_burns_each_turn_player() {
    let mut g = game(&[UMI, BURNING_LAND], &[]);
    let umi = in_hand(&g, P0, UMI);
    activate(&mut g, UMI);
    activate(&mut g, BURNING_LAND);
    assert_eq!(g.instance(umi).location, Location::Graveyard);
    next_turn(&mut g);
    assert_eq!((lp(&g, P0), lp(&g, P1)), (8000, 7500));
    next_turn(&mut g);
    assert_eq!((lp(&g, P0), lp(&g, P1)), (7500, 7500));
}

#[test]
fn insect_barrier_stops_only_the_opponents_insects() {
    let mut g = game(&[INSECT_BARRIER, GIROCHIN_KUWAGATA], &[GIROCHIN_KUWAGATA]);
    let mine = summon_now(&mut g, P0, GIROCHIN_KUWAGATA);
    activate(&mut g, INSECT_BARRIER);
    next_turn(&mut g);
    let theirs = summon_now(&mut g, P1, GIROCHIN_KUWAGATA);
    assert!(g.attack_restricted(theirs));
    assert!(!g.attack_restricted(mine));
}

#[test]
fn insect_imitation_tributes_for_an_insect_one_level_higher() {
    let mut g = game(&with_deck(&[INSECT_IMITATION], &[GIROCHIN_KUWAGATA]), &[]);
    let mammoth = summon_now(&mut g, P0, PAD);
    activate(&mut g, INSECT_IMITATION);
    assert_eq!(g.instance(mammoth).location, Location::Graveyard);
    let options = g.legal_actions();
    let Action::Choose { card: insect } = options[0] else {
        panic!("{options:?}")
    };
    assert_eq!(options.len(), 1, "only the Level 4 Insect");
    choose(&mut g, insect);
    answer(&mut g, 0);
    assert_eq!(g.stats(insect).id, GIROCHIN_KUWAGATA);
    assert_eq!(g.instance(insect).position, Position::FaceUpAttack);
}

#[test]
fn inspection_pays_500_to_look_at_a_card_in_the_opponents_standby_phase() {
    let mut g = game(&[INSPECTION], &[]);
    activate(&mut g, INSPECTION);
    pass_all(&mut g);
    play(&mut g, Action::EndTurn);
    pass_all(&mut g);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Triggers));
    let a = activations(&g, INSPECTION).remove(0);
    let events = play(&mut g, a);
    let mut all = events;
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Respond)
    {
        all.extend(play(&mut g, Action::Pass));
    }
    assert_eq!(lp(&g, P0), 7500);
    assert!(all.iter().any(
        |e| matches!(e, Event::LookedAt { player, cards } if *player == P0 && cards.len() == 1)
    ));
}

#[test]
fn limiter_removal_doubles_machines_and_destroys_them_at_the_end_phase() {
    let mut g = game(&[CYBER_FALCON, LIMITER_REMOVAL], &[]);
    let falcon = summon_now(&mut g, P0, CYBER_FALCON);
    activate(&mut g, LIMITER_REMOVAL);
    assert_eq!(g.atk(falcon), 2800);
    next_turn(&mut g);
    assert_eq!(g.instance(falcon).location, Location::Graveyard);
}

#[test]
fn monster_recovery_shuffles_the_monster_and_hand_and_redraws_the_hand() {
    let mut g = game(&[CELTIC_GUARDIAN, MONSTER_RECOVERY], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let hand = g.hand(P0).len();
    activate_on(&mut g, MONSTER_RECOVERY, &[celtic]);
    assert!(!g.on_field(celtic));
    assert_eq!(
        g.hand(P0).len(),
        hand - 1,
        "the hand without Monster Recovery"
    );
}

#[test]
fn nobleman_of_extermination_banishes_a_set_trap_and_its_copies_in_both_decks() {
    let mut g = game(
        &with_deck(&[NOBLEMAN_OF_EXTERMINATION], &[TRAP_HOLE]),
        &with_deck(&[TRAP_HOLE], &[TRAP_HOLE]),
    );
    next_turn(&mut g);
    let hole = set_card(&mut g, P1, TRAP_HOLE);
    next_turn(&mut g);
    activate_on(&mut g, NOBLEMAN_OF_EXTERMINATION, &[hole]);
    assert_eq!(g.instance(hole).location, Location::Banished);
    assert_eq!(in_deck(&g, P0, TRAP_HOLE) + in_deck(&g, P1, TRAP_HOLE), 0);
    assert_eq!(g.banished(P0).len() + g.banished(P1).len(), 3);
}

#[test]
fn rain_of_mercy_gives_both_players_1000() {
    let mut g = game(&[RAIN_OF_MERCY], &[]);
    activate(&mut g, RAIN_OF_MERCY);
    assert_eq!((lp(&g, P0), lp(&g, P1)), (9000, 9000));
}

#[test]
fn seven_completed_gives_a_machine_the_chosen_700() {
    let mut g = game(&[CYBER_FALCON, SEVEN_COMPLETED], &[]);
    let falcon = summon_now(&mut g, P0, CYBER_FALCON);
    activate_on(&mut g, SEVEN_COMPLETED, &[falcon]);
    answer(&mut g, 1);
    assert_eq!((g.atk(falcon), g.def(falcon)), (1400, 1900));
}

#[test]
fn the_shallow_grave_lets_each_player_set_a_monster_from_their_graveyard() {
    let mut g = game(
        &[CELTIC_GUARDIAN, DARK_HOLE, THE_SHALLOW_GRAVE],
        &[MYSTICAL_ELF],
    );
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let elf = summon_now(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    activate(&mut g, DARK_HOLE);
    activate_on(&mut g, THE_SHALLOW_GRAVE, &[celtic]);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P1, DecisionKind::Choose));
    choose(&mut g, elf);
    for (card, owner) in [(celtic, P0), (elf, P1)] {
        assert_eq!(g.instance(card).position, Position::FaceDownDefense);
        assert_eq!(g.instance(card).controller, owner);
    }
}

// ----- traps -----

#[test]
fn appropriate_draws_two_when_the_opponent_draws_outside_the_draw_phase() {
    let mut g = game(&[APPROPRIATE], &[POT_OF_GREED, GRACEFUL_CHARITY]);
    set_card(&mut g, P0, APPROPRIATE);
    next_turn(&mut g);
    let pot = activations(&g, POT_OF_GREED).remove(0);
    play(&mut g, pot);
    respond(&mut g, P0, APPROPRIATE, None);
    let before = g.hand(P0).len();
    activate(&mut g, GRACEFUL_CHARITY);
    while g.pending_decision().unwrap().kind == DecisionKind::Choose {
        let a = g.legal_actions()[0].clone();
        play(&mut g, a);
        pass_all(&mut g);
    }
    assert_eq!(g.hand(P0).len(), before + 2);
}

#[test]
fn attack_and_receive_burns_700_when_you_take_damage() {
    let mut g = game(&[ATTACK_AND_RECEIVE], &[HINOTAMA]);
    set_card(&mut g, P0, ATTACK_AND_RECEIVE);
    next_turn(&mut g);
    let hinotama = in_hand(&g, P1, HINOTAMA);
    let a = activations(&g, HINOTAMA)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { card, .. } if *card == hinotama))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, ATTACK_AND_RECEIVE, None);
    assert_eq!((lp(&g, P0), lp(&g, P1)), (7500, 7300));
}

#[test]
fn call_of_the_haunted_revives_and_takes_the_monster_with_it() {
    let mut g = game(&[CELTIC_GUARDIAN, CALL_OF_THE_HAUNTED, MST], &[DARK_HOLE]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let call = set_card(&mut g, P0, CALL_OF_THE_HAUNTED);
    next_turn(&mut g);
    activate(&mut g, DARK_HOLE);
    next_turn(&mut g);
    activate_on(&mut g, CALL_OF_THE_HAUNTED, &[celtic]);
    assert_eq!(g.instance(celtic).position, Position::FaceUpAttack);
    activate_on(&mut g, MST, &[call]);
    assert_eq!(g.instance(call).location, Location::Graveyard);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn call_of_the_haunted_is_destroyed_with_its_monster() {
    let mut g = game(
        &[CELTIC_GUARDIAN, CALL_OF_THE_HAUNTED, DARK_HOLE],
        &[DARK_HOLE],
    );
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let call = set_card(&mut g, P0, CALL_OF_THE_HAUNTED);
    next_turn(&mut g);
    activate(&mut g, DARK_HOLE);
    next_turn(&mut g);
    activate_on(&mut g, CALL_OF_THE_HAUNTED, &[celtic]);
    activate(&mut g, DARK_HOLE);
    assert_eq!(g.instance(call).location, Location::Graveyard);
}

#[test]
fn ceasefire_flips_face_down_monsters_without_flip_effects_and_burns() {
    let mut g = game(&[CEASEFIRE, MAN_EATER_BUG], &[MAD_SWORD_BEAST]);
    set_card(&mut g, P0, CEASEFIRE);
    let bug = set_monster(&mut g, P0, MAN_EATER_BUG);
    next_turn(&mut g);
    let beast = summon_now(&mut g, P1, MAD_SWORD_BEAST);
    let mammoth = set_monster(&mut g, P1, PAD);
    next_turn(&mut g);
    activate(&mut g, CEASEFIRE);
    assert_eq!(g.pending_decision().unwrap().kind, DecisionKind::MainPhase);
    for m in [bug, mammoth] {
        assert_eq!(g.instance(m).position, Position::FaceUpDefense);
    }
    assert!(g.on_field(beast) && g.on_field(mammoth), "no FLIP effect");
    assert_eq!(lp(&g, P1), 8000 - 1000, "Man-Eater Bug and Mad Sword Beast");
}

#[test]
fn chain_destruction_destroys_the_summoned_monsters_copies_in_hand_and_deck() {
    let mut g = game(
        &[CHAIN_DESTRUCTION],
        &with_deck(&[CELTIC_GUARDIAN, CELTIC_GUARDIAN], &[CELTIC_GUARDIAN]),
    );
    set_card(&mut g, P0, CHAIN_DESTRUCTION);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    respond(&mut g, P0, CHAIN_DESTRUCTION, Some(&[celtic]));
    assert!(g.on_field(celtic), "the monster itself stays");
    let left = g
        .hand(P1)
        .iter()
        .filter(|&&c| g.stats(c).id == CELTIC_GUARDIAN)
        .count();
    assert_eq!(left + in_deck(&g, P1, CELTIC_GUARDIAN), 0);
}

#[test]
fn driving_snow_answers_the_loss_of_a_trap_to_the_opponent() {
    let mut g = game(&[GRAVITY_BIND, DRIVING_SNOW], &[MST, TRAP_HOLE]);
    let bind = set_card(&mut g, P0, GRAVITY_BIND);
    set_card(&mut g, P0, DRIVING_SNOW);
    next_turn(&mut g);
    let hole = set_card(&mut g, P1, TRAP_HOLE);
    next_turn(&mut g);
    activate(&mut g, GRAVITY_BIND);
    next_turn(&mut g);
    let mst = in_hand(&g, P1, MST);
    let a = activations(&g, MST)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { card, targets, .. } if *card == mst && targets == &[bind]))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, DRIVING_SNOW, Some(&[hole]));
    assert_eq!(g.instance(hole).location, Location::Graveyard);
}

#[test]
fn earthshaker_destroys_the_attribute_the_opponent_picks() {
    let mut g = game(&[EARTHSHAKER], &[MYSTICAL_ELF, CELTIC_GUARDIAN]);
    set_card(&mut g, P0, EARTHSHAKER);
    next_turn(&mut g);
    let elf = summon_now(&mut g, P1, MYSTICAL_ELF);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, EARTHSHAKER);
    answer(&mut g, 0); // LIGHT
    answer(&mut g, 1); // EARTH, the second among the other five
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P1, DecisionKind::Choose));
    answer(&mut g, 1);
    assert!(g.on_field(elf));
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn forced_requisition_makes_the_opponent_discard_as_many() {
    let mut g = game(
        &[FORCED_REQUISITION, GRACEFUL_CHARITY, GRACEFUL_CHARITY],
        &[],
    );
    set_card(&mut g, P0, FORCED_REQUISITION);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, GRACEFUL_CHARITY);
    while g.pending_decision().unwrap().kind == DecisionKind::Choose {
        let a = g
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::Choose { card } if g.stats(*card).id != GRACEFUL_CHARITY))
            .unwrap();
        play(&mut g, a);
    }
    activate(&mut g, FORCED_REQUISITION);
    let theirs = g.hand(P1).len();
    activate(&mut g, GRACEFUL_CHARITY);
    while let Some(d) = g.pending_decision()
        && d.kind == DecisionKind::Choose
    {
        let a = g.legal_actions()[0].clone();
        play(&mut g, a);
        pass_all(&mut g);
    }
    assert_eq!(g.hand(P1).len(), theirs - 2);
}

#[test]
fn gravity_bind_stops_level_4_and_higher_from_attacking() {
    let mut g = game(&[GRAVITY_BIND], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, GRAVITY_BIND);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let mammoth = summon_now(&mut g, P1, PAD);
    next_turn(&mut g);
    activate(&mut g, GRAVITY_BIND);
    assert!(g.attack_restricted(celtic));
    assert!(!g.attack_restricted(mammoth));
}

#[test]
fn gust_answers_the_loss_of_a_spell_to_the_opponent() {
    let mut g = game(&[INSECT_BARRIER, GUST], &[MST, TRAP_HOLE]);
    activate(&mut g, INSECT_BARRIER);
    let barrier = g.spells_traps_controlled(P0)[0];
    set_card(&mut g, P0, GUST);
    next_turn(&mut g);
    let hole = set_card(&mut g, P1, TRAP_HOLE);
    let mst = in_hand(&g, P1, MST);
    let a = activations(&g, MST)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { card, targets, .. } if *card == mst && targets == &[barrier]))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, GUST, Some(&[hole]));
    assert_eq!(g.instance(hole).location, Location::Graveyard);
}

#[test]
fn infinite_dismissal_destroys_low_level_summons_in_the_end_phase() {
    let mut g = game(&[INFINITE_DISMISSAL, CELTIC_GUARDIAN], &[]);
    set_card(&mut g, P0, INFINITE_DISMISSAL);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, INFINITE_DISMISSAL);
    let mammoth = summon_now(&mut g, P0, PAD);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    assert_eq!(g.instance(mammoth).location, Location::Graveyard);
    assert!(g.on_field(celtic));
}

#[test]
fn magic_drain_negates_a_spell_unless_the_opponent_discards_a_spell() {
    let mut g = game(&[MAGIC_DRAIN], &[HINOTAMA]);
    set_card(&mut g, P0, MAGIC_DRAIN);
    next_turn(&mut g);
    let hinotama = in_hand(&g, P1, HINOTAMA);
    let a = activations(&g, HINOTAMA)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { card, .. } if *card == hinotama))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, MAGIC_DRAIN, None);
    assert_eq!(lp(&g, P0), 8000, "no other Spell to discard: negated");
    assert_eq!(g.instance(hinotama).location, Location::Graveyard);

    let mut g = game(&[MAGIC_DRAIN], &[HINOTAMA, SPARKS]);
    set_card(&mut g, P0, MAGIC_DRAIN);
    next_turn(&mut g);
    let hinotama = in_hand(&g, P1, HINOTAMA);
    let a = activations(&g, HINOTAMA)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { card, .. } if *card == hinotama))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, MAGIC_DRAIN, None);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P1, DecisionKind::Choose));
    answer(&mut g, 0);
    let sparks = in_hand(&g, P1, SPARKS);
    choose(&mut g, sparks);
    assert_eq!(lp(&g, P0), 7500, "Hinotama resolves");
}

#[test]
fn michizure_destroys_a_monster_when_yours_goes_to_the_graveyard() {
    let mut g = game(&[MICHIZURE], &[CELTIC_GUARDIAN]);
    let mammoth = summon_now(&mut g, P0, PAD);
    set_card(&mut g, P0, MICHIZURE);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    attack(&mut g, celtic, Some(mammoth));
    respond(&mut g, P0, MICHIZURE, Some(&[celtic]));
    assert_eq!(g.instance(mammoth).location, Location::Graveyard);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn minor_goblin_official_burns_500_in_each_opponent_standby_phase() {
    let shared = with_rules(&shared(), |r| r.life.starting_lp = 3000);
    let mut g = Game::new(shared, &stacked([&[MINOR_GOBLIN_OFFICIAL], &[]], PAD)).unwrap();
    set_card(&mut g, P0, MINOR_GOBLIN_OFFICIAL);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, MINOR_GOBLIN_OFFICIAL);
    next_turn(&mut g);
    assert_eq!(lp(&g, P1), 2500);
}

#[test]
fn mirror_wall_halves_an_attacker_and_costs_2000_each_standby() {
    let mut g = game(&[MIRROR_WALL], &[CELTIC_GUARDIAN]);
    let mammoth = summon_now(&mut g, P0, PAD);
    set_card(&mut g, P0, MIRROR_WALL);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    attack(&mut g, celtic, Some(mammoth));
    respond(&mut g, P0, MIRROR_WALL, None);
    pass_all(&mut g);
    assert_eq!(
        g.instance(celtic).location,
        Location::Graveyard,
        "700 against 1200"
    );
    assert_eq!(lp(&g, P1), 7500);
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Choose));
    answer(&mut g, 0);
    assert_eq!(lp(&g, P0), 6000);
}

#[test]
fn numinous_healer_gains_1000_when_you_take_damage() {
    let mut g = game(&[NUMINOUS_HEALER], &[HINOTAMA]);
    set_card(&mut g, P0, NUMINOUS_HEALER);
    next_turn(&mut g);
    let hinotama = in_hand(&g, P1, HINOTAMA);
    let a = activations(&g, HINOTAMA)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { card, .. } if *card == hinotama))
        .unwrap();
    play(&mut g, a);
    respond(&mut g, P0, NUMINOUS_HEALER, None);
    assert_eq!(lp(&g, P0), 8500);
}

#[test]
fn shadow_of_eyes_turns_a_set_monster_face_up_without_its_flip_effect() {
    let mut g = game(&[SHADOW_OF_EYES], &[MAN_EATER_BUG]);
    let mammoth = summon_now(&mut g, P0, PAD);
    set_card(&mut g, P0, SHADOW_OF_EYES);
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
    respond(&mut g, P0, SHADOW_OF_EYES, Some(&[bug]));
    assert_eq!(g.instance(bug).position, Position::FaceUpAttack);
    assert!(g.on_field(mammoth), "no FLIP effect");
}

#[test]
fn skull_invitation_burns_each_owner_300_per_card_sent() {
    let mut g = game(
        &[SKULL_INVITATION, CELTIC_GUARDIAN, DARK_HOLE],
        &[CELTIC_GUARDIAN],
    );
    set_card(&mut g, P0, SKULL_INVITATION);
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, SKULL_INVITATION);
    activate(&mut g, DARK_HOLE);
    pass_all(&mut g);
    assert_eq!(lp(&g, P0), 8000 - 600, "Celtic Guardian and Dark Hole");
    assert_eq!(lp(&g, P1), 8000 - 300);
}

#[test]
fn solemn_wishes_gains_500_when_you_draw() {
    let mut g = game(&[SOLEMN_WISHES, POT_OF_GREED], &[]);
    set_card(&mut g, P0, SOLEMN_WISHES);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, SOLEMN_WISHES);
    activate(&mut g, POT_OF_GREED);
    pass_all(&mut g);
    assert_eq!(lp(&g, P0), 8500);
}

#[test]
fn type_zero_magic_crusher_discards_a_spell_for_500_damage() {
    let mut g = game(&[TYPE_ZERO_MAGIC_CRUSHER, HINOTAMA], &[]);
    set_card(&mut g, P0, TYPE_ZERO_MAGIC_CRUSHER);
    next_turn(&mut g);
    next_turn(&mut g);
    activate(&mut g, TYPE_ZERO_MAGIC_CRUSHER);
    let hinotama = in_hand(&g, P0, HINOTAMA);
    let a = activations(&g, TYPE_ZERO_MAGIC_CRUSHER)
        .into_iter()
        .find(|x| matches!(x, Action::Activate { cost, .. } if cost == &[hinotama]))
        .expect("the Spell is the only cost");
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(lp(&g, P1), 7500);
    assert_eq!(g.instance(hinotama).location, Location::Graveyard);
}
