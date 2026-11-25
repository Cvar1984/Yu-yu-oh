//! The network-guided search (AlphaZero, Silver et al. 2018, on
//! single-observer information-set MCTS, Cowling, Powley and Whitehouse
//! 2012). Every iteration samples the cards the bot cannot see
//! ([`PlayerView::sample`]) and walks one shared tree, picking each move by
//! PUCT: its average value plus `exploration` times the network's prior
//! times sqrt(times it was available) / (1 + its visits). At a position
//! the tree has not reached yet, the network gives the priors of its moves
//! and the value of the position, and that value goes back up the path,
//! each move credited from its own player's side. A finished game counts 1
//! for the winner. No playouts: the network is the only judge.

use std::time::{Duration, Instant};

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_engine::{Action, Game, GameResult, PlayerId, PlayerView};

use crate::Player;
use crate::neural::NeuralPlayer;

/// How hard the search thinks. A bot setting, not a rule.
#[derive(Debug, Clone, PartialEq)]
pub struct MctsSettings {
    /// Network evaluations per decision.
    pub iterations: u32,
    /// Stop a decision early once this much time has passed. Unused in the
    /// browser, which has no clock; keep it `None` there.
    pub time_limit: Option<Duration>,
    /// Weight of the prior against the average value (PUCT).
    pub exploration: f64,
}

impl Default for MctsSettings {
    fn default() -> Self {
        Self {
            iterations: 100,
            time_limit: None,
            exploration: 0.75,
        }
    }
}

/// One move from a node, as some player made it in some samples.
struct Edge {
    player: PlayerId,
    action: Action,
    prior: f64,
    visits: u32,
    /// Iterations in which this move was legal when its node was reached.
    available: u32,
    /// Sum of values for `player`, each from -1 (a loss) to 1 (a win).
    value: f64,
    child: Option<usize>,
}

#[derive(Default)]
struct Node {
    edges: Vec<Edge>,
    /// The network has seen this position (in some sample).
    evaluated: bool,
}

pub struct MctsPlayer {
    net: NeuralPlayer,
    settings: MctsSettings,
    rng: ChaCha8Rng,
}

impl MctsPlayer {
    /// `net` gives the priors and values; its own choice (at its
    /// temperature) answers only when no iteration ran.
    pub fn new(net: NeuralPlayer, settings: MctsSettings, seed: u64) -> Self {
        Self {
            net,
            settings,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    pub fn settings(&self) -> &MctsSettings {
        &self.settings
    }

    /// Runs the search and returns each legal move with its visits and
    /// average value for the decider, in the order of `view.actions()`.
    pub fn analyse(&mut self, view: &PlayerView) -> Vec<(Action, u32, f64)> {
        let mut tree = vec![Node::default()];
        let started = Instant::now();
        for _ in 0..self.settings.iterations {
            if self
                .settings
                .time_limit
                .is_some_and(|limit| started.elapsed() >= limit)
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
                        (a.clone(), e.visits, e.value / f64::from(e.visits.max(1)))
                    })
            })
            .collect()
    }

    /// One iteration: down the tree by PUCT to a position the network has
    /// not judged or the end of the game, then the value back up the path.
    fn iterate(&mut self, tree: &mut Vec<Node>, mut g: Game) {
        let mut node = 0;
        let mut path: Vec<(usize, usize)> = Vec::new();
        let (judge, value) = loop {
            if let Some(result) = g.result() {
                break match result {
                    GameResult::Win { winner, .. } => (winner, 1.0),
                    GameResult::Draw { .. } => (PlayerId(0), 0.0),
                };
            }
            // A forced move is no choice: it gets no place in the tree.
            if g.actions().len() == 1 {
                let only = g.actions()[0].clone();
                g.apply(&only).expect("the only legal action");
                continue;
            }
            let view = PlayerView::new(&g).expect("a running game has a decision");
            let player = view.player();
            if !tree[node].evaluated {
                let (priors, value) = self.judge(&view);
                let n = &mut tree[node];
                n.evaluated = true;
                for (action, prior) in view.actions().iter().zip(priors) {
                    match n
                        .edges
                        .iter_mut()
                        .find(|e| e.player == player && &e.action == action)
                    {
                        Some(e) => e.prior = prior,
                        None => n.edges.push(new_edge(player, action, prior)),
                    }
                }
                break (player, f64::from(value));
            }
            // Moves legal in this sample that no earlier sample had.
            let fresh = 1.0 / view.actions().len() as f64;
            for action in view.actions() {
                if !tree[node]
                    .edges
                    .iter()
                    .any(|e| e.player == player && &e.action == action)
                {
                    tree[node].edges.push(new_edge(player, action, fresh));
                }
            }
            let c = self.settings.exploration;
            let edges = &mut tree[node].edges;
            let mut best: Option<(f64, usize)> = None;
            for (i, e) in edges.iter_mut().enumerate() {
                if e.player != player || !view.actions().contains(&e.action) {
                    continue;
                }
                e.available += 1;
                let n = f64::from(e.visits);
                let q = if e.visits == 0 { 0.0 } else { e.value / n };
                let score = q + c * e.prior * f64::from(e.available).sqrt() / (1.0 + n);
                if best.is_none_or(|b| score > b.0) {
                    best = Some((score, i));
                }
            }
            let (_, edge) = best.expect("a choice has two or more actions");
            path.push((node, edge));
            let action = tree[node].edges[edge].action.clone();
            g.apply(&action).expect("legal in this sample");
            node = match tree[node].edges[edge].child {
                Some(child) => child,
                None => {
                    tree.push(Node::default());
                    let child = tree.len() - 1;
                    tree[node].edges[edge].child = Some(child);
                    child
                }
            };
        };
        for (n, e) in path {
            let edge = &mut tree[n].edges[e];
            edge.visits += 1;
            edge.value += if edge.player == judge { value } else { -value };
        }
    }

    /// The network's priors (softmax) and value for the decider of `view`.
    fn judge(&self, view: &PlayerView) -> (Vec<f64>, f32) {
        let (logits, value) = self.net.evaluate(view);
        let top = logits.iter().copied().fold(f32::MIN, f32::max);
        let weights: Vec<f64> = logits.iter().map(|x| f64::from(x - top).exp()).collect();
        let total: f64 = weights.iter().sum();
        (weights.into_iter().map(|w| w / total).collect(), value)
    }
}

fn new_edge(player: PlayerId, action: &Action, prior: f64) -> Edge {
    Edge {
        player,
        action: action.clone(),
        prior,
        visits: 0,
        available: 0,
        value: 0.0,
        child: None,
    }
}

impl Player for MctsPlayer {
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
            // No time for a visit: the network's own choice.
            return self.net.choose(view);
        }
        best.0.clone()
    }
}
