//! Rulings tests for Effect Monsters batch B.

use std::sync::Arc;

use ygo_cards::monsters_b::*;
use ygo_cards::oneshot::SPARKS;
use ygo_cards::spells::MONSTER_REBORN;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, answer, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{
    Action, DecisionKind, Event, Game, GameSetup, InstanceId, Location, Phase, PlayerId, Position,
    Shared,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

/// No tributes needed, five summons a turn, room in the hand: lets a test
/// put any monster on the field at once.
fn free() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.summon.tributes_level_5_6 = 0;
        r.summon.tributes_level_7_plus = 0;
        r.summon.normal_or_tribute_per_turn = 5;
        r.hand.max_size = 30;
    })
}

fn game(top: [&[CardId]; 2]) -> Game {
    Game::new(free(), &stacked(top, MYSTICAL_ELF)).unwrap()
}

fn game_with_seed(top: [&[CardId]; 2], seed: u64) -> Game {
    let mut setup: GameSetup = stacked(top, MYSTICAL_ELF);
    setup.seed = seed;
    Game::new(free(), &setup).unwrap()
}

fn put(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

/// Activations of card `id`, and its summon procedure in Attack Position.
fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| match a {
            Action::Activate { card, .. } => g.stats(*card).id == id,
            Action::ProcedureSummon { card, position, .. } => {
                g.stats(*card).id == id && *position == Position::FaceUpAttack
            }
            _ => false,
        })
        .collect()
}

fn activate_where(g: &mut Game, id: CardId, keep: impl Fn(&Action) -> bool) -> Vec<Event> {
    let a = activations(g, id).into_iter().find(|a| keep(a));
    let a = a.unwrap_or_else(|| panic!("{id}: no matching activation in {:?}", g.legal_actions()));
    let events = play(g, a);
    pass_all(g);
    events
}

fn activate(g: &mut Game, id: CardId) -> Vec<Event> {
    activate_where(g, id, |_| true)
}

fn with_cost(card: InstanceId) -> impl Fn(&Action) -> bool {
    move |a| matches!(a, Action::Activate { cost, .. } if cost == &vec![card])
}

fn decision_is(g: &Game, player: PlayerId, kind: DecisionKind) -> bool {
    g.pending_decision()
        .is_some_and(|d| d.player == player && d.kind == kind)
}

fn coins(events: &[Event]) -> Vec<bool> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::CoinTossed { heads, .. } => Some(*heads),
            _ => None,
        })
        .collect()
}

fn attack(g: &mut Game, attacker: InstanceId, target: Option<InstanceId>) -> Vec<Event> {
    if g.phase() != Phase::Battle(ygo_engine::BattleStep::Battle) {
        play(g, Action::EnterBattlePhase);
    }
    let events = play(g, Action::Attack { attacker, target });
    pass_all(g);
    events
}

#[test]
fn catapult_turtle_and_cannon_soldier_burn_by_tributing() {
    let mut g = game([
        &[
            CATAPULT_TURTLE,
            CANNON_SOLDIER,
            CELTIC_GUARDIAN,
            MYSTICAL_ELF,
        ],
        &[],
    ]);
    put(&mut g, P0, CATAPULT_TURTLE);
    let cannon = put(&mut g, P0, CANNON_SOLDIER);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    let elf = put(&mut g, P0, MYSTICAL_ELF);
    activate_where(&mut g, CATAPULT_TURTLE, with_cost(celtic));
    assert_eq!(g.player(P1).lp, 8000 - 700, "half of 1400");
    assert!(activations(&g, CATAPULT_TURTLE).is_empty(), "once per turn");
    activate_where(&mut g, CANNON_SOLDIER, with_cost(elf));
    activate_where(&mut g, CANNON_SOLDIER, with_cost(cannon));
    assert_eq!(
        g.player(P1).lp,
        7300 - 1000,
        "Cannon Soldier is not once per turn, and may tribute itself"
    );
}

