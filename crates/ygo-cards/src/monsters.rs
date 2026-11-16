//! Monster scripts.

use ygo_data::CardId;
use ygo_engine::{CardScript, Registry, Tag};

/// Toon Monsters: "Cannot be Normal Summoned/Set." Their other effects
/// (Toon World, direct attacks) are M3.
const TOON_MONSTERS: [(CardId, &str); 4] = [
    (CardId(65458948), "Toon Mermaid"),
    (CardId(91842653), "Toon Summoned Skull"),
    (CardId(53183600), "Blue-Eyes Toon Dragon"),
    (CardId(38369349), "Manga Ryu-Ran"),
];

pub fn register(r: &mut Registry) {
    for (id, _name) in TOON_MONSTERS {
        r.register(
            id,
            CardScript {
                tags: vec![Tag::CannotNormalSummon],
                effects: Vec::new(),
            },
        );
    }
}
