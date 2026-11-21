//! Helpers for scenario tests in this crate and in `ygo-cards`. Enabled by
//! the `test-support` feature; never part of a normal build.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ygo_data::{Bundle, CardId, CardKind, Deck, Rules};

use std::collections::BTreeSet;

use crate::{
    Action, DecisionKind, DeckList, Event, Game, GameSetup, InstanceId, Location, PlayerId,
    Position, Registry, Shared, VisibleCard,
};

/// The repository root, where `config/` and `data/` live.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn bundle() -> Bundle {
    Bundle::load(&root().join("config/rules.toml"), &root()).expect("committed data loads")
}

/// The committed rules and card pool with `registry`.
pub fn shared_from(registry: Registry) -> Arc<Shared> {
    let b = bundle();
    Arc::new(Shared::new(b.rules, &b.cards, registry))
}

/// The committed rules and card pool with no scripts at all.
pub fn shared() -> Arc<Shared> {
    shared_from(Registry::default())
}

/// `base` with one change to its rules, for config-flip tests.
pub fn with_rules(base: &Shared, change: impl FnOnce(&mut Rules)) -> Arc<Shared> {
    let mut rules = base.rules.clone();
    change(&mut rules);
    rules.validate().expect("changed rules stay valid");
    Arc::new(base.with_rules(rules))
}

/// The committed rules with one change applied and no scripts.
pub fn shared_with(change: impl FnOnce(&mut Rules)) -> Arc<Shared> {
    with_rules(&shared(), change)
}

pub fn sample_deck(name: &str) -> DeckList {
    DeckList::from(&Deck::load(&root().join("data/decks").join(name)).unwrap())
}

/// The lowest `n` Normal Monster ids, three copies each where needed:
/// a deck with nothing but vanilla monsters.
pub fn normal_deck(n: usize) -> DeckList {
    let shared = shared();
    let pool = &shared.pool;
    let mut ids: Vec<CardId> = (0..pool.len() as u16)
        .map(crate::CardIndex)
        .map(|i| pool.get(i))
        .filter(|c| c.kind == CardKind::Normal)
        .map(|c| c.id)
        .collect();
    ids.sort();
    let main = ids.iter().flat_map(|&id| [id, id, id]).take(n).collect();
    DeckList {
        main,
        extra: Vec::new(),
    }
}

/// Known cards for scenario tests. Stats are asserted in the engine's
/// summon and battle tests.
pub const MYSTICAL_ELF: CardId = CardId(15025844); // level 4, 800/2000
pub const SUMMONED_SKULL: CardId = CardId(70781052); // level 6, 2500/1200
pub const BLUE_EYES: CardId = CardId(89631139); // level 8, 3000/2500
pub const SANGAN: CardId = CardId(26202165); // Effect Monster, 1000/600
pub const SEVEN_COLORED_FISH: CardId = CardId(23771716); // level 4, 1800/800
pub const CELTIC_GUARDIAN: CardId = CardId(91152256); // level 4, 1400/1200
pub const MAMMOTH_GRAVEYARD: CardId = CardId(40374923); // level 3, 1200/800

/// An unshuffled game: each player's deck starts with `top` in draw
/// order and is padded to 40 with `pad`. Player 0 goes first.
pub fn stacked(top: [&[CardId]; 2], pad: CardId) -> GameSetup {
    let deck = |ids: &[CardId]| DeckList {
        main: ids
            .iter()
            .copied()
            .chain(std::iter::repeat(pad))
            .take(40)
            .collect(),
        extra: Vec::new(),
    };
    let mut setup = GameSetup::new([deck(top[0]), deck(top[1])], 0);
    setup.first_player = Some(PlayerId(0));
    setup.shuffle = false;
    setup
}

/// The hand card of `player` with this id, first copy.
pub fn in_hand(game: &Game, player: PlayerId, id: CardId) -> InstanceId {
    game.player(player)
        .hand
        .iter()
        .copied()
        .find(|&c| game.stats(c).id == id)
        .unwrap_or_else(|| panic!("{id} not in player {player}'s hand"))
}

/// Plays `action` for whoever must decide, panicking if it is illegal.
pub fn play(game: &mut Game, action: Action) -> Vec<Event> {
    game.apply(&action)
        .unwrap_or_else(|e| panic!("{action:?} on turn {} {:?}: {e}", game.turn(), game.phase()))
}

/// Answers a pending question (a position, "pay or destroy") with option
/// number `option`, then declines responses.
pub fn answer(game: &mut Game, option: u8) {
    play(game, Action::Answer { option });
    pass_all(game);
}

/// Declines every pending response until the next decision is not a
/// response window.
pub fn pass_all(game: &mut Game) {
    while game
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Respond)
    {
        play(game, Action::Pass);
    }
}

/// Ends the turn, declining responses and playing discards for any
/// hand-limit decision, until the next player's Main Phase.
pub fn next_turn(game: &mut Game) {
    pass_all(game);
    play(game, Action::EndTurn);
    loop {
        match game.pending_decision().map(|d| d.kind) {
            Some(DecisionKind::Respond) => {
                play(game, Action::Pass);
            }
            Some(DecisionKind::HandLimit) => {
                let a = game.legal_actions()[0].clone();
                play(game, a);
            }
            _ => break,
        };
    }
}

/// Ends turns until the game is over, choosing the first legal action for
/// any other decision. Returns the number of actions played.
pub fn end_turns_until_over(game: &mut Game) -> usize {
    let mut played = 0;
    while game.result().is_none() {
        let actions = game.legal_actions();
        let action = actions
            .iter()
            .find(|a| **a == Action::EndTurn)
            .unwrap_or(&actions[0])
            .clone();
        play(game, action);
        played += 1;
    }
    played
}

