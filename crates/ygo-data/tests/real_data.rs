//! The milestone M0 gate, pinned as tests: the committed data bundle loads
//! and the sample decks check out as the handoff describes.

use std::path::{Path, PathBuf};

use ygo_data::{Bundle, CardId, CardKind, Collection, Deck, GameIndex, check_deck};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle() -> Bundle {
    Bundle::load(&root().join("config/rules.toml"), &root()).expect("committed data loads")
}

fn deck(name: &str) -> Deck {
    Deck::load(&root().join("data/decks").join(name)).unwrap()
}

fn collection(name: &str) -> Collection {
    let c = Collection::load(&root().join("data").join(name)).unwrap();
    c.validate(&bundle().cards).unwrap();
    c
}

#[test]
fn all_535_cards_load_with_the_handoff_counts() {
    let b = bundle();
    assert_eq!(b.cards.len(), 535);
    assert_eq!(
        b.cards.count_by_kind(),
        vec![
            (CardKind::Normal, 193),
            (CardKind::Effect, 119),
            (CardKind::Fusion, 26),
            (CardKind::Ritual, 5),
            (CardKind::Spell, 121),
            (CardKind::Trap, 71),
        ]
    );
    let by_index = |i: GameIndex| b.cards.iter().filter(|c| c.in_game_index == i).count();
    assert_eq!(
        [
            by_index(GameIndex::Lob),
            by_index(GameIndex::Mrd),
            by_index(GameIndex::Srl),
            by_index(GameIndex::Psv),
            by_index(GameIndex::Extra),
        ],
        [126, 144, 104, 105, 56]
    );
    // The 56 EXTRA cards are the ones data/extra_cards.txt names.
    let names = std::fs::read_to_string(root().join("data/extra_cards.txt")).unwrap();
    let mut listed: Vec<&str> = names
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let mut extra: Vec<&str> = b
        .cards
        .iter()
        .filter(|c| c.in_game_index == GameIndex::Extra)
        .map(|c| c.name.as_str())
        .collect();
    listed.sort();
    extra.sort();
    assert_eq!(listed, extra);
    // Handoff update, 2026-11-20: thirteen cards at 1, enemies included.
    assert_eq!(b.banlist.limits.len(), 13);
    assert!(b.banlist.limits.iter().all(|l| l.limit == 1));
    assert!(
        b.banlist.limits.iter().any(|l| l.id == CardId(72892473)),
        "Card Destruction, an EXTRA card"
    );
}

#[test]
fn both_saves_collections_load() {
    let a = collection("collection.json");
    assert_eq!((a.unique_cards(), a.total_copies()), (485, 1873));
    let b = collection("collection_save_b.json");
    assert_eq!((b.unique_cards(), b.total_copies()), (201, 421));
}

#[test]
fn every_deck_is_legal_and_owned_decks_fit_their_collection() {
    let b = bundle();
    let save_a = collection("collection.json");
    let save_b = collection("collection_save_b.json");
    let mut names: Vec<String> = std::fs::read_dir(root().join("data/decks"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names.len(), 12);
    for name in &names {
        let d = deck(name);
        let owned = match d.played_by.as_deref() {
            Some("save A") => Some(&save_a),
            Some("save B") => Some(&save_b),
            Some("enemy") => None,
            other => panic!("{name}: unknown player {other:?}"),
        };
        let problems = check_deck(&d, &b.cards, &b.banlist, &b.rules, owned);
        assert!(problems.is_empty(), "{name}: {problems:#?}");
    }
    for name in &names {
        assert_eq!(deck(name).main_size(), 40, "{name}");
    }
    assert_eq!(
        names.iter().filter(|n| n.starts_with("enemy_")).count(),
        6,
        "the six enemy decks"
    );
}
