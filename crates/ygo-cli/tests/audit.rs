//! `ygo audit` reviews the bots' decisions; the lookahead bot makes fewer
//! mistakes than the plain one on the same games.

use ygo_cli::{audit, bots::Bots};
use ygo_engine::testing::{bundle, root};

fn report(lookahead: bool) -> audit::Report {
    let b = bundle();
    let bots = Bots::load(
        &b,
        &root().join("config/styles.toml"),
        &root().join("config/card_values.toml"),
    )
    .unwrap();
    audit::run(
        &bots,
        &audit::AuditOptions {
            decks: &[],
            games: 20,
            seed: 5,
            style: "beatdown",
            lookahead,
            max_turns: ygo_engine::GameSetup::DEFAULT_MAX_TURNS,
            threads: 2,
        },
    )
    .unwrap()
}

#[test]
fn the_lookahead_bot_makes_fewer_mistakes_than_the_plain_one() {
    let (plain, look) = (report(false), report(true));
    assert_eq!((plain.games, look.games), (20, 20));
    assert!(plain.total.decisions > 0 && look.total.decisions > 0);
    let rate = |r: &audit::Report| f64::from(r.total.mistakes) / f64::from(r.total.decisions);
    assert!(
        rate(&look) < rate(&plain),
        "{} vs {}",
        rate(&look),
        rate(&plain)
    );
}
