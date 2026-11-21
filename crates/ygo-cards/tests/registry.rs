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
/// `docs/pool-440-plan.md`). Every Pharaoh's Servant card is scripted
/// (`docs/psv-plan.md`). The list only shrinks: every entry must be
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
