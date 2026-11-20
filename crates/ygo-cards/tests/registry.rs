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

/// Non-Normal EXTRA cards from the 440-card pool (handoff update,
/// 2026-11-20) whose scripts are still to be written; see
/// `docs/pool-440-plan.md`. The list only shrinks: every entry must be
/// unscripted, and every unscripted card must be listed.
const PENDING: &[(u32, &str)] = &[
    (17092736, "Ancient Telescope"),
    (36280194, "Backup Soldier"),
    (55761792, "Black Luster Ritual"),
    (5405694, "Black Luster Soldier"),
    (80163754, "Burst Breath"),
    (72892473, "Card Destruction"),
    (44209392, "Castle Walls"),
    (2833249, "D. Tribe"),
    (19159413, "De-Spell"),
    (84257639, "Dian Keto the Cure Master"),
    (24623598, "Disappear"),
    (60082869, "Dust Tornado"),
    (96355986, "Enchanted Javelin"),
    (97687912, "Fairy Meteor Crush"),
    (98299011, "Gift of The Mystical Elf"),
    (79571449, "Graceful Charity"),
    (55608151, "Gryphon Wing"),
    (98374133, "Invigoration"),
    (83968380, "Jar of Greed"),
    (24068492, "Just Desserts"),
    (85602018, "Last Will"),
    (62867251, "Light of Intervention"),
    (54098121, "Mysterious Puppeteer"),
    (71044499, "Nobleman of Crossout"),
    (19523799, "Ookazi"),
    (93920745, "Penguin Soldier"),
    (70828912, "Premature Burial"),
    (4178474, "Raigeki Break"),
    (17814387, "Reinforcements"),
    (77622396, "Reverse Trap"),
    (70344351, "Riryoku Field"),
    (73915051, "Scapegoat"),
    (29267084, "Shadow Spell"),
    (59560625, "Shift"),
    (68005187, "Soul Exchange"),
    (37120512, "Sword of Dark Destruction"),
    (34694160, "The Eye of Truth"),
    (43973174, "The Flute of Summoning Dragon"),
    (81820689, "The Inexperienced Spy"),
    (6285791, "The Wicked Worm Beast"),
    (46461247, "Trap Master"),
    (80604092, "Ultimate Offering"),
    (12607053, "Waboku"),
    (9786492, "White Dragon Ritual"),
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
    assert_eq!(non_normal, 272);
}
