//! Normal Summon, Set, Tribute Summon and Tribute Set, in two halves: the
//! declaration (tributes paid, negation window) and the completion.

use crate::chain::{Pending, PendingSummon, SummonKind};
use crate::{
    Action, Event, Game, InstanceId, Location, Phase, PlayerId, Position, Tag, WindowKind,
};

impl Game {
    /// Number of tributes a monster of `level` needs, from the rules.
    pub fn tributes_needed(&self, level: u8) -> usize {
        let summon = &self.shared.rules.summon;
        match level {
            0..=4 => 0,
            5..=6 => summon.tributes_level_5_6,
            _ => summon.tributes_level_7_plus,
        }
    }

    /// Whether `player` may still Normal Summon, Tribute Summon or Set in
    /// the current phase.
    fn may_summon(&self, player: PlayerId) -> bool {
        let rules = &self.shared.rules;
        let state = &self.players[player.index()];
        let phase_ok = match self.phase {
            Phase::Main1 => true,
            Phase::Main2 => rules.summon.summon_in_main_phase_2,
            _ => false,
        };
        let cap = rules.summon.max_monsters_entering_per_turn;
        phase_ok
            && usize::from(state.summons_used) < rules.summon.normal_or_tribute_per_turn
            && (cap == 0 || usize::from(state.monsters_entered) < cap)
    }

    pub(crate) fn summon_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !self.may_summon(player) {
            return;
        }
        let state = &self.players[player.index()];
        let free_zones = (0..state.monster_zones.len())
            .filter(|&i| state.monster_zones[i].is_none() && !self.zone_locked(player, i))
            .count();
        let mut on_field: Vec<InstanceId> = state
            .monsters()
            .filter(|&m| !self.script(m).is_some_and(|s| s.has(Tag::NoTributeSummon)))
            .collect();
        // Soul Exchange: the opponent's monster joins, and must be used.
        let must = self.must_tribute(player);
        // Scapegoat leaves Sets open, unless Light of Intervention makes
        // them summons too.
        let locked = self.summons_locked(player);
        let sets_locked = locked && self.face_down_forbidden();
        on_field.extend(must);
        for &card in &state.hand {
            let stats = self.stats(card);
            if !stats.can_normal_summon(self.script(card)) {
                continue;
            }
            let needed = self.tributes_needed(stats.level);
            if needed > on_field.len() {
                continue;
            }
            for tributes in combinations(&on_field, needed) {
                let freed = tributes.iter().filter(|&&t| Some(t) != must).count();
                if free_zones + freed == 0
                    || must.is_some_and(|m| needed > 0 && !tributes.contains(&m))
                {
                    continue;
                }
                for face_down in [false, true] {
                    if (!face_down && locked) || (face_down && sets_locked) {
                        continue;
                    }
                    actions.push(Action::Summon {
                        card,
                        tributes: tributes.clone(),
                        face_down,
                    });
                }
            }
        }
    }

    /// Pays the tributes, uses the summon, and opens the negation window.
    /// The card stays in the hand until the summon completes. A Set is not
    /// a summon, so it has no negation window and completes at once.
    pub(crate) fn declare_summon(
        &mut self,
        player: PlayerId,
        card: InstanceId,
        tributes: &[InstanceId],
        face_down: bool,
    ) {
        for &tribute in tributes {
            self.send_to_graveyard(tribute);
            self.emit(Event::Tributed {
                player,
                card: tribute,
            });
        }
        self.players[player.index()].summons_used += 1;
        // Under Light of Intervention a Set is a face-up Defense summon.
        let position = match (face_down, self.face_down_forbidden()) {
            (false, _) => Position::FaceUpAttack,
            (true, false) => Position::FaceDownDefense,
            (true, true) => Position::FaceUpDefense,
        };
        let face_down = position == Position::FaceDownDefense;
        let pending = PendingSummon {
            player,
            card,
            kind: SummonKind::Normal { position },
            face_down,
            negated: false,
        };
        if face_down {
            self.complete_summon(pending);
            return;
        }
        self.emit(Event::SummonDeclared {
            player,
            card,
            face_down,
        });
        self.pending.push(Pending::CompleteSummon(pending));
        self.open_window(WindowKind::SummonDeclared, self.turn_player);
    }

    /// A summon procedure from the hand: pays its cost (the tributes),
    /// then opens the summon-negation window like a Normal Summon. It is
    /// not an effect, so it starts no chain.
    pub(crate) fn declare_procedure_summon(
        &mut self,
        player: PlayerId,
        card: InstanceId,
        effect: u8,
        cost: Vec<InstanceId>,
        position: Position,
    ) {
        if let Some(pay) = self.effect(card, effect).cost {
            let mut ctx = crate::EffectCtx {
                source: card,
                controller: player,
                targets: Vec::new(),
                cost,
                event: None,
                value: 0,
            };
            self.resolving = Some(player);
            pay(self, &mut ctx);
            self.resolving = None;
        }
        self.emit(Event::SummonDeclared {
            player,
            card,
            face_down: false,
        });
        self.pending.push(Pending::CompleteSummon(PendingSummon {
            player,
            card,
            kind: SummonKind::Procedure { position },
            face_down: false,
            negated: false,
        }));
        self.open_window(WindowKind::SummonDeclared, self.turn_player);
    }

    pub(crate) fn declare_flip_summon(&mut self, player: PlayerId, card: InstanceId) {
        if self.shared.rules.summon.flip_summon_counts_toward_limit {
            self.players[player.index()].summons_used += 1;
        }
        self.emit(Event::SummonDeclared {
            player,
            card,
            face_down: false,
        });
        self.pending.push(Pending::CompleteSummon(PendingSummon {
            player,
            card,
            kind: SummonKind::Flip,
            face_down: false,
            negated: false,
        }));
        self.open_window(WindowKind::SummonDeclared, self.turn_player);
    }

    /// After the negation window: the monster enters the field (or is
    /// flipped), then the summon-success window opens.
    pub(crate) fn complete_summon(&mut self, ps: PendingSummon) {
        let PendingSummon {
            player,
            card,
            kind,
            face_down: _,
            negated,
        } = ps;
        if negated {
            self.send_to_graveyard(card);
            return;
        }
        match kind {
            SummonKind::Normal { position } => self.place_summoned(player, card, position),
            SummonKind::Procedure { position } => {
                self.special_summon_properly(card, player, position);
            }
            SummonKind::Flip => {
                self.players[player.index()].summoned = true;
                let inst = &mut self.instances[card.index()];
                inst.position = Position::FaceUpAttack;
                inst.position_changed_this_turn = true;
                self.emit(Event::FlipSummoned { player, card });
            }
        }
        self.open_window(WindowKind::SummonSuccess, self.turn_player);
    }
}

