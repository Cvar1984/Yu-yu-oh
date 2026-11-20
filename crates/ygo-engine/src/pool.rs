use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ygo_data::{CardDb, CardId, CardKind, Rules};

use crate::{CardScript, Registry, Tag};

/// Dense index of a card in the [`CardPool`]: what game state refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardIndex(pub u16);

/// Spell and Trap subtypes, from the `race` field. `None` for monsters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellTrapType {
    None,
    Normal,
    QuickPlay,
    Continuous,
    Equip,
    Field,
    Ritual,
    Counter,
}

impl SpellTrapType {
    fn from_race(kind: CardKind, race: &str) -> Self {
        if !matches!(kind, CardKind::Spell | CardKind::Trap) {
            return SpellTrapType::None;
        }
        match race {
            "Normal" => SpellTrapType::Normal,
            "Quick-Play" => SpellTrapType::QuickPlay,
            "Continuous" => SpellTrapType::Continuous,
            "Equip" => SpellTrapType::Equip,
            "Field" => SpellTrapType::Field,
            "Ritual" => SpellTrapType::Ritual,
            "Counter" => SpellTrapType::Counter,
            other => panic!("unknown Spell/Trap subtype {other:?}"),
        }
    }

    /// Goes to the Graveyard once its activation has resolved.
    pub fn leaves_after_resolving(self) -> bool {
        matches!(
            self,
            SpellTrapType::Normal
                | SpellTrapType::QuickPlay
                | SpellTrapType::Ritual
                | SpellTrapType::Counter
        )
    }
}

/// The numbers the engine needs about a card. Spells and Traps have zero
/// level, ATK and DEF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardStats {
    pub id: CardId,
    pub name: String,
    pub kind: CardKind,
    pub subtype: SpellTrapType,
    pub level: u8,
    pub atk: i32,
    pub def: i32,
    /// Monster type (Dragon, Fiend...) or empty for Spells and Traps.
    pub race: String,
    pub attribute: Option<String>,
    /// A Flip monster (Nobleman of Crossout checks this).
    pub flip: bool,
}

impl CardStats {
    pub fn is_monster(&self) -> bool {
        !matches!(self.kind, CardKind::Spell | CardKind::Trap)
    }

    /// Lives in the Extra Deck rather than the Main Deck.
    pub fn is_extra_deck(&self) -> bool {
        self.kind == CardKind::Fusion
    }

    /// May be Normal Summoned or Set from the hand, given its script.
    pub fn can_normal_summon(&self, script: Option<&CardScript>) -> bool {
        matches!(self.kind, CardKind::Normal | CardKind::Effect)
            && !script.is_some_and(|s| s.has(Tag::CannotNormalSummon))
    }
}

/// Every card the engine may see, in a flat table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardPool {
    cards: Vec<CardStats>,
    by_id: HashMap<CardId, CardIndex>,
}

impl CardPool {
    pub fn from_db(db: &CardDb) -> Self {
        let mut cards: Vec<CardStats> = db
            .iter()
            .map(|c| CardStats {
                id: c.id,
                name: c.name.clone(),
                kind: c.kind(),
                subtype: SpellTrapType::from_race(c.kind(), &c.race),
                level: c.level.unwrap_or(0),
                atk: c.atk.unwrap_or(0),
                def: c.def.unwrap_or(0),
                race: if c.card_type.is_monster() {
                    c.race.clone()
                } else {
                    String::new()
                },
                attribute: c.attribute.clone(),
                flip: c.card_type == ygo_data::CardType::FlipEffectMonster,
            })
            .collect();
        cards.sort_by_key(|c| c.id);
        let by_id = cards
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, CardIndex(i as u16)))
            .collect();
        Self { cards, by_id }
    }

    pub fn index_of(&self, id: CardId) -> Option<CardIndex> {
        self.by_id.get(&id).copied()
    }

    pub fn get(&self, index: CardIndex) -> &CardStats {
        &self.cards[index.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

/// Immutable data every game shares: built once, held by `Arc`, so a
/// game copy never clones it.
#[derive(Debug, Clone)]
pub struct Shared {
    pub rules: Rules,
    pub pool: CardPool,
    /// One slot per pool index; `None` for cards without a script.
    pub scripts: Vec<Option<CardScript>>,
}

impl PartialEq for Shared {
    /// Scripts are function pointers chosen by the registry, which is
    /// the same for every game of a process; rules and pool decide.
    fn eq(&self, other: &Self) -> bool {
        self.rules == other.rules && self.pool == other.pool
    }
}

impl Shared {
    pub fn new(rules: Rules, db: &CardDb, registry: Registry) -> Self {
        let pool = CardPool::from_db(db);
        let scripts = pool
            .cards
            .iter()
            .map(|c| registry.get(c.id).cloned())
            .collect();
        Self {
            rules,
            pool,
            scripts,
        }
    }

    pub fn script(&self, index: CardIndex) -> Option<&CardScript> {
        self.scripts[index.0 as usize].as_ref()
    }
}
