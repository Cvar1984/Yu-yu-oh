//! The position question for a Special Summon. When a card's text names
//! no position, the player chooses face-up Attack or face-up Defense
//! Position; a few texts offer face-down Defense instead.

use ygo_engine::Position;

/// "Special Summon it" with no position named.
pub(crate) const FACE_UP: [&str; 2] = ["Face-up Attack Position", "Face-up Defense Position"];

/// "in face-up Attack Position or face-down Defense Position".
pub(crate) const ATTACK_OR_SET: [&str; 2] =
    ["Face-up Attack Position", "Face-down Defense Position"];

pub(crate) fn face_up(option: usize) -> Position {
    if option == 0 {
        Position::FaceUpAttack
    } else {
        Position::FaceUpDefense
    }
}

pub(crate) fn attack_or_set(option: usize) -> Position {
    if option == 0 {
        Position::FaceUpAttack
    } else {
        Position::FaceDownDefense
    }
}
