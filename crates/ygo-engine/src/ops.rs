//! Operations card scripts call. Every one emits the events it causes
//! and keeps the instance table and zones consistent.

use rand_core::Rng;

use crate::{
    Effect, EffectKind, Event, Game, GameResult, InstanceId, Lingering, LingeringKind, Location,
    PlayerId, Position, Source, StatMod, Tag, WinReason,
};

/// A continuous modifier on the field: its source and its function.
pub(crate) type Modifier = (Source, fn(&Game, Source, InstanceId) -> StatMod);

impl Game {
    pub(crate) fn emit(&mut self, event: Event) {
        self.events.push(event.clone());
        self.recent.push(event);
    }

    /// Draws up to `n` cards for `player`; returns how many were drawn.
    /// Drawing from an empty Deck outside the Draw Phase is not a loss.
    pub fn draw(&mut self, player: PlayerId, n: usize) -> usize {
        let mut drawn = 0;
        for _ in 0..n {
            match self.draw_card(player) {
                Some(card) => {
                    self.emit(Event::Drew { player, card });
                    drawn += 1;
                    self.check_exodia(player);
                }
                None => {
                    self.emit(Event::DeckEmpty { player });
                    break;
                }
            }
        }
        drawn
    }

    /// Removes a card from play (any location) to its owner's banished pile.
    pub fn banish(&mut self, card: InstanceId) {
        let from = self.instances[card.index()].location;
        if from == Location::Banished {
            return;
        }
        self.remove_from_current_place(card);
        if from.on_field() {
            self.on_leave_field(card);
        }
        let owner = self.instances[card.index()].owner;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::Banished;
        inst.position = Position::FaceUp;
        inst.controller = owner;
        inst.turn_set = None;
        inst.turn_entered_field = None;
        self.players[owner.index()].banished.push(card);
        self.emit(Event::Banished { card, from });
    }

    pub fn banished(&self, player: PlayerId) -> &[InstanceId] {
        &self.players[player.index()].banished
    }

    /// A coin toss from the game's seeded RNG; `true` is heads.
    pub fn coin(&mut self, player: PlayerId) -> bool {
        let heads = self.rng.next_u32() & 1 == 1;
        self.emit(Event::CoinTossed { player, heads });
        heads
    }

    /// A uniformly random index below `n` from the game's seeded RNG.
    pub fn random_index(&mut self, n: usize) -> usize {
        (self.rng.next_u64() % n as u64) as usize
    }

    /// Whether both hands are public right now (Ceremonial Bell).
    pub fn hands_revealed(&self) -> bool {
        let mut revealed = false;
        self.for_each_face_up_script(|script| {
            if script.has(Tag::RevealHands) {
                revealed = true;
            }
        });
        revealed
    }

    fn for_each_face_up_script(&self, mut f: impl FnMut(&crate::CardScript)) {
        for p in PlayerId::BOTH {
            let state = &self.players[p.index()];
            let cards = state
                .monster_zones
                .iter()
                .chain(state.spell_trap_zones.iter())
                .flatten()
                .copied()
                .chain(state.field_zone);
            for card in cards {
                let inst = &self.instances[card.index()];
                if inst.position.is_face_up()
                    && let Some(script) = self.shared.script(inst.card)
                {
                    f(script);
                }
            }
        }
    }

    pub fn fusion_summons_allowed(&self) -> bool {
        self.shared.rules.summon.fusion_summons
    }

    pub fn ritual_summons_allowed(&self) -> bool {
        self.shared.rules.summon.ritual_summons
    }

    /// Fusion Summons `fusion` from the Extra Deck, sending the materials
    /// (from the hand or field) to the Graveyard first. The caller checks
    /// the materials; this checks `can_special_summon`.
    pub fn fusion_summon(
        &mut self,
        player: PlayerId,
        fusion: InstanceId,
        materials: &[InstanceId],
        position: Position,
    ) -> bool {
        if !self.can_special_summon(player) || materials.is_empty() {
            return false;
        }
        for &m in materials {
            self.send_to_graveyard(m);
        }
        if !self.special_summon(fusion, player, position) {
            return false;
        }
        self.emit(Event::FusionSummoned {
            player,
            card: fusion,
            materials: materials.to_vec(),
        });
        true
    }

