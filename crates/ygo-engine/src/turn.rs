//! The turn machine: phases advance by themselves until a player must
//! choose; `apply` executes that choice and advances again.

use ygo_data::HandLimitCheck;

use crate::{
    Action, BattleStep, Decision, DecisionKind, DrawReason, Event, Game, GameResult, InstanceId,
    Phase, PlayerId, RuleError, WinReason,
};

impl Game {
    /// Who must choose now, or `None` when the game is over.
    pub fn pending_decision(&self) -> Option<Decision> {
        if self.result.is_some() {
            return None;
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
                if self.phase == Phase::Main1 && self.attacks_allowed() {
                    actions.push(Action::EnterBattlePhase);
                }
                actions.push(Action::EndTurn);
            }
            DecisionKind::BattlePhase => self.battle_actions(decision.player, &mut actions),
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
        let mut events = Vec::new();
        match action {
            Action::Discard { card } => self.discard(decision.player, *card, &mut events),
            Action::Summon {
                card,
                tributes,
                face_down,
            } => self.summon(decision.player, *card, tributes, *face_down, &mut events),
            Action::FlipSummon { card } => self.flip_summon(decision.player, *card, &mut events),
            Action::ChangePosition { card } => self.change_position(*card, &mut events),
            Action::EnterBattlePhase => {
                self.enter_phase(Phase::Battle(BattleStep::Start), &mut events)
            }
            Action::Attack { attacker, target } => {
                self.attack(decision.player, *attacker, *target, &mut events)
            }
            Action::EndBattlePhase => self.enter_phase(Phase::Battle(BattleStep::End), &mut events),
            Action::EndTurn => self.enter_phase(Phase::End, &mut events),
        }
        self.advance(&mut events);
        Ok(events)
    }

    /// Runs automatic phase steps until a decision is pending or the game
    /// ends. Called at creation and after every action.
    pub(crate) fn advance(&mut self, events: &mut Vec<Event>) {
        while self.result.is_none() && self.pending_decision().is_none() {
            match self.phase {
                Phase::Draw => self.enter_phase(Phase::Standby, events),
                Phase::Standby => self.enter_phase(Phase::Main1, events),
                Phase::End => self.start_turn(self.turn + 1, self.turn_player.opponent(), events),
                Phase::Battle(BattleStep::Start) => {
                    self.enter_phase(Phase::Battle(BattleStep::Battle), events)
                }
                Phase::Battle(BattleStep::End) => self.enter_phase(Phase::Main2, events),
                Phase::Main1 | Phase::Main2 | Phase::Battle(_) => {
                    unreachable!("these phases always have a decision")
                }
            }
        }
    }

    pub(crate) fn enter_phase(&mut self, phase: Phase, events: &mut Vec<Event>) {
        self.phase = phase;
        events.push(Event::PhaseEntered { phase });
    }

    /// Starts `player`'s turn number `turn`: the turn cap, per-turn resets,
    /// then the Draw Phase with its draw.
    pub(crate) fn start_turn(&mut self, turn: u32, player: PlayerId, events: &mut Vec<Event>) {
        if turn > self.max_turns {
            self.finish(
                GameResult::Draw {
                    reason: DrawReason::TurnLimit,
                },
                events,
            );
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
        events.push(Event::TurnStarted { turn, player });
        self.enter_phase(Phase::Draw, events);

        let skip = turn == 1 && self.shared.rules.turn.first_player_skips_draw;
        if skip {
            return;
        }
        if let Some(card) = self.draw_card(player) {
            events.push(Event::Drew { player, card });
        } else {
            events.push(Event::DeckEmpty { player });
            if self.shared.rules.win.deck_out_loses {
                self.finish(
                    GameResult::Win {
                        winner: player.opponent(),
                        reason: WinReason::DeckOut,
                    },
                    events,
                );
            }
        }
    }

    fn hand_over_limit(&self, player: PlayerId, mode: HandLimitCheck) -> bool {
        let hand = &self.shared.rules.hand;
        hand.hand_limit_check == mode && self.players[player.index()].hand.len() > hand.max_size
    }

    fn discard(&mut self, player: PlayerId, card: InstanceId, events: &mut Vec<Event>) {
        self.send_to_graveyard(card, events);
        events.push(Event::Discarded { player, card });
    }

    pub(crate) fn finish(&mut self, result: GameResult, events: &mut Vec<Event>) {
        self.result = Some(result);
        events.push(Event::GameOver { result });
    }
}
