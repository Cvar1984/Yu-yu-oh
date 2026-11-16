//! Game state, rules, chains, effect scripts, legal actions and replays.
//!
//! No I/O, no threads, no system clock: randomness comes only from a seed
//! passed in by the caller, so the crate also builds to WebAssembly.
//! Starts at milestone M1.
