//! Engine rules corrected to the modern official rules (docs/card-audit.md,
//! batches 1 and 2), each shown with real cards.

use std::sync::Arc;

use ygo_cards::continuous::{LEGENDARY_SWORD, MALEVOLENT_NUZZLER, MESSENGER_OF_PEACE, UMI};
use ygo_cards::flips::HANE_HANE;
use ygo_cards::fusion::POLYMERIZATION;
use ygo_cards::monsters_a::MASKED_SORCERER;
use ygo_cards::monsters_b::{GATE_GUARDIAN, KAZEJIN, SANGA_OF_THE_THUNDER, SUIJIN};
use ygo_cards::oneshot::{CURSE_OF_FIEND, DARKNESS_APPROACHES};
use ygo_cards::reveal::CEREMONIAL_BELL;
use ygo_cards::spells::{DARK_HOLE, MONSTER_REBORN, POT_OF_GREED, TRIBUTE_TO_THE_DOOMED};
use ygo_cards::traps::MAGIC_JAMMER;
use ygo_cards::traps::SOLEMN_JUDGMENT as SOLEMN;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, SUMMONED_SKULL, answer, in_hand, next_turn,
    pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{
    Action, DecisionKind, DeckList, Event, Game, GameSetup, InstanceId, Location, PlayerId,
    Position, Shared, VisibleCard,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const M_WARRIOR_1: CardId = CardId(56342351);
const M_WARRIOR_2: CardId = CardId(92731455);
const KARBONALA_WARRIOR: CardId = CardId(54541900);

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn game(top: [&[CardId]; 2]) -> Game {
    Game::new(shared(), &stacked(top, MYSTICAL_ELF)).unwrap()
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
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

#[test]
fn a_set_is_not_a_summon_so_solemn_judgment_cannot_answer_it() {
    let mut g = game([&[SOLEMN], &[CELTIC_GUARDIAN]]);
    let solemn = in_hand(&g, P0, SOLEMN);
    play(&mut g, Action::SetSpellTrap { card: solemn });
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
    assert_eq!(
        g.instance(celtic).location,
        Location::MonsterZone(0),
        "a Set completes at once"
    );
    assert!(
        !g.legal_actions()
            .iter()
            .any(|a| matches!(a, Action::Activate { card, .. } if *card == solemn)),
        "no summon to negate"
    );
    // A Normal Summon still opens the window.
    next_turn(&mut g);
    next_turn(&mut g);
    let elf = in_hand(&g, P1, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert_eq!(
        g.pending_decision().map(|d| (d.player, d.kind)),
        Some((P0, DecisionKind::Respond))
    );
    assert!(!activations(&g, SOLEMN).is_empty());
}

#[test]
fn two_monsters_with_0_atk_survive_their_battle() {
    let mut g = game([&[CEREMONIAL_BELL], &[CEREMONIAL_BELL]]);
    let mine = in_hand(&g, P0, CEREMONIAL_BELL);
    play(&mut g, summon(mine));
    pass_all(&mut g);
    next_turn(&mut g);
    let theirs = in_hand(&g, P1, CEREMONIAL_BELL);
    play(&mut g, summon(theirs));
    pass_all(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    play(
        &mut g,
        Action::Attack {
            attacker: theirs,
            target: Some(mine),
        },
    );
    pass_all(&mut g);
    assert!(g.on_field(mine) && g.on_field(theirs));
}

#[test]
fn curse_of_fiend_forbids_changing_positions_by_hand_that_turn() {
    let mut g = game([&[CELTIC_GUARDIAN, CURSE_OF_FIEND], &[]]);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    let curse = in_hand(&g, P0, CURSE_OF_FIEND);
    play(&mut g, Action::SetSpellTrap { card: curse });
    pass_all(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EndTurn);
    // Player 0's Standby Phase: Curse of Fiend can be activated.
    while g.pending_decision().map(|d| d.player) != Some(P0) {
        play(&mut g, Action::Pass);
    }
    let a = activations(&g, CURSE_OF_FIEND);
    assert_eq!(a.len(), 1, "{:?}", g.legal_actions());
    play(&mut g, a[0].clone());
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).position, Position::FaceUpDefense);
    assert!(
        !g.legal_actions()
            .contains(&Action::ChangePosition { card: celtic }),
        "Curse of Fiend's text forbids it this turn"
    );
}

#[test]
fn a_bounced_fusion_monster_goes_back_to_the_extra_deck() {
    let deck = |ids: &[CardId]| DeckList {
        main: ids
            .iter()
            .copied()
            .chain(std::iter::repeat(MYSTICAL_ELF))
            .take(40)
            .collect(),
        extra: Vec::new(),
    };
    let mut decks = [
        deck(&[M_WARRIOR_1, M_WARRIOR_2, POLYMERIZATION, HANE_HANE]),
        deck(&[]),
    ];
    decks[0].extra = vec![KARBONALA_WARRIOR];
    let mut setup = GameSetup::new(decks, 0);
    setup.first_player = Some(P0);
    setup.shuffle = false;
    let mut g = Game::new(shared(), &setup).unwrap();
    let hane = set_monster(&mut g, P0, HANE_HANE);
    let poly = activations(&g, POLYMERIZATION);
    play(&mut g, poly[0].clone());
    pass_all(&mut g);
    answer(&mut g, 0);
    let karbonala = g
        .player(P0)
        .monsters()
        .find(|&m| g.stats(m).id == KARBONALA_WARRIOR)
        .expect("Karbonala Warrior on the field");
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: hane });
    pass_all(&mut g);
    let flip = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![karbonala]))
        .expect("Hane-Hane may target Karbonala Warrior");
    play(&mut g, flip);
    pass_all(&mut g);
    assert_eq!(g.instance(karbonala).location, Location::Extra);
    assert!(g.player(P0).extra.contains(&karbonala));
    assert!(!g.player(P0).hand.contains(&karbonala));
}