#[test]
fn time_wizard_follows_its_coin() {
    let mut seen = [false, false];
    for seed in 0..16 {
        let mut g = game_with_seed([&[TIME_WIZARD, MYSTICAL_ELF], &[CELTIC_GUARDIAN]], seed);
        let wizard = put(&mut g, P0, TIME_WIZARD);
        let elf = put(&mut g, P0, MYSTICAL_ELF);
        next_turn(&mut g);
        let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
        next_turn(&mut g);
        let events = activate(&mut g, TIME_WIZARD);
        let heads = coins(&events)[0];
        seen[usize::from(heads)] = true;
        if heads {
            assert_eq!(g.instance(celtic).location, Location::Graveyard);
            assert!(g.on_field(wizard) && g.on_field(elf));
        } else {
            assert!(g.on_field(celtic));
            assert_eq!(g.instance(wizard).location, Location::Graveyard);
            assert_eq!(g.instance(elf).location, Location::Graveyard);
            assert_eq!(g.player(P0).lp, 8000 - (500 + 800) / 2);
        }
        if g.on_field(wizard) {
            assert!(activations(&g, TIME_WIZARD).is_empty(), "once per turn");
        }
    }
    assert_eq!(seen, [true, true], "16 seeds show both outcomes");
}

#[test]
fn barrel_dragon_destroys_on_two_heads_of_three() {
    let mut seen = [false, false];
    for seed in 0..16 {
        let mut g = game_with_seed([&[BARREL_DRAGON], &[CELTIC_GUARDIAN]], seed);
        put(&mut g, P0, BARREL_DRAGON);
        next_turn(&mut g);
        let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
        next_turn(&mut g);
        let events = activate(&mut g, BARREL_DRAGON);
        let tosses = coins(&events);
        assert_eq!(tosses.len(), 3);
        let destroyed = tosses.iter().filter(|&&h| h).count() >= 2;
        seen[usize::from(destroyed)] = true;
        assert_eq!(
            g.instance(celtic).location == Location::Graveyard,
            destroyed,
            "seed {seed}"
        );
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn karate_man_doubles_its_attack_then_dies_in_the_end_phase() {
    let mut g = game([&[KARATE_MAN], &[]]);
    let karate = put(&mut g, P0, KARATE_MAN);
    activate(&mut g, KARATE_MAN);
    assert_eq!(g.atk(karate), 2000);
    assert!(activations(&g, KARATE_MAN).is_empty());
    next_turn(&mut g);
    assert_eq!(g.instance(karate).location, Location::Graveyard);
}

#[test]
fn the_little_swordsman_of_aile_tributes_for_700() {
    let mut g = game([&[THE_LITTLE_SWORDSMAN_OF_AILE, MYSTICAL_ELF], &[]]);
    let swordsman = put(&mut g, P0, THE_LITTLE_SWORDSMAN_OF_AILE);
    put(&mut g, P0, MYSTICAL_ELF);
    activate(&mut g, THE_LITTLE_SWORDSMAN_OF_AILE);
    assert_eq!(g.atk(swordsman), 1500);
    next_turn(&mut g);
    assert_eq!(g.atk(swordsman), 800, "until the end of the turn");
}

#[test]
fn blast_juggler_tributes_itself_in_the_standby_phase_to_destroy_two_small_monsters() {
    let mut g = game([
        &[BLAST_JUGGLER],
        &[MYSTICAL_ELF, MYSTICAL_ELF, SEVEN_COLORED_FISH],
    ]);
    let juggler = put(&mut g, P0, BLAST_JUGGLER);
    assert!(
        activations(&g, BLAST_JUGGLER).is_empty(),
        "only in the Standby Phase"
    );
    next_turn(&mut g);
    let e1 = put(&mut g, P1, MYSTICAL_ELF);
    let e2 = put(&mut g, P1, MYSTICAL_ELF);
    let fish = put(&mut g, P1, SEVEN_COLORED_FISH);
    play(&mut g, Action::EndTurn);
    assert_eq!(g.phase(), Phase::Standby);
    assert!(
        decision_is(&g, P0, DecisionKind::Respond),
        "{:?}",
        g.pending_decision()
    );
    let offered = activations(&g, BLAST_JUGGLER);
    assert_eq!(offered.len(), 1, "only the two Elves have 1000 or less ATK");
    activate(&mut g, BLAST_JUGGLER);
    for c in [juggler, e1, e2] {
        assert_eq!(g.instance(c).location, Location::Graveyard);
    }
    assert!(g.on_field(fish));
}

#[test]
fn thunder_dragon_discards_itself_to_add_two_more() {
    let mut g = game([
        &[
            THUNDER_DRAGON,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            THUNDER_DRAGON,
            THUNDER_DRAGON,
        ],
        &[],
    ]);
    let td = in_hand(&g, P0, THUNDER_DRAGON);
    let hand = g.player(P0).hand.len();
    activate(&mut g, THUNDER_DRAGON);
    assert_eq!(g.instance(td).location, Location::Graveyard);
    for _ in 0..2 {
        assert!(decision_is(&g, P0, DecisionKind::Choose));
        let a = g.legal_actions()[0].clone();
        play(&mut g, a);
        pass_all(&mut g);
    }
    assert_eq!(g.player(P0).hand.len(), hand - 1 + 2);
    assert_eq!(
        g.player(P0)
            .hand
            .iter()
            .filter(|&&c| g.stats(c).id == THUNDER_DRAGON)
            .count(),
        2
    );
}

#[test]
fn jirai_gumo_halves_its_controllers_life_points_on_a_wrong_call() {
    let mut seen = [false, false];
    for seed in 0..16 {
        let mut g = game_with_seed([&[JIRAI_GUMO], &[]], seed);
        let gumo = put(&mut g, P0, JIRAI_GUMO);
        next_turn(&mut g);
        next_turn(&mut g);
        let events = attack(&mut g, gumo, None);
        let heads = coins(&events)[0];
        seen[usize::from(heads)] = true;
        assert_eq!(g.player(P0).lp, if heads { 8000 } else { 4000 });
        assert_eq!(g.player(P1).lp, 8000 - 2200);
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn dark_elf_pays_1000_to_attack_and_cannot_without_it() {
    let mut g = game([&[DARK_ELF], &[]]);
    let elf = put(&mut g, P0, DARK_ELF);
    next_turn(&mut g);
    next_turn(&mut g);
    attack(&mut g, elf, None);
    assert_eq!((g.player(P0).lp, g.player(P1).lp), (7000, 6000));

    let base = free();
    // Exactly 1000 LP may pay the 1000 (and then loses); 900 may not.
    let exact = with_rules(&base, |r| r.life.starting_lp = 1000);
    let mut g = Game::new(exact, &stacked([&[DARK_ELF], &[]], MYSTICAL_ELF)).unwrap();
    let elf = put(&mut g, P0, DARK_ELF);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert!(g.legal_actions().contains(&Action::Attack {
        attacker: elf,
        target: None
    }));
    let poor = with_rules(&base, |r| r.life.starting_lp = 900);
    let mut g = Game::new(poor, &stacked([&[DARK_ELF], &[]], MYSTICAL_ELF)).unwrap();
    put(&mut g, P0, DARK_ELF);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(
        g.legal_actions(),
        vec![Action::EndBattlePhase],
        "900 LP cannot pay 1000"
    );
}

/// Player 0 has `id` face-up in Attack Position and player 1 has Celtic
/// Guardian; it is player 0's turn 3.
fn position_game(id: CardId) -> (Game, InstanceId, InstanceId) {
    let mut g = game([&[id, MYSTICAL_ELF, SPARKS], &[CELTIC_GUARDIAN]]);
    let card = put(&mut g, P0, id);
    next_turn(&mut g);
    let celtic = put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    (g, card, celtic)
}

#[test]
fn crass_clown_bounces_when_it_goes_from_defense_to_attack() {
    let (mut g, clown, celtic) = position_game(CRASS_CLOWN);
    play(&mut g, Action::ChangePosition { card: clown });
    pass_all(&mut g);
    assert!(g.on_field(celtic), "Attack to Defense does nothing");
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::ChangePosition { card: clown });
    pass_all(&mut g);
    assert!(
        decision_is(&g, P0, DecisionKind::Choose),
        "{:?}",
        g.pending_decision()
    );
    play(&mut g, Action::Choose { card: celtic });
    assert_eq!(g.instance(celtic).location, Location::Hand);
}

#[test]
fn dream_clown_destroys_when_it_goes_to_defense() {
    let (mut g, clown, celtic) = position_game(DREAM_CLOWN);
    play(&mut g, Action::ChangePosition { card: clown });
    pass_all(&mut g);
    play(&mut g, Action::Choose { card: celtic });
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn tainted_wisdom_shuffles_when_it_goes_to_defense() {
    let (mut g, wisdom, _) = position_game(TAINTED_WISDOM);
    let events = play(&mut g, Action::ChangePosition { card: wisdom });
    assert!(events.contains(&Event::DeckShuffled { player: P0 }));
}

#[test]
fn yado_karu_may_put_hand_cards_on_the_bottom_of_the_deck() {
    let (mut g, yado, _) = position_game(YADO_KARU);
    play(&mut g, Action::ChangePosition { card: yado });
    pass_all(&mut g);
    assert!(
        decision_is(&g, P0, DecisionKind::Triggers),
        "optional: {:?}",
        g.pending_decision()
    );
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { .. }))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    play(&mut g, Action::Answer { option: 0 });
    let sparks = in_hand(&g, P0, SPARKS);
    play(&mut g, Action::Choose { card: sparks });
    play(&mut g, Action::Answer { option: 1 });
    pass_all(&mut g);
    assert_eq!(g.instance(sparks).location, Location::Deck);
    assert_eq!(g.deck(P0)[0], sparks, "at the bottom");
}

#[test]
fn pumpking_grows_while_castle_of_dark_illusions_is_on_the_field() {
    let mut g = game([
        &[CASTLE_OF_DARK_ILLUSIONS, PUMPKING_THE_KING_OF_GHOSTS],
        &[],
    ]);
    put(&mut g, P0, CASTLE_OF_DARK_ILLUSIONS);
    let pumpking = put(&mut g, P0, PUMPKING_THE_KING_OF_GHOSTS);
    assert_eq!(
        g.atk(pumpking),
        1900,
        "100 while the Castle is on the field"
    );
    let mut expected = 1900;
    for _ in 0..5 {
        next_turn(&mut g);
        next_turn(&mut g);
        expected = (expected + 100).min(2300);
        assert_eq!(g.atk(pumpking), expected, "turn {}", g.turn());
    }
}

#[test]
fn mushroom_man_2_drains_its_controller_and_can_be_passed_on() {
    let mut g = game([&[MUSHROOM_MAN_2], &[]]);
    let mushroom = put(&mut g, P0, MUSHROOM_MAN_2);
    play(&mut g, Action::EndTurn);
    assert!(
        decision_is(&g, P0, DecisionKind::Triggers),
        "{:?}",
        g.pending_decision()
    );
    play(&mut g, Action::Pass);
    pass_all(&mut g);
    next_turn(&mut g);
    assert_eq!(g.player(P0).lp, 7700, "300 in player 0's Standby Phase");
    play(&mut g, Action::EndTurn);
    assert!(decision_is(&g, P0, DecisionKind::Triggers));
    let pay = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { .. }))
        .unwrap();
    play(&mut g, pay);
    pass_all(&mut g);
    assert_eq!(g.player(P0).lp, 7200);
    assert_eq!(g.instance(mushroom).controller, P1);
    assert_eq!(g.player(P1).lp, 7700, "player 1's Standby Phase");
}

