//! Rulings tests for Polymerization and the Fusion Monsters.

use std::sync::Arc;

use ygo_cards::fusion::*;
use ygo_data::CardId;
use ygo_engine::testing::{
    MYSTICAL_ELF, SUMMONED_SKULL, answer, bundle, in_hand, next_turn, pass_all, play, shared_from,
    stacked, summon, with_rules,
};
use ygo_engine::{Action, Game, GameSetup, InstanceId, Location, PlayerId, Shared};

const P0: PlayerId = PlayerId(0);
const GAIA: CardId = CardId(6368038);
const CURSE_OF_DRAGON: CardId = CardId(28279543);
const GAIA_CHAMPION: CardId = CardId(66889139);
const THUNDER_DRAGON: CardId = CardId(31786629);
const TWIN_HEADED_THUNDER_DRAGON: CardId = CardId(54752875);
const RED_EYES: CardId = CardId(74677422);
const BLACK_SKULL_DRAGON: CardId = CardId(11901678);

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| r.hand.max_size = 12)
}

fn setup(top: &[CardId], extra: &[CardId]) -> GameSetup {
    let mut s = stacked([top, &[]], MYSTICAL_ELF);
    s.decks[0].extra = extra.to_vec();
    s
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

/// Activates Polymerization, then makes its choices on resolution: the
/// Fusion Monster, its materials in order, and Attack Position.
fn fuse(g: &mut Game, fusion: InstanceId, materials: &[InstanceId]) {
    let a = activations(g, POLYMERIZATION);
    play(g, a[0].clone());
    pass_all(g);
    play(g, Action::Choose { card: fusion });
    for &m in materials {
        play(g, Action::Choose { card: m });
    }
    answer(g, 0);
}

/// Activates Polymerization and takes the first option at every choice.
fn fuse_first(g: &mut Game) {
    let a = activations(g, POLYMERIZATION);
    play(g, a[0].clone());
    pass_all(g);
    while let Some(Action::Choose { card }) = g.legal_actions().first().cloned() {
        play(g, Action::Choose { card });
    }
    answer(g, 0);
}

#[test]
fn every_fusion_row_names_cards_and_the_materials_are_monsters() {
    let b = bundle();
    for (fusion, materials) in FUSIONS {
        let card = b.cards.get(fusion).unwrap_or_else(|| panic!("{fusion}"));
        assert_eq!(
            card.card_type,
            ygo_data::CardType::FusionMonster,
            "{}",
            card.name
        );
        assert_eq!(materials.len(), 2, "{}", card.name);
        for m in materials {
            if let Some(c) = b.cards.get(*m) {
                assert!(c.card_type.is_monster(), "{}: {}", card.name, c.name);
            }
        }
    }
    let in_pool = FUSIONS
        .iter()
        .filter(|(_, m)| m.iter().all(|id| b.cards.contains(*id)))
        .count();
    assert_eq!(
        in_pool, 25,
        "Cyber Saurus needs a card outside the pool; Battle Ox (EXTRA) completes Rabid Horseman; \
         Thousand-Eyes Restrict (PSV) is the 25th"
    );
}

#[test]
fn polymerization_fuses_a_hand_card_and_a_field_card_from_the_extra_deck() {
    let mut g = Game::new(
        shared(),
        &setup(
            &[GAIA, CURSE_OF_DRAGON, MYSTICAL_ELF, POLYMERIZATION],
            &[GAIA_CHAMPION],
        ),
    )
    .unwrap();
    let offered = activations(&g, POLYMERIZATION);
    assert_eq!(
        offered.len(),
        1,
        "one way: Gaia + Curse of Dragon, both from the hand"
    );
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    next_turn(&mut g);
    next_turn(&mut g);
    let curse = in_hand(&g, P0, CURSE_OF_DRAGON);
    play(
        &mut g,
        Action::Summon {
            card: curse,
            tributes: vec![elf],
            face_down: false,
        },
    );
    let gaia = in_hand(&g, P0, GAIA);
    assert_eq!(activations(&g, POLYMERIZATION).len(), 1);
    // On resolution: Gaia the Dragon Champion, then Gaia from the hand and
    // Curse of Dragon from the field.
    let champion_card = g.player(P0).extra[0];
    fuse(&mut g, champion_card, &[gaia, curse]);
    let champion = g
        .player(P0)
        .monster_zones
        .iter()
        .flatten()
        .copied()
        .find(|&c| g.stats(c).id == GAIA_CHAMPION)
        .expect("Gaia the Dragon Champion on the field");
    assert_eq!(g.instance(gaia).location, Location::Graveyard);
    assert_eq!(g.instance(curse).location, Location::Graveyard);
    assert_eq!(g.atk(champion), 2600);
    assert!(g.player(P0).extra.is_empty());
}

#[test]
fn two_copies_in_the_hand_fuse_and_a_face_down_material_counts() {
    // Twin-Headed Thunder Dragon: both Thunder Dragons from the hand (they
    // are Level 5, so they cannot be Set without a tribute).
    let mut g = Game::new(
        shared(),
        &setup(
            &[THUNDER_DRAGON, POLYMERIZATION, THUNDER_DRAGON],
            &[TWIN_HEADED_THUNDER_DRAGON],
        ),
    )
    .unwrap();
    assert_eq!(activations(&g, POLYMERIZATION).len(), 1);
    fuse_first(&mut g);
    let twin = g.player(P0).monsters().next().unwrap();
    assert_eq!(g.stats(twin).id, TWIN_HEADED_THUNDER_DRAGON);
    assert_eq!(g.player(P0).monsters().count(), 1);

    // Karbonala Warrior: M-Warrior #1 Set face-down, M-Warrior #2 in the hand.
    const M_WARRIOR_1: CardId = CardId(56342351);
    const M_WARRIOR_2: CardId = CardId(92731455);
    const KARBONALA_WARRIOR: CardId = CardId(54541900);
    let mut g = Game::new(
        shared(),
        &setup(
            &[M_WARRIOR_1, M_WARRIOR_2, POLYMERIZATION],
            &[KARBONALA_WARRIOR],
        ),
    )
    .unwrap();
    let m1 = in_hand(&g, P0, M_WARRIOR_1);
    play(
        &mut g,
        Action::Summon {
            card: m1,
            tributes: vec![],
            face_down: true,
        },
    );
    assert_eq!(activations(&g, POLYMERIZATION).len(), 1);
    fuse_first(&mut g);
    let k = g.player(P0).monsters().next().unwrap();
    assert_eq!(g.stats(k).id, KARBONALA_WARRIOR);
    assert_eq!(g.instance(m1).location, Location::Graveyard);
}

#[test]
fn polymerization_waits_for_a_free_zone_and_obeys_fusion_summons() {
    let base = shared();
    let no_fusion = with_rules(&base, |r| r.summon.fusion_summons = false);
    let g = Game::new(
        no_fusion,
        &setup(
            &[SUMMONED_SKULL, RED_EYES, POLYMERIZATION],
            &[BLACK_SKULL_DRAGON],
        ),
    )
    .unwrap();
    assert!(
        activations(&g, POLYMERIZATION).is_empty(),
        "fusion_summons = false"
    );

    let base = shared();
    let capped = with_rules(&base, |r| r.summon.max_monsters_entering_per_turn = 1);
    let mut g = Game::new(
        capped,
        &setup(
            &[SUMMONED_SKULL, RED_EYES, POLYMERIZATION, MYSTICAL_ELF],
            &[BLACK_SKULL_DRAGON],
        ),
    )
    .unwrap();
    assert_eq!(activations(&g, POLYMERIZATION).len(), 1);
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    assert!(
        activations(&g, POLYMERIZATION).is_empty(),
        "one monster already entered this turn"
    );
}

#[test]
fn every_fusion_monster_with_materials_in_the_pool_can_be_fusion_summoned() {
    let b = bundle();
    let mut summoned = 0;
    for (fusion, materials) in FUSIONS {
        if !materials.iter().all(|m| b.cards.contains(*m)) {
            continue;
        }
        let top: Vec<CardId> = materials.iter().copied().chain([POLYMERIZATION]).collect();
        let mut g = Game::new(shared(), &setup(&top, &[fusion])).unwrap();
        assert!(
            !activations(&g, POLYMERIZATION).is_empty(),
            "{fusion}: Polymerization not offered"
        );
        fuse_first(&mut g);
        assert!(
            g.player(P0).monsters().any(|m| g.stats(m).id == fusion),
            "{fusion} not on the field"
        );
        assert_eq!(
            g.graveyard(P0).len(),
            3,
            "{fusion}: both materials and Polymerization"
        );
        summoned += 1;
    }
    assert_eq!(summoned, 25);
}