#[test]
fn a_monster_turned_face_down_loses_its_equip_cards() {
    let mut g = game([
        &[
            CELTIC_GUARDIAN,
            LEGENDARY_SWORD,
            DARKNESS_APPROACHES,
            SEVEN_COLORED_FISH,
            SEVEN_COLORED_FISH,
        ],
        &[],
    ]);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    let sword = in_hand(&g, P0, LEGENDARY_SWORD);
    let equip = activations(&g, LEGENDARY_SWORD);
    play(&mut g, equip[0].clone());
    pass_all(&mut g);
    assert_eq!(g.equip_target(sword), Some(celtic));
    let darkness = activations(&g, DARKNESS_APPROACHES)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .expect("Darkness Approaches on Celtic Guardian");
    play(&mut g, darkness);
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).position, Position::FaceDownDefense);
    assert_eq!(g.instance(sword).location, Location::Graveyard);
}

/// Player 0's game with Karbonala Warrior's materials and Polymerization
/// on top and Karbonala Warrior in the Extra Deck.
fn fusion_game(rest: &[CardId]) -> Game {
    let mut main: Vec<CardId> = vec![M_WARRIOR_1, M_WARRIOR_2, POLYMERIZATION];
    main.extend_from_slice(rest);
    let deck = |ids: &[CardId]| DeckList {
        main: ids
            .iter()
            .copied()
            .chain(std::iter::repeat(MYSTICAL_ELF))
            .take(40)
            .collect(),
        extra: Vec::new(),
    };
    let mut decks = [deck(&main), deck(&[])];
    decks[0].extra = vec![KARBONALA_WARRIOR];
    let mut setup = GameSetup::new(decks, 0);
    setup.first_player = Some(P0);
    setup.shuffle = false;
    let rules = with_rules(&shared(), |r| r.hand.max_size = 30);
    Game::new(rules, &setup).unwrap()
}

#[test]
fn only_a_properly_summoned_monster_can_be_revived() {
    // Gate Guardian discarded from the hand was never properly summoned.
    let mut g = game([
        &[TRIBUTE_TO_THE_DOOMED, GATE_GUARDIAN, MONSTER_REBORN],
        &[CELTIC_GUARDIAN],
    ]);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    next_turn(&mut g);
    let gate = in_hand(&g, P0, GATE_GUARDIAN);
    let doomed = activations(&g, TRIBUTE_TO_THE_DOOMED)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { cost, .. } if cost == &vec![gate]))
        .expect("discard Gate Guardian for Tribute to the Doomed");
    play(&mut g, doomed);
    pass_all(&mut g);
    assert_eq!(g.instance(gate).location, Location::Graveyard);
    assert!(!g.special_summonable(gate));
    let reborn = activations(&g, MONSTER_REBORN);
    assert!(
        reborn
            .iter()
            .all(|a| !matches!(a, Action::Activate { targets, .. } if targets.contains(&gate))),
        "Monster Reborn cannot target it: {reborn:?}"
    );

    // Karbonala Warrior, Fusion Summoned and then destroyed, can come back.
    let mut g = fusion_game(&[DARK_HOLE, MONSTER_REBORN]);
    let poly = activations(&g, POLYMERIZATION);
    play(&mut g, poly[0].clone());
    pass_all(&mut g);
    answer(&mut g, 0);
    let karbonala = g.player(P0).monsters().next().unwrap();
    let hole = activations(&g, DARK_HOLE);
    play(&mut g, hole[0].clone());
    pass_all(&mut g);
    assert_eq!(g.instance(karbonala).location, Location::Graveyard);
    assert!(g.special_summonable(karbonala));
    let reborn = activations(&g, MONSTER_REBORN)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![karbonala]))
        .expect("Monster Reborn may revive a properly summoned Fusion Monster");
    play(&mut g, reborn);
    pass_all(&mut g);
    answer(&mut g, 0);
    assert!(g.on_field(karbonala));
}

