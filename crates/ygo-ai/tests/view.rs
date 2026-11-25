//! `PlayerView::sample`: a sampled game looks exactly like the real one
//! to the decider, keeps every card with its owner, and redeals what the
//! decider cannot see.

use std::collections::BTreeMap;
use std::sync::Arc;

use ygo_ai::{Player, RandomPlayer};
use ygo_engine::testing::{sample_deck, shared_from};
use ygo_engine::{CardIndex, Game, GameSetup, Location, PlayerView, Shared};

fn shared() -> Arc<Shared> {
    shared_from(ygo_cards::registry())
}

fn new_game(seed: u64) -> Game {
    let decks = [
        sample_deck("save_a_owner_current_40.json"),
        sample_deck("save_b_dark_plasma_40.json"),
    ];
    Game::new(shared(), &GameSetup::new(decks, seed)).unwrap()
}

/// Each player's cards, counted by card.
fn cards_by_owner(g: &Game) -> [BTreeMap<CardIndex, usize>; 2] {
    let mut out = [BTreeMap::new(), BTreeMap::new()];
    for inst in g.instances() {
        *out[inst.owner.index()].entry(inst.card).or_default() += 1;
    }
    out
}

#[test]
fn a_sample_looks_the_same_to_the_decider_and_keeps_every_card() {
    let mut decisions = 0;
    let mut redealt = 0;
    for seed in 0..30 {
        let mut g = new_game(seed);
        let mut bots = [RandomPlayer::new(seed), RandomPlayer::new(seed + 100)];
        while let Some(view) = PlayerView::new(&g) {
            decisions += 1;
            let sample = view.sample(seed * 1000 + decisions);
            assert_eq!(
                &sample.observe(view.player()),
                view.observation(),
                "seed {seed}: the decider tells the sample from the game"
            );
            assert_eq!(sample.actions(), view.actions(), "seed {seed}");
            assert_eq!(cards_by_owner(&sample), cards_by_owner(&g));
            if sample.instances() != g.instances() {
                redealt += 1;
            }
            // A card on the chain keeps its identity wherever it went, or
            // its link would resolve another card's effect.
            for link in g.chain() {
                assert_eq!(
                    sample.instance(link.card).card,
                    g.instance(link.card).card,
                    "seed {seed}: a chain card was redealt"
                );
            }
            for (a, b) in sample.instances().iter().zip(g.instances()) {
                assert_eq!(
                    (a.owner, a.location, a.position),
                    (b.owner, b.location, b.position),
                    "only identities move"
                );
                let stats = sample.shared().pool.get(a.card);
                match a.location {
                    Location::MonsterZone(_) => assert!(stats.is_monster()),
                    Location::SpellTrapZone(_) | Location::FieldZone => {
                        assert!(!stats.is_monster())
                    }
                    Location::Extra => assert!(stats.is_extra_deck()),
                    Location::Hand | Location::Deck => assert!(!stats.is_extra_deck()),
                    _ => {}
                }
            }
            let i = view.player().index();
            let action = bots[i].choose(&view);
            g.apply(&action).unwrap();
        }
    }
    assert!(
        redealt * 10 > decisions * 9,
        "{redealt} of {decisions} samples redealt"
    );
}

#[test]
fn samples_vary_with_the_seed_and_repeat_with_it() {
    let g = new_game(5);
    let view = PlayerView::new(&g).unwrap();
    let opp = view.player().opponent();
    let hand = |s: &Game| -> Vec<CardIndex> {
        s.player(opp)
            .hand
            .iter()
            .map(|&id| s.instance(id).card)
            .collect()
    };
    assert_eq!(view.sample(1), view.sample(1));
    let hands: std::collections::BTreeSet<_> = (0..20).map(|s| hand(&view.sample(s))).collect();
    assert!(hands.len() > 10, "{} distinct hands", hands.len());
}

#[test]
fn the_deciders_own_cards_stay_put_except_the_deck_order() {
    let g = new_game(9);
    let view = PlayerView::new(&g).unwrap();
    let me = view.player();
    let sample = view.sample(3);
    for &id in &g.player(me).hand {
        assert_eq!(sample.instance(id).card, g.instance(id).card);
    }
    let deck = |s: &Game| -> Vec<CardIndex> {
        s.player(me)
            .deck
            .iter()
            .map(|&id| s.instance(id).card)
            .collect()
    };
    assert_ne!(deck(&sample), deck(&g), "own deck order is unknown");
    let mut a = deck(&sample);
    let mut b = deck(&g);
    a.sort();
    b.sort();
    assert_eq!(a, b);
}

