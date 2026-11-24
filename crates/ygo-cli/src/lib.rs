//! The `ygo` command line: the terminal screens, simulation and replay. The
//! play session, card text, bots and deck checks come from `ygo-play`.
//! `main.rs` only parses arguments and calls into these modules.

pub use ygo_play::{bots, decks, describe, session};

pub mod audit;
pub mod distill;
pub mod play;
pub mod replay;
pub mod sim;
pub mod tui;
