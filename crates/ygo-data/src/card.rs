use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::DataError;

/// A card's official password. The key for every data file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardId(pub u32);

impl fmt::Display for CardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The `type` field as YGOPRODeck spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardType {
    #[serde(rename = "Normal Monster")]
    NormalMonster,
    #[serde(rename = "Effect Monster")]
    EffectMonster,
    #[serde(rename = "Flip Effect Monster")]
    FlipEffectMonster,
    #[serde(rename = "Toon Monster")]
    ToonMonster,
    #[serde(rename = "Fusion Monster")]
    FusionMonster,
    #[serde(rename = "Ritual Monster")]
    RitualMonster,
    #[serde(rename = "Ritual Effect Monster")]
    RitualEffectMonster,
    #[serde(rename = "Spell Card")]
    SpellCard,
    #[serde(rename = "Trap Card")]
    TrapCard,
}

/// The six buckets the project counts cards in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CardKind {
    Normal,
    Effect,
    Fusion,
    Ritual,
    Spell,
    Trap,
}

impl CardKind {
    pub const ALL: [CardKind; 6] = [
        CardKind::Normal,
        CardKind::Effect,
        CardKind::Fusion,
        CardKind::Ritual,
        CardKind::Spell,
        CardKind::Trap,
    ];
}

impl fmt::Display for CardKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CardKind::Normal => "Normal Monster",
            CardKind::Effect => "Effect Monster",
            CardKind::Fusion => "Fusion Monster",
            CardKind::Ritual => "Ritual Monster",
            CardKind::Spell => "Spell",
            CardKind::Trap => "Trap",
        })
    }
}

impl CardType {
    pub const ALL: [CardType; 9] = [
        CardType::NormalMonster,
        CardType::EffectMonster,
        CardType::FlipEffectMonster,
        CardType::ToonMonster,
        CardType::FusionMonster,
        CardType::RitualMonster,
        CardType::RitualEffectMonster,
        CardType::SpellCard,
        CardType::TrapCard,
    ];

    /// The type as people read it, spelled as in `cards.json`.
    pub fn label(self) -> &'static str {
        match self {
            CardType::NormalMonster => "Normal Monster",
            CardType::EffectMonster => "Effect Monster",
            CardType::FlipEffectMonster => "Flip Effect Monster",
            CardType::ToonMonster => "Toon Monster",
            CardType::FusionMonster => "Fusion Monster",
            CardType::RitualMonster => "Ritual Monster",
            CardType::RitualEffectMonster => "Ritual Effect Monster",
            CardType::SpellCard => "Spell Card",
            CardType::TrapCard => "Trap Card",
        }
    }

    pub fn kind(self) -> CardKind {
        match self {
            CardType::NormalMonster => CardKind::Normal,
            CardType::EffectMonster | CardType::FlipEffectMonster | CardType::ToonMonster => {
                CardKind::Effect
            }
            CardType::FusionMonster => CardKind::Fusion,
            CardType::RitualMonster | CardType::RitualEffectMonster => CardKind::Ritual,
            CardType::SpellCard => CardKind::Spell,
            CardType::TrapCard => CardKind::Trap,
        }
    }

    pub fn is_monster(self) -> bool {
        !matches!(self, CardType::SpellCard | CardType::TrapCard)
    }

    /// Cards that live in the Extra Deck instead of the Main Deck.
    pub fn is_extra_deck(self) -> bool {
        matches!(self, CardType::FusionMonster)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Printing {
    pub set: String,
    pub number: u32,
    pub code: String,
    pub rarity: String,
    /// The set's full name; given for the EXTRA cards' printings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_name: Option<String>,
}

/// Which list a card appears under in the owner's game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GameIndex {
    #[serde(rename = "LOB")]
    Lob,
    #[serde(rename = "MRD")]
    Mrd,
    #[serde(rename = "SRL")]
    Srl,
    /// Pharaoh's Servant, the game's fourth set tab (2026-11-21).
    #[serde(rename = "PSV")]
    Psv,
    /// Listed in the game's "All" list outside the four set tabs.
    #[serde(rename = "EXTRA")]
    Extra,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Images {
    pub full: String,
    pub small: String,
    pub url_full: String,
    pub url_small: String,
}

/// What a card says. How the engine plays it lives in `ygo-cards`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub id: CardId,
    pub name: String,
    #[serde(rename = "type")]
    pub card_type: CardType,
    pub frame_type: String,
    pub text: String,
    pub level: Option<u8>,
    pub atk: Option<i32>,
    pub def: Option<i32>,
    /// Monster type for monsters (Dragon, Spellcaster...); Spell/Trap subtype
    /// otherwise (Normal, Continuous, Field, Equip, Quick-Play, Counter, Ritual).
    pub race: String,
    pub attribute: Option<String>,
    pub printings: Vec<Printing>,
    pub in_game_index: GameIndex,
    pub images: Images,
}

