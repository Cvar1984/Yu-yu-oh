//! Playing against a bot, independent of any screen: the session, the
//! sentences shown to a player, building bots by name and checking decks.
//! The terminal (`ygo-cli`) and the browser (`ygo-wasm`) both draw from it.

pub mod bots;
pub mod decks;
pub mod describe;
pub mod session;
#[cfg(feature = "test-support")]
pub mod testing;