/// A random 40-card Main Deck from the whole pool, at most 3 copies each.
fn random_pool_deck(shared: &Shared, seed: u64) -> ygo_engine::DeckList {
    let pool = &shared.pool;
    let ids: Vec<_> = (0..pool.len() as u16)
        .map(|i| pool.get(CardIndex(i)))
        .filter(|s| s.is_main_deck())
        .map(|s| s.id)
        .collect();
    let mut x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut main = Vec::new();
    while main.len() < 40 {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let id = ids[(x >> 33) as usize % ids.len()];
        if main.iter().filter(|&&c| c == id).count() < 3 {
            main.push(id);
        }
    }
    ygo_engine::DeckList {
        main,
        extra: Vec::new(),
    }
}

#[test]
fn samples_of_whole_pool_games_keep_chain_cards_and_play_on() {
    let shared = shared();
    for seed in 0..40 {
        let decks = [
            random_pool_deck(&shared, seed),
            random_pool_deck(&shared, seed + 1000),
        ];
        let mut g = Game::new(shared.clone(), &GameSetup::new(decks, seed)).unwrap();
        let mut bots = [RandomPlayer::new(seed), RandomPlayer::new(seed + 7)];
        let mut decisions = 0;
        while let Some(view) = PlayerView::new(&g) {
            decisions += 1;
            let mut sample = view.sample(seed * 1000 + decisions);
            for link in g.chain() {
                assert_eq!(
                    sample.instance(link.card).card,
                    g.instance(link.card).card,
                    "seed {seed}: a chain card was redealt"
                );
            }
            // The sample plays on like a real game.
            let mut walker = RandomPlayer::new(decisions);
            for _ in 0..8 {
                let Some(v) = PlayerView::new(&sample) else {
                    break;
                };
                let a = walker.choose(&v);
                sample.apply(&a).unwrap();
            }
            let action = bots[view.player().index()].choose(&view);
            g.apply(&action).unwrap();
        }
    }
}

#[test]
fn a_face_down_monster_on_the_chain_keeps_its_identity_in_samples() {
    use ygo_cards::psv_monsters::KISEITAI;
    use ygo_cards::traps::{MYSTICAL_SPACE_TYPHOON as MST, TRAP_HOLE};
    use ygo_engine::testing::{
        CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, in_hand, next_turn, pass_all, play, stacked, summon,
    };
    use ygo_engine::{Action, DecisionKind, PlayerId};
    let (p0, p1) = (PlayerId(0), PlayerId(1));
    let mut g = Game::new(
        shared(),
        &stacked(
            [&[KISEITAI, TRAP_HOLE], &[CELTIC_GUARDIAN, MST]],
            MAMMOTH_GRAVEYARD,
        ),
    )
    .unwrap();
    let kiseitai = in_hand(&g, p0, KISEITAI);
    play(
        &mut g,
        Action::Summon {
            card: kiseitai,
            tributes: vec![],
            face_down: true,
        },
    );
    pass_all(&mut g);
    let hole = in_hand(&g, p0, TRAP_HOLE);
    play(&mut g, Action::SetSpellTrap { card: hole });
    pass_all(&mut g);
    next_turn(&mut g);
    let celtic = in_hand(&g, p1, CELTIC_GUARDIAN);
    play(&mut g, summon(celtic));
    pass_all(&mut g);
    let mst = in_hand(&g, p1, MST);
    play(&mut g, Action::SetSpellTrap { card: mst });
    pass_all(&mut g);
    next_turn(&mut g);
    next_turn(&mut g);
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: Some(kiseitai),
        },
    );
    // Kiseitai's trigger is on the chain while it is still face-down, and
    // the attacker may respond with Mystical Space Typhoon.
    while g.chain().is_empty() {
        let a = g.legal_actions()[0].clone();
        play(&mut g, a);
    }
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (p1, DecisionKind::Respond));
    assert_eq!(g.chain()[0].card, kiseitai);
    let view = PlayerView::new(&g).unwrap();
    for seed in 0..20 {
        let mut sample = view.sample(seed);
        assert_eq!(sample.instance(kiseitai).card, g.instance(kiseitai).card);
        while let Some(v) = PlayerView::new(&sample) {
            if sample.chain().is_empty() {
                break;
            }
            let a = RandomPlayer::new(seed).choose(&v);
            sample.apply(&a).unwrap();
        }
    }
}