    /// Ritual Summons `monster` from the hand, tributing `tributes` from
    /// the hand or field. The caller checks the levels.
    pub fn ritual_summon(
        &mut self,
        player: PlayerId,
        monster: InstanceId,
        tributes: &[InstanceId],
        position: Position,
    ) -> bool {
        if !self.can_special_summon(player) {
            return false;
        }
        for &t in tributes {
            self.send_to_graveyard(t);
            self.emit(Event::Tributed { player, card: t });
        }
        if !self.special_summon(monster, player, position) {
            return false;
        }
        self.emit(Event::RitualSummoned {
            player,
            card: monster,
            tributes: tributes.to_vec(),
        });
        true
    }

    /// Gives `card` a temporary ATK/DEF change until this turn's End Phase.
    pub fn modify_until_end_phase(
        &mut self,
        source: InstanceId,
        card: InstanceId,
        atk: i32,
        def: i32,
    ) {
        let turn = self.turn;
        self.lingering.push(Lingering {
            source,
            kind: LingeringKind::Stats { card, atk, def },
            expires_turn: turn,
        });
    }

    /// Sends a card on the field to the Graveyard by a card effect.
    pub fn destroy(&mut self, card: InstanceId, source: InstanceId) {
        if !self.on_field(card) {
            return;
        }
        self.send_to_graveyard(card);
        self.emit(Event::DestroyedByEffect { card, source });
    }

    /// Sends a card from the hand to the Graveyard.
    pub fn discard(&mut self, card: InstanceId) {
        let inst = &self.instances[card.index()];
        if inst.location != Location::Hand {
            return;
        }
        let player = inst.controller;
        self.send_to_graveyard(card);
        self.emit(Event::Discarded { player, card });
    }

    /// Moves a card from anywhere (Deck, Graveyard, field) to its owner's hand.
    pub fn add_to_hand(&mut self, card: InstanceId) {
        let from = self.instances[card.index()].location;
        if from == Location::Hand {
            return;
        }
        self.remove_from_current_place(card);
        if from.on_field() {
            self.on_leave_field(card);
        }
        let owner = self.instances[card.index()].owner;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::Hand;
        inst.position = Position::FaceDown;
        inst.controller = owner;
        inst.turn_set = None;
        inst.turn_entered_field = None;
        self.players[owner.index()].hand.push(card);
        self.emit(Event::AddedToHand {
            player: owner,
            card,
            from,
        });
        self.check_exodia(owner);
    }

    /// Wins the game for `player` if their hand holds all five Exodia
    /// cards and the rules allow it.
    pub(crate) fn check_exodia(&mut self, player: PlayerId) {
        if !self.shared.rules.win.exodia || self.result.is_some() {
            return;
        }
        let mut head = false;
        let mut pieces = std::collections::BTreeSet::new();
        for &card in &self.players[player.index()].hand {
            if let Some(script) = self.script(card) {
                if script.has(Tag::ExodiaHead) {
                    head = true;
                }
                if script.has(Tag::ExodiaPiece) {
                    pieces.insert(self.instances[card.index()].card);
                }
            }
        }
        if head && pieces.len() >= 4 {
            self.finish(GameResult::Win {
                winner: player,
                reason: WinReason::Exodia,
            });
        }
    }

    /// Shuffles `player`'s Deck with the game's seeded RNG.
    pub fn shuffle_deck(&mut self, player: PlayerId) {
        let deck = &mut self.players[player.index()].deck;
        for i in (1..deck.len()).rev() {
            let j = (self.rng.next_u64() % (i as u64 + 1)) as usize;
            deck.swap(i, j);
        }
        self.emit(Event::DeckShuffled { player });
    }

    /// Cards in `player`'s Deck, top card last.
    pub fn deck(&self, player: PlayerId) -> &[InstanceId] {
        &self.players[player.index()].deck
    }

    pub fn text_version(&self) -> ygo_data::TextVersion {
        self.shared.rules.cards.text_version
    }

