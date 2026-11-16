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

/// The committed rules with one change applied, for config-flip tests.
pub fn shared_with(change: impl FnOnce(&mut ygo_data::Rules)) -> Arc<Shared> {
    let base = shared();
    let mut rules = base.rules.clone();
    change(&mut rules);
    rules.validate().expect("changed rules stay valid");
    Arc::new(Shared {
        rules,
        pool: base.pool.clone(),
    })
}

/// Plays `action` for whoever must decide, panicking if it is illegal.
pub fn play(game: &mut ygo_engine::Game, action: ygo_engine::Action) -> Vec<ygo_engine::Event> {
    game.apply(&action)
        .unwrap_or_else(|e| panic!("{action:?} on turn {} {:?}: {e}", game.turn(), game.phase()))
}

/// Ends turns until the game is over, choosing the first legal action for
/// any other decision. Returns the number of actions played.
pub fn end_turns_until_over(game: &mut ygo_engine::Game) -> usize {
    let mut played = 0;
    while game.result().is_none() {
        let actions = game.legal_actions();
        let action = actions
            .iter()
            .find(|a| **a == ygo_engine::Action::EndTurn)
            .unwrap_or(&actions[0])
            .clone();
        play(game, action);
        played += 1;
    }
    played
}
