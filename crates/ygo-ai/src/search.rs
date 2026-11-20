//! The search bot: single-observer information-set Monte Carlo tree
//! search (Cowling, Powley and Whitehouse, 2012). Every iteration samples
//! the cards the bot cannot see ([`PlayerView::sample`]), walks one shared
//! tree with the moves legal in that sample, adds one move, plays on with
//! the heuristic bot on both sides for a few turns, scores the position
//! it reaches, and credits that score to every move on the way. The most
//! visited first move is played.

use std::time::Duration;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_engine::{Action, Game, GameResult, Location, PlayerId, PlayerView, Position};

use crate::{HeuristicPlayer, Player};

/// How hard the search bot thinks. A bot setting, not a rule.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchSettings {
    /// Playouts per decision.
    pub iterations: u32,
    /// Stop a decision early once this much time has passed. Unused in the
    /// browser, which has no clock; keep it `None` there.
    pub time_limit: Option<Duration>,
    /// Weight of exploration against the average result (UCB1).
    pub exploration: f64,
    /// A playout stops at the start of the turn this many turns after the
    /// one being decided, and the position is scored; 0 plays to the end.
    pub horizon_turns: u32,
    /// A playout still running after this many actions is scored as it stands.
    pub max_playout_actions: u32,
    /// Scoring an unfinished game: Life Points worth one point of card
    /// value (`card_values.toml` runs 0 to 100).
    pub lp_per_point: f64,
    /// A card in the hand is worth this share of its value on the field.
    pub hand_worth: f64,
    /// A lead of this many points scores about 0.73.
    pub score_scale: f64,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            iterations: 200,
            time_limit: None,
            exploration: 0.7,
            horizon_turns: 2,
            max_playout_actions: 4000,
            lp_per_point: 50.0,
            hand_worth: 0.5,
            score_scale: 40.0,
        }
    }
}

pub struct SearchPlayer {
    /// Plays both sides in the playouts.
    rollout: HeuristicPlayer,
    settings: SearchSettings,
    rng: ChaCha8Rng,
}

/// One move from a tree node, as some player made it in some samples.
struct Edge {
    player: PlayerId,
    action: Action,
    visits: u32,
    /// Iterations in which this move was legal when its node was reached.
    available: u32,
    /// Sum of scores for `player`, each from 0 (a loss) to 1 (a win).
    reward: f64,
    child: Option<usize>,
}

#[derive(Default)]
struct Node {
    edges: Vec<Edge>,
}

