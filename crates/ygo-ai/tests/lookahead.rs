//! The heuristic bot's one-step lookahead: each activation, answer and
//! choice is tried on a sample and the best position kept. Each test shows
//! a mistake the plain bot makes and the lookahead bot does not.

use std::sync::Arc;

use ygo_ai::{HeuristicPlayer, Player};
use ygo_cards::continuous::MESSENGER_OF_PEACE;
use ygo_cards::spells::DARK_HOLE;
use ygo_data::{CardId, CardValues, Styles};
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, bundle, in_hand, next_turn, pass_all, play, root,
    shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, PlayerId, PlayerView, Shared};

const P0: PlayerId = PlayerId(0);
const PAD: CardId = MAMMOTH_GRAVEYARD;

fn bot(shared: Arc<Shared>, lookahead: bool) -> HeuristicPlayer {
    let b = bundle();
    let values = CardValues::load(&root().join("config/card_values.toml"), &b.cards).unwrap();
    let styles = Styles::load(&root().join("config/styles.toml"), &b.cards).unwrap();
    HeuristicPlayer::new(
        shared,
        &b.cards,
        &values,
        styles.get("beatdown").unwrap(),
        1,
    )
    .with_lookahead(lookahead)
}

fn pick(bot: &mut HeuristicPlayer, g: &Game) -> Action {
    bot.choose(&PlayerView::new(g).expect("a decision is pending"))
}

fn activates(g: &Game, a: &Action, id: CardId) -> bool {
    matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id)
}

#[test]
fn does_not_cast_dark_hole_on_its_own_board() {
    let shared = shared_from(ygo_cards::registry());
    let mut g = Game::new(
        shared.clone(),
        &stacked([&[CELTIC_GUARDIAN, DARK_HOLE], &[]], PAD),
    )
    .unwrap();
    let celtic = in_hand(&g, P0, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    assert!(
        activates(&g, &pick(&mut bot(shared.clone(), false), &g), DARK_HOLE),
        "the plain bot casts it: a card worth 80 scores high whatever it hits"
    );
    let choice = pick(&mut bot(shared, true), &g);
    assert!(!activates(&g, &choice, DARK_HOLE), "{choice:?}");
}

#[test]
fn does_not_pay_its_last_life_points_to_keep_messenger_of_peace() {
    let base = shared_from(ygo_cards::registry());
    let shared = with_rules(&base, |r| r.life.starting_lp = 100);
    let mut g = Game::new(shared.clone(), &stacked([&[MESSENGER_OF_PEACE], &[]], PAD)).unwrap();
    let messenger = ygo_engine::testing::in_hand(&g, P0, MESSENGER_OF_PEACE);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == messenger))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    let d = g.pending_decision().unwrap();
    assert_eq!(
        (d.player, d.kind),
        (P0, DecisionKind::Choose),
        "pay 100 LP or destroy"
    );
    assert_eq!(
        pick(&mut bot(shared.clone(), false), &g),
        Action::Answer { option: 0 },
        "the plain bot pays, and loses"
    );
    assert_eq!(
        pick(&mut bot(shared, true), &g),
        Action::Answer { option: 1 }
    );
}

const RED_EYES: CardId = CardId(74677422);
const DARK_ASSAILANT: CardId = CardId(41949033);

// Prototype v9 "Discard plan off without a revive card": Tribute to the
// Doomed's discard is not Red-Eyes when nothing can revive it.
#[test]
fn keeps_red_eyes_without_a_revive_card() {
    use ygo_cards::spells::TRIBUTE_TO_THE_DOOMED;
    for lookahead in [false, true] {
        let shared = shared_from(ygo_cards::registry());
        let mut g = Game::new(
            shared.clone(),
            &stacked(
                [
                    &[TRIBUTE_TO_THE_DOOMED, RED_EYES, DARK_ASSAILANT],
                    &[CELTIC_GUARDIAN],
                ],
                PAD,
            ),
        )
        .unwrap();
        next_turn(&mut g);
        let celtic = in_hand(&g, ygo_engine::PlayerId(1), CELTIC_GUARDIAN);
        play(&mut g, summon(celtic));
        pass_all(&mut g);
        next_turn(&mut g);
        let mut b = bot(shared, lookahead);
        assert!(
            g.legal_actions().iter().any(|a| matches!(a,
                Action::Activate { cost, .. } if cost.len() == 1 && g.stats(cost[0]).id == RED_EYES)),
            "discarding Red-Eyes is offered"
        );
        let choice = pick(&mut b, &g);
        let discards_red_eyes = matches!(&choice,
            Action::Activate { cost, .. } if cost.iter().any(|&c| g.stats(c).id == RED_EYES));
        assert!(!discards_red_eyes, "lookahead {lookahead}: {choice:?}");
    }
}
