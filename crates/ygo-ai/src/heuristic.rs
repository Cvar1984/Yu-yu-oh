//! The heuristic bot: scores every legal action from the observation and
//! plays the best, breaking ties with its seeded RNG. One program; the
//! playstyle is a `Style` from `config/styles.toml` and card knowledge is
//! the value table from `config/card_values.toml`.

use std::sync::Arc;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_data::{CardDb, CardId, CardKind, CardValues, Style};
use ygo_engine::Tag;
use ygo_engine::{
    Action, CardStats, DecisionKind, Game, InstanceId, Location, Observation, Place, PlayerId,
    PlayerView, Position, Shared, SpellTrapType, VisibleCard, WindowKind,
};

use crate::Player;
use crate::eval::{Worth, lead, settle};

/// Actions a lookahead may take to settle one option.
const SETTLE_CAP: usize = 300;
/// Options of one `Choose` decision the lookahead tries, best scored first.
const LOOKAHEAD_OPTIONS: usize = 12;

/// Scores closer than this are a tie.
const TIE: f64 = 1e-9;
/// Never chosen while anything else is legal.
const NEVER: f64 = -1000.0;

#[derive(Clone)]
pub struct HeuristicPlayer {
    shared: Arc<Shared>,
    style: Style,
    /// Card value by pool index, style overrides applied.
    values: Vec<f64>,
    hidden: f64,
    rng: ChaCha8Rng,
    /// Activations made this turn, so a repeatable effect loses appeal.
    turn: u32,
    activated: Vec<(InstanceId, u8)>,
    /// Try each activation, answer and choice on a sample and keep the one
    /// that leaves the best position (`option_values`).
    lookahead: bool,
    worth: Worth,
}

/// What every score needs to know about the board, computed once per
/// decision.
struct Board {
    me: PlayerId,
    /// Highest ATK among the opponent's face-up monsters, 0 if none.
    opp_best_atk: i32,
    opp_monsters: usize,
    /// A Set Spell or Trap of our own to protect a summon.
    have_cover: bool,
}

impl HeuristicPlayer {
    pub fn new(
        shared: Arc<Shared>,
        cards: &CardDb,
        config: &CardValues,
        style: &Style,
        seed: u64,
    ) -> Self {
        let pool = &shared.pool;
        let values = (0..pool.len() as u16)
            .map(|i| {
                let id = pool.get(ygo_engine::CardIndex(i)).id;
                let v = style
                    .values
                    .get(&id)
                    .copied()
                    .or_else(|| cards.get(id).map(|c| config.value(c)));
                f64::from(v.unwrap_or(config.defaults.hidden))
            })
            .collect();
        Self {
            shared,
            style: style.clone(),
            values,
            hidden: f64::from(config.defaults.hidden),
            rng: ChaCha8Rng::seed_from_u64(seed),
            turn: 0,
            activated: Vec::new(),
            lookahead: false,
            worth: Worth::default(),
        }
    }

    /// The same bot that looks one step ahead before activating a card,
    /// answering a question or choosing a card (or not).
    pub fn with_lookahead(self, on: bool) -> Self {
        Self {
            lookahead: on,
            ..self
        }
    }

