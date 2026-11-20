//! The heuristic bot, one test per rule of thumb in docs/m4-plan.md, each
//! in a stacked scenario, plus whole games for every style.

use std::sync::Arc;

use ygo_ai::{HeuristicPlayer, Player, RandomPlayer, play_game};
use ygo_cards::{
    reveal::CONFISCATION, spells::POT_OF_GREED, spells::RAIGEKI, traps::SOLEMN_JUDGMENT,
    traps::TRAP_HOLE,
};
use ygo_data::{CardId, CardValues, Styles};
use ygo_engine::testing::{
    BLUE_EYES, CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, MYSTICAL_ELF, SEVEN_COLORED_FISH,
    SUMMONED_SKULL, bundle, in_hand, next_turn, pass_all, play, root, sample_deck, shared_from,
    stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, GameSetup, InstanceId, PlayerId, PlayerView, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const SNAKE_FANG: CardId = CardId(596051);
const PAD: CardId = MAMMOTH_GRAVEYARD; // 1200/800, worth 34

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn bot(style: &str) -> HeuristicPlayer {
    let b = bundle();
    let values = CardValues::load(&root().join("config/card_values.toml"), &b.cards).unwrap();
    let styles = Styles::load(&root().join("config/styles.toml"), &b.cards).unwrap();
    HeuristicPlayer::new(shared(), &b.cards, &values, styles.get(style).unwrap(), 1)
}

fn game(p0: &[CardId], p1: &[CardId]) -> Game {
    Game::new(shared(), &stacked([p0, p1], PAD)).unwrap()
}

/// What the bot plays at the current decision.
fn pick(bot: &mut HeuristicPlayer, g: &Game) -> Action {
    bot.choose(&PlayerView::new(g).expect("a decision is pending"))
}

fn summon_now(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

fn set_monster(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(
        g,
        Action::Summon {
            card,
            tributes: vec![],
            face_down: true,
        },
    );
    pass_all(g);
    card
}

fn enter_battle(g: &mut Game) {
    play(g, Action::EnterBattlePhase);
    pass_all(g);
}

#[test]
fn summons_the_monster_that_beats_the_board() {
    let g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF], &[]);
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    assert_eq!(pick(&mut bot("beatdown"), &g), summon(celtic));
}

#[test]
fn sets_the_best_defender_when_outclassed() {
    let mut g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF], &[SEVEN_COLORED_FISH]);
    next_turn(&mut g);
    summon_now(&mut g, P1, SEVEN_COLORED_FISH);
    next_turn(&mut g);
    let elf = in_hand(&g, P0, MYSTICAL_ELF);
    assert_eq!(
        pick(&mut bot("beatdown"), &g),
        Action::Summon {
            card: elf,
            tributes: vec![],
            face_down: true
        }
    );
}

