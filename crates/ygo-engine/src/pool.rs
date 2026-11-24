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
    /// A Token (`tokens.json`): never in a deck, gone once it leaves the field.
    pub token: bool,
}

impl CardStats {
    pub fn is_monster(&self) -> bool {
        !matches!(self.kind, CardKind::Spell | CardKind::Trap)
    }

    /// Lives in the Extra Deck rather than the Main Deck.
    pub fn is_extra_deck(&self) -> bool {
        self.kind == CardKind::Fusion
    }

    /// May be put in a Main Deck: not an Extra Deck card, not a Token.
    pub fn is_main_deck(&self) -> bool {
        !self.is_extra_deck() && !self.token
    }

    /// May be Normal Summoned or Set from the hand, given its script.
    pub fn can_normal_summon(&self, script: Option<&CardScript>) -> bool {
        matches!(self.kind, CardKind::Normal | CardKind::Effect)
            && !self.token
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
                token: false,
            })
            // A Token without effects is treated as a Normal Monster.
            .chain(db.tokens().map(|t| CardStats {
                id: t.id,
                name: t.name.clone(),
                kind: CardKind::Normal,
                subtype: SpellTrapType::None,
                level: t.level,
                atk: t.atk,
                def: t.def,
                race: t.race.clone(),
                attribute: Some(t.attribute.clone()),
                flip: false,
                token: true,
            }))
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
    /// One slot per pool index: which rarely used hooks the card's script
    /// has (`HOOK_*` bits), so a game without them never scans for them.
    pub(crate) hooks: Vec<u8>,
}

/// A script with a `negate` hook.
pub(crate) const HOOK_NEGATE: u8 = 1;
/// A script with a `forbid` hook.
pub(crate) const HOOK_FORBID: u8 = 2;
/// A script with a `race` hook.
pub(crate) const HOOK_RACE: u8 = 4;
/// A script with a `spell_immune` or `unattackable` hook.
pub(crate) const HOOK_SELF: u8 = 8;

fn hooks_of(script: &CardScript) -> u8 {
    script.effects.iter().fold(0, |bits, e| {
        bits | if e.negate.is_some() { HOOK_NEGATE } else { 0 }
            | if e.forbid.is_some() { HOOK_FORBID } else { 0 }
            | if e.race.is_some() { HOOK_RACE } else { 0 }
            | if e.spell_immune.is_some() || e.unattackable.is_some() {
                HOOK_SELF
            } else {
                0
            }
    })
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
        let scripts: Vec<Option<CardScript>> = pool
            .cards
            .iter()
            .map(|c| registry.get(c.id).cloned())
            .collect();
        let hooks = scripts
            .iter()
            .map(|s| s.as_ref().map_or(0, hooks_of))
            .collect();
        Self {
            rules,
            pool,
            scripts,
            hooks,
        }
    }

    /// The same pool and scripts under other rules.
    pub fn with_rules(&self, rules: Rules) -> Self {
        Self {
            rules,
            pool: self.pool.clone(),
            scripts: self.scripts.clone(),
            hooks: self.hooks.clone(),
        }
    }

    pub fn script(&self, index: CardIndex) -> Option<&CardScript> {
        self.scripts[index.0 as usize].as_ref()
    }
}
