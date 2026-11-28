//! The league (M7) with the random bot: mutants are legal, fixed decks
//! never change or leave, every lineage keeps a living agent, and a league
//! stopped and resumed is the same as one run straight through.

use std::sync::Arc;

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use ygo_ai::{Player, RandomPlayer, play_game};
use ygo_data::{Bundle, Deck, check_deck};
use ygo_engine::testing::{bundle, root, shared_from};
use ygo_engine::{DeckList, GameResult, GameSetup, PlayerId, Shared, WinReason};
use ygo_train::league::{Arena, Kind, League, Results, Settings, mutate};

struct RandomArena {
    shared: Arc<Shared>,
}

impl Arena for RandomArena {
    fn play(&self, a: &Deck, b: &Deck, games: u32, seed: u64) -> Result<Results, String> {
        let mut r: Results = [[0; 3]; 2];
        for i in 0..games {
            let mut setup = GameSetup::new(
                [DeckList::from(a), DeckList::from(b)],
                seed.wrapping_add(u64::from(i)),
            );
            let first = (i % 2) as u8;
            setup.first_player = Some(PlayerId(first));
            let mut pa = RandomPlayer::new(setup.seed ^ 0xA);
            let mut pb = RandomPlayer::new(setup.seed ^ 0xB);
            let mut players: [&mut dyn Player; 2] = [&mut pa, &mut pb];
            let (_, record) = play_game(
                self.shared.clone(),
                &setup,
                &mut players,
                ["a".into(), "b".into()],
            )
            .map_err(|e| e.to_string())?;
            let slot = match record.result.expect("a finished game") {
                GameResult::Win {
                    reason: WinReason::DeckOut,
                    ..
                }
                | GameResult::Draw { .. } => 2,
                GameResult::Win { winner, .. } => winner.index(),
            };
            r[usize::from(first)][slot] += 1;
        }
        Ok(r)
    }
}

fn shipped() -> Vec<(String, Deck)> {
    let mut files: Vec<_> = std::fs::read_dir(root().join("data/decks"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                Deck::load(&p).unwrap(),
            )
        })
        .collect()
}

fn small() -> Settings {
    Settings {
        games: 2,
        population: 5,
        hall_size: 3,
        eliminated: 2,
        exploiter_candidates: 2,
        exploiter_games: 2,
        max_exploiters: 1,
    }
}

fn legal(deck: &Deck, b: &Bundle) -> bool {
    check_deck(deck, &b.cards, &b.banlist, &b.rules, None).is_empty()
}

#[test]
fn mutants_are_legal_and_change_only_their_swaps() {
    let b = bundle();
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let mut made = 0;
    for (_, deck) in shipped() {
        for swaps in 1..=3 {
            for _ in 0..10 {
                let Some(m) = mutate(&deck, "m".into(), &b, swaps, &mut rng) else {
                    continue;
                };
                made += 1;
                assert!(
                    legal(&m, &b),
                    "{:?}",
                    check_deck(&m, &b.cards, &b.banlist, &b.rules, None)
                );
                assert_eq!(m.main_size(), deck.main_size());
                assert_eq!(m.extra, deck.extra, "the Extra Deck stays");
                // Copies that differ: at most one out and one in per swap.
                let (old, new) = (deck.copies(), m.copies());
                let gone: usize = old
                    .iter()
                    .map(|(id, &n)| n.saturating_sub(new.get(id).copied().unwrap_or(0)))
                    .sum();
                assert!(
                    gone >= 1 && gone <= swaps,
                    "{gone} copies changed for {swaps} swaps"
                );
            }
        }
    }
    assert!(made > 300, "most mutations succeed: {made}");
}

#[test]
fn a_league_keeps_its_rules_over_generations() {
    let b = bundle();
    let arena = RandomArena {
        shared: shared_from(ygo_cards::registry()),
    };
    let decks = shipped();
    let mut league = League::new(decks.clone(), small(), 11, &b);
    assert_eq!(league.agents.len(), decks.len() + 5);
    for _ in 0..4 {
        let note = league.run_generation(&arena, &b).unwrap();
        assert!(note.games > 0);
        // Fixed decks: all alive, unchanged.
        for (i, (lineage, deck)) in decks.iter().enumerate() {
            let a = &league.agents[i];
            assert_eq!(
                (a.kind, &a.lineage, &a.deck, a.alive()),
                (Kind::Fixed, lineage, deck, true)
            );
        }
        // The population is full again, every deck legal.
        assert_eq!(
            league
                .alive()
                .filter(|a| a.kind == Kind::Population)
                .count(),
            5
        );
        assert!(league.alive().filter(|a| a.kind == Kind::Exploiter).count() <= 1);
        for a in league.alive() {
            assert!(legal(&a.deck, &b), "{}", a.name());
        }
        // The two weakest population agents left, two mutants were born.
        let left_population = note
            .eliminated
            .iter()
            .filter(|&&id| league.agents[id as usize].kind == Kind::Population)
            .count();
        assert_eq!((left_population, note.born.len()), (2, 2), "{note:?}");
        for &id in &note.eliminated {
            assert!(!league.hall.contains(&id), "a hall member never leaves");
        }
        // The hall holds only non-fixed agents.
        assert!(
            league
                .hall
                .iter()
                .all(|&id| league.agents[id as usize].kind != Kind::Fixed)
        );
        // Every living pair has played, once.
        let alive: Vec<u32> = league.alive().map(|a| a.id).collect();
        let played = |x: u32, y: u32| {
            league
                .pairs
                .iter()
                .filter(|p| (p.a, p.b) == (x.min(y), x.max(y)))
                .count()
        };
        for (i, &x) in alive.iter().enumerate() {
            for &y in &alive[i + 1..] {
                let born_now = note.born.contains(&x)
                    || note.born.contains(&y)
                    || note.exploiter == Some(x)
                    || note.exploiter == Some(y);
                assert_eq!(played(x, y), usize::from(!born_now), "{x} against {y}");
            }
        }
    }
    let table = league.render_table();
    assert_eq!(table.lines().count(), 2 + league.alive().count());
}

#[test]
fn a_resumed_league_is_the_same_as_one_run_straight_through() {
    let b = bundle();
    let arena = RandomArena {
        shared: shared_from(ygo_cards::registry()),
    };
    let mut straight = League::new(shipped(), small(), 23, &b);
    for _ in 0..3 {
        straight.run_generation(&arena, &b).unwrap();
    }
    let dir = std::env::temp_dir().join(format!("ygo-league-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("league.json");
    let mut resumed = League::new(shipped(), small(), 23, &b);
    resumed.run_generation(&arena, &b).unwrap();
    resumed.save(&path).unwrap();
    let mut resumed = League::load(&path).unwrap();
    for _ in 0..2 {
        resumed.run_generation(&arena, &b).unwrap();
        resumed.save(&path).unwrap();
        resumed = League::load(&path).unwrap();
    }
    assert_eq!(resumed.generation, 3);
    assert_eq!(
        serde_json::to_string(&resumed).unwrap(),
        serde_json::to_string(&straight).unwrap()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
