//! The turn machine: phases advance by themselves until a player must
//! choose; `apply` executes that choice and advances again.

use ygo_data::HandLimitCheck;

use crate::chain::Pending;
use crate::{
    Action, BattleStep, Decision, DecisionKind, DrawReason, Event, Game, GameResult, InstanceId,
    Phase, PlayerId, RuleError, WinReason, WindowKind,
};

impl Game {
    /// Who must choose now, or `None` when the game is over.
    pub fn pending_decision(&self) -> Option<Decision> {
        self.decision
    }

    /// Every legal answer to the pending decision; empty when none is pending.
    pub fn legal_actions(&self) -> Vec<Action> {
        self.actions.clone()
    }

    /// The same as `legal_actions`, borrowed: for players that pick one.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The open-state decision for the current phase, if any, with its
    /// actions.
    fn phase_decision(&self) -> Option<(Decision, Vec<Action>)> {
        let player = self.turn_player;
        let kind = match self.phase {
            Phase::Draw if self.hand_over_limit(player, HandLimitCheck::AfterDraw) => {
                DecisionKind::HandLimit
            }
            Phase::End if self.hand_over_limit(player, HandLimitCheck::EndPhase) => {
                DecisionKind::HandLimit
            }
            Phase::Main1 | Phase::Main2 => DecisionKind::MainPhase,
            Phase::Battle(BattleStep::Battle) => DecisionKind::BattlePhase,
            _ => return None,
        };
        let mut actions = Vec::new();
        match kind {
            DecisionKind::HandLimit => {
                for &card in &self.players[player.index()].hand {
                    actions.push(Action::Discard { card });
                }
            }
            DecisionKind::MainPhase => {
                self.summon_actions(player, &mut actions);
                self.position_actions(player, &mut actions);
                self.set_spell_trap_actions(player, &mut actions);
                self.activation_actions(player, &mut actions);
                self.ignition_actions(player, &mut actions);
                let second = self.phase == Phase::Main2
                    && self.extra_battle_phase == Some((player, self.turn));
                if (self.phase == Phase::Main1 || second) && self.attacks_allowed() {
                    actions.push(Action::EnterBattlePhase);
                }
                actions.push(Action::EndTurn);
            }
            DecisionKind::BattlePhase => self.battle_actions(player, &mut actions),
            _ => unreachable!(),
        }
        Some((Decision { player, kind }, actions))
    }

    fn stop_at(&mut self, decision: Decision, mut actions: Vec<Action>) {
        let toll = self.hand_play_toll();
        if toll > 0 && self.players[decision.player.index()].lp <= toll {
            actions.retain(|a| !self.plays_from_hand(a));
        }
        self.decision = Some(decision);
        self.actions = actions;
    }

    /// Applies one legal action, then advances until the next decision or
    /// the end of the game. Returns everything that happened.
    pub fn apply(&mut self, action: &Action) -> Result<Vec<Event>, RuleError> {
        if self.result.is_some() {
            return Err(RuleError::GameOver);
        }
        let decision = self.decision.ok_or(RuleError::NoDecisionPending)?;
        if !self.actions.contains(action) {
            return Err(RuleError::IllegalAction(action.clone()));
        }
        let player = decision.player;
        let toll = self.hand_play_toll();
        if toll > 0 && self.plays_from_hand(action) {
            self.pay_lp(player, toll);
        }
        match action {
            Action::Discard { card } => self.discard(*card),
            Action::Summon {
                card,
                tributes,
                face_down,
            } => self.declare_summon(player, *card, tributes, *face_down),
            Action::FlipSummon { card } => self.declare_flip_summon(player, *card),
            Action::ChangePosition { card } => {
                self.change_position(*card);
                self.open_window(WindowKind::Open, self.turn_player);
            }
            Action::SetSpellTrap { card } => {
                self.set_spell_trap(player, *card);
                self.open_window(WindowKind::Open, self.turn_player);
            }
            Action::Activate {
                card,
                effect,
                targets,
                cost,
            } => self.activate(player, *card, *effect, targets.clone(), cost.clone()),
            Action::ProcedureSummon {
                card,
                effect,
                cost,
                position,
            } => self.declare_procedure_summon(player, *card, *effect, cost.clone(), *position),
            Action::Choose { card } => self.choose(*card),
            Action::Answer { option } => self.answer(usize::from(*option)),
            Action::Pass => match decision.kind {
                DecisionKind::Triggers => self.drop_trigger_group(),
                DecisionKind::Replay => {
                    let Some(Pending::Replay { attacker }) = self.pending.last() else {
                        unreachable!()
                    };
                    let attacker = *attacker;
                    self.replay(attacker, None);
                }
                _ => self.pass(),
            },
            Action::EnterBattlePhase => self.enter_battle_phase(),
            Action::Attack { attacker, target } => {
                if decision.kind == DecisionKind::Replay {
                    self.replay(*attacker, Some(*target));
                } else {
                    self.declare_attack(*attacker, *target);
                }
            }
            Action::EndBattlePhase => self.enter_phase(Phase::Battle(BattleStep::End)),
            Action::EndTurn => self.enter_phase(Phase::End),
        }
        self.advance();
        Ok(std::mem::take(&mut self.events))
    }

