//! The league (M7, `docs/m7-plan.md`): decks played by the shared
//! network, rated with Glicko-2 generation after generation. Fixed agents
//! (the shipped decks) never change; the population's weakest leave and
//! mutants of the best take their place; a hall of fame keeps past
//! champions in play; an exploiter is built each generation to beat the
//! leader. An agent's deck never changes, so a pair of agents plays once
//! and its games are kept. The games come from an `Arena`, so tests can
//! run a league with the random bot.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use ygo_data::{Bundle, CardId, Deck, DeckEntry, SCHEMA_VERSION, check_deck};

use crate::glicko::{Rating, TAU};

pub const LEAGUE_SCHEMA_VERSION: u32 = 1;

/// Results of games between two decks, indexed by who went first (0: the
/// first deck), then [first deck wins, second deck wins, halves]. A draw
/// or a win by deck-out counts a half (owner, 2026-11-22: a deck should win
/// by playing).
pub type Results = [[u32; 3]; 2];

/// Plays games between two decks.
pub trait Arena {
    /// Plays `games` games of deck `a` against deck `b`, `a` first in the
    /// even-numbered ones; game `i` uses seed `seed + i`.
    fn play(&self, a: &Deck, b: &Deck, games: u32, seed: u64) -> Result<Results, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A shipped deck: never mutates, never leaves.
    Fixed,
    /// A mutant (or a seed copy of a shipped deck) that may be eliminated.
    Population,
    /// Built to beat the leader of its generation.
    Exploiter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub id: u32,
    pub kind: Kind,
    /// The shipped deck it descends from (its file name without `.json`).
    pub lineage: String,
    pub parent: Option<u32>,
    /// The generation that made it (0 for the first members).
    pub born: u32,
    /// The generation it left the league in, if it has.
    pub left: Option<u32>,
    pub rating: Rating,
    pub deck: Deck,
}

impl Agent {
    pub fn alive(&self) -> bool {
        self.left.is_none()
    }

    pub fn name(&self) -> String {
        match self.kind {
            Kind::Fixed => self.lineage.clone(),
            _ => format!("{} #{}", self.lineage, self.id),
        }
    }
}

/// The games two agents played, in the generation they played them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub a: u32,
    pub b: u32,
    pub generation: u32,
    pub results: Results,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Games each pair plays (each deck first in half).
    pub games: u32,
    /// Living population agents (exploiters not counted).
    pub population: usize,
    pub hall_size: usize,
    /// Population agents removed each generation.
    pub eliminated: usize,
    pub exploiter_candidates: usize,
    /// Games each exploiter candidate plays against the leader.
    pub exploiter_games: u32,
    pub max_exploiters: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            games: 20,
            population: 13,
            hall_size: 8,
            eliminated: 2,
            exploiter_candidates: 6,
            exploiter_games: 40,
            max_exploiters: 2,
        }
    }
}

/// What a generation did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub generation: u32,
    /// Games played between agents, and in the exploiter search.
    pub games: u32,
    pub exploiter_games: u32,
    pub leader: u32,
    /// The agent that joined the hall of fame, if one did.
    pub joined_hall: Option<u32>,
    pub eliminated: Vec<u32>,
    pub born: Vec<u32>,
    pub exploiter: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct League {
    pub schema_version: u32,
    pub seed: u64,
    /// Generations completed.
    pub generation: u32,
    pub settings: Settings,
    /// Every agent ever made, its id its index; those that left too.
    pub agents: Vec<Agent>,
    /// Hall of fame, oldest first.
    pub hall: Vec<u32>,
    pub pairs: Vec<Pair>,
    pub notes: Vec<Note>,
}

/// SplitMix64: seeds from seeds.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn pick(rng: &mut ChaCha8Rng, n: usize) -> usize {
    (rng.next_u64() % n as u64) as usize
}

/// The first deck's score: wins count 1, halves a half.
pub fn score(r: &Results) -> f64 {
    let n: u32 = r.iter().flatten().sum();
    let won = f64::from(r[0][0] + r[1][0]) + f64::from(r[0][2] + r[1][2]) / 2.0;
    if n == 0 { 0.5 } else { won / f64::from(n) }
}

