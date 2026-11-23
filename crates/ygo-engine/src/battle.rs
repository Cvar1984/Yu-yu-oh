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
                || inst.attacks_this_turn >= self.attacks_per_battle_phase(attacker)
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
        targets.retain(|&t| !self.cannot_be_attacked(t));
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

    /// The monsters `attacker` could choose as an attack target now.
    pub fn attack_targets(&self, attacker: InstanceId) -> Vec<InstanceId> {
        let mut actions = Vec::new();
        self.attack_options(attacker, &mut actions);
        actions
            .into_iter()
            .filter_map(|a| match a {
                Action::Attack { target, .. } => target,
                _ => None,
            })
            .collect()
    }

    /// Moves the attack under way to `target`, without a replay (Shift).
    /// Returns `false` if no attack is waiting for its Damage Step.
    pub fn retarget_attack(&mut self, target: InstanceId) -> bool {
        let Some((attacker, _)) = self.current_attack else {
            return false;
        };
        let step = self.pending.iter_mut().rev().find_map(|p| match p {
            Pending::DamageStep {
                attacker: a,
                target,
            } if *a == attacker => Some(target),
            _ => None,
        });
        let Some(slot) = step else {
            return false;
        };
        *slot = Some(target);
        self.current_attack = Some((attacker, Some(target)));
        self.emit(Event::AttackRetargeted { attacker, target });
        true
    }

    /// How many attacks `attacker` may declare each Battle Phase.
    fn attacks_per_battle_phase(&self, attacker: InstanceId) -> u8 {
        if self
            .script(attacker)
            .is_some_and(|s| s.has(crate::Tag::SecondAttack))
        {
            2
        } else {
            1
        }
    }

    /// Marks the attack, runs the attack costs of continuous effects and
    /// opens the window Mirror Force lives in.
    pub(crate) fn declare_attack(&mut self, attacker: InstanceId, target: Option<InstanceId>) {
        let inst = &mut self.instances[attacker.index()];
        inst.attacks_this_turn = inst.attacks_this_turn.saturating_add(1);
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
                        1 => {
                            destroyed.push(target);
                            if self.pierces(attacker) {
                                damage = Some((defender, diff));
                            }
                        }
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
            self.emit(Event::BattleDamageDealt {
                attacker,
                target,
                damaged,
                amount,
            });
            self.run_battle_damage_hooks(BattleDamage {
                attacker,
                target,
                damaged,
                amount,
            });
        }
        for card in destroyed {
            let controller = self.instances[card.index()].controller;
            let by = if card == attacker {
                target.unwrap_or(attacker)
            } else {
                attacker
            };
            if !self.battle_shielded(controller) {
                self.destroy_by_battle(card, by);
            }
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
    fn destroy_by_battle(&mut self, card: InstanceId, by: InstanceId) {
        let victim = self.battle_destruction_replacement(card).unwrap_or(card);
        self.send_to_graveyard(victim);
        self.instances[victim.index()].destroyed_by_battle = Some((by, self.turn));
        self.emit(Event::DestroyedByBattle { card: victim });
    }

    /// Deals battle damage unless prevented; returns the amount dealt.
    fn battle_damage(&mut self, player: PlayerId, amount: i32) -> Option<u32> {
        // A player's battle damage always comes from the opponent's
        // monsters, so a shield (Waboku) prevents all of it.
        if self.damage_prevented[player.index()] || self.battle_shielded(player) {
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
