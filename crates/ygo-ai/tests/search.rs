//! The search bot: legal, repeatable, and better than its playout bot
//! where the heuristic plays a card naively.

use std::sync::Arc;

use ygo_ai::{HeuristicPlayer, Player, Prior, SearchPlayer, SearchSettings, play_game};
use ygo_cards::spells::DARK_HOLE;
use ygo_data::{CardId, CardValues, Styles};
use ygo_engine::testing::{
    BLUE_EYES, CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, bundle, in_hand, next_turn, pass_all, play,
    root, sample_deck, shared_from, stacked, summon,
};
use ygo_engine::{Action, Game, GameSetup, PlayerId, PlayerView, Shared};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn heuristic(style: &str, seed: u64) -> HeuristicPlayer {
    let b = bundle();
    let values = CardValues::load(&root().join("config/card_values.toml"), &b.cards).unwrap();
    let styles = Styles::load(&root().join("config/styles.toml"), &b.cards).unwrap();
    HeuristicPlayer::new(
        shared(),
        &b.cards,
        &values,
        styles.get(style).unwrap(),
        seed,
    )
}

fn search(iterations: u32, seed: u64) -> SearchPlayer {
    let settings = SearchSettings {
        iterations,
        ..SearchSettings::default()
    };
    SearchPlayer::new(heuristic("beatdown", seed), settings, seed)
}

fn game(p0: &[CardId], p1: &[CardId]) -> Game {
    Game::new(shared(), &stacked([p0, p1], MAMMOTH_GRAVEYARD)).unwrap()
}

fn pick(bot: &mut dyn Player, g: &Game) -> Action {
    bot.choose(&PlayerView::new(g).expect("a decision is pending"))
}

#[test]
fn keeps_dark_hole_while_its_own_monster_rules_the_field() {
    // Blue-Eyes faces one Celtic Guardian. Dark Hole would destroy both;
    // the heuristic bot plays it anyway, the search bot does not.
    let mut g = game(&[BLUE_EYES, DARK_HOLE], &[CELTIC_GUARDIAN]);
    next_turn(&mut g);
    let celtic = in_hand(&g, P1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    next_turn(&mut g);
    // Two pads on two turns, then Blue-Eyes tributing both.
    let pad = in_hand(&g, P0, MAMMOTH_GRAVEYARD);
    play(&mut g, summon(pad));
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    let pad2 = g
        .player(P0)
        .hand
        .iter()
        .copied()
        .find(|&id| id != pad && g.stats(id).id == MAMMOTH_GRAVEYARD)
        .expect("another pad in hand");
    play(&mut g, summon(pad2));
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    let blue_eyes = in_hand(&g, P0, BLUE_EYES);
    play(
        &mut g,
        Action::Summon {
            card: blue_eyes,
            tributes: vec![pad, pad2],
            face_down: false,
        },
    );
    pass_all(&mut g);
    assert_eq!(g.player(P1).monsters().count(), 1);

    let dark_hole = in_hand(&g, P0, DARK_HOLE);
    let is_dark_hole =
        |a: &Action| matches!(a, Action::Activate { card, .. } if *card == dark_hole);
    assert!(g.actions().iter().any(is_dark_hole));
    assert!(
        is_dark_hole(&pick(&mut heuristic("beatdown", 1), &g)),
        "the playout bot plays Dark Hole here"
    );
    let choice = pick(&mut search(150, 1), &g);
    assert!(!is_dark_hole(&choice), "{choice:?}");
}

#[test]
fn the_same_seed_gives_the_same_choice_and_analysis() {
    let g = game(&[CELTIC_GUARDIAN, DARK_HOLE], &[]);
    let view = PlayerView::new(&g).unwrap();
    let a = search(40, 3).analyse(&view);
    let b = search(40, 3).analyse(&view);
    assert_eq!(a, b);
    assert_eq!(a.len(), view.actions().len());
    let visits: u32 = a.iter().map(|(_, v, _)| v).sum();
    assert_eq!(visits, 40, "every playout starts from one first move");
}

#[test]
fn whole_games_against_the_heuristic_bot_replay_from_their_records() {
    let shared = shared();
    for seed in 0..2u64 {
        let mut setup = GameSetup::new(
            [
                sample_deck("save_a_owner_current_40.json"),
                sample_deck("save_b_dark_plasma_40.json"),
            ],
            seed,
        );
        setup.first_player = Some(PlayerId(seed as u8));
        let mut s = search(12, seed);
        let mut h = heuristic("control", seed);
        let mut players: [&mut dyn Player; 2] = [&mut s, &mut h];
        let labels = ["search".to_string(), "heuristic:control".to_string()];
        let (game, record) = play_game(shared.clone(), &setup, &mut players, labels).unwrap();
        assert!(game.result().is_some());
        assert_eq!(record.replay(&shared).unwrap(), game, "seed {seed}");
    }
}

#[test]
fn a_zero_time_limit_still_answers_like_the_playout_bot() {
    let g = game(&[CELTIC_GUARDIAN], &[]);
    let settings = SearchSettings {
        time_limit: Some(std::time::Duration::ZERO),
        ..SearchSettings::default()
    };
    let mut s = SearchPlayer::new(heuristic("beatdown", 5), settings, 5);
    assert_eq!(pick(&mut s, &g), pick(&mut heuristic("beatdown", 5), &g));
}

/// A prior that puts nearly everything on one action.
struct Favour(usize);

impl Prior for Favour {
    fn prior(&mut self, view: &PlayerView) -> Vec<f64> {
        let n = view.actions().len();
        let rest = 0.01 / (n - 1) as f64;
        (0..n)
            .map(|i| if i == self.0 { 0.99 } else { rest })
            .collect()
    }
}

#[test]
fn a_prior_steers_the_visits_of_the_decision() {
    let g = game(&[CELTIC_GUARDIAN, DARK_HOLE], &[]);
    let view = PlayerView::new(&g).unwrap();
    assert!(view.actions().len() > 2);
    let settings = SearchSettings {
        iterations: 60,
        prior_weight: 50.0,
        ..SearchSettings::default()
    };
    for favoured in [0, view.actions().len() - 1] {
        let analyse = || {
            SearchPlayer::new(heuristic("beatdown", 2), settings.clone(), 2)
                .with_prior(Box::new(Favour(favoured)))
                .analyse(&view)
        };
        let a = analyse();
        assert_eq!(a, analyse(), "the same seed gives the same analysis");
        assert_eq!(a.iter().map(|(_, v, _)| v).sum::<u32>(), 60);
        let most = (0..a.len()).max_by_key(|&i| a[i].1).unwrap();
        assert_eq!(most, favoured, "{a:?}");
        assert!(a[favoured].1 >= 50, "{a:?}");
    }
}
