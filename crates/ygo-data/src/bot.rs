//! Configuration for the heuristic bots: card values (`card_values.toml`)
//! and playstyles (`styles.toml`). Card knowledge lives here, not in code.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::{Card, CardDb, CardId, CardKind, DataError, check_schema};

/// A card's value with its name for humans; the name is never checked.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedValue {
    #[allow(dead_code)]
    name: String,
    value: i32,
}

/// Values for cards not listed by id.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueDefaults {
    pub spell: i32,
    pub trap: i32,
    /// A monster is worth `monster_base + max(ATK, DEF) / monster_stat_divisor`.
    pub monster_base: i32,
    pub monster_stat_divisor: i32,
    /// A card whose identity the bot cannot see.
    pub hidden: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardValuesFile {
    schema_version: u32,
    defaults: ValueDefaults,
    values: BTreeMap<String, NamedValue>,
}

/// `card_values.toml`: what each card is worth, 0 to 100.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardValues {
    pub defaults: ValueDefaults,
    pub values: BTreeMap<CardId, i32>,
}

pub const MAX_CARD_VALUE: i32 = 100;

impl CardValues {
    pub fn load(path: &Path, cards: &CardDb) -> Result<Self, DataError> {
        Self::parse(&crate::read_text(path)?, path, cards)
    }

    /// Reads `card_values.toml` text; `origin` names it in errors.
    pub fn parse(text: &str, origin: &Path, cards: &CardDb) -> Result<Self, DataError> {
        const FILE: &str = "card_values.toml";
        let file: CardValuesFile = crate::parse_toml(text, origin)?;
        check_schema(FILE, file.schema_version)?;
        let d = &file.defaults;
        for (what, v) in [
            ("defaults.spell", d.spell),
            ("defaults.trap", d.trap),
            ("defaults.monster_base", d.monster_base),
            ("defaults.hidden", d.hidden),
        ] {
            check_value(what, v)?;
        }
        if d.monster_stat_divisor <= 0 {
            return invalid(format!(
                "defaults.monster_stat_divisor must be positive, not {}",
                d.monster_stat_divisor
            ));
        }
        Ok(Self {
            values: card_table(FILE, "values", file.values, cards)?,
            defaults: file.defaults,
        })
    }

    /// The listed value, or the default for the card's kind and stats.
    pub fn value(&self, card: &Card) -> i32 {
        if let Some(&v) = self.values.get(&card.id) {
            return v;
        }
        let d = &self.defaults;
        match card.kind() {
            CardKind::Spell => d.spell,
            CardKind::Trap => d.trap,
            _ => {
                let stat = card.atk.unwrap_or(0).max(card.def.unwrap_or(0)).max(0);
                (d.monster_base + stat / d.monster_stat_divisor).min(MAX_CARD_VALUE)
            }
        }
    }
}

/// One playstyle's weights. See `config/styles.toml` for their meaning.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    pub aggression: f64,
    pub lp_weight: f64,
    pub caution: f64,
    pub tribute_margin: i32,
    pub tribute_needs_cover: bool,
    pub counter_min_value: i32,
    pub facedown_attack_min_atk: i32,
    /// Card values that replace those in `card_values.toml`.
    #[serde(skip)]
    pub values: BTreeMap<CardId, i32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleEntry {
    #[serde(flatten)]
    style: Style,
    #[serde(default)]
    values: BTreeMap<String, NamedValue>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StylesFile {
    schema_version: u32,
    styles: BTreeMap<String, StyleEntry>,
}

/// `styles.toml`: the named playstyles.
#[derive(Debug, Clone, PartialEq)]
pub struct Styles {
    pub styles: BTreeMap<String, Style>,
}

impl Styles {
    pub fn load(path: &Path, cards: &CardDb) -> Result<Self, DataError> {
        Self::parse(&crate::read_text(path)?, path, cards)
    }

    /// Reads `styles.toml` text; `origin` names it in errors.
    pub fn parse(text: &str, origin: &Path, cards: &CardDb) -> Result<Self, DataError> {
        const FILE: &str = "styles.toml";
        let file: StylesFile = crate::parse_toml(text, origin)?;
        check_schema(FILE, file.schema_version)?;
        if file.styles.is_empty() {
            return invalid("styles.toml defines no style".into());
        }
        let mut styles = BTreeMap::new();
        for (name, entry) in file.styles {
            let mut style = entry.style;
            for (what, w) in [
                ("aggression", style.aggression),
                ("lp_weight", style.lp_weight),
                ("caution", style.caution),
            ] {
                if !(w.is_finite() && w >= 0.0) {
                    return invalid(format!(
                        "style {name}: {what} must be a non-negative number, not {w}"
                    ));
                }
            }
            check_value(
                &format!("style {name}: counter_min_value"),
                style.counter_min_value,
            )?;
            if style.tribute_margin < -MAX_CARD_VALUE || style.tribute_margin > MAX_CARD_VALUE {
                return invalid(format!(
                    "style {name}: tribute_margin must be within ±{MAX_CARD_VALUE}"
                ));
            }
            style.values = card_table(FILE, &format!("styles.{name}.values"), entry.values, cards)?;
            styles.insert(name, style);
        }
        Ok(Self { styles })
    }

    pub fn get(&self, name: &str) -> Option<&Style> {
        self.styles.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.styles.keys().map(String::as_str)
    }
}

fn invalid<T>(msg: String) -> Result<T, DataError> {
    Err(DataError::InvalidBotConfig(msg))
}

fn check_value(what: &str, v: i32) -> Result<(), DataError> {
    if (0..=MAX_CARD_VALUE).contains(&v) {
        Ok(())
    } else {
        invalid(format!(
            "{what} must be within 0 to {MAX_CARD_VALUE}, not {v}"
        ))
    }
}

/// Turns string keys into card ids that exist in the pool, values in range.
fn card_table(
    file: &'static str,
    table: &str,
    raw: BTreeMap<String, NamedValue>,
    cards: &CardDb,
) -> Result<BTreeMap<CardId, i32>, DataError> {
    let mut out = BTreeMap::new();
    for (key, entry) in raw {
        let Ok(id) = key.parse::<u32>().map(CardId) else {
            return invalid(format!("{table}: key {key:?} is not a card id"));
        };
        if cards.get(id).is_none() {
            return Err(DataError::UnknownCard { file, id });
        }
        check_value(&format!("{table}.{key}"), entry.value)?;
        out.insert(id, entry.value);
    }
    Ok(out)
}