#[test]
fn dark_zebra_alone_turns_to_defense_in_the_standby_phase() {
    let mut g = game([&[DARK_ZEBRA], &[]]);
    let zebra = put(&mut g, P0, DARK_ZEBRA);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(g.instance(zebra).position, Position::FaceUpDefense);
    assert!(
        !g.legal_actions()
            .contains(&Action::ChangePosition { card: zebra }),
        "locked this turn"
    );
}

#[test]
fn sanga_kazejin_and_suijin_zero_the_attackers_atk_once() {
    for guardian in [SANGA_OF_THE_THUNDER, KAZEJIN, SUIJIN] {
        let mut g = game([&[SEVEN_COLORED_FISH, CELTIC_GUARDIAN], &[guardian]]);
        let fish = put(&mut g, P0, SEVEN_COLORED_FISH);
        let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
        next_turn(&mut g);
        let gd = put(&mut g, P1, guardian);
        let guardian_atk = g.atk(gd);
        next_turn(&mut g);
        play(&mut g, Action::EnterBattlePhase);
        play(
            &mut g,
            Action::Attack {
                attacker: fish,
                target: Some(gd),
            },
        );
        assert!(
            decision_is(&g, P1, DecisionKind::Respond),
            "{guardian}: {:?}",
            g.pending_decision()
        );
        let a = activations(&g, guardian)[0].clone();
        play(&mut g, a);
        // The limit counts at activation: it cannot chain to itself (a bot
        // once activated Suijin forever in one Damage Step).
        if decision_is(&g, P0, DecisionKind::Respond) {
            play(&mut g, Action::Pass);
        }
        assert!(
            activations(&g, guardian).is_empty(),
            "{guardian}: once while face-up, even within one chain"
        );
        pass_all(&mut g);
        assert_eq!(g.instance(fish).location, Location::Graveyard, "{guardian}");
        assert_eq!(
            g.player(P0).lp,
            8000 - guardian_atk as u32,
            "the Fish fought with 0 ATK"
        );
        assert!(
            g.lingering().is_empty(),
            "the change ended with damage calculation"
        );
        assert_eq!(g.atk(celtic), 1400);
        play(
            &mut g,
            Action::Attack {
                attacker: celtic,
                target: Some(gd),
            },
        );
        assert!(
            !decision_is(&g, P1, DecisionKind::Respond),
            "{guardian}: only once while face-up"
        );
    }
}

