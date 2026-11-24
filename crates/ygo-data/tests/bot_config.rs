//! The heuristic bots' config files: the committed ones load, and every
//! kind of mistake in them is refused with a clear error.

use std::path::{Path, PathBuf};

use ygo_data::{CardDb, CardId, CardValues, DataError, Styles};

const POT_OF_GREED: CardId = CardId(55144522);
const EXODIA_HEAD: CardId = CardId(33396948);
const CELTIC_GUARDIAN: CardId = CardId(91152256);
const MIRROR_FORCE: CardId = CardId(44095762);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cards() -> CardDb {
    CardDb::load(&root().join("data/cards.json")).unwrap()
}

/// Writes `text` to a fresh temporary file and returns its path.
fn temp(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ygo-bot-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

const VALUES: &str = r#"
schema_version = 1
[defaults]
spell = 30
trap = 30
monster_base = 10
monster_stat_divisor = 50
hidden = 40
[values]
55144522 = { name = "Pot of Greed", value = 95 }
"#;

const STYLE: &str = r#"
schema_version = 1
[styles.beatdown]
aggression = 1
lp_weight = 1.0
caution = 0.5
tribute_margin = 5
tribute_needs_cover = false
counter_min_value = 50
facedown_attack_min_atk = 1400
use_cost = 0.5
revive_worth = 0.75
"#;

#[test]
fn the_committed_config_loads() {
    let cards = cards();
    let values = CardValues::load(&root().join("config/card_values.toml"), &cards).unwrap();
    assert_eq!(values.values.len(), 51);
    assert_eq!(values.value(cards.get(POT_OF_GREED).unwrap()), 95);
    // Celtic Guardian 1400/1200: 10 + 1400 / 50.
    assert_eq!(values.value(cards.get(CELTIC_GUARDIAN).unwrap()), 38);
    let styles = Styles::load(&root().join("config/styles.toml"), &cards).unwrap();
    assert_eq!(
        styles.names().collect::<Vec<_>>(),
        ["beatdown", "burn", "control", "exodia", "patient", "stall"]
    );
    assert_eq!(styles.get("exodia").unwrap().values[&EXODIA_HEAD], 100);
    assert!(styles.get("patient").unwrap().tribute_needs_cover);
    assert!(styles.get("beatdown").unwrap().values.is_empty());
}

#[test]
fn defaults_follow_kind_and_stats() {
    let cards = cards();
    let values = CardValues::load(&temp("v.toml", VALUES), &cards).unwrap();
    assert_eq!(values.value(cards.get(MIRROR_FORCE).unwrap()), 30);
    let styles = Styles::load(&temp("s.toml", STYLE), &cards).unwrap();
    assert_eq!(
        styles.get("beatdown").unwrap().aggression,
        1.0,
        "integers read as weights"
    );
}

fn values_error(text: &str) -> String {
    CardValues::load(&temp("bad-values.toml", text), &cards())
        .unwrap_err()
        .to_string()
}

fn styles_error(text: &str) -> String {
    match Styles::load(&temp("bad-styles.toml", text), &cards()) {
        Err(DataError::Toml { source, .. }) => source.to_string(),
        Err(e) => e.to_string(),
        Ok(s) => panic!("accepted: {s:?}"),
    }
}

#[test]
fn mistakes_in_card_values_are_refused() {
    let e = values_error(&VALUES.replace("55144522 =", "12345 ="));
    assert!(e.contains("unknown card id 12345"), "{e}");
    let e = values_error(&VALUES.replace("value = 95", "value = 101"));
    assert!(e.contains("within 0 to 100"), "{e}");
    let e = values_error(&VALUES.replace("55144522 =", "pot ="));
    assert!(e.contains("not a card id"), "{e}");
    let e = values_error(&VALUES.replace("monster_stat_divisor = 50", "monster_stat_divisor = 0"));
    assert!(e.contains("monster_stat_divisor"), "{e}");
    let e = values_error(&VALUES.replace("schema_version = 1", "schema_version = 2"));
    assert!(e.contains("schema_version 2"), "{e}");
    let e = values_error(&VALUES.replace("hidden = 40", "hidden = 40\nhiden = 3"));
    assert!(e.contains("invalid TOML"), "{e}");
    let e = values_error(&VALUES.replace("value = 95 }", "value = 95, note = \"x\" }"));
    assert!(e.contains("invalid TOML"), "{e}");
}

#[test]
fn mistakes_in_styles_are_refused() {
    let e = styles_error(&STYLE.replace("caution = 0.5", "caution = 0.5\ncautoin = 1.0"));
    assert!(e.contains("cautoin"), "a misspelt key is refused: {e}");
    let e = styles_error(&STYLE.replace("caution = 0.5\n", ""));
    assert!(e.contains("caution"), "a missing key is refused: {e}");
    let e = styles_error(&STYLE.replace("caution = 0.5", "caution = -0.5"));
    assert!(e.contains("non-negative"), "{e}");
    let e = styles_error(&STYLE.replace("counter_min_value = 50", "counter_min_value = 150"));
    assert!(e.contains("within 0 to 100"), "{e}");
    let e = styles_error(&format!(
        "{STYLE}[styles.beatdown.values]\n1 = {{ name = \"x\", value = 5 }}\n"
    ));
    assert!(e.contains("unknown card id 1"), "{e}");
    let e = styles_error("schema_version = 1\n[styles]\n");
    assert!(e.contains("no style"), "{e}");
}

#[test]
fn revive_cards_name_whose_graveyard_they_reach() {
    use ygo_data::ReviveFrom;
    let cards = cards();
    let values = CardValues::load(&root().join("config/card_values.toml"), &cards).unwrap();
    assert_eq!(values.revive.len(), 3);
    assert_eq!(values.revive[&CardId(83764718)], ReviveFrom::Either);
    assert_eq!(values.revive[&CardId(97077563)], ReviveFrom::Own);
    let bad_from = format!(
        "{VALUES}[revive]\n83764718 = {{ name = \"Monster Reborn\", from = \"theirs\" }}\n"
    );
    assert!(CardValues::load(&temp("r1.toml", &bad_from), &cards).is_err());
    let unknown = format!("{VALUES}[revive]\n1 = {{ name = \"Nothing\", from = \"own\" }}\n");
    assert!(matches!(
        CardValues::load(&temp("r2.toml", &unknown), &cards),
        Err(DataError::UnknownCard { .. })
    ));
}
