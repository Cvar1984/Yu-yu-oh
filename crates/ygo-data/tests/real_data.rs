//! The milestone M0 gate, pinned as tests: the committed data bundle loads
//! and the sample decks check out as the handoff describes.

use std::path::{Path, PathBuf};

use ygo_data::{Bundle, CardId, CardKind, Collection, Deck, DeckProblem, check_deck};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle() -> Bundle {
    Bundle::load(&root().join("config/rules.toml"), &root()).expect("committed data loads")
}

fn deck(name: &str) -> Deck {
    Deck::load(&root().join("data/decks").join(name)).unwrap()
}

#[test]
fn all_374_cards_load_with_the_handoff_counts() {
    let b = bundle();
    assert_eq!(b.cards.len(), 374);
    assert_eq!(
        b.cards.count_by_kind(),
        vec![
            (CardKind::Normal, 146),
            (CardKind::Effect, 95),
            (CardKind::Fusion, 25),
            (CardKind::Ritual, 4),
            (CardKind::Spell, 89),
            (CardKind::Trap, 15),
        ]
    );
    // Owner, 2026-11-17: one copy each of Pot of Greed, Change of Heart, Raigeki.
    let limited: Vec<(CardId, usize)> = b.banlist.limits.iter().map(|l| (l.id, l.limit)).collect();
    assert_eq!(
        limited,
        [
            (CardId(55144522), 1),
            (CardId(4031928), 1),
            (CardId(12580477), 1)
        ]
    );
}

#[test]
fn collection_has_226_cards_and_427_copies() {
    let b = bundle();
    let c = Collection::load(&root().join("data/collection.json")).unwrap();
    c.validate(&b.cards).unwrap();
    assert_eq!(c.unique_cards(), 226);
    assert_eq!(c.total_copies(), 427);
}

#[test]
fn owned_card_decks_are_legal_under_the_collection() {
    let b = bundle();
    let c = Collection::load(&root().join("data/collection.json")).unwrap();
    for name in [
        "owner_main_45.json",
        "defense_tuned_45.json",
        "umiiruka_water_40.json",
    ] {
        let d = deck(name);
        let problems = check_deck(&d, &b.cards, &b.banlist, &b.rules, Some(&c));
        assert!(problems.is_empty(), "{name}: {problems:#?}");
    }
    assert_eq!(deck("owner_main_45.json").main_size(), 45);
    assert_eq!(deck("umiiruka_water_40.json").main_size(), 40);
}

#[test]
fn full_pool_exodia_deck_breaks_only_the_pot_of_greed_limit_and_the_collection() {
    let b = bundle();
    let d = deck("exodia_stall_full_pool_40.json");
    // It runs 3 Pot of Greed, written before the 2026-11-17 limits.
    assert_eq!(
        check_deck(&d, &b.cards, &b.banlist, &b.rules, None),
        [DeckProblem::OverLimit {
            id: CardId(55144522),
            name: "Pot of Greed".into(),
            copies: 3,
            limit: 1,
        }]
    );

    let c = Collection::load(&root().join("data/collection.json")).unwrap();
    let problems = check_deck(&d, &b.cards, &b.banlist, &b.rules, Some(&c));
    assert!(!problems.is_empty());
    assert!(problems.iter().all(|p| matches!(
        p,
        DeckProblem::NotOwned { .. } | DeckProblem::OverLimit { .. }
    )));
    assert!(problems.contains(&DeckProblem::NotOwned {
        id: CardId(33396948),
        name: "Exodia the Forbidden One".into(),
        copies: 3,
        owned: 0,
    }));
}
