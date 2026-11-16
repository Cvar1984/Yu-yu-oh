//! The Battle Phase: attack declaration, its response window, then the
//! Damage Step. The Damage Step is still one atomic step; step 4 opens
//! windows inside it.

use crate::chain::Pending;
use crate::{
    Action, BattleStep, DrawReason, Event, Game, GameResult, InstanceId, Location, Phase, PlayerId,
    Position, WinReason, WindowKind,
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
            if inst.position != Position::FaceUpAttack
                || inst.attacked_this_turn
                || self.attack_restricted(attacker)
            {
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

    /// Marks the attack and opens the window Mirror Force lives in.
    pub(crate) fn declare_attack(&mut self, attacker: InstanceId, target: Option<InstanceId>) {
        self.instances[attacker.index()].attacked_this_turn = true;
        self.emit(Event::AttackDeclared { attacker, target });
        self.pending.push(Pending::DamageStep { attacker, target });
        self.open_window(WindowKind::AttackDeclared, self.turn_player);
    }

    /// After the attack-declaration window. The attack goes ahead only if
    /// the attacker is still a face-up Attack Position monster and the
    /// target, if any, is still on the field (no replays yet). A face-down
    /// target is flipped, then the damage-calculation window opens.
    pub(crate) fn damage_step(&mut self, attacker: InstanceId, target: Option<InstanceId>) {
        let attacker_ok = matches!(
            self.instances[attacker.index()].location,
            Location::MonsterZone(_)
        ) && self.instances[attacker.index()].position == Position::FaceUpAttack;
        let target_ok = target
            .is_none_or(|t| matches!(self.instances[t.index()].location, Location::MonsterZone(_)));
        if !attacker_ok || !target_ok {
            self.emit(Event::AttackCancelled { attacker });
            return;
        }
        self.enter_phase(Phase::Battle(BattleStep::Damage));
        if let Some(target) = target
            && self.instances[target.index()].position == Position::FaceDownDefense
        {
            self.instances[target.index()].position = Position::FaceUpDefense;
            self.emit(Event::Flipped { card: target });
        }
        self.pending.push(Pending::DamageCalc { attacker, target });
        self.open_window(WindowKind::DamageCalculation, self.turn_player);
    }

    /// Prevents all battle damage to `player` for the current battle.
    pub fn prevent_battle_damage(&mut self, player: PlayerId) {
        self.damage_prevented[player.index()] = true;
        self.emit(Event::BattleDamagePrevented { player });
    }

    /// After the damage-calculation window: compare, destroy, inflict.
    pub(crate) fn damage_calculation(&mut self, attacker: InstanceId, target: Option<InstanceId>) {
        let player = self.instances[attacker.index()].controller;
        let atk = self.atk(attacker);
        match target {
            None => self.battle_damage(player.opponent(), atk),
            Some(target) => {
                let defender = self.instances[target.index()].controller;
                let (target_atk, target_def) = (self.atk(target), self.def(target));
                if self.instances[target.index()].position == Position::FaceUpAttack {
                    let diff = atk - target_atk;
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
                    let diff = atk - target_def;
                    match diff.signum() {
                        1 => self.destroy_by_battle(target),
                        -1 => self.battle_damage(player, -diff),
                        _ => {}
                    }
                }
            }
        }
        self.damage_prevented = [false, false];
        self.check_life_points();
        if self.result.is_none() {
            self.phase = Phase::Battle(BattleStep::Battle);
            self.emit(Event::PhaseEntered {
                phase: Phase::Battle(BattleStep::Battle),
            });
            self.open_window(WindowKind::AfterBattle, self.turn_player);
        }
    }

    fn destroy_by_battle(&mut self, card: InstanceId) {
        self.send_to_graveyard(card);
        self.emit(Event::DestroyedByBattle { card });
    }

    fn battle_damage(&mut self, player: PlayerId, amount: i32) {
        if self.damage_prevented[player.index()] {
            return;
        }
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
