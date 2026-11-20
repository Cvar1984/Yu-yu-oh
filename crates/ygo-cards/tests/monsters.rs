//! Rulings tests for the M2 step 4 monsters: triggers, SEGOC, FLIP
//! effects, Kuriboh and Exodia.

use std::sync::Arc;

use ygo_cards::monsters::*;
use ygo_cards::spells::{POT_OF_GREED, RAIGEKI, TRIBUTE_TO_THE_DOOMED};
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, SEVEN_COLORED_FISH, SUMMONED_SKULL, in_hand, next_turn,
    pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{
    Action, DecisionKind, Event, Game, GameResult, Location, PlayerId, Position, Shared, WinReason,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

fn decision_is(g: &Game, player: PlayerId, kind: DecisionKind) -> bool {
    g.pending_decision()
        .is_some_and(|d| d.player == player && d.kind == kind)
}

fn set_monster(g: &mut Game, player: PlayerId, id: CardId) -> ygo_engine::InstanceId {
    let card = in_hand(g, player, id);
    play(
        g,
        Action::Summon {
            card,
            tributes: vec![],
            face_down: true,
        },
    );
    card
}

#[test]
fn sangan_searches_when_tributed_and_the_deck_is_shuffled() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    SANGAN,
                    SUMMONED_SKULL,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    SEVEN_COLORED_FISH,
                    CELTIC_GUARDIAN,
                ],
                &[],
            ],
            SUMMONED_SKULL,
        ),
    )
    .unwrap();
    let sangan = in_hand(&g, P0, SANGAN);
    play(&mut g, summon(sangan));
    next_turn(&mut g);
    next_turn(&mut g);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let deck_before: Vec<_> = g.deck(P0).to_vec();
    play(
        &mut g,
        Action::Summon {
            card: skull,
            tributes: vec![sangan],
            face_down: false,
        },
    );
    // Sangan's trigger fires in the summon-success window and resolves
    // into a search: only Celtic Guardian (1400) qualifies; the Fish is 1800
    // and Summoned Skull 2500.
    assert!(
        decision_is(&g, P0, DecisionKind::Choose),
        "{:?}",
        g.pending_decision()
    );
    let options = g.legal_actions();
    assert!(
        options
            .iter()
            .all(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == CELTIC_GUARDIAN))
    );
    assert!(g.on_field(skull), "the Tribute Summon completed first");
    let Action::Choose { card: celtic } = options[0] else {
        unreachable!()
    };
    let events = play(&mut g, options[0].clone());
    assert_eq!(g.instance(celtic).location, Location::Hand);
    assert!(events.contains(&Event::DeckShuffled { player: P0 }));
    assert_eq!(g.deck(P0).len(), deck_before.len() - 1);
    assert_ne!(
        g.deck(P0),
        &deck_before[..deck_before.len() - 1],
        "shuffled"
    );
    assert!(decision_is(&g, P0, DecisionKind::MainPhase));
}

#[test]
fn sangan_searches_when_destroyed_by_battle_but_not_when_discarded() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[SANGAN, TRIBUTE_TO_THE_DOOMED, SANGAN],
                &[SEVEN_COLORED_FISH],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let sangan = in_hand(&g, P0, SANGAN);
    play(&mut g, summon(sangan));
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(sangan),
        },
    );
    assert!(
        decision_is(&g, P0, DecisionKind::Choose),
        "search after the battle: {:?}",
        g.pending_decision()
    );
    let a = g.legal_actions()[0].clone();
    play(&mut g, a);
    assert_eq!(g.player(P0).hand.len(), 5);
    pass_all(&mut g);
    play(&mut g, Action::EndBattlePhase);
    next_turn(&mut g);

    // Discarding the second Sangan as Tribute to the Doomed's cost: no search.
    let sangan2 = in_hand(&g, P0, SANGAN);
    let offered = activations(&g, TRIBUTE_TO_THE_DOOMED);
    let a = offered
        .iter()
        .find(|a| matches!(a, Action::Activate { cost, targets, .. } if cost == &vec![sangan2] && targets == &vec![fish]))
        .unwrap()
        .clone();
    play(&mut g, a);
    assert_eq!(g.instance(sangan2).location, Location::Graveyard);
    assert!(!decision_is(&g, P0, DecisionKind::Choose));
    assert_eq!(g.instance(fish).location, Location::Graveyard);
}

