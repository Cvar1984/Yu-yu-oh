//! Shared helpers: the committed data bundle and simple test decks.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use ygo_data::{Bundle, CardId, CardKind, Deck};
use ygo_engine::{DeckList, Shared};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed rules and card pool, built once per test binary.
pub fn shared() -> Arc<Shared> {
    static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let b = Bundle::load(&root().join("config/rules.toml"), &root()).unwrap();
            Arc::new(Shared::new(b.rules, &b.cards))
        })
        .clone()
}

pub fn sample_deck(name: &str) -> DeckList {
    DeckList::from(&Deck::load(&root().join("data/decks").join(name)).unwrap())
}

/// The lowest `n` Normal Monster ids, three copies each where needed:
/// a deck with nothing but vanilla monsters.
pub fn normal_deck(n: usize) -> DeckList {
    let pool = &shared().pool;
    let mut ids: Vec<CardId> = (0..pool.len() as u16)
        .map(ygo_engine::CardIndex)
        .map(|i| pool.get(i))
        .filter(|c| c.kind == CardKind::Normal)
        .map(|c| c.id)
        .collect();
    ids.sort();
    let main = ids.iter().flat_map(|&id| [id, id, id]).take(n).collect();
    DeckList {
        main,
        extra: Vec::new(),
    }
}