#[test]
fn gate_guardian_is_summoned_by_procedure_and_solemn_judgment_can_negate_it() {
    // Three Normal Summons a turn and no tributes for Level 7, to put the
    // three parts on the field quickly.
    let rules = with_rules(&shared(), |r| {
        r.summon.normal_or_tribute_per_turn = 3;
        r.summon.tributes_level_5_6 = 0;
        r.summon.tributes_level_7_plus = 0;
    });
    let setup = stacked(
        [
            &[SANGA_OF_THE_THUNDER, KAZEJIN, SUIJIN, GATE_GUARDIAN],
            &[SOLEMN],
        ],
        MYSTICAL_ELF,
    );
    let mut g = Game::new(rules, &setup).unwrap();
    next_turn(&mut g);
    let solemn = in_hand(&g, P1, SOLEMN);
    play(&mut g, Action::SetSpellTrap { card: solemn });
    next_turn(&mut g);
    for id in [SANGA_OF_THE_THUNDER, KAZEJIN, SUIJIN] {
        let c = in_hand(&g, P0, id);
        play(&mut g, summon(c));
        pass_all(&mut g);
    }
    let gate = in_hand(&g, P0, GATE_GUARDIAN);
    let procedures: Vec<Action> = g
        .legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::ProcedureSummon { card, .. } if *card == gate))
        .collect();
    assert_eq!(procedures.len(), 2, "Attack or Defense Position");
    assert!(
        !g.legal_actions()
            .iter()
            .any(|a| matches!(a, Action::Activate { card, .. } if *card == gate)),
        "no longer an effect"
    );
    let events = play(&mut g, procedures[1].clone());
    assert!(
        !events.iter().any(|e| matches!(e, Event::Activated { .. })),
        "a summon procedure starts no chain"
    );
    assert_eq!(
        g.player(P0).monsters().count(),
        0,
        "the three parts were Tributed"
    );
    assert_eq!(
        g.pending_decision().map(|d| (d.player, d.kind)),
        Some((P1, DecisionKind::Respond))
    );
    let negate = activations(&g, SOLEMN);
    assert_eq!(negate.len(), 1, "Solemn Judgment may negate the summon");
    play(&mut g, negate[0].clone());
    pass_all(&mut g);
    assert_eq!(g.instance(gate).location, Location::Graveyard);

    // Unanswered, it arrives in the chosen position, properly summoned.
    let rules = with_rules(&shared(), |r| {
        r.summon.normal_or_tribute_per_turn = 3;
        r.summon.tributes_level_5_6 = 0;
        r.summon.tributes_level_7_plus = 0;
    });
    let mut g = Game::new(rules, &setup).unwrap();
    for id in [SANGA_OF_THE_THUNDER, KAZEJIN, SUIJIN] {
        let c = in_hand(&g, P0, id);
        play(&mut g, summon(c));
        pass_all(&mut g);
    }
    let gate = in_hand(&g, P0, GATE_GUARDIAN);
    let parts: Vec<InstanceId> = g.player(P0).monsters().collect();
    play(
        &mut g,
        Action::ProcedureSummon {
            card: gate,
            effect: 0,
            cost: parts,
            position: Position::FaceUpDefense,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.instance(gate).position, Position::FaceUpDefense);
    assert!(g.instance(gate).properly_summoned);
}

