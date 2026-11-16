//! Loading and cross-checking the fixture bundle, plus the deck checker.

use std::path::{Path, PathBuf};

use ygo_data::{
    Banlist, Bundle, CardDb, CardFile, CardId, CardKind, Collection, DataError, Deck, DeckProblem,
    HandLimitCheck, Rules, TextVersion, check_deck,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn bundle() -> Bundle {
    Bundle::load(&fixtures().join("rules.toml"), &fixtures()).expect("fixture bundle loads")
}

const POT_OF_GREED: CardId = CardId(55144522);
const BLUE_EYES: CardId = CardId(89631139);
const BLUE_EYES_ULTIMATE: CardId = CardId(23995346);
const EXODIA: CardId = CardId(33396948);

#[test]
fn bundle_loads_and_counts_kinds() {
    let b = bundle();
    assert_eq!(b.cards.len(), 20);
    let counts = b.cards.count_by_kind();
    assert_eq!(
        counts,
        vec![
            (CardKind::Normal, 6),
            (CardKind::Effect, 4),
            (CardKind::Fusion, 1),
            (CardKind::Ritual, 1),
            (CardKind::Spell, 6),
            (CardKind::Trap, 2),
        ]
    );
    assert_eq!(b.cards.get(BLUE_EYES).unwrap().atk, Some(3000));
    assert_eq!(b.banlist.limit_for(POT_OF_GREED, 3), 1);
    assert_eq!(b.banlist.limit_for(EXODIA, 3), 0);
    assert_eq!(b.banlist.limit_for(BLUE_EYES, 3), 3);
}

#[test]
fn rules_match_the_committed_defaults() {
    let r = bundle().rules;
    assert_eq!(r.deck.main_min, 40);
    assert_eq!(r.deck.main_max, 60);
    assert_eq!(r.deck.extra_max, 15);
    assert_eq!(r.deck.default_copy_limit, 3);
    assert_eq!(r.hand.opening_size, 5);
    assert_eq!(r.hand.max_size, 6);
    assert_eq!(r.hand.hand_limit_check, HandLimitCheck::AfterDraw);
    assert!(r.turn.first_player_skips_draw && !r.turn.first_player_can_attack);
    assert!(r.turn.trap_activation_turn_set);
    assert!(!r.turn.spell_target_monster_set_this_turn);
    assert_eq!(r.life.starting_lp, 8000);
    assert_eq!(r.summon.normal_or_tribute_per_turn, 1);
    assert!(!r.summon.summon_in_main_phase_2);
    assert_eq!(r.summon.max_monsters_entering_per_turn, 0);
    assert!(r.summon.fusion_summons && r.summon.ritual_summons);
    assert_eq!(r.cards.text_version, TextVersion::Modern);
    assert!(r.win.deck_out_loses && r.win.exodia);
}

#[test]
fn committed_rules_file_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/rules.toml");
    let rules = Rules::load(&path).expect("config/rules.toml is valid");
    assert_eq!(rules.cards.banlist, Path::new("data/banlist.json"));
}

#[test]
fn committed_banlist_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/banlist.json");
    let banlist = Banlist::load(&path).expect("data/banlist.json is valid");
    // Owner's decision: no limits yet, so every card falls back to the default.
    assert!(banlist.limits.is_empty());
    assert_eq!(banlist.limit_for(POT_OF_GREED, 3), 3);
}

#[test]
fn unknown_rules_key_is_rejected() {
    let bad = std::fs::read_to_string(fixtures().join("rules.toml")).unwrap()
        + "\n[deck]\nmain_minimum = 40\n";
    let err = toml::from_str::<Rules>(&bad).unwrap_err().to_string();
    assert!(
        err.contains("main_minimum") || err.contains("duplicate"),
        "{err}"
    );
}

#[test]
fn impossible_rules_are_rejected() {
    let mut rules = bundle().rules;
    rules.deck.main_min = 61;
    assert!(matches!(rules.validate(), Err(DataError::InvalidRules(_))));
}

fn fixture_cards() -> Vec<ygo_data::Card> {
    let file: CardFile =
        serde_json::from_str(&std::fs::read_to_string(fixtures().join("cards.json")).unwrap())
            .unwrap();
    file.cards
}

#[test]
fn duplicate_card_id_is_rejected() {
    let cards = fixture_cards();
    let mut dup = cards.clone();
    let mut copy = cards[0].clone();
    copy.name = "Different name".into();
    dup.push(copy);
    assert!(matches!(
        CardDb::from_cards(dup),
        Err(DataError::DuplicateCardId(id)) if id == cards[0].id
    ));
}

#[test]
fn monster_without_stats_is_rejected() {
    let mut cards = fixture_cards();
    cards[0].level = None;
    assert!(matches!(
        CardDb::from_cards(cards),
        Err(DataError::MonsterMissingStats { .. })
    ));
}

