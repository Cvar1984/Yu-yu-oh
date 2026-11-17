//! One script per non-Normal card, registered by card id. Nothing here
//! is hard-coded in the engine: the engine only sees [`ygo_engine::CardScript`]s.
//!
//! Module per card type; each module has a `register` function.

pub mod continuous;
pub mod flips;
pub mod fusion;
pub mod monsters;
pub mod monsters_a;
pub mod monsters_b;
pub mod oneshot;
pub mod reveal;
pub mod ritual;
pub mod spells;
pub mod tables;
pub mod traps;

use ygo_engine::Registry;

/// Every script this crate knows.
pub fn registry() -> Registry {
    let mut r = Registry::default();
    monsters::register(&mut r);
    spells::register(&mut r);
    traps::register(&mut r);
    continuous::register(&mut r);
    tables::register(&mut r);
    oneshot::register(&mut r);
    reveal::register(&mut r);
    fusion::register(&mut r);
    ritual::register(&mut r);
    monsters_a::register(&mut r);
    monsters_b::register(&mut r);
    flips::register(&mut r);
    r
}