#[test]
fn tribute_summons_only_when_worth_it_and_patient_waits_for_cover() {
    let mut g = game(&[CELTIC_GUARDIAN, SUMMONED_SKULL, TRAP_HOLE], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    next_turn(&mut g);
    let skull = in_hand(&g, P0, SUMMONED_SKULL);
    let tribute = Action::Summon {
        card: skull,
        tributes: vec![celtic],
        face_down: false,
    };
    assert!(g.actions().contains(&tribute));
    assert_eq!(pick(&mut bot("beatdown"), &g), tribute);

    let mut patient = bot("patient");
    let first = pick(&mut patient, &g);
    assert!(
        !matches!(first, Action::Summon { ref tributes, .. } if !tributes.is_empty()),
        "{first:?}"
    );
    let trap = in_hand(&g, P0, TRAP_HOLE);
    play(&mut g, Action::SetSpellTrap { card: trap });
    pass_all(&mut g);
    assert_eq!(pick(&mut patient, &g), tribute, "with a Set card behind it");

    // Two monsters worth 38 and 34 are not worth Blue-Eyes, which
    // card_values.toml lists at 40.
    let mut g = game(&[CELTIC_GUARDIAN, BLUE_EYES], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    next_turn(&mut g);
    let mammoth = summon_now(&mut g, P0, PAD);
    next_turn(&mut g);
    next_turn(&mut g);
    let blue_eyes = in_hand(&g, P0, BLUE_EYES);
    let both = g
        .actions()
        .iter()
        .find(|a| {
            matches!(a, Action::Summon { card, tributes, face_down: false }
            if *card == blue_eyes && tributes.contains(&celtic) && tributes.contains(&mammoth))
        })
        .cloned()
        .expect("Blue-Eyes can be Tribute Summoned");
    let choice = pick(&mut bot("beatdown"), &g);
    assert_ne!(choice, both);
    assert!(
        !matches!(choice, Action::Summon { card, .. } if card == blue_eyes),
        "{choice:?}"
    );
}

#[test]
fn attacks_the_best_monster_it_can_beat() {
    let mut g = game(&[SEVEN_COLORED_FISH], &[CELTIC_GUARDIAN]);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let fish = summon_now(&mut g, P0, SEVEN_COLORED_FISH);
    next_turn(&mut g);
    summon_now(&mut g, P1, PAD);
    next_turn(&mut g);
    enter_battle(&mut g);
    assert_eq!(
        pick(&mut bot("beatdown"), &g),
        Action::Attack {
            attacker: fish,
            target: Some(celtic)
        }
    );
}

#[test]
fn attacks_directly_when_it_can() {
    let mut g = game(&[CELTIC_GUARDIAN], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    next_turn(&mut g);
    let mut b = bot("beatdown");
    assert!(b.score(&g.observe(P0), &Action::EnterBattlePhase) > 0.0);
    enter_battle(&mut g);
    assert_eq!(
        pick(&mut b, &g),
        Action::Attack {
            attacker: celtic,
            target: None
        }
    );
}

#[test]
fn never_attacks_into_a_known_loss() {
    let mut g = game(&[CELTIC_GUARDIAN], &[SEVEN_COLORED_FISH]);
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    summon_now(&mut g, P1, SEVEN_COLORED_FISH);
    next_turn(&mut g);
    let mut b = bot("beatdown");
    assert!(b.score(&g.observe(P0), &Action::EnterBattlePhase) < 0.0);
    enter_battle(&mut g);
    assert_eq!(pick(&mut b, &g), Action::EndBattlePhase);
}

#[test]
fn attacks_a_face_down_monster_only_with_enough_atk() {
    let mut g = game(&[SEVEN_COLORED_FISH], &[]);
    let mammoth = summon_now(&mut g, P0, PAD);
    next_turn(&mut g);
    let set = set_monster(&mut g, P1, PAD);
    next_turn(&mut g);
    let fish = summon_now(&mut g, P0, SEVEN_COLORED_FISH);
    enter_battle(&mut g);
    let mut b = bot("beatdown");
    let weak = Action::Attack {
        attacker: mammoth,
        target: Some(set),
    };
    assert!(g.actions().contains(&weak));
    assert!(
        b.score(&g.observe(P0), &weak) < 0.0,
        "1200 ATK is below the style's 1400"
    );
    assert_eq!(
        pick(&mut b, &g),
        Action::Attack {
            attacker: fish,
            target: Some(set)
        }
    );
}

#[test]
fn activates_removal_when_the_opponent_has_something() {
    let mut g = game(&[RAIGEKI], &[CELTIC_GUARDIAN]);
    let mut b = bot("beatdown");
    let first = pick(&mut b, &g);
    assert!(
        !matches!(first, Action::Activate { .. }),
        "nothing to remove yet: {first:?}"
    );
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let raigeki = in_hand(&g, P0, RAIGEKI);
    assert!(matches!(pick(&mut b, &g), Action::Activate { card, .. } if card == raigeki));
}

#[test]
fn spends_a_counter_trap_only_on_a_big_enough_summon() {
    for (summoned, expect_counter) in [(MYSTICAL_ELF, true), (PAD, false)] {
        let mut g = game(&[SOLEMN_JUDGMENT], &[MYSTICAL_ELF]);
        let solemn = in_hand(&g, P0, SOLEMN_JUDGMENT);
        play(&mut g, Action::SetSpellTrap { card: solemn });
        next_turn(&mut g);
        let card = in_hand(&g, P1, summoned);
        play(&mut g, summon(card));
        let d = g.pending_decision().unwrap();
        assert_eq!((d.player, d.kind), (P0, DecisionKind::Respond));
        let choice = pick(&mut bot("beatdown"), &g);
        assert_eq!(
            matches!(choice, Action::Activate { card, .. } if card == solemn),
            expect_counter,
            "{summoned}: {choice:?}"
        );
    }
}

#[test]
fn takes_the_opponents_best_card() {
    let mut g = game(&[CONFISCATION], &[CELTIC_GUARDIAN, POT_OF_GREED]);
    let conf = in_hand(&g, P0, CONFISCATION);
    let activate = g
        .actions()
        .iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == conf))
        .cloned()
        .expect("Confiscation can be activated");
    play(&mut g, activate);
    while g.pending_decision().map(|d| d.kind) == Some(DecisionKind::Respond) {
        play(&mut g, Action::Pass);
    }
    assert_eq!(
        g.pending_decision().map(|d| (d.player, d.kind)),
        Some((P0, DecisionKind::Choose))
    );
    let pot = in_hand(&g, P1, POT_OF_GREED);
    assert_eq!(pick(&mut bot("beatdown"), &g), Action::Choose { card: pot });
}

#[test]
fn discards_the_lowest_value_card_for_the_hand_limit() {
    let shared = with_rules(&shared(), |r| {
        r.hand.max_size = 5;
        r.hand.hand_limit_check = ygo_data::HandLimitCheck::AfterDraw;
    });
    let setup = stacked(
        [
            &[
                POT_OF_GREED,
                CELTIC_GUARDIAN,
                MYSTICAL_ELF,
                SUMMONED_SKULL,
                BLUE_EYES,
            ],
            &[],
        ],
        PAD,
    );
    let mut g = Game::new(shared, &setup).unwrap();
    next_turn(&mut g);
    pass_all(&mut g);
    play(&mut g, Action::EndTurn);
    while g.pending_decision().map(|d| d.kind) == Some(DecisionKind::Respond) {
        play(&mut g, Action::Pass);
    }
    assert_eq!(
        g.pending_decision().map(|d| (d.player, d.kind)),
        Some((P0, DecisionKind::HandLimit))
    );
    let drawn = in_hand(&g, P0, PAD);
    assert_eq!(
        pick(&mut bot("beatdown"), &g),
        Action::Discard { card: drawn }
    );
}

#[test]
fn keeps_a_trap_for_a_window_the_opponent_opens() {
    // Traps usable the turn they are Set, so the choice comes up at once.
    let rules = with_rules(&shared(), |r| r.turn.trap_activation_turn_set = true);
    let mut g = Game::new(rules, &stacked([&[CELTIC_GUARDIAN, SNAKE_FANG], &[]], PAD)).unwrap();
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let fang = in_hand(&g, P0, SNAKE_FANG);
    play(&mut g, Action::SetSpellTrap { card: fang });
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Respond));
    assert!(
        g.actions()
            .iter()
            .any(|a| matches!(a, Action::Activate { card, .. } if *card == fang)),
        "Snake Fang can be activated here, so holding it is a choice"
    );
    assert_eq!(pick(&mut bot("beatdown"), &g), Action::Pass);
}