#[test]
fn steel_scorpion_destroys_its_non_machine_attacker_two_turns_later() {
    let mut g = game([&[CELTIC_GUARDIAN], &[STEEL_SCORPION]]);
    next_turn(&mut g);
    let scorpion = put(&mut g, P1, STEEL_SCORPION);
    next_turn(&mut g);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    attack(&mut g, celtic, Some(scorpion));
    assert_eq!(g.instance(scorpion).location, Location::Graveyard);
    play(&mut g, Action::EndBattlePhase);
    for _ in 0..3 {
        next_turn(&mut g);
        assert!(g.on_field(celtic), "turn {}", g.turn());
    }
    assert_eq!(g.turn(), 6);
    next_turn(&mut g);
    next_turn(&mut g);
    assert_eq!(
        g.instance(celtic).location,
        Location::Graveyard,
        "End Phase of turn 7"
    );
}

#[test]
fn electric_lizard_stops_its_attacker_on_the_next_turn() {
    let mut g = game([&[CELTIC_GUARDIAN], &[ELECTRIC_LIZARD]]);
    next_turn(&mut g);
    let lizard = put(&mut g, P1, ELECTRIC_LIZARD);
    next_turn(&mut g);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    attack(&mut g, celtic, Some(lizard));
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert_eq!(
        g.legal_actions(),
        vec![Action::EndBattlePhase],
        "turn 5: no attack"
    );
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    assert!(
        g.legal_actions().contains(&Action::Attack {
            attacker: celtic,
            target: None
        }),
        "turn 7"
    );
}

