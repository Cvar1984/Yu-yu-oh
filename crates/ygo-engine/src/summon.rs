//! Normal Summon, Set, Tribute Summon and Tribute Set.

use crate::{Action, Event, Game, InstanceId, Location, Phase, PlayerId, Position};

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
        let free_zones = state.monster_zones.iter().filter(|z| z.is_none()).count();
        let on_field: Vec<InstanceId> = state.monsters().collect();
        for &card in &state.hand {
            let stats = self.stats(card);
            if !stats.can_normal_summon(self.script(card)) {
                continue;
            }
            let needed = self.tributes_needed(stats.level);
            if needed > on_field.len() || free_zones + needed == 0 {
                continue;
            }
            for tributes in combinations(&on_field, needed) {
                for face_down in [false, true] {
                    actions.push(Action::Summon {
                        card,
                        tributes: tributes.clone(),
                        face_down,
                    });
                }
            }
        }
    }

    pub(crate) fn summon(
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
        let position = if face_down {
            Position::FaceDownDefense
        } else {
            Position::FaceUpAttack
        };
        self.remove_from_current_place(card);
        let state = &mut self.players[player.index()];
        let zone = state
            .monster_zones
            .iter()
            .position(|z| z.is_none())
            .expect("a free zone was checked when the action was offered");
        state.monster_zones[zone] = Some(card);
        state.summons_used += 1;
        state.monsters_entered += 1;
        let turn = self.turn;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::MonsterZone(zone as u8);
        inst.position = position;
        inst.controller = player;
        inst.turn_entered_field = Some(turn);
        inst.turn_set = face_down.then_some(turn);
        inst.position_changed_this_turn = false;
        inst.attacked_this_turn = false;
        self.emit(Event::Summoned {
            player,
            card,
            position,
        });
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
