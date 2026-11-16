//! The turn machine: phases advance by themselves until a player must
//! choose; `apply` executes that choice and advances again.

use ygo_data::HandLimitCheck;

use crate::{
    Action, Decision, DecisionKind, DrawReason, Event, Game, GameResult, InstanceId, Location,
    Phase, PlayerId, Position, RuleError, WinReason,
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
                actions.push(Action::EndTurn);
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
        let mut events = Vec::new();
        match action {
            Action::Discard { card } => self.discard(decision.player, *card, &mut events),
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
                Phase::Main1 | Phase::Main2 => unreachable!("main phases always have a decision"),
                Phase::Battle(_) => unreachable!("battle arrives in M1 step 5"),
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
        let state = &mut self.players[player.index()];
        state.hand.retain(|&c| c != card);
        state.graveyard.push(card);
        let inst = &mut self.instances[card.index()];
        inst.location = Location::Graveyard;
        inst.position = Position::FaceUpAttack;
        events.push(Event::Discarded { player, card });
    }

    pub(crate) fn finish(&mut self, result: GameResult, events: &mut Vec<Event>) {
        self.result = Some(result);
        events.push(Event::GameOver { result });
    }
}