impl Game {
    /// Puts a Normal Summoned or Set monster from the hand on the field.
    fn place_summoned(&mut self, player: PlayerId, card: InstanceId, position: Position) {
        self.remove_from_current_place(card);
        let zone = self
            .free_monster_zone(player)
            .expect("a free zone was checked when the summon was offered");
        let state = &mut self.players[player.index()];
        state.monster_zones[zone] = Some(card);
        state.monsters_entered += 1;
        let face_down = position == Position::FaceDownDefense;
        state.summoned |= !face_down;
        let turn = self.turn;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::MonsterZone(zone as u8);
        inst.position = position;
        inst.controller = player;
        inst.turn_entered_field = Some(turn);
        inst.turn_set = face_down.then_some(turn);
        inst.position_changed_this_turn = false;
        inst.attacks_this_turn = 0;
        inst.counter = 0;
        self.emit(Event::Summoned {
            player,
            card,
            position,
        });
    }

    /// A Normal Summon or Set an effect grants on its resolution (Ultimate
    /// Offering): `tributes` go first, and the turn's own Normal Summon is
    /// not used. It happens inside the chain, so it opens no summon windows
    /// of its own; the summon is seen once the chain has resolved. Returns
    /// `false`, doing nothing, if `card` is not in its controller's hand or
    /// no Monster Zone would be free.
    pub fn normal_summon_by_effect(
        &mut self,
        card: InstanceId,
        tributes: &[InstanceId],
        face_down: bool,
    ) -> bool {
        let player = self.instances[card.index()].controller;
        let frees = tributes.iter().any(|&t| {
            matches!(self.instances[t.index()].location, Location::MonsterZone(_))
                && self.instances[t.index()].controller == player
        });
        if self.instances[card.index()].location != Location::Hand
            || (self.free_monster_zone(player).is_none() && !frees)
        {
            return false;
        }
        for &tribute in tributes {
            self.send_to_graveyard(tribute);
            self.emit(Event::Tributed {
                player,
                card: tribute,
            });
        }
        let position = match (face_down, self.face_down_forbidden()) {
            (false, _) => Position::FaceUpAttack,
            (true, false) => Position::FaceDownDefense,
            (true, true) => Position::FaceUpDefense,
        };
        self.place_summoned(player, card, position);
        true
    }
}

/// All `k`-element subsets of `items`, in index order.
fn combinations(items: &[InstanceId], k: usize) -> Vec<Vec<InstanceId>> {
    fn go(
        items: &[InstanceId],
        k: usize,
        start: usize,
        cur: &mut Vec<InstanceId>,
        out: &mut Vec<Vec<InstanceId>>,
    ) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..items.len() {
            cur.push(items[i]);
            go(items, k, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(items, k, 0, &mut Vec::new(), &mut out);
    out
}
