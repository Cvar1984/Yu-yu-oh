//! Flip Summon and manual position change: once per monster per turn,
//! never in the turn the monster entered the field, never after it attacked.

use crate::{Action, Event, Game, InstanceId, Phase, PlayerId, Position};

impl Game {
    fn in_main_phase(&self) -> bool {
        matches!(self.phase, Phase::Main1 | Phase::Main2)
    }

    /// A monster may change position (or Flip Summon) if it did not enter
    /// the field, change position or attack this turn.
    fn position_change_allowed(&self, card: InstanceId) -> bool {
        let inst = &self.instances[card.index()];
        inst.turn_entered_field != Some(self.turn)
            && !inst.position_changed_this_turn
            && !inst.attacked_this_turn
    }

    fn flip_summon_allowed(&self, player: PlayerId) -> bool {
        let rules = &self.shared.rules.summon;
        !rules.flip_summon_counts_toward_limit
            || usize::from(self.players[player.index()].summons_used)
                < rules.normal_or_tribute_per_turn
    }

    pub(crate) fn position_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !self.in_main_phase() {
            return;
        }
        for card in self.players[player.index()].monsters() {
            if !self.position_change_allowed(card) {
                continue;
            }
            match self.instances[card.index()].position {
                Position::FaceDownDefense => {
                    if self.flip_summon_allowed(player) {
                        actions.push(Action::FlipSummon { card });
                    }
                }
                Position::FaceUpAttack | Position::FaceUpDefense => {
                    actions.push(Action::ChangePosition { card });
                }
                Position::FaceDown | Position::FaceUp => {}
            }
        }
    }

    pub(crate) fn flip_summon(&mut self, player: PlayerId, card: InstanceId) {
        if self.shared.rules.summon.flip_summon_counts_toward_limit {
            self.players[player.index()].summons_used += 1;
        }
        let inst = &mut self.instances[card.index()];
        inst.position = Position::FaceUpAttack;
        inst.position_changed_this_turn = true;
        self.emit(Event::FlipSummoned { player, card });
    }

    pub(crate) fn change_position(&mut self, card: InstanceId) {
        let inst = &mut self.instances[card.index()];
        inst.position = match inst.position {
            Position::FaceUpAttack => Position::FaceUpDefense,
            Position::FaceUpDefense => Position::FaceUpAttack,
            other => other,
        };
        inst.position_changed_this_turn = true;
        let position = inst.position;
        self.emit(Event::PositionChanged { card, position });
    }
}
