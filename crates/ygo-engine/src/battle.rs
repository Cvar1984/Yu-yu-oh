//! The Battle Phase: attack declaration and damage calculation. In M1 the
//! Damage Step resolves atomically; M2 opens response windows inside it.

use crate::{
    Action, BattleStep, DrawReason, Event, Game, GameResult, InstanceId, Phase, PlayerId, Position,
    WinReason,
};

impl Game {
    /// Whether the turn player may attack this turn at all.
    pub(crate) fn attacks_allowed(&self) -> bool {
        self.turn != 1 || self.shared.rules.turn.first_player_can_attack
    }

    pub(crate) fn battle_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        let opponent = player.opponent();
        let targets: Vec<InstanceId> = self.players[opponent.index()].monsters().collect();
        for attacker in self.players[player.index()].monsters() {
            let inst = &self.instances[attacker.index()];
            if inst.position != Position::FaceUpAttack || inst.attacked_this_turn {
                continue;
            }
            if targets.is_empty() {
                actions.push(Action::Attack {
                    attacker,
                    target: None,
                });
            } else {
                for &target in &targets {
                    actions.push(Action::Attack {
                        attacker,
                        target: Some(target),
                    });
                }
            }
        }
        actions.push(Action::EndBattlePhase);
    }

    pub(crate) fn attack(
        &mut self,
        player: PlayerId,
        attacker: InstanceId,
        target: Option<InstanceId>,
    ) {
        self.instances[attacker.index()].attacked_this_turn = true;
        self.emit(Event::AttackDeclared { attacker, target });
        self.enter_phase(Phase::Battle(BattleStep::Damage));

        let atk = self.stats(attacker).atk;
        match target {
            None => self.battle_damage(player.opponent(), atk),
            Some(target) => {
                if self.instances[target.index()].position == Position::FaceDownDefense {
                    self.instances[target.index()].position = Position::FaceUpDefense;
                    self.emit(Event::Flipped { card: target });
                }
                let defender = self.instances[target.index()].controller;
                let stats = self.stats(target);
                if self.instances[target.index()].position == Position::FaceUpAttack {
                    let diff = atk - stats.atk;
                    match diff.signum() {
                        1 => {
                            self.destroy_by_battle(target);
                            self.battle_damage(defender, diff);
                        }
                        -1 => {
                            self.destroy_by_battle(attacker);
                            self.battle_damage(player, -diff);
                        }
                        _ => {
                            self.destroy_by_battle(target);
                            self.destroy_by_battle(attacker);
                        }
                    }
                } else {
                    let diff = atk - stats.def;
                    match diff.signum() {
                        1 => self.destroy_by_battle(target),
                        -1 => self.battle_damage(player, -diff),
                        _ => {}
                    }
                }
            }
        }
        self.check_life_points();
        if self.result.is_none() {
            self.enter_phase(Phase::Battle(BattleStep::Battle));
        }
    }

    fn destroy_by_battle(&mut self, card: InstanceId) {
        self.send_to_graveyard(card);
        self.emit(Event::DestroyedByBattle { card });
    }

    fn battle_damage(&mut self, player: PlayerId, amount: i32) {
        let amount = u32::try_from(amount).unwrap_or(0);
        let state = &mut self.players[player.index()];
        state.lp = state.lp.saturating_sub(amount);
        let lp = state.lp;
        self.emit(Event::DamageTaken { player, amount, lp });
    }

    pub(crate) fn check_life_points(&mut self) {
        let dead: Vec<PlayerId> = PlayerId::BOTH
            .into_iter()
            .filter(|p| self.players[p.index()].lp == 0)
            .collect();
        match dead.as_slice() {
            [] => {}
            [loser] => self.finish(GameResult::Win {
                winner: loser.opponent(),
                reason: WinReason::LifePoints,
            }),
            _ => self.finish(GameResult::Draw {
                reason: DrawReason::BothLifePointsZero,
            }),
        }
    }
}
