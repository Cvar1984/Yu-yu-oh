//! Rendering: known states read as expected, and nothing hidden from a
//! player is ever named in what that player is shown.

use std::sync::Arc;

use ygo_cli::describe;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SUMMONED_SKULL, in_hand, next_turn, pass_all, play, sample_deck,
    shared_from, stacked, summon,
};
use ygo_engine::{Action, CardIndex, Game, GameSetup, Observation, PlayerId, Shared, VisibleCard};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const MIRROR_FORCE: CardId = CardId(44095762);

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
}

/// Names of every card the viewer can see right now.
fn visible_names<'a>(obs: &Observation, shared: &'a Shared) -> Vec<&'a str> {
    let mut ids: Vec<CardId> = Vec::new();
    let mut add = |c: &VisibleCard| {
        if let VisibleCard::Known { card, .. } = c {
            ids.push(*card);
        }
    };
    for side in &obs.sides {
        side.hand
            .iter()
            .chain(&side.graveyard)
            .chain(&side.banished)
            .chain(&side.extra)
            .for_each(&mut add);
        side.monster_zones
            .iter()
            .chain(&side.spell_trap_zones)
            .flatten()
            .for_each(&mut add);
        side.field_zone.iter().for_each(&mut add);
    }
    obs.revealed.iter().for_each(&mut add);
    ids.extend(obs.chain.iter().map(|l| l.card));
    ids.iter()
        .map(|&id| describe::card_name(&shared.pool, id))
        .collect()
}

/// Fails if `text` names any card except those `obs` shows.
fn assert_no_hidden_names(text: &str, obs: &Observation, shared: &Shared) {
    let mut visible = visible_names(obs, shared);
    visible.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut rest = text.to_string();
    for name in &visible {
        rest = rest.replace(name, "#");
    }
    for i in 0..shared.pool.len() as u16 {
        let name = &shared.pool.get(CardIndex(i)).name;
        assert!(
            !rest.contains(name.as_str()),
            "hidden card {name:?} named in:\n{text}"
        );
    }
}

#[test]
fn nothing_hidden_is_ever_named_in_random_games() {
    let shared = shared();
    let decks = [
        "owner_main_45.json",
        "umiiruka_water_40.json",
        "exodia_stall_full_pool_40.json",
        "defense_tuned_45.json",
    ];
    let mut lcg = 11u64;
    for seed in 0..24u64 {
        let setup = GameSetup::new(
            [
                sample_deck(decks[seed as usize % 4]),
                sample_deck(decks[(seed as usize + 1) % 4]),
            ],
            seed,
        );
        let mut g = Game::new(shared.clone(), &setup).unwrap();
        let mut events = Vec::new();
        while g.result().is_none() && g.turn() < 40 {
            for viewer in PlayerId::BOTH {
                let obs = g.observe(viewer);
                let mut text = describe::board(&obs, &shared.pool);
                for e in &events {
                    if let Some(line) = describe::event(&obs, &shared.pool, e) {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
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
            events = g.apply(&a).unwrap();
        }
    }
}