    /// Runs automatic steps until a decision is pending or the game ends,
    /// then stores that decision and its legal actions. Called at creation
    /// and after every action. Every enumeration happens once per state.
    pub(crate) fn advance(&mut self) {
        loop {
            if self.result.is_some() {
                self.decision = None;
                self.actions.clear();
                return;
            }
            if let Some(pc) = &self.pending_choice {
                let decision = Decision {
                    player: pc.decider(),
                    kind: DecisionKind::Choose,
                };
                let mut actions = Vec::new();
                self.choose_actions(&mut actions);
                self.stop_at(decision, actions);
                return;
            }
            match self.pending.last() {
                Some(Pending::Window { priority, .. }) => {
                    let priority = *priority;
                    let group = self.trigger_group();
                    if let Some(first) = group.first() {
                        let actions = self.trigger_actions();
                        match actions.first() {
                            None => self.drop_trigger_group(),
                            Some(Action::Activate {
                                card,
                                effect,
                                targets,
                                cost,
                            }) if first.mandatory && actions.len() == 1 => {
                                let (card, effect, targets, cost) =
                                    (*card, *effect, targets.clone(), cost.clone());
                                self.activate(first.controller, card, effect, targets, cost);
                            }
                            Some(_) => {
                                let mut actions = actions;
                                if !first.mandatory {
                                    actions.push(Action::Pass);
                                }
                                let decision = Decision {
                                    player: first.controller,
                                    kind: DecisionKind::Triggers,
                                };
                                self.stop_at(decision, actions);
                                return;
                            }
                        }
                        continue;
                    }
                    let mut responses = self.response_actions(priority);
                    if responses.is_empty() {
                        self.pass();
                    } else {
                        responses.push(Action::Pass);
                        let decision = Decision {
                            player: priority,
                            kind: DecisionKind::Respond,
                        };
                        self.stop_at(decision, responses);
                        return;
                    }
                }
                Some(Pending::ResolveChain) => {
                    if self.chain.is_empty() {
                        self.shielded.clear();
                        self.pending.pop();
                        self.open_window(WindowKind::AfterChain, self.turn_player);
                    } else {
                        self.resolve_top_link();
                    }
                }
                Some(Pending::CompleteSummon(ps)) => {
                    let ps = *ps;
                    self.pending.pop();
                    self.complete_summon(ps);
                }
                Some(Pending::DamageStep { attacker, target }) => {
                    let (attacker, target) = (*attacker, *target);
                    self.pending.pop();
                    self.damage_step(attacker, target);
                }
                Some(Pending::DamageCalc { attacker, target }) => {
                    let (attacker, target) = (*attacker, *target);
                    self.pending.pop();
                    self.damage_calculation(attacker, target);
                }
                Some(Pending::Replay { attacker }) => {
                    let attacker = *attacker;
                    let player = self.instances[attacker.index()].controller;
                    let mut actions = Vec::new();
                    self.attack_options(attacker, &mut actions);
                    actions.push(Action::Pass);
                    let decision = Decision {
                        player,
                        kind: DecisionKind::Replay,
                    };
                    self.stop_at(decision, actions);
                    return;
                }
                None => {
                    if let Some((decision, actions)) = self.phase_decision() {
                        self.stop_at(decision, actions);
                        return;
                    }
                    match self.phase {
                        Phase::Draw => self.enter_phase(Phase::Standby),
                        Phase::Standby => self.enter_phase(Phase::Main1),
                        Phase::Battle(BattleStep::Start) => {
                            self.enter_phase(Phase::Battle(BattleStep::Battle))
                        }
                        Phase::Battle(BattleStep::End) => self.enter_phase(Phase::Main2),
                        Phase::End => self.start_turn(self.turn + 1, self.turn_player.opponent()),
                        Phase::Main1 | Phase::Main2 | Phase::Battle(_) => {
                            unreachable!("these phases always have a decision")
                        }
                    }
                }
            }
        }
    }

