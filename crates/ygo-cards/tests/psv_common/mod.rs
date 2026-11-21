//! Helpers shared by the Pharaoh's Servant rulings tests (psv_*.rs).
#![allow(dead_code)]

use std::sync::Arc;

use ygo_data::{CardId, Rules};
use ygo_engine::testing::{
    MAMMOTH_GRAVEYARD, in_hand, pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{Action, DecisionKind, Game, InstanceId, PlayerId, Shared};

pub const P0: PlayerId = PlayerId(0);
pub const P1: PlayerId = PlayerId(1);
pub const PAD: CardId = MAMMOTH_GRAVEYARD; // Level 3, 1200/800 EARTH Dinosaur
pub const KOUMORI_DRAGON: CardId = CardId(67724379); // Level 4 DARK Dragon
pub const CYBER_FALCON: CardId = CardId(30655537); // Level 4 WIND Machine, 1400 ATK
pub const GIROCHIN_KUWAGATA: CardId = CardId(84620194); // Level 4 Insect

/// Ten-card opening hands, five Normal Summons a turn and no hand limit
/// to speak of, so one turn can set up a board.
pub fn rules(r: &mut Rules) {
    r.summon.normal_or_tribute_per_turn = 5;
    r.hand.opening_size = 10;
    r.hand.max_size = 30;
}

pub fn shared() -> Arc<Shared> {
    with_rules(&shared_from(ygo_cards::registry()), rules)
}

pub fn game(p0: &[CardId], p1: &[CardId]) -> Game {
    Game::new(shared(), &stacked([p0, p1], PAD)).unwrap()
}

/// `hand` in the opening hand, then the pads drawn in this player's next
/// two Draw Phases, then `deck` in draw order.
pub fn with_deck(hand: &[CardId], deck: &[CardId]) -> Vec<CardId> {
    let mut cards = hand.to_vec();
    cards.resize(12, PAD);
    cards.extend_from_slice(deck);
    cards
}

pub fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

/// Activates card `id` (the first activation offered) and passes.
pub fn activate(g: &mut Game, id: CardId) {
    let a = activations(g, id)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{id} not offered: {:?}", g.legal_actions()));
    play(g, a);
    pass_all(g);
}

/// Activates card `id` with exactly these targets and passes.
pub fn activate_on(g: &mut Game, id: CardId, targets: &[InstanceId]) {
    let a = activations(g, id)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets: t, .. } if t == targets))
        .unwrap_or_else(|| panic!("{id} not offered on {targets:?}: {:?}", g.legal_actions()));
    play(g, a);
    pass_all(g);
}

/// Passes response windows until `player` may activate `id` (on `targets`
/// if given), then activates it and passes.
pub fn respond(g: &mut Game, player: PlayerId, id: CardId, targets: Option<&[InstanceId]>) {
    loop {
        let d = g
            .pending_decision()
            .unwrap_or_else(|| panic!("no decision before {id}"));
        assert_eq!(d.kind, DecisionKind::Respond, "waiting for {id}: {d:?}");
        let offer = activations(g, id).into_iter().find(|a| {
            matches!(a, Action::Activate { targets: t, .. } if targets.is_none_or(|x| x == t.as_slice()))
        });
        match offer {
            Some(a) if d.player == player => {
                play(g, a);
                pass_all(g);
                return;
            }
            _ => {
                play(g, Action::Pass);
            }
        }
    }
}

pub fn set_card(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    pass_all(g);
    card
}

pub fn summon_now(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

pub fn set_monster(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
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

pub fn flip_summon(g: &mut Game, card: InstanceId) {
    play(g, Action::FlipSummon { card });
    pass_all(g);
}

pub fn attack(g: &mut Game, attacker: InstanceId, target: Option<InstanceId>) {
    if g.legal_actions().contains(&Action::EnterBattlePhase) {
        play(g, Action::EnterBattlePhase);
        pass_all(g);
    }
    play(g, Action::Attack { attacker, target });
}

pub fn choose(g: &mut Game, card: InstanceId) {
    play(g, Action::Choose { card });
    pass_all(g);
}

pub fn answer(g: &mut Game, option: u8) {
    play(g, Action::Answer { option });
    pass_all(g);
}

pub fn lp(g: &Game, p: PlayerId) -> u32 {
    g.player(p).lp
}

pub fn in_deck(g: &Game, p: PlayerId, id: CardId) -> usize {
    g.deck(p).iter().filter(|&&c| g.stats(c).id == id).count()
}