#[test]
fn every_style_finishes_games_against_the_random_bot() {
    let shared = shared();
    let styles = Styles::load(&root().join("config/styles.toml"), &bundle().cards).unwrap();
    for (i, style) in styles.names().enumerate() {
        for seed in 0..4u64 {
            let setup = GameSetup::new(
                [
                    sample_deck("owner_main_45.json"),
                    sample_deck("umiiruka_water_40.json"),
                ],
                seed + 10 * i as u64,
            );
            let mut h = bot(style);
            let mut r = RandomPlayer::new(seed);
            let mut players: [&mut dyn Player; 2] = [&mut h, &mut r];
            let labels = [format!("heuristic:{style}"), "random".to_string()];
            let (game, record) = play_game(shared.clone(), &setup, &mut players, labels).unwrap();
            assert!(game.result().is_some());
            assert_eq!(record.replay(&shared).unwrap(), game, "{style} seed {seed}");
        }
    }
}

#[test]
fn never_summons_or_sets_a_piece_of_exodia() {
    const HEAD: CardId = CardId(33396948);
    const LEFT_ARM: CardId = CardId(7902349);
    const RIGHT_ARM: CardId = CardId(70903634);
    const LEFT_LEG: CardId = CardId(44519536);
    let g = game(&[HEAD, LEFT_ARM, RIGHT_ARM, LEFT_LEG, CELTIC_GUARDIAN], &[]);
    let pieces: Vec<InstanceId> = [HEAD, LEFT_ARM, RIGHT_ARM, LEFT_LEG]
        .iter()
        .map(|&id| in_hand(&g, P0, id))
        .collect();
    assert!(
        g.actions()
            .iter()
            .any(|a| matches!(a, Action::Summon { card, .. } if pieces.contains(card))),
        "the pieces could be summoned, so keeping them is a choice"
    );
    for style in ["exodia", "beatdown", "stall"] {
        let mut b = bot(style);
        let choice = pick(&mut b, &g);
        assert!(
            !matches!(choice, Action::Summon { card, .. } if pieces.contains(&card)),
            "{style}: {choice:?}"
        );
    }
    // With the Exodia win switched off, a piece is just a monster.
    let off = with_rules(&shared(), |r| r.win.exodia = false);
    let g = Game::new(
        off.clone(),
        &stacked([&[HEAD, LEFT_ARM, RIGHT_ARM, LEFT_LEG, LEFT_LEG], &[]], PAD),
    )
    .unwrap();
    let b = bundle();
    let values = CardValues::load(&root().join("config/card_values.toml"), &b.cards).unwrap();
    let styles = Styles::load(&root().join("config/styles.toml"), &b.cards).unwrap();
    let mut beat = HeuristicPlayer::new(off, &b.cards, &values, styles.get("beatdown").unwrap(), 1);
    assert!(matches!(pick(&mut beat, &g), Action::Summon { .. }));
}