pub fn summon(card: InstanceId) -> Action {
    Action::Summon {
        card,
        tributes: Vec::new(),
        face_down: false,
    }
}

pub fn summons_offered(game: &Game) -> Vec<Action> {
    game.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Summon { .. }))
        .collect()
}

/// Every rule that must hold in any reachable state.
pub fn check_invariants(g: &Game) {
    let rules = &g.shared().rules;
    let mut seen = BTreeSet::new();
    for p in PlayerId::BOTH {
        let s = g.player(p);
        let zones = [
            (Location::Deck, s.deck.clone()),
            (Location::Hand, s.hand.clone()),
            (Location::Graveyard, s.graveyard.clone()),
            (Location::Extra, s.extra.clone()),
            (Location::Banished, s.banished.clone()),
        ];
        for (zone, ids) in zones {
            for id in ids {
                assert!(seen.insert(id), "{id:?} is in two places");
                let inst = g.instance(id);
                assert_eq!(inst.location, zone, "{id:?}");
                assert_eq!(inst.controller, p, "{id:?}");
                // Off-field cards are with their owner, except a card put
                // face-up into the opponent's Deck (Parasite Paracide) and
                // then drawn by them.
                let lent = inst.position == Position::FaceUp
                    && matches!(zone, Location::Deck | Location::Hand);
                assert!(
                    inst.owner == p || lent,
                    "{id:?} off-field cards are with their owner"
                );
            }
        }
        assert_eq!(s.monster_zones.len(), rules.field.monster_zones);
        assert_eq!(s.spell_trap_zones.len(), rules.field.spell_trap_zones);
        for (i, zone) in s.monster_zones.iter().enumerate() {
            if let Some(id) = zone {
                assert!(seen.insert(*id), "{id:?} is in two places");
                let inst = g.instance(*id);
                assert_eq!(inst.location, Location::MonsterZone(i as u8));
                assert_eq!(inst.controller, p);
                assert!(g.stats(*id).is_monster() || g.is_hat(*id), "{id:?}");
                assert_ne!(
                    inst.position,
                    Position::FaceDown,
                    "monsters use the Defense positions"
                );
                assert!(inst.turn_entered_field.is_some());
            }
        }
        for (i, zone) in s.spell_trap_zones.iter().enumerate() {
            if let Some(id) = zone {
                assert!(seen.insert(*id), "{id:?} is in two places");
                let inst = g.instance(*id);
                assert_eq!(inst.location, Location::SpellTrapZone(i as u8));
                assert_eq!(inst.controller, p);
                assert!(
                    !g.stats(*id).is_monster() || inst.equipped_to.is_some(),
                    "a monster in a Spell/Trap zone is an absorbed Equip"
                );
                assert!(matches!(
                    inst.position,
                    Position::FaceDown | Position::FaceUp
                ));
            }
        }
        if let Some(id) = s.field_zone {
            assert!(seen.insert(id), "{id:?} is in two places");
            let inst = g.instance(id);
            assert_eq!(inst.location, Location::FieldZone);
            assert_eq!(inst.controller, p);
            assert_eq!(g.stats(id).subtype, crate::SpellTrapType::Field);
            // Activated, or Set (modern: a Field Spell may be Set).
            assert!(matches!(
                inst.position,
                Position::FaceUp | Position::FaceDown
            ));
        }
        for id in s.spell_trap_zones.iter().flatten() {
            if let Some(to) = g.equip_target(*id) {
                assert!(
                    g.instance(to).location.on_field(),
                    "an Equip's monster is on the field"
                );
            }
        }
        for id in s.monsters() {
            assert!(g.atk(id) >= 0 && g.def(id) >= 0);
        }
        assert!(usize::from(s.summons_used) <= rules.summon.normal_or_tribute_per_turn);
    }
    assert_eq!(seen.len(), g.instances().len(), "every card is somewhere");
    assert!(g.turn() <= g.max_turns());
    let any_dead = PlayerId::BOTH.iter().any(|p| g.player(*p).lp == 0);
    assert_eq!(
        any_dead,
        matches!(
            g.result(),
            Some(crate::GameResult::Win {
                reason: crate::WinReason::LifePoints,
                ..
            }) | Some(crate::GameResult::Draw {
                reason: crate::DrawReason::BothLifePointsZero
            })
        )
    );
    assert_eq!(g.result().is_none(), g.pending_decision().is_some());
    if g.pending_decision().is_some_and(|d| {
        matches!(
            d.kind,
            DecisionKind::MainPhase | DecisionKind::BattlePhase | DecisionKind::HandLimit
        )
    }) {
        assert!(
            g.chain().is_empty(),
            "a chain resolves before the next open-state decision"
        );
        assert!(g.window().is_none());
    }
    assert_eq!(g.result().is_none(), !g.legal_actions().is_empty());

    for viewer in PlayerId::BOTH {
        let obs = g.observe(viewer);
        let opp = obs.opponent();
        if !g.hand_revealed(viewer.opponent()) {
            // Only a monster being summoned face-up is shown from the hand,
            // unless the hand is revealed (Ceremonial Bell, The Eye of Truth).
            let declared = g.pending_summon().filter(|p| !p.face_down).map(|p| p.card);
            assert!(
                opp.hand
                    .iter()
                    .all(|c| matches!(c, VisibleCard::Hidden { .. })
                        || Some(c.instance()) == declared),
                "only a declared face-up summon is visible in the opponent's hand"
            );
        }
        for zone in opp.monster_zones.iter().flatten() {
            let face_up = g.instance(zone.instance()).position.is_face_up();
            assert_eq!(matches!(zone, VisibleCard::Known { .. }), face_up);
        }
        assert!(
            obs.me()
                .hand
                .iter()
                .all(|c| matches!(c, VisibleCard::Known { .. }))
        );
    }
}
