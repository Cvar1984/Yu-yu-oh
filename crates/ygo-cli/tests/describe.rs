//! Rendering: known states read as expected, and nothing hidden from a
//! player is ever named in what that player is shown.

use std::sync::Arc;

mod common;

use common::{assert_no_hidden_names, assert_no_hidden_names_across};
use ygo_cli::describe;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SUMMONED_SKULL, in_hand, next_turn, pass_all, play, sample_deck,
    shared_from, stacked, summon,
};
use ygo_engine::{Action, Game, GameSetup, PlayerId, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const MIRROR_FORCE: CardId = CardId(44095762);
const LEGENDARY_SWORD: CardId = CardId(61854111);
const DARK_HOLE: CardId = CardId(53129443);

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

#[test]
fn the_board_and_actions_read_as_sentences() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[CELTIC_GUARDIAN, MIRROR_FORCE, MYSTICAL_ELF, SUMMONED_SKULL],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let pool = &g.shared().pool.clone();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    let obs = g.observe(P0);
    assert_eq!(
        describe::action(&obs, pool, &summon(celtic)),
        "Normal Summon Celtic Guardian"
    );
    play(&mut g, summon(celtic));
    let mirror = in_hand(&g, P0, MIRROR_FORCE);
    play(&mut g, Action::SetSpellTrap { card: mirror });
    pass_all(&mut g);

    let mine = describe::board(&g.observe(P0), pool);
    assert!(mine.contains("Celtic Guardian 1400/1200 ATK"), "{mine}");
    assert!(mine.contains("Mirror Force (set)"), "{mine}");
    assert!(mine.contains("Hand: "), "{mine}");
    let theirs = describe::board(&g.observe(P1), pool);
    assert!(theirs.contains("Celtic Guardian 1400/1200 ATK"), "{theirs}");
    assert!(theirs.contains("[set card]"), "{theirs}");
    assert!(!theirs.contains("Mirror Force"), "{theirs}");

    next_turn(&mut g);
    next_turn(&mut g);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let obs = g.observe(P0);
    let tribute = Action::Summon {
        card: skull,
        tributes: vec![celtic],
        face_down: false,
    };
    assert_eq!(
        describe::action(&obs, pool, &tribute),
        "Tribute Summon Summoned Skull, tributing Celtic Guardian"
    );
    assert_eq!(
        describe::action(&obs, pool, &Action::EndTurn),
        "End the turn"
    );
    assert_eq!(
        describe::action(
            &obs,
            pool,
            &Action::Attack {
                attacker: celtic,
                target: None
            }
        ),
        "Attack directly with Celtic Guardian"
    );
    // Attacker first, then the target, so the attacker is read first.
    assert_eq!(
        describe::action(
            &obs,
            pool,
            &Action::Attack {
                attacker: celtic,
                target: Some(celtic)
            }
        ),
        "Attack with Celtic Guardian → Celtic Guardian"
    );
}

#[test]
fn two_copies_on_the_field_are_told_apart_by_zone() {
    let mut g = Game::new(
        shared(),
        &stacked([&[CELTIC_GUARDIAN, CELTIC_GUARDIAN], &[]], MYSTICAL_ELF),
    )
    .unwrap();
    let pool = &g.shared().pool.clone();
    let first = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(first));
    pass_all(&mut g);
    assert_eq!(
        describe::card(&g.observe(P0), pool, first),
        "Celtic Guardian"
    );
    next_turn(&mut g);
    next_turn(&mut g);
    let second = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(second));
    pass_all(&mut g);
    let mine = g.observe(P0);
    let (a, b) = (
        describe::card(&mine, pool, first),
        describe::card(&mine, pool, second),
    );
    assert!(a.starts_with("Celtic Guardian in your zone "), "{a}");
    assert!(b.starts_with("Celtic Guardian in your zone "), "{b}");
    assert_ne!(a, b);
    let theirs = describe::card(&g.observe(P1), pool, first);
    assert!(
        theirs.starts_with("Celtic Guardian in your opponent's zone "),
        "{theirs}"
    );
}

#[test]
fn nothing_hidden_is_ever_named_in_random_games() {
    let shared = shared();
    let decks = [
        "owner_main_45.json",
        "umiiruka_water_40.json",
        "defense_tuned_45.json",
    ];
    let mut lcg = 11u64;
    for seed in 0..24u64 {
        let setup = GameSetup::new(
            [
                sample_deck(decks[seed as usize % decks.len()]),
                sample_deck(decks[(seed as usize + 1) % decks.len()]),
            ],
            seed,
        );
        let mut g = Game::new(shared.clone(), &setup).unwrap();
        let mut events = Vec::new();
        let mut before = PlayerId::BOTH.map(|v| g.observe(v));
        while g.result().is_none() && g.turn() < 40 {
            for viewer in PlayerId::BOTH {
                let obs = g.observe(viewer);
                // Log lines may name a card as the viewer saw it before the
                // action (a card returned to the Deck).
                let prior = &before[viewer.index()];
                let lines = describe::events(prior, &obs, &shared.pool, &events).join("\n");
                assert_no_hidden_names_across(&lines, &[prior, &obs], &shared);
                let mut text = describe::board(&obs, &shared.pool);
                if g.pending_decision().is_some_and(|d| d.player == viewer) {
                    for a in g.actions() {
                        text.push_str(&describe::action(&obs, &shared.pool, a));
                        text.push('\n');
                    }
                }
                assert_no_hidden_names(&text, &obs, &shared);
            }
            let actions = g.legal_actions();
            lcg = lcg
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let a = actions[(lcg >> 33) as usize % actions.len()].clone();
            before = PlayerId::BOTH.map(|v| g.observe(v));
            events = g.apply(&a).unwrap();
        }
    }
}

#[test]
fn the_log_tells_a_card_sent_to_the_graveyard_unless_something_else_says_why() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[CELTIC_GUARDIAN, LEGENDARY_SWORD, DARK_HOLE], &[]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let pool = &g.shared().pool.clone();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    let sword = in_hand(&g, P0, LEGENDARY_SWORD);
    let equip = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == sword))
        .unwrap();
    play(&mut g, equip);
    pass_all(&mut g);
    let dark_hole = in_hand(&g, P0, DARK_HOLE);
    let activate = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == dark_hole))
        .unwrap();
    let before = g.observe(P0);
    let events = play(&mut g, activate);
    let lines = describe::events(&before, &g.observe(P0), pool, &events);
    assert!(
        lines.contains(&"Legendary Sword was sent to the Graveyard".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"Celtic Guardian was destroyed".to_string()),
        "{lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("Celtic Guardian was sent") || l.contains("Dark Hole was sent")),
        "a destroyed monster and a resolved Spell need no second line: {lines:?}"
    );
}
