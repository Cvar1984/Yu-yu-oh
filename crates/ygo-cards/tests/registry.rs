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