    /// Whether `player` may have one more monster Special Summoned now.
    pub fn can_special_summon(&self, player: PlayerId) -> bool {
        let rules = &self.shared.rules.summon;
        let state = &self.players[player.index()];
        let cap = rules.max_monsters_entering_per_turn;
        self.free_monster_zone(player).is_some()
            && (cap == 0 || usize::from(state.monsters_entered) < cap)
            && (!rules.special_summon_counts_toward_limit
                || usize::from(state.summons_used) < rules.normal_or_tribute_per_turn)
    }

    /// Special Summons `card` from wherever it is to `player`'s field.
    /// Returns `false` and does nothing if `can_special_summon` is false.
    pub fn special_summon(
        &mut self,
        card: InstanceId,
        player: PlayerId,
        position: Position,
    ) -> bool {
        if !self.can_special_summon(player) {
            return false;
        }
        self.remove_from_current_place(card);
        let zone = self.free_monster_zone(player).expect("checked above");
        let rules = &self.shared.rules.summon;
        let counts = rules.special_summon_counts_toward_limit;
        let state = &mut self.players[player.index()];
        state.monster_zones[zone] = Some(card);
        state.monsters_entered += 1;
        if counts {
            state.summons_used += 1;
        }
        let turn = self.turn;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::MonsterZone(zone as u8);
        inst.position = position;
        inst.controller = player;
        inst.turn_entered_field = Some(turn);
        inst.turn_set = None;
        inst.position_changed_this_turn = false;
        inst.attacked_this_turn = false;
        self.emit(Event::SpecialSummoned {
            player,
            card,
            position,
        });
        true
    }

    /// Moves a monster to `to`'s field, keeping its position. Returns
    /// `false` if `to` has no free Monster Zone.
    pub fn change_control(&mut self, card: InstanceId, to: PlayerId) -> bool {
        let inst = &self.instances[card.index()];
        if !matches!(inst.location, Location::MonsterZone(_)) || inst.controller == to {
            return false;
        }
        let Some(zone) = self.free_monster_zone(to) else {
            return false;
        };
        self.remove_from_current_place(card);
        self.players[to.index()].monster_zones[zone] = Some(card);
        let turn = self.turn;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::MonsterZone(zone as u8);
        inst.controller = to;
        inst.turn_entered_field = Some(turn);
        inst.position_changed_this_turn = false;
        inst.attacked_this_turn = false;
        self.emit(Event::ControlChanged { card, to });
        true
    }

    pub fn gain_lp(&mut self, player: PlayerId, amount: u32) {
        let state = &mut self.players[player.index()];
        state.lp = state.lp.saturating_add(amount);
        let lp = state.lp;
        self.emit(Event::LifePointsGained { player, amount, lp });
    }

    /// Pays Life Points as a cost. Scripts check affordability in `timing`;
    /// a payment that reaches 0 still ends the game.
    pub fn pay_lp(&mut self, player: PlayerId, amount: u32) {
        let state = &mut self.players[player.index()];
        state.lp = state.lp.saturating_sub(amount);
        let lp = state.lp;
        self.emit(Event::LifePointsPaid { player, amount, lp });
        self.check_life_points();
    }

    /// Inflicts effect damage; the game may end at once.
    pub fn take_damage(&mut self, player: PlayerId, amount: u32) {
        let state = &mut self.players[player.index()];
        state.lp = state.lp.saturating_sub(amount);
        let lp = state.lp;
        self.emit(Event::DamageTaken { player, amount, lp });
        self.check_life_points();
    }

    pub fn add_lingering(&mut self, effect: Lingering) {
        self.lingering.push(effect);
    }

    /// Runs and removes the lingering effects that expire in this End Phase.
    pub(crate) fn expire_lingering(&mut self) {
        let turn = self.turn;
        let due: Vec<Lingering> = self
            .lingering
            .iter()
            .copied()
            .filter(|l| l.expires_turn <= turn)
            .collect();
        self.lingering.retain(|l| l.expires_turn > turn);
        for l in due {
            match l.kind {
                LingeringKind::ReturnControl { card, to } => {
                    self.change_control(card, to);
                }
                LingeringKind::Stats { .. } => {}
            }
        }
    }