#[test]
fn banlist_with_unknown_card_is_rejected() {
    let b = bundle();
    let banlist: Banlist =
        serde_json::from_str(r#"{"schema_version":1,"limits":[{"id":1,"limit":1}]}"#).unwrap();
    assert!(matches!(
        banlist.validate(&b.cards, 3),
        Err(DataError::UnknownCard {
            file: "banlist",
            id: CardId(1)
        })
    ));
}

#[test]
fn banlist_default_limit_must_match_rules() {
    let b = bundle();
    let banlist: Banlist =
        serde_json::from_str(r#"{"schema_version":1,"default_limit":2,"limits":[]}"#).unwrap();
    assert!(matches!(
        banlist.validate(&b.cards, 3),
        Err(DataError::DefaultLimitMismatch {
            banlist: 2,
            rules: 3
        })
    ));
}

#[test]
fn unsupported_schema_version_is_rejected() {
    let banlist: Banlist = serde_json::from_str(r#"{"schema_version":2,"limits":[]}"#).unwrap();
    let dir = std::env::temp_dir().join("ygo-data-schema-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("banlist.json");
    std::fs::write(&path, serde_json::to_string(&banlist).unwrap()).unwrap();
    assert!(matches!(
        Banlist::load(&path),
        Err(DataError::UnsupportedSchema {
            file: "banlist",
            found: 2,
            expected: 1
        })
    ));
}

#[test]
fn banlist_limit_at_default_is_dead_data() {
    let b = bundle();
    let banlist: Banlist =
        serde_json::from_str(r#"{"schema_version":1,"limits":[{"id":89631139,"limit":3}]}"#)
            .unwrap();
    assert!(matches!(
        banlist.validate(&b.cards, 3),
        Err(DataError::LimitNotBelowDefault { .. })
    ));
}

#[test]
fn collection_loads_and_validates() {
    let b = bundle();
    let c = Collection::load(&fixtures().join("collection.json")).unwrap();
    c.validate(&b.cards).unwrap();
    assert_eq!(c.unique_cards(), 4);
    assert_eq!(c.total_copies(), 7);
    assert_eq!(c.owned(BLUE_EYES), 1);
    assert_eq!(c.owned(EXODIA), 0);
}

#[test]
fn legal_deck_has_no_problems() {
    let b = bundle();
    let deck = Deck::load(&fixtures().join("decks/legal.json")).unwrap();
    assert_eq!(deck.main_size(), 40);
    assert!(check_deck(&deck, &b.cards, &b.banlist, &b.rules, None).is_empty());
}

#[test]
fn illegal_deck_reports_every_problem() {
    let b = bundle();
    let deck = Deck::load(&fixtures().join("decks/illegal.json")).unwrap();
    let problems = check_deck(&deck, &b.cards, &b.banlist, &b.rules, None);

    assert!(problems.contains(&DeckProblem::UnknownCard(CardId(1))));
    assert!(problems.contains(&DeckProblem::MainSize {
        size: 32,
        min: 40,
        max: 60
    }));
    assert!(problems.contains(&DeckProblem::OverLimit {
        id: POT_OF_GREED,
        name: "Pot of Greed".into(),
        copies: 2,
        limit: 1,
    }));
    assert!(problems.contains(&DeckProblem::OverLimit {
        id: EXODIA,
        name: "Exodia the Forbidden One".into(),
        copies: 1,
        limit: 0,
    }));
    assert!(problems.contains(&DeckProblem::ExtraCardInMain {
        id: BLUE_EYES_ULTIMATE,
        name: "Blue-Eyes Ultimate Dragon".into(),
    }));
    assert!(problems.contains(&DeckProblem::MainCardInExtra {
        id: BLUE_EYES,
        name: "Blue-Eyes White Dragon".into(),
    }));
    // Copies are counted across Main and Extra Deck together.
    assert!(problems.contains(&DeckProblem::OverLimit {
        id: BLUE_EYES,
        name: "Blue-Eyes White Dragon".into(),
        copies: 4,
        limit: 3,
    }));
    assert_eq!(problems.len(), 7, "{problems:#?}");
}

#[test]
fn collection_limits_deck_copies() {
    let b = bundle();
    let c = Collection::load(&fixtures().join("collection.json")).unwrap();
    let deck = Deck::load(&fixtures().join("decks/legal.json")).unwrap();
    let problems = check_deck(&deck, &b.cards, &b.banlist, &b.rules, Some(&c));
    assert!(problems.contains(&DeckProblem::NotOwned {
        id: BLUE_EYES,
        name: "Blue-Eyes White Dragon".into(),
        copies: 3,
        owned: 1,
    }));
    assert!(
        problems
            .iter()
            .all(|p| matches!(p, DeckProblem::NotOwned { .. }))
    );
}

#[test]
fn copy_limit_change_in_rules_is_honoured() {
    let mut b = bundle();
    b.rules.deck.default_copy_limit = 2;
    let deck = Deck::load(&fixtures().join("decks/legal.json")).unwrap();
    let problems = check_deck(&deck, &b.cards, &b.banlist, &b.rules, None);
    assert!(problems.iter().any(|p| matches!(
        p,
        DeckProblem::OverLimit { id, copies: 3, limit: 2, .. } if *id == BLUE_EYES
    )));
}