#[test]
fn kotodama_destroys_a_newcomer_sharing_a_face_up_name() {
    let mut g = game([&[KOTODAMA, MYSTICAL_ELF, MYSTICAL_ELF], &[]]);
    put(&mut g, P0, KOTODAMA);
    let elf1 = put(&mut g, P0, MYSTICAL_ELF);
    let elf2 = put(&mut g, P0, MYSTICAL_ELF);
    assert!(g.on_field(elf1));
    assert_eq!(g.instance(elf2).location, Location::Graveyard);
}

#[test]
fn boar_soldier_dies_when_normal_summoned_and_is_weaker_facing_monsters() {
    let mut g = game([&[BOAR_SOLDIER, MONSTER_REBORN], &[CELTIC_GUARDIAN]]);
    let boar = put(&mut g, P0, BOAR_SOLDIER);
    assert_eq!(g.instance(boar).location, Location::Graveyard);
    next_turn(&mut g);
    put(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    activate(&mut g, MONSTER_REBORN);
    answer(&mut g, 0);
    assert!(g.on_field(boar), "a Special Summon is not a Normal Summon");
    assert_eq!(g.atk(boar), 1000);
}

#[test]
fn the_unhappy_maiden_ends_the_battle_phase() {
    let mut g = game([
        &[CELTIC_GUARDIAN, SEVEN_COLORED_FISH],
        &[THE_UNHAPPY_MAIDEN],
    ]);
    next_turn(&mut g);
    let maiden = put(&mut g, P1, THE_UNHAPPY_MAIDEN);
    next_turn(&mut g);
    let celtic = put(&mut g, P0, CELTIC_GUARDIAN);
    put(&mut g, P0, SEVEN_COLORED_FISH);
    attack(&mut g, celtic, Some(maiden));
    assert_eq!(g.instance(maiden).location, Location::Graveyard);
    assert_eq!(g.phase(), Phase::Main2, "the Fish never gets to attack");
}

#[test]
fn banisher_of_the_light_banishes_what_would_go_to_the_graveyard() {
    let mut g = game([&[BANISHER_OF_THE_LIGHT, SPARKS], &[]]);
    put(&mut g, P0, BANISHER_OF_THE_LIGHT);
    let sparks = in_hand(&g, P0, SPARKS);
    activate(&mut g, SPARKS);
    assert_eq!(g.instance(sparks).location, Location::Banished);
    assert!(g.graveyard(P0).is_empty());
}

#[test]
fn gate_guardian_needs_its_three_parts() {
    let mut g = game([&[GATE_GUARDIAN, SANGA_OF_THE_THUNDER, KAZEJIN, SUIJIN], &[]]);
    let gate = in_hand(&g, P0, GATE_GUARDIAN);
    assert!(
        !g.legal_actions()
            .iter()
            .any(|a| matches!(a, Action::Summon { card, .. } if *card == gate))
    );
    let parts: Vec<_> = [SANGA_OF_THE_THUNDER, KAZEJIN]
        .iter()
        .map(|&id| put(&mut g, P0, id))
        .collect();
    assert!(
        activations(&g, GATE_GUARDIAN).is_empty(),
        "Suijin is missing"
    );
    let suijin = put(&mut g, P0, SUIJIN);
    activate(&mut g, GATE_GUARDIAN);
    assert!(g.on_field(gate));
    for p in parts.into_iter().chain([suijin]) {
        assert_eq!(g.instance(p).location, Location::Graveyard);
    }
}

#[test]
fn cocoon_of_evolution_then_larvae_moth_and_great_moth() {
    for (moth, turns) in [(LARVAE_MOTH, 4), (GREAT_MOTH, 8)] {
        let mut g = game([
            &[PETIT_MOTH, COCOON_OF_EVOLUTION, moth, MONSTER_REBORN],
            &[],
        ]);
        let petit = put(&mut g, P0, PETIT_MOTH);
        activate(&mut g, COCOON_OF_EVOLUTION);
        assert_eq!(
            (g.atk(petit), g.def(petit)),
            (0, 2000),
            "the Cocoon's stats"
        );
        let start = g.turn();
        while g.turn() < start + turns {
            assert!(
                activations(&g, moth).is_empty(),
                "{moth} too early on turn {}",
                g.turn()
            );
            next_turn(&mut g);
            next_turn(&mut g);
        }
        activate(&mut g, moth);
        let m = g.player(P0).monsters().find(|&c| g.stats(c).id == moth);
        let m = m.unwrap_or_else(|| panic!("{moth} on the field"));
        assert_eq!(g.instance(petit).location, Location::Graveyard);

        // "Can only be Special Summoned by Tributing Petit Moth": even after
        // that proper summon, Monster Reborn cannot bring it back.
        g.destroy(m, m);
        assert_eq!(g.instance(m).location, Location::Graveyard);
        assert!(!g.special_summonable(m));
        next_turn(&mut g);
        next_turn(&mut g);
        let reborn: Vec<Action> = g
            .legal_actions()
            .into_iter()
            .filter(|a| {
                matches!(a, Action::Activate { card, .. } if g.stats(*card).id == MONSTER_REBORN)
            })
            .collect();
        assert!(!reborn.is_empty(), "Petit Moth can still be revived");
        assert!(
            reborn
                .iter()
                .all(|a| matches!(a, Action::Activate { targets, .. } if !targets.contains(&m))),
            "{moth} is never a target"
        );
    }
}

#[test]
fn elegant_egotist_calls_harpie_lady_sisters_from_the_deck() {
    let mut g = game([
        &[
            HARPIE_LADY,
            ELEGANT_EGOTIST,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            HARPIE_LADY_SISTERS,
        ],
        &[],
    ]);
    assert!(
        activations(&g, ELEGANT_EGOTIST).is_empty(),
        "no Harpie Lady on the field"
    );
    put(&mut g, P0, HARPIE_LADY);
    activate(&mut g, ELEGANT_EGOTIST);
    let sisters = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == HARPIE_LADY_SISTERS))
        .unwrap();
    play(&mut g, sisters);
    answer(&mut g, 0);
    assert!(
        g.player(P0)
            .monsters()
            .any(|m| g.stats(m).id == HARPIE_LADY_SISTERS)
    );
}

#[test]
fn magical_labyrinth_turns_labyrinth_wall_into_wall_shadow() {
    let mut g = game([
        &[
            LABYRINTH_WALL,
            MAGICAL_LABYRINTH,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            MYSTICAL_ELF,
            WALL_SHADOW,
        ],
        &[],
    ]);
    let wall = put(&mut g, P0, LABYRINTH_WALL);
    let labyrinth = in_hand(&g, P0, MAGICAL_LABYRINTH);
    activate_where(&mut g, MAGICAL_LABYRINTH, |a| {
        matches!(a, Action::Activate { effect: 0, .. })
    });
    assert_eq!(g.equip_target(labyrinth), Some(wall));
    activate(&mut g, MAGICAL_LABYRINTH);
    answer(&mut g, 0);
    assert_eq!(g.instance(wall).location, Location::Graveyard);
    assert!(
        g.player(P0)
            .monsters()
            .any(|m| g.stats(m).id == WALL_SHADOW)
    );
}