/// Main Deck cards of the pool, in id order.
fn main_deck_cards(bundle: &Bundle) -> Vec<CardId> {
    let mut ids: Vec<CardId> = bundle
        .cards
        .iter()
        .filter(|c| !c.card_type.is_extra_deck())
        .map(|c| c.id)
        .collect();
    ids.sort();
    ids
}

/// `deck` with `swaps` Main Deck copies replaced: each swap takes out one
/// copy and puts in another card of the pool (half the time another copy
/// of a card already in the deck) under the copy limits. `None` if the
/// result is not legal (cards, ban list, deck rules).
pub fn mutate(
    deck: &Deck,
    name: String,
    bundle: &Bundle,
    swaps: usize,
    rng: &mut ChaCha8Rng,
) -> Option<Deck> {
    let pool = main_deck_cards(bundle);
    let default_limit = bundle.rules.deck.default_copy_limit;
    let mut main: BTreeMap<CardId, usize> = BTreeMap::new();
    for e in &deck.main {
        *main.entry(e.id).or_default() += e.count;
    }
    let extra = |id: CardId| {
        deck.extra
            .iter()
            .filter(|e| e.id == id)
            .map(|e| e.count)
            .sum::<usize>()
    };
    let mut done = 0;
    for _ in 0..swaps * 50 {
        if done == swaps {
            break;
        }
        let copies: Vec<CardId> = main
            .iter()
            .flat_map(|(&id, &n)| std::iter::repeat_n(id, n))
            .collect();
        let out = copies[pick(rng, copies.len())];
        let inn = if rng.next_u64().is_multiple_of(2) {
            let present: Vec<CardId> = main.keys().copied().collect();
            present[pick(rng, present.len())]
        } else {
            pool[pick(rng, pool.len())]
        };
        if inn == out {
            continue;
        }
        let limit = bundle.banlist.limit_for(inn, default_limit);
        if main.get(&inn).copied().unwrap_or(0) + extra(inn) + 1 > limit {
            continue;
        }
        *main.get_mut(&out).expect("a copy in the deck") -= 1;
        if main[&out] == 0 {
            main.remove(&out);
        }
        *main.entry(inn).or_default() += 1;
        done += 1;
    }
    let entry = |id: CardId, count: usize| DeckEntry {
        id,
        name: bundle.cards.get(id).map(|c| c.name.clone()),
        count,
    };
    let mutant = Deck {
        schema_version: SCHEMA_VERSION,
        name,
        // Any card of the pool, like the enemy decks.
        played_by: Some("enemy".to_string()),
        notes: None,
        main: main.into_iter().map(|(id, n)| entry(id, n)).collect(),
        extra: deck.extra.clone(),
    };
    let legal = check_deck(&mutant, &bundle.cards, &bundle.banlist, &bundle.rules, None).is_empty();
    (done == swaps && legal).then_some(mutant)
}

/// A legal mutant with 1 to 3 swaps (the deck itself if none is found).
fn mutant_of(deck: &Deck, name: String, bundle: &Bundle, rng: &mut ChaCha8Rng) -> Deck {
    for _ in 0..20 {
        let swaps = 1 + pick(rng, 3);
        if let Some(d) = mutate(deck, name.clone(), bundle, swaps, rng) {
            return d;
        }
    }
    Deck {
        name,
        ..deck.clone()
    }
}

/// One agent's line in the table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub in_hall: bool,
    pub rating: Rating,
    /// Games, score; as the first player; as the second.
    pub games: u32,
    pub score: f64,
    pub first: (u32, f64),
    pub second: (u32, f64),
}

