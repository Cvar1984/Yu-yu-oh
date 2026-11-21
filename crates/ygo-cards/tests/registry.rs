//! The registry matches the card pool and the summon tag works.

use std::collections::BTreeSet;

use ygo_data::CardKind;
use ygo_engine::testing::{MYSTICAL_ELF, SANGAN, bundle, shared_from, stacked, summons_offered};
use ygo_engine::{Game, Tag};

#[test]
fn every_scripted_id_is_in_the_pool() {
    let registry = ygo_cards::registry();
    let b = bundle();
    for id in registry.ids() {
        assert!(
            b.cards.contains(id),
            "scripted card {id} is not in cards.json"
        );
    }
}

#[test]
fn all_four_toon_monsters_carry_the_tag() {
    let registry = ygo_cards::registry();
    let b = bundle();
    let toons: BTreeSet<_> = b
        .cards
        .iter()
        .filter(|c| c.card_type == ygo_data::CardType::ToonMonster)
        .map(|c| c.id)
        .collect();
    assert_eq!(toons.len(), 4);
    for id in toons {
        let script = registry
            .get(id)
            .unwrap_or_else(|| panic!("{id} unscripted"));
        assert!(script.has(Tag::CannotNormalSummon));
    }
}

#[test]
fn effect_monsters_summon_as_vanilla_but_toons_cannot() {
    let toon_mermaid = ygo_data::CardId(65458948);
    let shared = shared_from(ygo_cards::registry());
    let g = Game::new(
        shared.clone(),
        &stacked([&[SANGAN, toon_mermaid], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let offered = summons_offered(&g);
    let ids: BTreeSet<_> = offered
        .iter()
        .filter_map(|a| match a {
            ygo_engine::Action::Summon { card, .. } => Some(g.stats(*card).id),
            _ => None,
        })
        .collect();
    assert!(
        ids.contains(&SANGAN),
        "an unscripted Effect Monster plays as vanilla"
    );
    assert!(ids.contains(&MYSTICAL_ELF));
    assert!(
        !ids.contains(&toon_mermaid),
        "Toon Mermaid cannot be Normal Summoned"
    );
    assert_eq!(
        shared.pool.get(shared.pool.index_of(SANGAN).unwrap()).kind,
        CardKind::Effect
    );
}

/// Non-Normal cards whose scripts are still to be written: EXTRA cards
/// from the 440-card pool (handoff update, 2026-11-20, see
/// `docs/pool-440-plan.md`) and Pharaoh's Servant (2026-11-21). The list only shrinks: every entry must be
/// unscripted, and every unscripted card must be listed.
const PENDING: &[(u32, &str)] = &[
    (2833249, "D. Tribe"),
    (62867251, "Light of Intervention"),
    (77622396, "Reverse Trap"),
    (70344351, "Riryoku Field"),
    (73915051, "Scapegoat"),
    (59560625, "Shift"),
    (68005187, "Soul Exchange"),
    (80604092, "Ultimate Offering"),
    // Pharaoh's Servant (2026-11-21), in set order; see `docs/psv-plan.md`.
    (77585513, "Jinzo"),
    (27911549, "Parasite Paracide"),
    (86198326, "7 Completed"),
    (49587034, "Lightforce Sword"),
    (1248895, "Chain Destruction"),
    (35316708, "Time Seal"),
    (61705417, "Graverobber"),
    (97077563, "Call of the Haunted"),
    (23471572, "Solomon's Lawbook"),
    (60866277, "Earthshaker"),
    (22359980, "Mirror Wall"),
    (73079365, "Gust"),
    (473469, "Driving Snow"),
    (36868108, "Armored Glass"),
    (12253117, "World Suppression"),
    (49251811, "Mystic Probe"),
    (75646520, "Metal Detector"),
    (2130625, "Numinous Healer"),
    (48539234, "Appropriate"),
    (74923978, "Forced Requisition"),
    (74701381, "DNA Surgery"),
    (296499, "The Regulation of Tribe"),
    (9074847, "Major Riot"),
    (36468556, "Ceasefire"),
    (8951260, "Respect Play"),
    (81210420, "Magical Hats"),
    (17449108, "Nobleman of Extermination"),
    (43434803, "The Shallow Grave"),
    (16227556, "Inspection"),
    (43711255, "Prohibition"),
    (79106360, "Morphing Jar #2"),
    (4266839, "Kiseitai"),
    (78193831, "Buster Blader"),
    (37580756, "Michizure"),
    (1918087, "Minor Goblin Official"),
    (37313786, "Gamble"),
    (63689843, "Attack and Receive"),
    (35346968, "Solemn Wishes"),
    (98139712, "Skull Invitation"),
    (6104968, "Bubonic Vermin"),
    (24294108, "Burning Land"),
    (60682203, "Cold Wave"),
    (23171610, "Limiter Removal"),
    (66719324, "Rain of Mercy"),
    (93108433, "Monster Recovery"),
    (96965364, "Insect Imitation"),
    (22959079, "Dimensionhole"),
    (90502999, "Ground Collapse"),
    (59344077, "Magic Drain"),
    (54109233, "Infinite Dismissal"),
    (85742772, "Gravity Bind"),
    (21237481, "Type Zero Magic Crusher"),
    (58621589, "Shadow of Eyes"),
    (3643300, "The Legendary Fisherman"),
    (51345461, "Sword Hunter"),
    (88733579, "Drill Bug"),
    (24128274, "Deepsea Warrior"),
    (50122883, "Bite Shoes"),
    (52675689, "Invitation to a Dark Sleep"),
    (63519819, "Thousand-Eyes Restrict"),
    (21015833, "Hayabusa Knight"),
    (57409948, "Bombardment Beetle"),
    (83994646, "4-Starred Ladybug of Doom"),
    (56387350, "Vampire Baby"),
    (79870141, "Mad Sword Beast"),
    (78658564, "Goblin Attack Force"),
    (66362965, "The Fiend Megacyber"),
    (423705, "Gearfried the Iron Knight"),
    (23615409, "Insect Barrier"),
    (61740673, "Imperial Order"),
];

#[test]
fn every_non_normal_card_in_the_pool_has_a_script() {
    let registry = ygo_cards::registry();
    let b = bundle();
    let missing: BTreeSet<u32> = b
        .cards
        .iter()
        .filter(|c| c.kind() != CardKind::Normal && registry.get(c.id).is_none())
        .map(|c| c.id.0)
        .collect();
    let pending: BTreeSet<u32> = PENDING.iter().map(|&(id, _)| id).collect();
    let unlisted: Vec<_> = missing.difference(&pending).collect();
    assert!(
        unlisted.is_empty(),
        "unscripted and not pending: {unlisted:?}"
    );
    let done: Vec<_> = PENDING
        .iter()
        .filter(|(id, _)| !missing.contains(id))
        .collect();
    assert!(done.is_empty(), "scripted now, drop from PENDING: {done:?}");
    let non_normal = b
        .cards
        .iter()
        .filter(|c| c.kind() != CardKind::Normal)
        .count();
    assert_eq!(non_normal, 342);
}
