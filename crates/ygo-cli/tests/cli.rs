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