impl League {
    /// A new league: each shipped deck (lineage name, deck) as a fixed
    /// agent, and `settings.population` agents seeded from them by one
    /// swap each.
    pub fn new(
        shipped: Vec<(String, Deck)>,
        settings: Settings,
        seed: u64,
        bundle: &Bundle,
    ) -> Self {
        let mut league = League {
            schema_version: LEAGUE_SCHEMA_VERSION,
            seed,
            generation: 0,
            settings,
            agents: Vec::new(),
            hall: Vec::new(),
            pairs: Vec::new(),
            notes: Vec::new(),
        };
        let n = shipped.len();
        for (lineage, deck) in shipped {
            league.add(Kind::Fixed, lineage, None, 0, deck);
        }
        let mut rng = ChaCha8Rng::seed_from_u64(mix(seed ^ 0x5EED));
        for i in 0..league.settings.population {
            let parent = &league.agents[i % n];
            let (lineage, parent_id) = (parent.lineage.clone(), parent.id);
            let id = league.agents.len() as u32;
            let deck = (0..20)
                .find_map(|_| {
                    mutate(
                        &parent.deck,
                        format!("{lineage} #{id}"),
                        bundle,
                        1,
                        &mut rng,
                    )
                })
                .unwrap_or_else(|| parent.deck.clone());
            league.add(Kind::Population, lineage, Some(parent_id), 0, deck);
        }
        league
    }

    fn add(
        &mut self,
        kind: Kind,
        lineage: String,
        parent: Option<u32>,
        born: u32,
        deck: Deck,
    ) -> u32 {
        let id = self.agents.len() as u32;
        self.agents.push(Agent {
            id,
            kind,
            lineage,
            parent,
            born,
            left: None,
            rating: Rating::default(),
            deck,
        });
        id
    }

    pub fn alive(&self) -> impl Iterator<Item = &Agent> {
        self.agents.iter().filter(|a| a.alive())
    }

    fn by_rating(&self, filter: impl Fn(&Agent) -> bool) -> Vec<u32> {
        let mut ids: Vec<u32> = self.alive().filter(|a| filter(a)).map(|a| a.id).collect();
        ids.sort_by(|&x, &y| {
            let (a, b) = (&self.agents[x as usize], &self.agents[y as usize]);
            b.rating
                .conservative()
                .total_cmp(&a.rating.conservative())
                .then(x.cmp(&y))
        });
        ids
    }

    /// The best living agent of any kind.
    pub fn leader(&self) -> u32 {
        self.by_rating(|_| true)[0]
    }

    fn pair_seed(&self, a: u32, b: u32) -> u64 {
        mix(self.seed ^ mix((u64::from(a) << 32) | u64::from(b)))
    }