    /// Enters a phase. The Standby Phase, the start and end of the Battle
    /// Phase and the End Phase open a response window.
    pub(crate) fn enter_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.emit(Event::PhaseEntered { phase });
        if phase == Phase::End {
            self.expire_lingering();
        }
        if matches!(
            phase,
            Phase::Standby
                | Phase::Battle(BattleStep::Start)
                | Phase::Battle(BattleStep::End)
                | Phase::End
        ) {
            self.open_window(WindowKind::Open, self.turn_player);
        }
    }

    /// Starts `player`'s turn number `turn`: the turn cap, per-turn resets,
    /// then the Draw Phase with its draw.
    pub(crate) fn start_turn(&mut self, turn: u32, player: PlayerId) {
        if turn > self.max_turns {
            self.finish(GameResult::Draw {
                reason: DrawReason::TurnLimit,
            });
            return;
        }
        self.turn = turn;
        self.turn_player = player;
        for p in self.players.iter_mut() {
            p.summons_used = 0;
            p.monsters_to_graveyard = 0;
            p.monsters_entered = 0;
            p.names_used.clear();
            p.names_locked.clear();
        }
        for card in self.instances.iter_mut() {
            card.position_changed_this_turn = false;
            card.attacked_this_turn = false;
            card.effect_uses = 0;
        }
        self.emit(Event::TurnStarted { turn, player });
        self.enter_phase(Phase::Draw);

        let rules = &self.shared.rules.turn;
        let skip = (turn == 1 && rules.first_player_skips_draw)
            || (turn == 2 && rules.second_player_skips_draw);
        if skip {
            return;
        }
        if let Some(card) = self.draw_card(player) {
            self.emit(Event::Drew { player, card });
            self.check_exodia(player);
        } else {
            self.emit(Event::DeckEmpty { player });
            if self.shared.rules.win.deck_out_loses {
                self.finish(GameResult::Win {
                    winner: player.opponent(),
                    reason: WinReason::DeckOut,
                });
            }
        }
    }

    fn hand_over_limit(&self, player: PlayerId, mode: HandLimitCheck) -> bool {
        let hand = &self.shared.rules.hand;
        hand.hand_limit_check == mode && self.players[player.index()].hand.len() > hand.max_size
    }

    /// Spells and Traps in the hand that may be Set now.
    /// Sets from the hand: a Spell or Trap into a free Spell/Trap Zone, or a
    /// Field Spell into the Field Zone (replacing the player's own).
    fn set_spell_trap_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !matches!(self.phase, Phase::Main1 | Phase::Main2) {
            return;
        }
        let has_zone = self.free_spell_trap_zone(player).is_some();
        for &card in &self.players[player.index()].hand {
            let stats = self.stats(card);
            if !matches!(
                stats.kind,
                ygo_data::CardKind::Spell | ygo_data::CardKind::Trap
            ) {
                continue;
            }
            if stats.subtype == crate::SpellTrapType::Field || has_zone {
                actions.push(Action::SetSpellTrap { card });
            }
        }
    }

    fn set_spell_trap(&mut self, player: PlayerId, card: InstanceId) {
        if self.stats(card).subtype == crate::SpellTrapType::Field {
            self.place_field_spell(player, card, crate::Position::FaceDown);
        } else {
            let zone = self
                .free_spell_trap_zone(player)
                .expect("checked when the action was offered");
            self.place_spell_trap(player, card, zone, crate::Position::FaceDown);
        }
        self.emit(Event::SetCard { player, card });
    }

    /// From Main Phase 1, or from Main Phase 2 with a granted second Battle
    /// Phase, in which monsters may attack again.
    fn enter_battle_phase(&mut self) {
        if self.phase == Phase::Main2 {
            self.extra_battle_phase = None;
            let player = self.turn_player;
            for card in self.instances.iter_mut() {
                if card.controller == player {
                    card.attacked_this_turn = false;
                }
            }
        }
        self.enter_phase(Phase::Battle(BattleStep::Start));
    }

    pub(crate) fn finish(&mut self, result: GameResult) {
        if self.result.is_none() {
            self.result = Some(result);
            self.emit(Event::GameOver { result });
        }
    }
}