#[test]
fn simultaneous_mandatory_triggers_are_ordered_by_their_controller() {
    let base = shared();
    let shared = with_rules(&base, |r| r.summon.normal_or_tribute_per_turn = 2);
    let mut g = Game::new(
        shared,
        &stacked(
            [
                &[RAIGEKI],
                &[
                    SANGAN,
                    WITCH_OF_THE_BLACK_FOREST,
                    SEVEN_COLORED_FISH,
                    SEVEN_COLORED_FISH,
                    SEVEN_COLORED_FISH,
                    SEVEN_COLORED_FISH,
                    CELTIC_GUARDIAN,
                    MYSTICAL_ELF,
                ],
            ],
            SEVEN_COLORED_FISH,
        ),
    )
    .unwrap();
    next_turn(&mut g);
    let sangan = in_hand(&g, P1, SANGAN);
    let witch = in_hand(&g, P1, WITCH_OF_THE_BLACK_FOREST);
    play(&mut g, summon(sangan));
    play(&mut g, summon(witch));
    next_turn(&mut g);
    let raigeki = activations(&g, RAIGEKI)[0].clone();
    play(&mut g, raigeki);
    assert!(
        decision_is(&g, P1, DecisionKind::Triggers),
        "{:?}",
        g.pending_decision()
    );
    let offered = g.legal_actions();
    assert_eq!(
        offered.len(),
        2,
        "Sangan or Witch first; no Pass for mandatory triggers"
    );
    assert!(!offered.contains(&Action::Pass));
    let witch_first = offered
        .iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == witch))
        .unwrap()
        .clone();
    play(&mut g, witch_first);
    // Sangan is the only trigger left: it goes on the chain by itself. The
    // chain resolves last link first, so Sangan's search comes before the Witch's.
    assert!(decision_is(&g, P1, DecisionKind::Choose));
    let options = g.legal_actions();
    assert!(
        options
            .iter()
            .all(|a| matches!(a, Action::Choose { card } if g.stats(*card).atk <= 1500)),
        "Sangan resolves first"
    );
    let elf = options
        .iter()
        .find(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == MYSTICAL_ELF))
        .unwrap()
        .clone();
    play(&mut g, elf);
    assert!(decision_is(&g, P1, DecisionKind::Choose), "then the Witch");
    let options = g.legal_actions();
    assert!(
        options
            .iter()
            .all(|a| matches!(a, Action::Choose { card } if g.stats(*card).def <= 1500))
    );
    play(&mut g, options[0].clone());
    // Five in the opening hand (no draw on their first turn), two summoned,
    // two searched.
    assert_eq!(g.player(P1).hand.len(), 5 - 2 + 2);
}

#[test]
fn man_eater_bug_flip_summoned_destroys_a_chosen_monster() {
    let mut g = Game::new(
        shared(),
        &stacked([&[MAN_EATER_BUG], &[CELTIC_GUARDIAN]], MYSTICAL_ELF),
    )
    .unwrap();
    let bug = set_monster(&mut g, P0, MAN_EATER_BUG);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: bug });
    assert!(
        decision_is(&g, P0, DecisionKind::Triggers),
        "two possible targets: {:?}",
        g.pending_decision()
    );
    let on_celtic = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]))
        .unwrap();
    play(&mut g, on_celtic);
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(g.on_field(bug));
}

#[test]
fn man_eater_bug_flipped_by_an_attack_destroys_after_damage_calculation() {
    let mut g = Game::new(
        shared(),
        &stacked([&[MAN_EATER_BUG], &[SEVEN_COLORED_FISH]], MYSTICAL_ELF),
    )
    .unwrap();
    let bug = set_monster(&mut g, P0, MAN_EATER_BUG);
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(bug),
        },
    );
    // The Bug (600 DEF) is destroyed by the 1800 ATK Fish, and its FLIP
    // effect still goes off: the Fish is the only monster left to target.
    assert_eq!(g.instance(bug).location, Location::Graveyard);
    assert_eq!(
        g.instance(fish).location,
        Location::Graveyard,
        "one target, one option: automatic"
    );
    assert_eq!(
        g.player(P0).lp,
        8000,
        "no damage against a Defense Position monster"
    );
}

#[test]
fn magician_of_faith_returns_a_spell_from_the_graveyard() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[POT_OF_GREED, MAGICIAN_OF_FAITH], &[SEVEN_COLORED_FISH]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let pot = in_hand(&g, P0, POT_OF_GREED);
    let pot_activation = activations(&g, POT_OF_GREED)[0].clone();
    play(&mut g, pot_activation);
    let magician = set_monster(&mut g, P0, MAGICIAN_OF_FAITH);
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(magician),
        },
    );
    assert_eq!(g.instance(magician).location, Location::Graveyard);
    assert_eq!(
        g.instance(pot).location,
        Location::Hand,
        "the only Spell in the Graveyard came back"
    );
}