impl SearchPlayer {
    pub fn new(rollout: HeuristicPlayer, settings: SearchSettings, seed: u64) -> Self {
        Self {
            rollout,
            settings,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    pub fn settings(&self) -> &SearchSettings {
        &self.settings
    }

    /// Runs the search and returns each first move with its visits and
    /// average result, in the order of `view.actions()`.
    pub fn analyse(&mut self, view: &PlayerView) -> Vec<(Action, u32, f64)> {
        let mut tree = vec![Node::default()];
        let started = self.settings.time_limit.map(|_| std::time::Instant::now());
        for _ in 0..self.settings.iterations {
            if let (Some(limit), Some(t)) = (self.settings.time_limit, started)
                && t.elapsed() >= limit
            {
                break;
            }
            let sample = view.sample(self.rng.next_u64());
            self.iterate(&mut tree, sample);
        }
        let root = &tree[0];
        view.actions()
            .iter()
            .map(|a| {
                root.edges
                    .iter()
                    .find(|e| e.player == view.player() && &e.action == a)
                    .map_or((a.clone(), 0, 0.0), |e| {
                        (a.clone(), e.visits, e.reward / f64::from(e.visits.max(1)))
                    })
            })
            .collect()
    }

    /// One iteration: down the tree, one new move, a playout, the result
    /// back up the path.
    fn iterate(&mut self, tree: &mut Vec<Node>, mut g: Game) {
        let turn = g.turn();
        let mut node = 0;
        let mut path: Vec<(usize, usize)> = Vec::new();
        while let Some(decision) = g.pending_decision() {
            // A forced move is no choice: it gets no place in the tree.
            if g.actions().len() == 1 {
                let only = g.actions()[0].clone();
                g.apply(&only).expect("the only legal action");
                continue;
            }
            let player = decision.player;
            let mut legal = Vec::new();
            let mut untried = Vec::new();
            for action in g.actions() {
                match tree[node]
                    .edges
                    .iter()
                    .position(|e| e.player == player && &e.action == action)
                {
                    Some(i) => legal.push(i),
                    None => untried.push(action),
                }
            }
            for &i in &legal {
                tree[node].edges[i].available += 1;
            }
            if !untried.is_empty() {
                let pick = untried[self.index(untried.len())].clone();
                tree[node].edges.push(Edge {
                    player,
                    action: pick.clone(),
                    visits: 0,
                    available: 1,
                    reward: 0.0,
                    child: None,
                });
                path.push((node, tree[node].edges.len() - 1));
                g.apply(&pick).expect("legal in this sample");
                break;
            }
            let c = self.settings.exploration;
            let edges = &tree[node].edges;
            let best = legal
                .iter()
                .copied()
                .max_by(|&a, &b| ucb(&edges[a], c).total_cmp(&ucb(&edges[b], c)))
                .expect("a choice has two or more actions");
            path.push((node, best));
            let action = tree[node].edges[best].action.clone();
            g.apply(&action).expect("legal in this sample");
            node = match tree[node].edges[best].child {
                Some(child) => child,
                None => {
                    tree.push(Node::default());
                    let child = tree.len() - 1;
                    tree[node].edges[best].child = Some(child);
                    child
                }
            };
        }
        let first = self.playout(&mut g, turn);
        for (n, e) in path {
            let edge = &mut tree[n].edges[e];
            edge.visits += 1;
            edge.reward += if edge.player == PlayerId(0) {
                first
            } else {
                1.0 - first
            };
        }
    }

    /// Plays on with the heuristic bot on both sides until the horizon or
    /// the end, and returns the score for player 0.
    fn playout(&mut self, g: &mut Game, root_turn: u32) -> f64 {
        let horizon = match self.settings.horizon_turns {
            0 => u32::MAX,
            h => root_turn.saturating_add(h),
        };
        let mut bots = [
            self.rollout.fresh(self.rng.next_u64()),
            self.rollout.fresh(self.rng.next_u64()),
        ];
        let mut steps = 0;
        while g.result().is_none()
            && g.turn() < horizon
            && steps < self.settings.max_playout_actions
        {
            steps += 1;
            let action = if g.actions().len() == 1 {
                g.actions()[0].clone()
            } else {
                let view = PlayerView::new(g).expect("a running game has a decision");
                bots[view.player().index()].choose(&view)
            };
            g.apply(&action).expect("bots choose legal actions");
        }
        match g.result() {
            Some(GameResult::Win { winner, .. }) => {
                if winner == PlayerId(0) {
                    1.0
                } else {
                    0.0
                }
            }
            Some(GameResult::Draw { .. }) => 0.5,
            None => self.evaluate(g),
        }
    }

    /// How good an unfinished game is for player 0, from 0 to 1: Life
    /// Points and the worth of each side's cards (`card_values.toml`),
    /// cards on the field counting fully and cards in the hand by
    /// `hand_worth`.
    fn evaluate(&self, g: &Game) -> f64 {
        let set = &self.settings;
        let side = |p: PlayerId| -> f64 {
            let st = g.player(p);
            let mut worth = f64::from(st.lp) / set.lp_per_point;
            for (i, inst) in g.instances().iter().enumerate() {
                if inst.controller != p {
                    continue;
                }
                let value = self.rollout.card_value(inst.card);
                worth += match inst.location {
                    Location::MonsterZone(_) => {
                        // A monster's worth includes its ATK; count boosts too.
                        let id = ygo_engine::InstanceId(i as u8);
                        let boost = if inst.position == Position::FaceUpAttack {
                            f64::from(g.atk(id) - g.stats(id).atk) / set.lp_per_point
                        } else {
                            0.0
                        };
                        value + boost
                    }
                    Location::SpellTrapZone(_) | Location::FieldZone => value,
                    Location::Hand => value * set.hand_worth,
                    _ => 0.0,
                };
            }
            worth
        };
        let diff = side(PlayerId(0)) - side(PlayerId(1));
        1.0 / (1.0 + (-diff / set.score_scale).exp())
    }

    fn index(&mut self, len: usize) -> usize {
        (self.rng.next_u64() % len as u64) as usize
    }
}

/// UCB1 with availability counts in place of the parent's visits.
fn ucb(e: &Edge, c: f64) -> f64 {
    if e.visits == 0 {
        return f64::INFINITY;
    }
    let n = f64::from(e.visits);
    e.reward / n + c * (f64::from(e.available).ln() / n).sqrt()
}

impl Player for SearchPlayer {
    fn choose(&mut self, view: &PlayerView) -> Action {
        if view.actions().len() == 1 {
            return view.actions()[0].clone();
        }
        let analysis = self.analyse(view);
        let best = analysis
            .iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)))
            .expect("actions are never empty");
        if best.1 == 0 {
            // No time for a single playout: play as the heuristic bot would.
            return self.rollout.choose(view);
        }
        best.0.clone()
    }
}
