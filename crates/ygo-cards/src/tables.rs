//! Cards whose whole effect is a number in a table: the type Equips, the
//! type and attribute Field Spells, and the attribute-booster monsters.
//! A script reads its own row by card id, since scripts hold no state.

use ygo_data::CardId;
use ygo_engine::{CardScript, Effect, Game, InstanceId, Registry, Source, StatMod};

use crate::continuous::{equip_resolve, equipped_bonus, face_up_monsters};

/// "Equip only to a <type> monster. It gains 300 ATK/DEF."
pub const TYPE_EQUIPS: [(CardId, &str); 14] = [
    (CardId(46009906), "Beast"),         // Beast Fangs
    (CardId(15052462), "Zombie"),        // Violet Crystal
    (CardId(91595718), "Spellcaster"),   // Book of Secret Arts
    (CardId(77027445), "Aqua"),          // Power of Kaishin
    (CardId(4614116), "Fiend"),          // Dark Energy
    (CardId(77007920), "Insect"),        // Laser Cannon Armor
    (CardId(39774685), "Plant"),         // Vile Germs
    (CardId(1557499), "Fairy"),          // Silver Bow and Arrow
    (CardId(1435851), "Dragon"),         // Dragon Treasure
    (CardId(37820550), "Thunder"),       // Electro-Whip
    (CardId(36607978), "Beast-Warrior"), // Mystical Moon
    (CardId(25769732), "Machine"),       // Machine Conversion Factory
    (CardId(51267887), "Dinosaur"),      // Raise Body Heat
    (CardId(98252586), "Winged Beast"),  // Follow Wind
];

/// Field Spells by monster type: the types that gain 200 ATK/DEF and
/// the types that lose 200.
pub const TYPE_FIELDS: [(CardId, &[&str], &[&str]); 5] = [
    (
        CardId(87430998),
        &["Insect", "Beast", "Plant", "Beast-Warrior"],
        &[],
    ), // Forest
    (CardId(23424603), &["Dinosaur", "Zombie", "Rock"], &[]), // Wasteland
    (
        CardId(50913601),
        &["Dragon", "Winged Beast", "Thunder"],
        &[],
    ), // Mountain
    (CardId(86318356), &["Warrior", "Beast-Warrior"], &[]),   // Sogen
    (CardId(59197169), &["Fiend", "Spellcaster"], &["Fairy"]), // Yami
];

/// Field Spells by attribute: +500 ATK and -400 DEF.
pub const ATTRIBUTE_FIELDS: [(CardId, &str); 6] = [
    (CardId(56594520), "EARTH"), // Gaia Power
    (CardId(82999629), "WATER"), // Umiiruka
    (CardId(19384334), "FIRE"),  // Molten Destruction
    (CardId(45778932), "WIND"),  // Rising Air Current
    (CardId(81777047), "LIGHT"), // Luminous Spark
    (CardId(18161786), "DARK"),  // Mystic Plasma Zone
];

/// Monsters that, face-up, give +500 ATK to one attribute and -400 ATK
/// to another.
pub const ATTRIBUTE_BOOSTERS: [(CardId, &str, &str); 6] = [
    (CardId(8201910), "WATER", "FIRE"),  // Star Boy
    (CardId(7489323), "EARTH", "WIND"),  // Milus Radiant
    (CardId(28470714), "WIND", "EARTH"), // Bladefly
    (CardId(80741828), "DARK", "LIGHT"), // Witch's Apprentice
    (CardId(67629977), "LIGHT", "DARK"), // Hoshiningen
    (CardId(68658728), "FIRE", "WATER"), // Little Chimera
];

pub const SWAMP_BATTLEGUARD: CardId = CardId(40453765);
pub const LAVA_BATTLEGUARD: CardId = CardId(20394040);

fn race_of_equip(g: &Game, s: Source) -> &'static str {
    let id = g.stats(s.card).id;
    TYPE_EQUIPS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, r)| *r)
        .expect("a type Equip's row")
}