#[test]
fn kuriboh_prevents_the_battle_damage_of_one_attack() {
    let mut g = Game::new(
        shared(),
        &stacked([&[KURIBOH], &[SEVEN_COLORED_FISH]], MYSTICAL_ELF),
    )
    .unwrap();
    let kuriboh = in_hand(&g, P0, KURIBOH);
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: None,
        },
    );
    assert!(
        decision_is(&g, P0, DecisionKind::Respond),
        "during damage calculation: {:?}",
        g.pending_decision()
    );
    let offered = activations(&g, KURIBOH);
    assert_eq!(offered.len(), 1);
    let events = play(&mut g, offered[0].clone());
    assert_eq!(
        g.instance(kuriboh).location,
        Location::Graveyard,
        "discarded as the cost"
    );
    assert!(events.contains(&Event::BattleDamagePrevented { player: P0 }));
    assert_eq!(g.player(P0).lp, 8000);
    assert!(g.on_field(fish));
}

#[test]
fn kuriboh_does_not_answer_your_own_attack_or_prevent_destruction() {
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[KURIBOH, MYSTICAL_ELF], &[SEVEN_COLORED_FISH]],
            CELTIC_GUARDIAN,
        ),
    )
    .unwrap();
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf));
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: elf,
            target: Some(fish),
        },
    );
    assert!(
        !decision_is(&g, P0, DecisionKind::Respond),
        "Kuriboh only answers the opponent's attack"
    );
    pass_all(&mut g);
    assert_eq!(g.instance(elf).location, Location::Graveyard);
    assert_eq!(g.player(P0).lp, 7000);
}

#[test]
fn exodia_wins_on_the_draw_and_on_a_search() {
    let [ra, la, rl, ll] = EXODIA_LIMBS;
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[ra, la, rl, ll, MYSTICAL_ELF, EXODIA_THE_FORBIDDEN_ONE],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert!(g.result().is_none(), "four limbs and no head");
    next_turn(&mut g);
    let events = play(&mut g, Action::EndTurn);
    assert_eq!(
        g.result(),
        Some(GameResult::Win {
            winner: P0,
            reason: WinReason::Exodia
        })
    );
    assert!(matches!(events.last(), Some(Event::GameOver { .. })));

    // Sangan's search finds the last piece.
    let mut g = Game::new(
        shared(),
        &stacked(
            [
                &[
                    ra,
                    la,
                    rl,
                    ll,
                    SANGAN,
                    MYSTICAL_ELF,
                    EXODIA_THE_FORBIDDEN_ONE,
                ],
                &[SEVEN_COLORED_FISH],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let sangan = in_hand(&g, P0, SANGAN);
    play(&mut g, summon(sangan));
    next_turn(&mut g);
    let fish = in_hand(&g, P1, SEVEN_COLORED_FISH);
    play(&mut g, summon(fish));
    play(&mut g, Action::EnterBattlePhase);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(sangan),
        },
    );
    let head = g.legal_actions().into_iter().find(|a| matches!(a, Action::Choose { card } if g.stats(*card).id == EXODIA_THE_FORBIDDEN_ONE)).unwrap();
    play(&mut g, head);
    assert_eq!(
        g.result(),
        Some(GameResult::Win {
            winner: P0,
            reason: WinReason::Exodia
        })
    );
}

#[test]
fn flip_exodia_off() {
    let base = shared();
    let shared = with_rules(&base, |r| r.win.exodia = false);
    let [ra, la, rl, ll] = EXODIA_LIMBS;
    let mut g = Game::new(
        shared,
        &stacked(
            [
                &[ra, la, rl, ll, MYSTICAL_ELF, EXODIA_THE_FORBIDDEN_ONE],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    next_turn(&mut g);
    play(&mut g, Action::EndTurn);
    assert!(g.result().is_none());
}

#[test]
fn exodia_in_the_opening_hand_wins_at_once() {
    let [ra, la, rl, ll] = EXODIA_LIMBS;
    let g = Game::new(
        shared(),
        &stacked(
            [&[ra, la, rl, ll, EXODIA_THE_FORBIDDEN_ONE], &[]],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    assert_eq!(
        g.result(),
        Some(GameResult::Win {
            winner: P0,
            reason: WinReason::Exodia
        })
    );
    let _ = Position::FaceUp;
}