    pub fn lingering(&self) -> &[Lingering] {
        &self.lingering
    }

    // ----- continuous effects -----

    /// Calls `f` for every continuous effect of a face-up card on the
    /// field, without allocating.
    fn for_each_continuous(&self, mut f: impl FnMut(Source, &Effect)) {
        for p in PlayerId::BOTH {
            let state = &self.players[p.index()];
            let cards = state
                .monster_zones
                .iter()
                .chain(state.spell_trap_zones.iter())
                .flatten()
                .copied()
                .chain(state.field_zone);
            for card in cards {
                let inst = &self.instances[card.index()];
                if !inst.position.is_face_up() {
                    continue;
                }
                let Some(script) = self.shared.script(inst.card) else {
                    continue;
                };
                let source = Source {
                    card,
                    controller: inst.controller,
                };
                for effect in &script.effects {
                    if effect.kind == EffectKind::Continuous {
                        f(source, effect);
                    }
                }
            }
        }
    }

    /// The continuous modifiers currently on the field, collected once so
    /// many cards' stats can be computed without rescanning.
    pub(crate) fn modifiers(&self) -> Vec<Modifier> {
        let mut out = Vec::new();
        self.for_each_continuous(|source, effect| {
            if let Some(modify) = effect.modify {
                out.push((source, modify));
            }
        });
        out
    }

    /// ATK and DEF of `card` under the given modifiers.
    pub(crate) fn stats_with(&self, modifiers: &[Modifier], card: InstanceId) -> (i32, i32) {
        let base = self.stats(card);
        let mut total = self.temporary_mod(card);
        for (source, modify) in modifiers {
            let m = modify(self, *source, card);
            total.atk += m.atk;
            total.def += m.def;
        }
        ((base.atk + total.atk).max(0), (base.def + total.def).max(0))
    }

    fn temporary_mod(&self, card: InstanceId) -> StatMod {
        let mut total = StatMod::default();
        for l in &self.lingering {
            if let LingeringKind::Stats { card: c, atk, def } = l.kind
                && c == card
            {
                total.atk += atk;
                total.def += def;
            }
        }
        total
    }

    fn stat_mod(&self, card: InstanceId) -> StatMod {
        let mut total = self.temporary_mod(card);
        self.for_each_continuous(|source, effect| {
            if let Some(modify) = effect.modify {
                let m = modify(self, source, card);
                total.atk += m.atk;
                total.def += m.def;
            }
        });
        total
    }

    /// Current ATK: base plus every continuous modifier, never below 0.
    pub fn atk(&self, card: InstanceId) -> i32 {
        (self.stats(card).atk + self.stat_mod(card).atk).max(0)
    }

    /// Current DEF: base plus every continuous modifier, never below 0.
    pub fn def(&self, card: InstanceId) -> i32 {
        (self.stats(card).def + self.stat_mod(card).def).max(0)
    }

    /// Runs every continuous `on_attack` hook for a declared attack.
    pub(crate) fn run_attack_hooks(&mut self, attacker: InstanceId) {
        let mut hooks: Vec<(Source, crate::AttackHook)> = Vec::new();
        self.for_each_continuous(|source, effect| {
            if let Some(h) = effect.on_attack {
                hooks.push((source, h));
            }
        });
        for (source, hook) in hooks {
            hook(self, source, attacker);
        }
    }

    /// The only monster `attacker` may attack, if a continuous effect
    /// says so and that monster is still on the field.
    pub fn attack_lock(&self, attacker: InstanceId) -> Option<InstanceId> {
        let mut lock = None;
        self.for_each_continuous(|source, effect| {
            if lock.is_none()
                && let Some(f) = effect.attack_lock
                && let Some(t) = f(self, source, attacker)
                && self.on_field(t)
            {
                lock = Some(t);
            }
        });
        lock
    }