    /// Runs one generation; see the module and `docs/m7-plan.md`.
    pub fn run_generation(&mut self, arena: &dyn Arena, bundle: &Bundle) -> Result<Note, String> {
        let g = self.generation;
        let mut rng = ChaCha8Rng::seed_from_u64(mix(self.seed ^ mix(u64::from(g) + 1)));

        // 1. Every living pair that has not played yet plays.
        let played: BTreeSet<(u32, u32)> = self.pairs.iter().map(|p| (p.a, p.b)).collect();
        let alive: Vec<u32> = self.alive().map(|a| a.id).collect();
        let mut games = 0;
        for (i, &a) in alive.iter().enumerate() {
            for &b in &alive[i + 1..] {
                if played.contains(&(a, b)) {
                    continue;
                }
                let n = self.settings.games;
                let (da, db) = (&self.agents[a as usize].deck, &self.agents[b as usize].deck);
                let results = arena.play(da, db, n, self.pair_seed(a, b))?;
                self.pairs.push(Pair {
                    a,
                    b,
                    generation: g,
                    results,
                });
                games += n;
            }
        }

        // 2. Ratings: one Glicko-2 period over this generation's games.
        let before: Vec<Rating> = self.agents.iter().map(|a| a.rating).collect();
        let mut period: BTreeMap<u32, Vec<(Rating, f64)>> = BTreeMap::new();
        for p in self.pairs.iter().filter(|p| p.generation == g) {
            let r = &p.results;
            let (wa, wb, half) = (r[0][0] + r[1][0], r[0][1] + r[1][1], r[0][2] + r[1][2]);
            for (me, opp, won, lost) in [(p.a, p.b, wa, wb), (p.b, p.a, wb, wa)] {
                let list = period.entry(me).or_default();
                let o = before[opp as usize];
                list.extend(std::iter::repeat_n((o, 1.0), won as usize));
                list.extend(std::iter::repeat_n((o, 0.0), lost as usize));
                list.extend(std::iter::repeat_n((o, 0.5), half as usize));
            }
        }
        for id in alive.iter().copied() {
            let list = period.get(&id).map(Vec::as_slice).unwrap_or(&[]);
            self.agents[id as usize].rating = before[id as usize].update(list, TAU);
        }

        // 3. The best non-fixed agent joins the hall of fame.
        let leader = self.leader();
        let mutable = self.by_rating(|a| a.kind != Kind::Fixed);
        let joined_hall = mutable
            .first()
            .copied()
            .filter(|id| !self.hall.contains(id));
        if let Some(id) = joined_hall {
            self.hall.push(id);
            if self.hall.len() > self.settings.hall_size {
                self.hall.remove(0);
            }
        }

        // 4. The weakest population agents leave (not the hall). Every
        // style stays in the league through its fixed deck.
        let mut eliminated = Vec::new();
        for &id in mutable.iter().rev() {
            if eliminated.len() == self.settings.eliminated {
                break;
            }
            if self.agents[id as usize].kind == Kind::Population && !self.hall.contains(&id) {
                eliminated.push(id);
            }
        }
        for &id in &eliminated {
            self.agents[id as usize].left = Some(g);
        }

        // 5. Mutants of the best fill the population again; parents come
        // from the four best agents of different lineages, so the population
        // does not collapse onto one style.
        let mut born = Vec::new();
        loop {
            let population = self.alive().filter(|a| a.kind == Kind::Population).count();
            if population >= self.settings.population {
                break;
            }
            let mut top: Vec<u32> = Vec::new();
            for id in self.by_rating(|a| a.kind != Kind::Fixed) {
                let lineage = &self.agents[id as usize].lineage;
                if top.len() < 4
                    && top
                        .iter()
                        .all(|&t| &self.agents[t as usize].lineage != lineage)
                {
                    top.push(id);
                }
            }
            let parent = &self.agents[top[pick(&mut rng, top.len())] as usize];
            let (lineage, parent_id) = (parent.lineage.clone(), parent.id);
            let name = format!("{lineage} #{}", self.agents.len());
            let deck = mutant_of(&parent.deck, name, bundle, &mut rng);
            born.push(self.add(Kind::Population, lineage, Some(parent_id), g, deck));
        }

        // 6. An exploiter: mutants of a top-half agent, kept only if one
        // scores better against the leader than its parent does (same seeds).
        let mut exploiter_games = 0;
        let mut exploiter = None;
        let bases: Vec<u32> = mutable
            .iter()
            .copied()
            .filter(|&id| id != leader && !born.contains(&id))
            .collect();
        if self.settings.exploiter_candidates > 0 && !bases.is_empty() {
            let base = self.agents[bases[pick(&mut rng, bases.len().div_ceil(2))] as usize].clone();
            let target = self.agents[leader as usize].deck.clone();
            let (n, seed) = (
                self.settings.exploiter_games,
                mix(self.seed ^ 0xE ^ mix(u64::from(g))),
            );
            let parent_score = score(&arena.play(&base.deck, &target, n, seed)?);
            exploiter_games += n;
            let mut best: Option<(f64, Deck)> = None;
            for _ in 0..self.settings.exploiter_candidates {
                let name = format!("{} #{}", base.lineage, self.agents.len());
                let cand = mutant_of(&base.deck, name, bundle, &mut rng);
                let s = score(&arena.play(&cand, &target, n, seed)?);
                exploiter_games += n;
                if best.as_ref().is_none_or(|(b, _)| s > *b) {
                    best = Some((s, cand));
                }
            }
            if let Some((_, deck)) = best.filter(|(s, _)| *s > parent_score) {
                let id = self.add(
                    Kind::Exploiter,
                    base.lineage.clone(),
                    Some(base.id),
                    g,
                    deck,
                );
                exploiter = Some(id);
                // Too many exploiters: the oldest outside the hall leaves.
                let mut alive_exploiters: Vec<u32> = self
                    .alive()
                    .filter(|a| a.kind == Kind::Exploiter && !self.hall.contains(&a.id))
                    .map(|a| a.id)
                    .collect();
                alive_exploiters.sort();
                while alive_exploiters.len() > self.settings.max_exploiters {
                    let old = alive_exploiters.remove(0);
                    self.agents[old as usize].left = Some(g);
                    eliminated.push(old);
                }
            }
        }

        let note = Note {
            generation: g,
            games,
            exploiter_games,
            leader,
            joined_hall,
            eliminated,
            born,
            exploiter,
        };
        self.notes.push(note.clone());
        self.generation += 1;
        Ok(note)
    }

