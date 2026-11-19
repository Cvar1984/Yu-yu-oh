//! The Battle Phase: attack declaration, its response window, then the
//! Damage Step. The Damage Step is still one atomic step; step 4 opens
//! windows inside it.

use crate::chain::Pending;
use crate::{
    Action, BattleDamage, BattleStep, DrawReason, Event, Game, GameResult, InstanceId, Location,
    Phase, PlayerId, Position, WinReason, WindowKind,
};

impl Game {
    /// Whether the turn player may attack this turn at all.
    pub(crate) fn attacks_allowed(&self) -> bool {
        self.turn != 1 || self.shared.rules.turn.first_player_can_attack
    }

    pub(crate) fn battle_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        for attacker in self.players[player.index()].monsters() {
            let inst = &self.instances[attacker.index()];
            if inst.position != Position::FaceUpAttack
                || inst.attacked_this_turn
                || self.attack_restricted(attacker)
            {
                continue;
            }
            self.attack_options(attacker, actions);
        }
        actions.push(Action::EndBattlePhase);
    }

    /// Every attack `attacker` may declare now: the opponent's monsters
    /// (or only the one a lock names), plus a direct attack when the
    /// opponent has none or the attacker may attack directly anyway.
    pub(crate) fn attack_options(&self, attacker: InstanceId, actions: &mut Vec<Action>) {
        let player = self.instances[attacker.index()].controller;
        let opponent = player.opponent();
        let mut targets: Vec<InstanceId> = match self.attack_lock(attacker) {
            Some(t) => vec![t],
            None => self.players[opponent.index()].monsters().collect(),
        };
        let has = |card: InstanceId, tag: crate::Tag| self.script(card).is_some_and(|s| s.has(tag));
        let mut direct = has(attacker, crate::Tag::DirectAttacker);
        // A Toon attacks directly unless the opponent controls a face-up
        // Toon, in which case it may only attack Toons.
        if has(attacker, crate::Tag::Toon) {
            let toons: Vec<InstanceId> = self.players[opponent.index()]
                .monsters()
                .filter(|&m| {
                    has(m, crate::Tag::Toon) && self.instances[m.index()].position.is_face_up()
                })
                .collect();
            if toons.is_empty() {
                direct = true;
            } else {
                targets.retain(|t| toons.contains(t));
                direct = false;
            }
        }
        if targets.is_empty() || direct {
            actions.push(Action::Attack {
                attacker,
                target: None,
            });
        }
        for target in targets {
            actions.push(Action::Attack {
                attacker,
                target: Some(target),
            });
        }
    }

    /// Marks the attack, runs the attack costs of continuous effects and
    /// opens the window Mirror Force lives in.
    pub(crate) fn declare_attack(&mut self, attacker: InstanceId, target: Option<InstanceId>) {
        self.instances[attacker.index()].attacked_this_turn = true;
        self.current_attack = Some((attacker, target));
        self.emit(Event::AttackDeclared { attacker, target });
        self.run_attack_hooks(attacker);
        self.pending.push(Pending::DamageStep { attacker, target });
        self.open_window(WindowKind::AttackDeclared, self.turn_player);
    }

    /// During a replay: a new target was chosen, or the attack stops.
    pub(crate) fn replay(&mut self, attacker: InstanceId, target: Option<Option<InstanceId>>) {
        self.pending.pop();
        self.current_attack = None;
        match target {
            Some(target) => {
                self.current_attack = Some((attacker, target));
                self.emit(Event::AttackDeclared { attacker, target });
                self.pending.push(Pending::DamageStep { attacker, target });
                self.open_window(WindowKind::AttackDeclared, self.turn_player);
            }
            None => self.emit(Event::AttackCancelled { attacker }),
        }
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
        if !attacker_ok {
            self.current_attack = None;
            self.emit(Event::AttackCancelled { attacker });
            return;
        }
        if !target_ok {
            // A replay: the attacker's controller picks another target or stops.
            self.emit(Event::AttackReplay { attacker });
            self.pending.push(Pending::Replay { attacker });
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
        let mut damage: Option<(PlayerId, i32)> = None;
        let mut destroyed: Vec<InstanceId> = Vec::new();
        match target {
            None => damage = Some((player.opponent(), atk)),
            Some(target) => {
                let defender = self.instances[target.index()].controller;
                let (target_atk, target_def) = (self.atk(target), self.def(target));
                if self.instances[target.index()].position == Position::FaceUpAttack {
                    let diff = atk - target_atk;
                    match diff.signum() {
                        1 => {
                            destroyed.push(target);
                            damage = Some((defender, diff));
                        }
                        -1 => {
                            destroyed.push(attacker);
                            damage = Some((player, -diff));
                        }
                        // Equal ATK destroys both, unless both are 0.
                        _ if atk > 0 => {
                            destroyed.push(target);
                            destroyed.push(attacker);
                        }
                        _ => {}
                    }
                } else {
                    let diff = atk - target_def;
                    match diff.signum() {
                        1 => destroyed.push(target),
                        -1 => damage = Some((player, -diff)),
                        _ => {}
                    }
                }
            }
        }
        // Damage and destruction are simultaneous in the rules; damage goes
        // first here so replacement and damage hooks see the field intact.
        if let Some((damaged, amount)) = damage
            && let Some(amount) = self.battle_damage(damaged, amount)
        {
            self.run_battle_damage_hooks(BattleDamage {
                attacker,
                target,
                damaged,
                amount,
            });
        }
        for card in destroyed {
            self.destroy_by_battle(card);
        }
        self.damage_prevented = [false, false];
        self.current_attack = None;
        self.lingering
            .retain(|l| !matches!(l.kind, crate::LingeringKind::DamageCalcStats { .. }));
        self.check_life_points();
        if self.result.is_none() {
            self.phase = Phase::Battle(BattleStep::Battle);
            self.emit(Event::PhaseEntered {
                phase: Phase::Battle(BattleStep::Battle),
            });
            self.open_window(WindowKind::AfterBattle, self.turn_player);
        }
    }

    /// Destroys `card` by battle, or the card a continuous effect names in
    /// its place (Relinquished's absorbed monster).
    fn destroy_by_battle(&mut self, card: InstanceId) {
        let victim = self.battle_destruction_replacement(card).unwrap_or(card);
        self.send_to_graveyard(victim);
        self.emit(Event::DestroyedByBattle { card: victim });
    }

    /// Deals battle damage unless prevented; returns the amount dealt.
    fn battle_damage(&mut self, player: PlayerId, amount: i32) -> Option<u32> {
        if self.damage_prevented[player.index()] {
            return None;
        }
        let amount = u32::try_from(amount).unwrap_or(0);
        if amount == 0 {
            return None;
        }
        let state = &mut self.players[player.index()];
        state.lp = state.lp.saturating_sub(amount);
        let lp = state.lp;
        self.emit(Event::DamageTaken { player, amount, lp });
        Some(amount)
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