    /// Whether some continuous effect forbids `card` from attacking.
    pub fn attack_restricted(&self, card: InstanceId) -> bool {
        let mut restricted = false;
        self.for_each_continuous(|source, effect| {
            if !restricted && effect.restrict.is_some_and(|r| r(self, source, card)) {
                restricted = true;
            }
        });
        restricted
    }

    /// Attaches an Equip Spell on the field to a face-up monster.
    pub fn equip(&mut self, equip: InstanceId, to: InstanceId) -> bool {
        if !matches!(
            self.instances[equip.index()].location,
            Location::SpellTrapZone(_)
        ) || !matches!(
            self.instances[to.index()].location,
            Location::MonsterZone(_)
        ) || !self.instances[to.index()].position.is_face_up()
        {
            return false;
        }
        self.instances[equip.index()].equipped_to = Some(to);
        self.emit(Event::Equipped { card: equip, to });
        true
    }

    /// Turns a face-down Defense Position monster face-up by an effect.
    /// Its FLIP effect fires in the next window.
    pub fn flip_face_up(&mut self, card: InstanceId) -> bool {
        let inst = &mut self.instances[card.index()];
        if inst.position != Position::FaceDownDefense {
            return false;
        }
        inst.position = Position::FaceUpDefense;
        self.emit(Event::Flipped { card });
        true
    }

    pub fn equip_target(&self, equip: InstanceId) -> Option<InstanceId> {
        self.instances[equip.index()].equipped_to
    }

    /// Spells and Traps `player` controls on the field, Field Zone included.
    pub fn spells_traps_controlled(&self, player: PlayerId) -> Vec<InstanceId> {
        let state = &self.players[player.index()];
        state
            .spell_trap_zones
            .iter()
            .flatten()
            .copied()
            .chain(state.field_zone)
            .collect()
    }

    pub fn field_spell(&self, player: PlayerId) -> Option<InstanceId> {
        self.players[player.index()].field_zone
    }

    pub fn counter(&self, card: InstanceId) -> u8 {
        self.instances[card.index()].counter
    }

    pub fn set_counter(&mut self, card: InstanceId, value: u8) {
        self.instances[card.index()].counter = value;
    }

    /// Places a card from anywhere on top of its owner's Deck.
    pub fn place_on_top_of_deck(&mut self, card: InstanceId) {
        self.remove_from_current_place(card);
        let owner = self.instances[card.index()].owner;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::Deck;
        inst.position = Position::FaceDown;
        inst.controller = owner;
        inst.turn_set = None;
        inst.turn_entered_field = None;
        inst.equipped_to = None;
        inst.counter = 0;
        self.players[owner.index()].deck.push(card);
        self.emit(Event::PlacedOnTopOfDeck {
            player: owner,
            card,
        });
    }

    /// Activates a Field Spell from the hand: it takes `player`'s Field
    /// Zone, and `field.field_spells_on_field` says how many may stay on
    /// the whole field, oldest and the opponent's going first.
    pub(crate) fn place_field_spell(&mut self, player: PlayerId, card: InstanceId) {
        let limit = self.shared.rules.field.field_spells_on_field.max(1);
        if let Some(old) = self.players[player.index()].field_zone {
            self.send_to_graveyard(old);
        }
        let mut on_field: Vec<InstanceId> = PlayerId::BOTH
            .into_iter()
            .filter_map(|p| self.players[p.index()].field_zone)
            .collect();
        while on_field.len() + 1 > limit {
            let victim = on_field.remove(0);
            self.send_to_graveyard(victim);
        }
        self.remove_from_current_place(card);
        self.players[player.index()].field_zone = Some(card);
        let turn = self.turn;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::FieldZone;
        inst.position = Position::FaceUp;
        inst.controller = player;
        inst.turn_set = None;
        inst.turn_entered_field = Some(turn);
    }

    /// Housekeeping when a card leaves the field: its equips go to the
    /// Graveyard and its counters reset. Lingering effects it created
    /// stay, since a resolved Spell's effect outlives the card.
    fn on_leave_field(&mut self, card: InstanceId) {
        let equips: Vec<InstanceId> = self
            .instances
            .iter()
            .enumerate()
            .filter(|(_, i)| i.equipped_to == Some(card) && i.location.on_field())
            .map(|(i, _)| InstanceId(i as u8))
            .collect();
        for e in equips {
            self.send_to_graveyard(e);
        }
        let inst = &mut self.instances[card.index()];
        inst.equipped_to = None;
        inst.counter = 0;
    }