impl Card {
    pub fn kind(&self) -> CardKind {
        self.card_type.kind()
    }
}

/// The `cards.json` file: metadata about the fetch plus the cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardFile {
    pub schema_version: u32,
    pub source: String,
    pub sets: Vec<String>,
    pub count: usize,
    /// What the fields mean, for humans.
    pub note: String,
    pub cards: Vec<Card>,
}

/// Every card in the pool, indexed by id.
#[derive(Debug, Clone, Default)]
pub struct CardDb {
    cards: HashMap<CardId, Card>,
    /// Tokens cards create; never deck cards (`tokens.json`).
    pub(crate) tokens: Vec<crate::Token>,
}

impl CardDb {
    /// Loads `cards.json` (a [`CardFile`]) and checks its declared count.
    pub fn load(path: &Path) -> Result<Self, DataError> {
        Self::parse(&crate::read_text(path)?, path)
    }

    /// Reads `cards.json` text; `origin` names it in errors.
    pub fn parse(text: &str, origin: &Path) -> Result<Self, DataError> {
        let file: CardFile = crate::parse_json(text, origin)?;
        crate::check_schema_version("cards", file.schema_version, crate::CARDS_SCHEMA_VERSION)?;
        if file.count != file.cards.len() {
            return Err(DataError::CountMismatch {
                file: "cards",
                declared: file.count,
                actual: file.cards.len(),
            });
        }
        Self::from_cards(file.cards)
    }

    /// Builds the database, rejecting duplicate ids or names and monsters
    /// without stats.
    pub fn from_cards(cards: Vec<Card>) -> Result<Self, DataError> {
        let mut by_id = HashMap::with_capacity(cards.len());
        let mut names = HashSet::with_capacity(cards.len());
        for card in cards {
            if card.card_type.is_monster()
                && (card.level.is_none()
                    || card.atk.is_none()
                    || card.def.is_none()
                    || card.attribute.is_none())
            {
                return Err(DataError::MonsterMissingStats {
                    id: card.id,
                    name: card.name,
                });
            }
            if !names.insert(card.name.clone()) {
                return Err(DataError::DuplicateCardName(card.name));
            }
            let id = card.id;
            if by_id.insert(id, card).is_some() {
                return Err(DataError::DuplicateCardId(id));
            }
        }
        Ok(Self {
            cards: by_id,
            tokens: Vec::new(),
        })
    }

    pub fn get(&self, id: CardId) -> Option<&Card> {
        self.cards.get(&id)
    }

    pub fn contains(&self, id: CardId) -> bool {
        self.cards.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Card> {
        self.cards.values()
    }

    /// Number of cards in each [`CardKind`], in `CardKind::ALL` order.
    pub fn count_by_kind(&self) -> Vec<(CardKind, usize)> {
        CardKind::ALL
            .iter()
            .map(|kind| (*kind, self.iter().filter(|c| c.kind() == *kind).count()))
            .collect()
    }
}
