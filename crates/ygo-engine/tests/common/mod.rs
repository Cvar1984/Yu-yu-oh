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

/// Known Normal Monsters for scenario tests. Levels are asserted in
/// `cards_used_by_tests_are_what_we_think`.
pub const MYSTICAL_ELF: CardId = CardId(15025844); // level 4, 800/2000
pub const SUMMONED_SKULL: CardId = CardId(70781052); // level 6, 2500/1200
pub const BLUE_EYES: CardId = CardId(89631139); // level 8, 3000/2500
pub const SANGAN: CardId = CardId(26202165); // Effect Monster

/// An unshuffled game: each player's deck starts with `top` in draw
/// order and is padded to 40 with `pad`. Player 0 goes first.
pub fn stacked(top: [&[CardId]; 2], pad: CardId) -> ygo_engine::GameSetup {
    let deck = |ids: &[CardId]| DeckList {
        main: ids
            .iter()
            .copied()
            .chain(std::iter::repeat(pad))
            .take(40)
            .collect(),
        extra: Vec::new(),
    };
    let mut setup = ygo_engine::GameSetup::new([deck(top[0]), deck(top[1])], 0);
    setup.first_player = Some(ygo_engine::PlayerId(0));
    setup.shuffle = false;
    setup
}

/// The hand card of `player` with this id, first copy.
pub fn in_hand(
    game: &ygo_engine::Game,
    player: ygo_engine::PlayerId,
    id: CardId,
) -> ygo_engine::InstanceId {
    game.player(player)
        .hand
        .iter()
        .copied()
        .find(|&c| game.stats(c).id == id)
        .unwrap_or_else(|| panic!("{id} not in player {player}'s hand"))
}

pub fn summon(card: ygo_engine::InstanceId) -> ygo_engine::Action {
    ygo_engine::Action::Summon {
        card,
        tributes: Vec::new(),
        face_down: false,
    }
}

pub fn summons_offered(game: &ygo_engine::Game) -> Vec<ygo_engine::Action> {
    game.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, ygo_engine::Action::Summon { .. }))
        .collect()
}

/// Ends the turn, then plays discards for any hand-limit decision until
/// the next player's Main Phase.
pub fn next_turn(game: &mut ygo_engine::Game) {
    play(game, ygo_engine::Action::EndTurn);
    while let Some(d) = game.pending_decision()
        && d.kind == ygo_engine::DecisionKind::HandLimit
    {
        let a = game.legal_actions()[0].clone();
        play(game, a);
    }
}
