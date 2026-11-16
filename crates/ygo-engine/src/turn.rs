//! The turn machine: phases advance by themselves until a player must
//! choose; `apply` executes that choice and advances again.

use ygo_data::HandLimitCheck;

use crate::chain::Pending;
use crate::{
    Action, BattleStep, Decision, DecisionKind, DrawReason, Event, Game, GameResult, InstanceId,
    Phase, PlayerId, RuleError, WinReason, WindowKind,
};

impl Game {
    /// Who must choose now, or `None` when the game is over or the
    /// engine has automatic work to do first.
    pub fn pending_decision(&self) -> Option<Decision> {
        if self.result.is_some() {
            return None;
        }
        if let Some(pc) = &self.pending_choice {
            return Some(Decision {
                player: pc.ctx.controller,
                kind: DecisionKind::Choose,
            });
        }
        match self.pending.last() {
            Some(Pending::Window { priority, .. }) => {
                let group = self.trigger_group();
                if let Some(first) = group.first() {
                    let actions = self.trigger_actions();
                    // One mandatory trigger with one way to activate needs
                    // no decision; nothing legal to do drops the group.
                    let automatic = actions.is_empty() || (first.mandatory && actions.len() == 1);
                    return (!automatic).then_some(Decision {
                        player: first.controller,
                        kind: DecisionKind::Triggers,
                    });
                }
                return (!self.response_actions(*priority).is_empty()).then_some(Decision {
                    player: *priority,
                    kind: DecisionKind::Respond,
                });
            }
            Some(_) => return None,
            None => {}
        }
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
        Some(Decision { player, kind })
    }

    /// Every legal answer to the pending decision; empty when none is pending.
    pub fn legal_actions(&self) -> Vec<Action> {
        let Some(decision) = self.pending_decision() else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        match decision.kind {
            DecisionKind::HandLimit => {
                for &card in &self.players[decision.player.index()].hand {
                    actions.push(Action::Discard { card });
                }
            }
            DecisionKind::MainPhase => {
                self.summon_actions(decision.player, &mut actions);
                self.position_actions(decision.player, &mut actions);
                self.set_spell_trap_actions(decision.player, &mut actions);
                self.activation_actions(decision.player, &mut actions);
                if self.phase == Phase::Main1 && self.attacks_allowed() {
                    actions.push(Action::EnterBattlePhase);
                }
                actions.push(Action::EndTurn);
            }
            DecisionKind::BattlePhase => self.battle_actions(decision.player, &mut actions),
            DecisionKind::Choose => self.choose_actions(&mut actions),
            DecisionKind::Respond => {
                actions = self.response_actions(decision.player);
                actions.push(Action::Pass);
            }
            DecisionKind::Triggers => {
                actions = self.trigger_actions();
                if !self.trigger_group()[0].mandatory {
                    actions.push(Action::Pass);
                }
            }
        }
        actions
    }

    /// Applies one legal action, then advances until the next decision or
    /// the end of the game. Returns everything that happened.
    pub fn apply(&mut self, action: &Action) -> Result<Vec<Event>, RuleError> {
        if self.result.is_some() {
            return Err(RuleError::GameOver);
        }
        let decision = self
            .pending_decision()
            .ok_or(RuleError::NoDecisionPending)?;
        if !self.legal_actions().contains(action) {
            return Err(RuleError::IllegalAction(action.clone()));
        }
        let player = decision.player;
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
            Action::Choose { card } => self.choose(*card),
            Action::Pass => {
                if decision.kind == DecisionKind::Triggers {
                    self.drop_trigger_group();
                } else {
                    self.pass();
                }
            }
            Action::EnterBattlePhase => self.enter_phase(Phase::Battle(BattleStep::Start)),
            Action::Attack { attacker, target } => self.declare_attack(*attacker, *target),
            Action::EndBattlePhase => self.enter_phase(Phase::Battle(BattleStep::End)),
            Action::EndTurn => self.enter_phase(Phase::End),
        }
        self.advance();
        Ok(std::mem::take(&mut self.events))
    }

    /// Runs automatic steps until a decision is pending or the game ends.
    /// Called at creation and after every action.
    pub(crate) fn advance(&mut self) {
        while self.result.is_none() && self.pending_decision().is_none() {
            match self.pending.last() {
                Some(Pending::Window { .. }) => {
                    let group = self.trigger_group();
                    if group.is_empty() {
                        self.pass();
                    } else {
                        let actions = self.trigger_actions();
                        match actions.first() {
                            Some(Action::Activate {
                                card,
                                effect,
                                targets,
                                cost,
                            }) if group[0].mandatory && actions.len() == 1 => {
                                let (card, effect, targets, cost) =
                                    (*card, *effect, targets.clone(), cost.clone());
                                self.activate(group[0].controller, card, effect, targets, cost);
                            }
                            _ => self.drop_trigger_group(),
                        }
                    }
                }
                Some(Pending::ResolveChain) => {
                    if self.chain.is_empty() {
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
                None => match self.phase {
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
                },
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
            p.monsters_entered = 0;
        }
        for card in self.instances.iter_mut() {
            card.position_changed_this_turn = false;
            card.attacked_this_turn = false;
        }
        self.emit(Event::TurnStarted { turn, player });
        self.enter_phase(Phase::Draw);

        let skip = turn == 1 && self.shared.rules.turn.first_player_skips_draw;
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
    fn set_spell_trap_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !matches!(self.phase, Phase::Main1 | Phase::Main2)
            || self.free_spell_trap_zone(player).is_none()
        {
            return;
        }
        for &card in &self.players[player.index()].hand {
            let stats = self.stats(card);
            if matches!(
                stats.kind,
                ygo_data::CardKind::Spell | ygo_data::CardKind::Trap
            ) && stats.subtype != crate::SpellTrapType::Field
            {
                actions.push(Action::SetSpellTrap { card });
            }
        }
    }

    fn set_spell_trap(&mut self, player: PlayerId, card: InstanceId) {
        let zone = self
            .free_spell_trap_zone(player)
            .expect("checked when the action was offered");
        self.place_spell_trap(player, card, zone, crate::Position::FaceDown);
        self.emit(Event::SetCard { player, card });
    }

    pub(crate) fn finish(&mut self, result: GameResult) {
        if self.result.is_none() {
            self.result = Some(result);
            self.emit(Event::GameOver { result });
        }
    }
}