fn type_equip_targets(g: &Game, s: Source) -> ygo_engine::TargetSets {
    let race = race_of_equip(g, s);
    face_up_monsters(g, |g, m| g.race(m) == race)
}

fn type_field_modify(g: &Game, s: Source, target: InstanceId) -> StatMod {
    let id = g.stats(s.card).id;
    let (_, gains, loses) = TYPE_FIELDS
        .iter()
        .find(|(i, _, _)| *i == id)
        .expect("a type Field Spell's row");
    let race = g.race(target);
    if gains.contains(&race) {
        StatMod { atk: 200, def: 200 }
    } else if loses.contains(&race) {
        StatMod {
            atk: -200,
            def: -200,
        }
    } else {
        StatMod::default()
    }
}

fn attribute_field_modify(g: &Game, s: Source, target: InstanceId) -> StatMod {
    let id = g.stats(s.card).id;
    let (_, attribute) = ATTRIBUTE_FIELDS
        .iter()
        .find(|(i, _)| *i == id)
        .expect("an attribute Field Spell's row");
    if g.stats(target).attribute.as_deref() == Some(*attribute) {
        StatMod {
            atk: 500,
            def: -400,
        }
    } else {
        StatMod::default()
    }
}

fn booster_modify(g: &Game, s: Source, target: InstanceId) -> StatMod {
    let id = g.stats(s.card).id;
    let (_, up, down) = ATTRIBUTE_BOOSTERS
        .iter()
        .find(|(i, _, _)| *i == id)
        .expect("a booster's row");
    match g.stats(target).attribute.as_deref() {
        Some(a) if a == *up => StatMod { atk: 500, def: 0 },
        Some(a) if a == *down => StatMod { atk: -400, def: 0 },
        _ => StatMod::default(),
    }
}

/// +500 ATK for each monster of the partner's name the controller controls.
fn battleguard_modify(g: &Game, s: Source, target: InstanceId) -> StatMod {
    if target != s.card {
        return StatMod::default();
    }
    let partner = if g.stats(s.card).id == SWAMP_BATTLEGUARD {
        LAVA_BATTLEGUARD
    } else {
        SWAMP_BATTLEGUARD
    };
    let n = g
        .monsters_controlled(s.controller)
        .into_iter()
        .filter(|&m| g.stats(m).id == partner && g.instance(m).position.is_face_up())
        .count() as i32;
    StatMod {
        atk: 500 * n,
        def: 0,
    }
}

fn continuous(modify: fn(&Game, Source, InstanceId) -> StatMod) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![
            Effect::activate(1, ygo_engine::nothing),
            Effect {
                modify: Some(modify),
                ..Effect::continuous()
            },
        ],
    }
}

pub fn register(r: &mut Registry) {
    for (id, _) in TYPE_EQUIPS {
        r.register(
            id,
            CardScript {
                tags: Vec::new(),
                effects: vec![
                    Effect {
                        targets: Some(type_equip_targets),
                        timing: |g, s, _| !type_equip_targets(g, s).is_empty(),
                        ..Effect::activate(1, equip_resolve)
                    },
                    Effect {
                        modify: Some(|g, s, t| equipped_bonus(g, s, t, 300, 300)),
                        ..Effect::continuous()
                    },
                ],
            },
        );
    }
    for (id, _, _) in TYPE_FIELDS {
        r.register(id, continuous(type_field_modify));
    }
    for (id, _) in ATTRIBUTE_FIELDS {
        r.register(id, continuous(attribute_field_modify));
    }
    for (id, _, _) in ATTRIBUTE_BOOSTERS {
        r.register(
            id,
            CardScript {
                tags: Vec::new(),
                effects: vec![Effect {
                    modify: Some(booster_modify),
                    ..Effect::continuous()
                }],
            },
        );
    }
    for id in [SWAMP_BATTLEGUARD, LAVA_BATTLEGUARD] {
        r.register(
            id,
            CardScript {
                tags: Vec::new(),
                effects: vec![Effect {
                    modify: Some(battleguard_modify),
                    ..Effect::continuous()
                }],
            },
        );
    }
}