    /// What each of `options` (legal actions of `g`'s pending decision)
    /// leads to for the deciding player: the action, then play on by this
    /// bot without lookahead on both sides until the next open decision,
    /// then their lead in points (`eval::lead`). Activated Spells and
    /// Traps pay the style's `use_cost`; the cards any option activates count for
    /// nothing in every result, so keeping one is no gain by itself.
    /// `None` for an option `g` does not allow.
    pub fn option_values(&self, g: &Game, options: &[Action], seed: u64) -> Vec<Option<f64>> {
        let Some(player) = g.pending_decision().map(|d| d.player) else {
            return vec![None; options.len()];
        };
        let ignore: Vec<InstanceId> = options
            .iter()
            .filter_map(|a| match a {
                Action::Activate { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        let value = |c| self.card_value(c);
        let blind = self.fresh(seed).with_lookahead(false);
        options
            .iter()
            .map(|a| {
                if !g.actions().contains(a) {
                    return None;
                }
                let cost = match a {
                    Action::Activate { card, .. } => self.use_cost(g, *card),
                    _ => 0.0,
                };
                let mut next = g.clone();
                next.apply(a).ok()?;
                settle(&mut next, &mut blind.clone(), SETTLE_CAP);
                Some(lead(&next, player, &value, self.worth, &ignore) - cost)
            })
            .collect()
    }

    /// For audits, on the real game `g` rather than a sample: the
    /// lookahead value of `chosen` and of the best option this bot would
    /// weigh in its decision, and that option. `None` when the decision
    /// has fewer than two such options.
    pub fn review(&self, g: &Game, chosen: &Action, seed: u64) -> Option<(f64, f64, Action)> {
        let d = g.pending_decision()?;
        let obs = g.observe(d.player);
        let board = self.board(&obs);
        let actions = g.actions();
        let scores: Vec<f64> = actions
            .iter()
            .map(|a| self.score_on(&obs, &board, a))
            .collect();
        let mut candidates = self.lookahead_candidates(d.kind, actions, &scores);
        if candidates.len() < 2 {
            return None;
        }
        if let Some(i) = actions.iter().position(|a| a == chosen)
            && !candidates.contains(&i)
        {
            candidates.push(i);
        }
        let options: Vec<Action> = candidates.iter().map(|&i| actions[i].clone()).collect();
        let values = self.option_values(g, &options, seed);
        let chosen_value = options
            .iter()
            .zip(&values)
            .find(|(a, _)| *a == chosen)
            .and_then(|(_, v)| *v)?;
        let (best, best_value) = options
            .iter()
            .zip(values)
            .filter_map(|(a, v)| v.map(|v| (a, v)))
            .max_by(|x, y| x.1.total_cmp(&y.1))?;
        Some((chosen_value, best_value, best.clone()))
    }

    /// What activating `card` spends: a Spell or Trap Card played from the
    /// hand or from face-down. A face-up card's or a monster's effect is free.
    fn use_cost(&self, g: &Game, card: InstanceId) -> f64 {
        let inst = g.instance(card);
        let played = !g.stats(card).is_monster()
            && (inst.location == Location::Hand || inst.position == Position::FaceDown);
        if played {
            self.card_value(inst.card) * self.worth.hand_worth * self.style.use_cost
        } else {
            0.0
        }
    }

    /// The actions worth a lookahead in this decision, by index: every
    /// activation, answer and choice (and passing), against the best other
    /// action by score. Empty when there is nothing of that kind.
    fn lookahead_candidates(
        &self,
        kind: DecisionKind,
        actions: &[Action],
        scores: &[f64],
    ) -> Vec<usize> {
        let special = |a: &Action| {
            matches!(
                a,
                Action::Activate { .. } | Action::Answer { .. } | Action::Choose { .. }
            )
        };
        if !actions.iter().any(special) {
            return Vec::new();
        }
        let mut order: Vec<usize> = (0..actions.len()).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
        match kind {
            DecisionKind::Respond | DecisionKind::Choose | DecisionKind::Triggers => {
                order.truncate(LOOKAHEAD_OPTIONS);
                order
            }
            DecisionKind::MainPhase | DecisionKind::BattlePhase => {
                let other = order.iter().copied().find(|&i| !special(&actions[i]));
                let mut picked: Vec<usize> = order
                    .iter()
                    .copied()
                    .filter(|&i| special(&actions[i]))
                    .take(LOOKAHEAD_OPTIONS)
                    .collect();
                picked.extend(other);
                picked
            }
            _ => Vec::new(),
        }
    }

    /// A copy at the start of a game, with its own random generator.
    pub(crate) fn fresh(&self, seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            turn: 0,
            activated: Vec::new(),
            ..self.clone()
        }
    }

    /// What a card is worth to this style.
    pub(crate) fn card_value(&self, card: ygo_engine::CardIndex) -> f64 {
        self.values[card.0 as usize]
    }

    fn stats(&self, id: CardId) -> Option<&CardStats> {
        let pool = &self.shared.pool;
        pool.index_of(id).map(|i| pool.get(i))
    }

    fn value_of_id(&self, id: CardId) -> f64 {
        self.shared
            .pool
            .index_of(id)
            .map_or(self.hidden, |i| self.values[i.0 as usize])
    }

    fn value(&self, obs: &Observation, id: InstanceId) -> f64 {
        match obs.find(id).map(|f| f.card) {
            Some(VisibleCard::Known { card, .. }) => self.value_of_id(*card),
            _ => self.hidden,
        }
    }

    /// Current ATK and DEF: on the field from the observation, elsewhere
    /// the printed values. `None` for hidden cards and non-monsters.
    fn atk_def(&self, obs: &Observation, id: InstanceId) -> Option<(i32, i32)> {
        match obs.find(id)?.card {
            VisibleCard::Known { stats: Some(s), .. } => Some(*s),
            VisibleCard::Known { card, .. } => self
                .stats(*card)
                .filter(|s| s.is_monster())
                .map(|s| (s.atk, s.def)),
            VisibleCard::Hidden { .. } => None,
        }
    }

    /// A piece of Exodia, while the rules let the five pieces in hand win.
    fn exodia_piece(&self, obs: &Observation, id: InstanceId) -> bool {
        let shared = &self.shared;
        shared.rules.win.exodia
            && obs
                .find(id)
                .and_then(|f| f.card.card())
                .and_then(|c| shared.pool.index_of(c))
                .and_then(|i| shared.script(i))
                .is_some_and(|s| s.has(Tag::ExodiaPiece) || s.has(Tag::ExodiaHead))
    }

    fn board(&self, obs: &Observation) -> Board {
        let opp = obs.opponent();
        let opp_best_atk = opp
            .monster_zones
            .iter()
            .flatten()
            .filter_map(|c| match c {
                VisibleCard::Known {
                    stats: Some((atk, _)),
                    ..
                } => Some(*atk),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        Board {
            me: obs.viewer,
            opp_best_atk,
            opp_monsters: opp.monster_zones.iter().flatten().count(),
            have_cover: obs
                .me()
                .spell_trap_zones
                .iter()
                .flatten()
                .any(|c| c.position() == Position::FaceDown),
        }
    }

    /// How much this action is worth now; higher is better, 0 is neutral
    /// (passing, ending a phase).
    pub fn score(&self, obs: &Observation, action: &Action) -> f64 {
        let board = self.board(obs);
        self.score_on(obs, &board, action)
    }

    fn score_on(&self, obs: &Observation, b: &Board, action: &Action) -> f64 {
        let st = &self.style;
        match action {
            Action::Discard { card } => -self.value(obs, *card),
            Action::Summon {
                card,
                tributes,
                face_down,
            } => self.summon_score(obs, b, *card, tributes, *face_down),
            // A summon procedure costs its tributes; Defense is scored like a Set.
            Action::ProcedureSummon {
                card,
                cost,
                position,
                ..
            } => self.summon_score(obs, b, *card, cost, *position == Position::FaceUpDefense),
            Action::FlipSummon { card } => {
                let (atk, _) = self.atk_def(obs, *card).unwrap_or((0, 0));
                let effect = self.value(obs, *card) * 0.1;
                if atk > b.opp_best_atk {
                    5.0 + st.aggression * f64::from(atk) / 100.0 + effect
                } else {
                    effect - 5.0
                }
            }
            Action::ChangePosition { card } => {
                let Some(VisibleCard::Known {
                    position,
                    stats: Some((atk, _)),
                    ..
                }) = obs.find(*card).map(|f| f.card)
                else {
                    return -10.0;
                };
                match position {
                    Position::FaceUpAttack if *atk < b.opp_best_atk => {
                        st.caution
                            * (self.value(obs, *card) * 0.5
                                + f64::from(b.opp_best_atk - atk) / 100.0)
                    }
                    Position::FaceUpDefense if *atk > b.opp_best_atk => {
                        st.aggression * f64::from(*atk) / 200.0
                    }
                    _ => -10.0,
                }
            }
            Action::SetSpellTrap { card } => {
                let kind = obs
                    .find(*card)
                    .and_then(|f| f.card.card())
                    .and_then(|id| self.stats(id));
                match kind {
                    Some(s) if s.kind == CardKind::Trap => 15.0 + self.value(obs, *card) / 10.0,
                    Some(s) if s.subtype == SpellTrapType::QuickPlay => 5.0,
                    _ => -5.0,
                }
            }
            Action::Activate {
                card,
                effect,
                targets,
                cost,
            } => self.activate_score(obs, b, *card, *effect, targets, cost),
            Action::Choose { card } => self.choose_score(obs, b, *card),
            Action::Answer { option } => {
                // Scripts list the active option first ("Pay 100 LP", "Banish another").
                if *option == 0 { 1.0 } else { 0.0 }
            }
            Action::Pass | Action::EndBattlePhase | Action::EndTurn => 0.0,
            Action::EnterBattlePhase => {
                let best = self.best_attack(obs, b);
                if best > 0.0 { 2.0 } else { -1.0 }
            }
            Action::Attack { attacker, target } => self.attack_score(obs, *attacker, *target),
        }
    }

    fn summon_score(
        &self,
        obs: &Observation,
        b: &Board,
        card: InstanceId,
        tributes: &[InstanceId],
        face_down: bool,
    ) -> f64 {
        let st = &self.style;
        let (atk, def) = self.atk_def(obs, card).unwrap_or((0, 0));
        let worth = self.value(obs, card);
        if self.exodia_piece(obs, card) {
            // A piece wins only from the hand.
            return NEVER;
        }
        let mut s = 10.0;
        if !tributes.is_empty() {
            if st.tribute_needs_cover && !b.have_cover {
                return NEVER;
            }
            let given: f64 = tributes.iter().map(|&t| self.value(obs, t)).sum();
            s += worth - given - f64::from(st.tribute_margin);
        }
        if face_down {
            // Setting pays only when the monster would lose in Attack Position.
            let outclassed = atk <= b.opp_best_atk;
            let guard = if outclassed {
                st.caution * f64::from(def) / 100.0
            } else {
                0.0
            };
            s + guard + worth * 0.1
        } else {
            s += st.aggression * f64::from(atk) / 100.0;
            if atk < b.opp_best_atk {
                // It would be run over: count the monster as lost.
                s -= st.caution * (worth + f64::from(b.opp_best_atk - atk) / 100.0);
            }
            s
        }
    }

    fn activate_score(
        &self,
        obs: &Observation,
        b: &Board,
        card: InstanceId,
        effect: u8,
        targets: &[InstanceId],
        cost: &[InstanceId],
    ) -> f64 {
        let id = obs.find(card).and_then(|f| f.card.card());
        let stats = id.and_then(|id| self.stats(id));
        let worth = self.value(obs, card);
        let opp_targets: f64 = targets
            .iter()
            .filter(|&&t| obs.find(t).and_then(|f| f.owner) == Some(b.me.opponent()))
            .map(|&t| self.value(obs, t))
            .sum();
        let paid: f64 = cost.iter().map(|&c| self.value(obs, c)).sum();
        let repeats = self
            .activated
            .iter()
            .filter(|&&(c, e)| c == card && e == effect)
            .count() as f64;
        let mut s = 5.0 + worth * 0.5 + opp_targets * 0.5 - paid * 0.5 - 20.0 * repeats;
        if let Some(st) = stats.filter(|s| s.kind == CardKind::Trap) {
            if st.subtype == SpellTrapType::Counter {
                let threat = self.threat(obs, b);
                return if threat >= f64::from(self.style.counter_min_value) {
                    threat - paid * 0.5
                } else {
                    NEVER / 10.0
                };
            }
            if opp_targets == 0.0 && !self.reactive(obs, b) {
                // Keep a Trap for a window the opponent opens.
                s = -1.0;
            }
        }
        s
    }

    /// Value of what the opponent is summoning or activating right now.
    fn threat(&self, obs: &Observation, b: &Board) -> f64 {
        if let Some(link) = obs.chain.last().filter(|l| l.controller != b.me) {
            return self.value_of_id(link.card);
        }
        match &obs.pending_summon {
            Some(p) if p.player != b.me => self.value(obs, p.card),
            _ => 0.0,
        }
    }

    /// The open window was opened by the opponent's summon, attack or
    /// activation.
    fn reactive(&self, obs: &Observation, b: &Board) -> bool {
        let their_turn = obs.turn_player != b.me;
        match obs.window.map(|(kind, _)| kind) {
            Some(WindowKind::Chain) => obs.chain.last().is_some_and(|l| l.controller != b.me),
            Some(
                WindowKind::SummonDeclared
                | WindowKind::SummonSuccess
                | WindowKind::AttackDeclared
                | WindowKind::DamageCalculation,
            ) => their_turn,
            _ => false,
        }
    }

    /// Cards we gain or the opponent loses: the best. Cards we give up:
    /// the worst.
    fn choose_score(&self, obs: &Observation, b: &Board, card: InstanceId) -> f64 {
        let v = self.value(obs, card);
        match obs.find(card) {
            Some(f) if f.owner == Some(b.me) => match f.place {
                Place::Hand | Place::Monster(_) | Place::SpellTrap(_) | Place::Field => -v,
                _ => v,
            },
            _ => v,
        }
    }

    fn attack_score(
        &self,
        obs: &Observation,
        attacker: InstanceId,
        target: Option<InstanceId>,
    ) -> f64 {
        let st = &self.style;
        let (atk, _) = self.atk_def(obs, attacker).unwrap_or((0, 0));
        let mine = self.value(obs, attacker);
        let s = match target.and_then(|t| obs.find(t)) {
            None => 5.0 + st.lp_weight * f64::from(atk) / 100.0,
            Some(f) => match f.card {
                VisibleCard::Known {
                    position,
                    stats: Some((t_atk, t_def)),
                    ..
                } => {
                    let theirs = self.value(obs, f.card.instance());
                    if *position == Position::FaceUpAttack {
                        let diff = atk - t_atk;
                        if diff > 0 {
                            theirs + st.lp_weight * f64::from(diff) / 100.0
                        } else if diff == 0 {
                            theirs - mine
                        } else {
                            -mine - st.caution * f64::from(-diff) / 100.0 - 10.0
                        }
                    } else if atk > *t_def {
                        theirs
                    } else {
                        -5.0 - st.caution * f64::from(t_def - atk) / 100.0
                    }
                }
                _ if atk >= st.facedown_attack_min_atk => self.hidden * 0.5,
                _ => -5.0,
            },
        };
        if s > 0.0 { s * st.aggression } else { s }
    }

    /// The best attack any of our face-up Attack Position monsters has.
    fn best_attack(&self, obs: &Observation, b: &Board) -> f64 {
        let targets: Vec<Option<InstanceId>> = if b.opp_monsters == 0 {
            vec![None]
        } else {
            obs.opponent()
                .monster_zones
                .iter()
                .flatten()
                .map(|c| Some(c.instance()))
                .collect()
        };
        obs.me()
            .monster_zones
            .iter()
            .flatten()
            .filter(|c| c.position() == Position::FaceUpAttack)
            .flat_map(|c| targets.iter().map(move |&t| (c.instance(), t)))
            .map(|(a, t)| self.attack_score(obs, a, t))
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

impl Player for HeuristicPlayer {
    fn choose(&mut self, view: &PlayerView) -> Action {
        let (obs, actions) = (view.observation(), view.actions());
        if obs.turn != self.turn {
            self.turn = obs.turn;
            self.activated.clear();
        }
        let board = self.board(obs);
        let scores: Vec<f64> = actions
            .iter()
            .map(|a| self.score_on(obs, &board, a))
            .collect();
        let kind = view
            .observation()
            .decision
            .map_or(DecisionKind::MainPhase, |d| d.kind);
        let candidates = if self.lookahead {
            self.lookahead_candidates(kind, actions, &scores)
        } else {
            Vec::new()
        };
        let looked = if candidates.len() > 1 {
            let sample = view.sample(self.rng.next_u64());
            let options: Vec<Action> = candidates.iter().map(|&i| actions[i].clone()).collect();
            let seed = self.rng.next_u64();
            let values = self.option_values(&sample, &options, seed);
            // Best result first, the score breaking ties.
            candidates
                .iter()
                .zip(values)
                .filter_map(|(&i, v)| v.map(|v| (i, v)))
                .max_by(|a, b| {
                    a.1.total_cmp(&b.1)
                        .then(scores[a.0].total_cmp(&scores[b.0]))
                })
                .map(|(i, _)| i)
        } else {
            None
        };
        let pick = looked.unwrap_or_else(|| {
            let best = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let ties: Vec<usize> = (0..actions.len())
                .filter(|&i| scores[i] >= best - TIE)
                .collect();
            ties[(self.rng.next_u64() % ties.len() as u64) as usize]
        });
        let action = actions[pick].clone();
        if let Action::Activate { card, effect, .. } = action {
            self.activated.push((card, effect));
        }
        action
    }
}
