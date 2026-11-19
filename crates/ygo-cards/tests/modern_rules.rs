//! Engine rules corrected to the modern official rules (docs/card-audit.md,
//! batch 1), each shown with real cards.

use std::sync::Arc;

use ygo_cards::continuous::LEGENDARY_SWORD;
use ygo_cards::flips::HANE_HANE;
use ygo_cards::fusion::POLYMERIZATION;
use ygo_cards::oneshot::{CURSE_OF_FIEND, DARKNESS_APPROACHES};
use ygo_cards::reveal::CEREMONIAL_BELL;
use ygo_cards::traps::SOLEMN_JUDGMENT;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, answer, in_hand, next_turn, pass_all, play,
    shared_from, stacked, summon,
};
use ygo_engine::{
    Action, DecisionKind, DeckList, Game, GameSetup, InstanceId, Location, PlayerId, Position,
    Shared,
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
    let mut g = game([&[SOLEMN_JUDGMENT], &[CELTIC_GUARDIAN]]);
    let solemn = in_hand(&g, P0, SOLEMN_JUDGMENT);
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
    assert!(!activations(&g, SOLEMN_JUDGMENT).is_empty());
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