#[test]
fn magic_jammer_negates_card_activations_not_effects_of_face_up_spells() {
    let mut g = game([
        &[MESSENGER_OF_PEACE, POT_OF_GREED],
        &[MAGIC_JAMMER, CELTIC_GUARDIAN],
    ]);
    let messenger = activations(&g, MESSENGER_OF_PEACE);
    play(&mut g, messenger[0].clone());
    pass_all(&mut g);
    next_turn(&mut g);
    let jammer = in_hand(&g, P1, MAGIC_JAMMER);
    play(&mut g, Action::SetSpellTrap { card: jammer });
    pass_all(&mut g);
    play(&mut g, Action::EndTurn);
    // Player 0's Standby Phase: Messenger of Peace's effect goes on the
    // chain. Magic Jammer is never on offer against it (player 1, with
    // nothing to activate, is passed automatically).
    let lp = g.player(P0).lp;
    while g.pending_decision().map(|d| d.kind) != Some(DecisionKind::MainPhase) {
        let d = g.pending_decision().unwrap();
        if d.player == P1 {
            assert!(
                activations(&g, MAGIC_JAMMER).is_empty(),
                "not a card activation"
            );
        }
        let a = g.legal_actions();
        let pass_or_pay = a
            .iter()
            .find(|x| matches!(x, Action::Pass | Action::Answer { option: 0 }))
            .cloned()
            .unwrap_or_else(|| a[0].clone());
        play(&mut g, pass_or_pay);
    }
    assert_eq!(
        g.player(P0).lp,
        lp - 100,
        "Messenger of Peace's effect resolved"
    );
    assert!(g.on_field(jammer), "Magic Jammer was not used");
    // A Spell Card's activation can still be answered.
    let pot = activations(&g, POT_OF_GREED);
    play(&mut g, pot[0].clone());
    assert!(!activations(&g, MAGIC_JAMMER).is_empty());
}

#[test]
fn masked_sorcerer_draws_through_a_chain_link() {
    let mut g = game([&[MASKED_SORCERER], &[]]);
    let sorcerer = in_hand(&g, P0, MASKED_SORCERER);
    play(&mut g, summon(sorcerer));
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    let hand = g.player(P0).hand.len();
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    let mut events = play(
        &mut g,
        Action::Attack {
            attacker: sorcerer,
            target: None,
        },
    );
    while g.pending_decision().map(|d| d.kind) == Some(DecisionKind::Respond) {
        events.extend(play(&mut g, Action::Pass));
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Activated { card, .. } if *card == sorcerer)),
        "the draw is a trigger effect on the chain"
    );
    assert_eq!(g.player(P0).hand.len(), hand + 1);
}

#[test]
fn a_field_spell_can_be_set_and_activated_later() {
    let mut g = game([&[UMI, SEVEN_COLORED_FISH], &[]]);
    let umi = in_hand(&g, P0, UMI);
    assert!(
        g.legal_actions()
            .contains(&Action::SetSpellTrap { card: umi })
    );
    play(&mut g, Action::SetSpellTrap { card: umi });
    pass_all(&mut g);
    assert_eq!(g.instance(umi).location, Location::FieldZone);
    assert_eq!(g.instance(umi).position, Position::FaceDown);
    assert!(matches!(
        g.observe(P1).opponent().field_zone,
        Some(VisibleCard::Hidden { .. })
    ));
    let fish = in_hand(&g, P0, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    pass_all(&mut g);
    assert_eq!(g.atk(fish), 1800, "a Set Field Spell does nothing yet");
    let activate = activations(&g, UMI);
    assert_eq!(activate.len(), 1, "activated from the Field Zone");
    play(&mut g, activate[0].clone());
    pass_all(&mut g);
    assert_eq!(g.instance(umi).position, Position::FaceUp);
    assert_eq!(g.atk(fish), 2000);
}

#[test]
fn malevolent_nuzzler_misses_the_timing_when_its_monster_is_tributed_for_a_summon() {
    let mut g = game([&[CELTIC_GUARDIAN, MALEVOLENT_NUZZLER, SUMMONED_SKULL], &[]]);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    let nuzzler = in_hand(&g, P0, MALEVOLENT_NUZZLER);
    let equip = activations(&g, MALEVOLENT_NUZZLER);
    play(&mut g, equip[0].clone());
    pass_all(&mut g);
    assert_eq!(g.equip_target(nuzzler), Some(celtic));
    next_turn(&mut g);
    next_turn(&mut g);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    play(
        &mut g,
        Action::Summon {
            card: skull,
            tributes: vec![celtic],
            face_down: false,
        },
    );
    assert_eq!(g.instance(nuzzler).location, Location::Graveyard);
    assert!(
        activations(&g, MALEVOLENT_NUZZLER).is_empty(),
        "the summon was the last thing to happen"
    );
}
