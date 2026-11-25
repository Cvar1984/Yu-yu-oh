//! The neural bots by name (`neural:<checkpoint>`,
//! `neural-search:<checkpoint>`): `ygo sim` loads a checkpoint and plays
//! whole games with it; a missing file is a clear error before the first
//! game.

use std::path::PathBuf;

use ygo_ai::neural::{Net, cpu};
use ygo_cli::bots::Bots;
use ygo_cli::decks;
use ygo_cli::sim::{self, SimOptions};
use ygo_engine::testing::{bundle, root};

fn bots() -> Bots {
    Bots::new(&bundle())
}

#[test]
fn sim_plays_a_saved_network_by_name() {
    let path: PathBuf =
        std::env::temp_dir().join(format!("ygo-cli-neural-{}.bpk", std::process::id()));
    Net::new(&cpu()).save(&path).unwrap();
    let b = bundle();
    let deck = decks::load_legal(&root().join("data/decks/enemy_e3_removal.json"), &b).unwrap();
    let name = format!("neural:{}", path.display());
    let opts = SimOptions {
        deck_a: &deck,
        deck_b: &deck,
        bot_a: &name,
        bot_b: "random",
        games: 4,
        seed: 1,
        max_turns: ygo_engine::GameSetup::DEFAULT_MAX_TURNS,
        record_dir: None,
        threads: 2,
    };
    let tally = sim::run(&bots(), &opts).unwrap();
    std::fs::remove_file(&path).ok();
    let games: u32 = tally.by_first.iter().flatten().sum();
    assert_eq!(games, 4);

    let missing = SimOptions {
        bot_a: "neural:/no/such/checkpoint.bpk",
        ..opts
    };
    let err = sim::run(&bots(), &missing).unwrap_err().to_string();
    assert!(err.contains("/no/such/checkpoint.bpk"), "{err}");
}

#[test]
fn sim_plays_the_network_guided_search_bot_by_name() {
    let path: PathBuf =
        std::env::temp_dir().join(format!("ygo-cli-nsearch-{}.bpk", std::process::id()));
    Net::new(&cpu()).save(&path).unwrap();
    let b = bundle();
    let deck = decks::load_legal(&root().join("data/decks/enemy_e3_removal.json"), &b).unwrap();
    let name = format!("neural-search:{}", path.display());
    let mut bots = bots();
    bots.mcts.iterations = 5;
    let opts = SimOptions {
        deck_a: &deck,
        deck_b: &deck,
        bot_a: &name,
        bot_b: "random",
        games: 2,
        seed: 1,
        max_turns: ygo_engine::GameSetup::DEFAULT_MAX_TURNS,
        record_dir: None,
        threads: 2,
    };
    let tally = sim::run(&bots, &opts).unwrap();
    std::fs::remove_file(&path).ok();
    let games: u32 = tally.by_first.iter().flatten().sum();
    assert_eq!(games, 2);
}

#[test]
fn the_shipped_network_plays_by_the_name_network() {
    let b = bundle();
    let deck = decks::load_legal(&root().join("data/decks/enemy_e3_removal.json"), &b).unwrap();
    let opts = SimOptions {
        deck_a: &deck,
        deck_b: &deck,
        bot_a: "network",
        bot_b: "random",
        games: 2,
        seed: 1,
        max_turns: ygo_engine::GameSetup::DEFAULT_MAX_TURNS,
        record_dir: None,
        threads: 2,
    };
    // Without a network file the name is a clear error.
    let err = sim::run(&bots(), &opts).unwrap_err().to_string();
    assert!(err.contains("no network file"), "{err}");
    let mut bots = bots();
    bots.network = Some(root().join("data/networks/best.bpk"));
    let tally = sim::run(&bots, &opts).unwrap();
    assert_eq!(tally.by_first.iter().flatten().sum::<u32>(), 2);
}