    // ----- queries scripts use -----

    pub fn on_field(&self, card: InstanceId) -> bool {
        self.instances[card.index()].location.on_field()
    }

    pub fn monsters_controlled(&self, player: PlayerId) -> Vec<InstanceId> {
        self.players[player.index()].monsters().collect()
    }

    pub fn graveyard(&self, player: PlayerId) -> &[InstanceId] {
        &self.players[player.index()].graveyard
    }

    pub fn hand(&self, player: PlayerId) -> &[InstanceId] {
        &self.players[player.index()].hand
    }

    pub fn deck_size(&self, player: PlayerId) -> usize {
        self.players[player.index()].deck.len()
    }

    pub fn free_monster_zone(&self, player: PlayerId) -> Option<usize> {
        self.players[player.index()]
            .monster_zones
            .iter()
            .position(|z| z.is_none())
    }

    pub fn free_spell_trap_zone(&self, player: PlayerId) -> Option<usize> {
        self.players[player.index()]
            .spell_trap_zones
            .iter()
            .position(|z| z.is_none())
    }

    /// Whether a Spell may target this monster, under the owner's rule
    /// that a monster Set this turn is off limits.
    pub fn spell_targetable(&self, card: InstanceId) -> bool {
        let inst = &self.instances[card.index()];
        if !matches!(inst.location, Location::MonsterZone(_)) {
            return false;
        }
        self.shared.rules.turn.spell_target_monster_set_this_turn
            || inst.turn_set != Some(self.turn)
    }

    // ----- internals shared with the turn machine -----

    pub(crate) fn place_spell_trap(
        &mut self,
        player: PlayerId,
        card: InstanceId,
        zone: usize,
        position: Position,
    ) {
        self.remove_from_current_place(card);
        self.players[player.index()].spell_trap_zones[zone] = Some(card);
        let turn = self.turn;
        let inst = &mut self.instances[card.index()];
        inst.location = Location::SpellTrapZone(zone as u8);
        inst.position = position;
        inst.controller = player;
        inst.turn_set = (position == Position::FaceDown).then_some(turn);
        inst.turn_entered_field = Some(turn);
    }

    /// Detaches a card from whatever list or zone holds it.
    pub(crate) fn remove_from_current_place(&mut self, card: InstanceId) {
        let inst = &self.instances[card.index()];
        let holder = &mut self.players[inst.controller.index()];
        match inst.location {
            Location::Hand => holder.hand.retain(|&c| c != card),
            Location::MonsterZone(z) => holder.monster_zones[z as usize] = None,
            Location::SpellTrapZone(z) => holder.spell_trap_zones[z as usize] = None,
            Location::FieldZone => holder.field_zone = None,
            Location::Deck => holder.deck.retain(|&c| c != card),
            Location::Extra => holder.extra.retain(|&c| c != card),
            Location::Banished => {
                let owner = inst.owner;
                self.players[owner.index()].banished.retain(|&c| c != card);
            }
            Location::Graveyard => {
                let owner = inst.owner;
                self.players[owner.index()].graveyard.retain(|&c| c != card);
            }
        }
    }

    /// Moves a card from wherever it is to its owner's Graveyard.
    pub(crate) fn send_to_graveyard(&mut self, card: InstanceId) {
        let from = self.instances[card.index()].location;
        if from == Location::Graveyard {
            return;
        }
        self.remove_from_current_place(card);
        if from.on_field() {
            self.on_leave_field(card);
        }
        let inst = &mut self.instances[card.index()];
        let owner = inst.owner;
        inst.location = Location::Graveyard;
        inst.position = Position::FaceUp;
        inst.controller = owner;
        inst.turn_set = None;
        inst.turn_entered_field = None;
        self.players[owner.index()].graveyard.push(card);
        self.emit(Event::SentToGraveyard { card, from });
    }
}
