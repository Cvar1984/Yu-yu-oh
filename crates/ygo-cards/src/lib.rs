//! One script per non-Normal card, registered by card id. Nothing here
//! is hard-coded in the engine: the engine only sees [`ygo_engine::CardScript`]s.
//!
//! Module per card type; each module has a `register` function.

mod monsters;
pub mod spells;

use ygo_engine::Registry;

/// Every script this crate knows.
pub fn registry() -> Registry {
    let mut r = Registry::default();
    monsters::register(&mut r);
    spells::register(&mut r);
    r
}