    /// The living agents, best first, with their records.
    pub fn table(&self) -> Vec<Row> {
        let mut acc: BTreeMap<u32, [(u32, f64); 2]> = BTreeMap::new();
        for p in &self.pairs {
            let r = &p.results;
            // [as first, as second] for each side: (games, points).
            let a_first = (
                r[0].iter().sum::<u32>(),
                f64::from(r[0][0]) + f64::from(r[0][2]) / 2.0,
            );
            let a_second = (
                r[1].iter().sum::<u32>(),
                f64::from(r[1][0]) + f64::from(r[1][2]) / 2.0,
            );
            let b_first = (
                r[1].iter().sum::<u32>(),
                f64::from(r[1][1]) + f64::from(r[1][2]) / 2.0,
            );
            let b_second = (
                r[0].iter().sum::<u32>(),
                f64::from(r[0][1]) + f64::from(r[0][2]) / 2.0,
            );
            for (id, first, second) in [(p.a, a_first, a_second), (p.b, b_first, b_second)] {
                let e = acc.entry(id).or_insert([(0, 0.0); 2]);
                e[0].0 += first.0;
                e[0].1 += first.1;
                e[1].0 += second.0;
                e[1].1 += second.1;
            }
        }
        let rate = |(n, pts): (u32, f64)| (n, if n == 0 { 0.5 } else { pts / f64::from(n) });
        self.by_rating(|_| true)
            .into_iter()
            .map(|id| {
                let a = &self.agents[id as usize];
                let [f, s] = acc.get(&id).copied().unwrap_or([(0, 0.0); 2]);
                let (games, score) = rate((f.0 + s.0, f.1 + s.1));
                Row {
                    id,
                    name: a.name(),
                    kind: a.kind,
                    in_hall: self.hall.contains(&id),
                    rating: a.rating,
                    games,
                    score,
                    first: rate(f),
                    second: rate(s),
                }
            })
            .collect()
    }

    /// The table in Markdown: rank, agent, rating ±2 deviations, and
    /// scores with 95% intervals overall, going first and going second.
    pub fn render_table(&self) -> String {
        let ci = |n: u32, p: f64| {
            if n == 0 {
                0.0
            } else {
                1.96 * (p * (1.0 - p) / f64::from(n)).sqrt()
            }
        };
        let pct = |(n, p): (u32, f64)| format!("{:.1}% ±{:.1}", 100.0 * p, 100.0 * ci(n, p));
        let mut out = String::from(
            "| # | Agent | Kind | Rating | Games | Score | First | Second |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n",
        );
        for (rank, r) in self.table().iter().enumerate() {
            let kind = match r.kind {
                Kind::Fixed => "fixed",
                Kind::Population => "population",
                Kind::Exploiter => "exploiter",
            };
            out.push_str(&format!(
                "| {} | {}{} | {} | {:.0} ±{:.0} | {} | {} | {} | {} |\n",
                rank + 1,
                r.name,
                if r.in_hall { " (hall)" } else { "" },
                kind,
                r.rating.rating,
                2.0 * r.rating.deviation,
                r.games,
                pct((r.games, r.score)),
                pct(r.first),
                pct(r.second),
            ));
        }
        out
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let league: League =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if league.schema_version != LEAGUE_SCHEMA_VERSION {
            return Err(format!(
                "{}: league schema {} (this build reads {LEAGUE_SCHEMA_VERSION})",
                path.display(),
                league.schema_version
            ));
        }
        Ok(league)
    }

    /// Writes the league (to a temporary file first, then renamed, so a
    /// stopped run never leaves half a file).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }
}
