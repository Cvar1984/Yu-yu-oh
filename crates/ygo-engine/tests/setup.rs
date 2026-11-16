//! M1 step 1: creating a game from a setup.

mod common;

use std::collections::BTreeSet;

use common::{normal_deck, sample_deck, shared};
use ygo_data::CardId;
use ygo_engine::{DeckList, Game, GameSetup, InstanceId, Location, Phase, PlayerId, SetupError};

fn new_game(seed: u64) -> Game {
    let setup = GameSetup::new([normal_deck(40), normal_deck(40)], seed);
    Game::new(shared(), &setup).unwrap()
}

#[test]
fn opening_state_matches_the_rules() {
    let g = new_game(1);
    let rules = &g.shared().rules;
    for p in PlayerId::BOTH {
        let s = g.player(p);
        assert_eq!(s.hand.len(), rules.hand.opening_size);
        assert_eq!(s.deck.len(), 40 - rules.hand.opening_size);
        assert_eq!(s.lp, rules.life.starting_lp);
        assert_eq!(s.monster_zones.len(), rules.field.monster_zones);
        assert_eq!(s.spell_trap_zones.len(), rules.field.spell_trap_zones);
        assert!(s.graveyard.is_empty() && s.extra.is_empty());
    }
    assert_eq!(g.turn(), 1);
    assert_eq!(g.phase(), Phase::Draw);
    assert!(g.result().is_none());
}

#[test]
fn every_card_is_in_exactly_one_place() {
    let g = new_game(2);
    let mut seen = BTreeSet::new();
    for p in PlayerId::BOTH {
        let s = g.player(p);
        for (zone, ids) in [
            (Location::Deck, &s.deck),
            (Location::Hand, &s.hand),
            (Location::Graveyard, &s.graveyard),
            (Location::Extra, &s.extra),
        ] {
            for &id in ids {
                assert!(seen.insert(id), "{id:?} listed twice");
                let inst = g.instance(id);
                assert_eq!(inst.location, zone);
                assert_eq!(inst.owner, p);
                assert_eq!(inst.controller, p);
            }
        }
    }
    assert_eq!(seen.len(), 80);
    assert_eq!(g.instances().len(), 80);
}

#[test]
fn same_seed_gives_the_same_game_and_different_seeds_differ() {
    let a = new_game(7);
    let b = new_game(7);
    let c = new_game(8);
    let order = |g: &Game, p: PlayerId| -> Vec<InstanceId> { g.player(p).deck.clone() };
    assert_eq!(order(&a, PlayerId(0)), order(&b, PlayerId(0)));
    assert_eq!(order(&a, PlayerId(1)), order(&b, PlayerId(1)));
    assert_eq!(a.turn_player(), b.turn_player());
    assert_ne!(order(&a, PlayerId(0)), order(&c, PlayerId(0)));
}

#[test]
fn shuffling_changes_the_deck_order() {
    let g = new_game(3);
    let dealt: Vec<InstanceId> = g.player(PlayerId(0)).deck.clone();
    let mut sorted = dealt.clone();
    sorted.sort();
    assert_ne!(dealt, sorted, "a 35-card deck left in dealt order");
}

#[test]
fn first_player_is_chosen_by_the_seed_or_the_setup() {
    let picks: BTreeSet<PlayerId> = (0..32).map(|seed| new_game(seed).turn_player()).collect();
    assert_eq!(picks.len(), 2, "32 seeds never let one player start");

    for p in PlayerId::BOTH {
        let mut setup = GameSetup::new([normal_deck(40), normal_deck(40)], 5);
        setup.first_player = Some(p);
        assert_eq!(Game::new(shared(), &setup).unwrap().turn_player(), p);
    }
}

#[test]
fn sample_decks_load_including_effect_cards() {
    let setup = GameSetup::new(
        [
            sample_deck("owner_main_45.json"),
            sample_deck("umiiruka_water_40.json"),
        ],
        11,
    );
    let g = Game::new(shared(), &setup).unwrap();
    assert_eq!(g.player(PlayerId(0)).deck.len(), 40);
    assert_eq!(g.player(PlayerId(1)).deck.len(), 35);
}

#[test]
fn deck_size_is_checked_per_player() {
    let setup = GameSetup::new([normal_deck(40), normal_deck(39)], 1);
    assert_eq!(
        Game::new(shared(), &setup).unwrap_err(),
        SetupError::MainDeckSize {
            player: PlayerId(1),
            size: 39,
            min: 40,
            max: 60
        }
    );
}

#[test]
fn unknown_and_misplaced_cards_are_rejected() {
    let mut bad = normal_deck(40);
    bad.main[0] = CardId(1);
    let setup = GameSetup::new([bad, normal_deck(40)], 1);
    assert_eq!(
        Game::new(shared(), &setup).unwrap_err(),
        SetupError::UnknownCard {
            player: PlayerId(0),
            id: CardId(1)
        }
    );

    // Fusionist, the lowest Fusion Monster id in the pool.
    let fusion = CardId(1641882);
    let mut fusion_in_main = normal_deck(39);
    fusion_in_main.main.push(fusion);
    let setup = GameSetup::new([normal_deck(40), fusion_in_main], 1);
    assert!(matches!(
        Game::new(shared(), &setup).unwrap_err(),
        SetupError::WrongDeckList { player: PlayerId(1), id, .. } if id == fusion
    ));

    let mut extra_too_big = normal_deck(40);
    extra_too_big.extra = vec![fusion; 16];
    let setup = GameSetup::new([extra_too_big, normal_deck(40)], 1);
    assert_eq!(
        Game::new(shared(), &setup).unwrap_err(),
        SetupError::ExtraDeckSize {
            player: PlayerId(0),
            size: 16,
            max: 15
        }
    );
}

#[test]
fn extra_deck_cards_start_in_the_extra_zone() {
    let mut deck = normal_deck(40);
    deck.extra = vec![CardId(1641882); 3];
    let setup = GameSetup::new([deck, normal_deck(40)], 1);
    let g = Game::new(shared(), &setup).unwrap();
    let extra = &g.player(PlayerId(0)).extra;
    assert_eq!(extra.len(), 3);
    assert!(
        extra
            .iter()
            .all(|&id| g.instance(id).location == Location::Extra)
    );
}

#[test]
fn deck_list_expands_entry_counts() {
    let deck = ygo_data::Deck::load(&common::root().join("data/decks/owner_main_45.json")).unwrap();
    let list = DeckList::from(&deck);
    assert_eq!(list.main.len(), 45);
    assert!(list.extra.is_empty());
}

#[test]
fn a_game_clone_is_independent() {
    let a = new_game(4);
    let b = a.clone();
    assert_eq!(a.player(PlayerId(0)).hand, b.player(PlayerId(0)).hand);
    assert_eq!(a.instances(), b.instances());
}
