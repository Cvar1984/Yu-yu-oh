//! Runs the built `ygo` binary against the ygo-data fixtures.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ygo-data/tests/fixtures")
}

fn ygo(args: &[&str]) -> Output {
    let f = fixtures();
    Command::new(env!("CARGO_BIN_EXE_ygo"))
        .arg("--rules")
        .arg(f.join("rules.toml"))
        .arg("--data-root")
        .arg(&f)
        .args(args)
        .output()
        .expect("ygo runs")
}

#[test]
fn check_data_summarises_the_pool() {
    let out = ygo(&["check-data"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("cards:   20 loaded"), "{stdout}");
    assert!(stdout.contains("5 limited cards"), "{stdout}");
}

#[test]
fn check_deck_accepts_a_legal_deck() {
    let deck = fixtures().join("decks/legal.json");
    let out = ygo(&["check-deck", deck.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(stdout.contains("legal"), "{stdout}");
}

#[test]
fn check_deck_flags_an_illegal_deck_with_exit_code_1() {
    let deck = fixtures().join("decks/illegal.json");
    let out = ygo(&["check-deck", deck.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("7 problem(s)"), "{stdout}");
    assert!(stdout.contains("Pot of Greed"), "{stdout}");
}

#[test]
fn check_deck_honours_the_collection() {
    let deck = fixtures().join("decks/legal.json");
    let coll = fixtures().join("collection.json");
    let out = ygo(&[
        "check-deck",
        deck.to_str().unwrap(),
        "--collection",
        coll.to_str().unwrap(),
    ]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("but the collection has"), "{stdout}");
}

#[test]
fn missing_file_exits_with_code_2() {
    let out = ygo(&["check-deck", "does-not-exist.json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot read"));
}

/// Runs `ygo` against the committed data and config.
fn ygo_real(args: &[&str]) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Command::new(env!("CARGO_BIN_EXE_ygo"))
        .arg("--rules")
        .arg(root.join("config/rules.toml"))
        .arg("--data-root")
        .arg(&root)
        .arg("--styles")
        .arg(root.join("config/styles.toml"))
        .arg("--card-values")
        .arg(root.join("config/card_values.toml"))
        .args(args)
        .output()
        .expect("ygo runs")
}

#[test]
fn sim_plays_named_bots_and_refuses_unknown_ones() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let deck = root.join("data/decks/owner_main_45.json");
    let deck = deck.to_str().unwrap();
    let out = ygo_real(&[
        "sim",
        deck,
        deck,
        "-n",
        "6",
        "--bot-a",
        "heuristic:beatdown",
        "--bot-b",
        "random",
    ]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("games: 6  heuristic:beatdown vs random"),
        "{stdout}"
    );

    let out = ygo_real(&["sim", deck, deck, "-n", "1", "--bot-a", "heuristic:nope"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("unknown bot `heuristic:nope`"), "{stderr}");
}

#[test]
fn play_and_sim_refuse_a_deck_over_the_ban_list() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let legal = root.join("data/decks/owner_main_45.json");
    // The owner's deck with its one Raigeki doubled: 46 cards, 2 Raigeki.
    let text = std::fs::read_to_string(&legal).unwrap();
    let illegal_text = text.replace(
        "\"name\": \"Raigeki\",\n   \"count\": 1",
        "\"name\": \"Raigeki\",\n   \"count\": 2",
    );
    assert_ne!(text, illegal_text, "the deck lists Raigeki once");
    let dir = std::env::temp_dir().join(format!("ygo-cli-illegal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let illegal = dir.join("two_raigeki.json");
    std::fs::write(&illegal, illegal_text).unwrap();
    let (legal, illegal) = (legal.to_str().unwrap(), illegal.to_str().unwrap());

    for args in [
        vec!["sim", illegal, legal, "-n", "1"],
        vec!["sim", legal, illegal, "-n", "1"],
        vec!["play", "--deck", illegal],
        vec!["play", "--deck", legal, "--bot-deck", illegal],
    ] {
        let out = ygo_real(&args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains("is not legal"), "{stderr}");
        assert!(
            stderr.contains("2 copies of Raigeki (12580477); limit is 1"),
            "{stderr}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
