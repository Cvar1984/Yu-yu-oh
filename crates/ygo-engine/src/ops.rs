//! Operations card scripts call. Every one emits the events it causes
//! and keeps the instance table and zones consistent.

use rand_core::Rng;

use crate::{
    Event, Game, GameResult, InstanceId, Lingering, LingeringKind, Location, PlayerId, Position,
    Tag, WinReason,
};

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

    /// Pays Life Points as a cost. Scripts check affordability in `timing`.
    pub fn pay_lp(&mut self, player: PlayerId, amount: u32) {
        let state = &mut self.players[player.index()];
        state.lp = state.lp.saturating_sub(amount);
        let lp = state.lp;
        self.emit(Event::LifePointsPaid { player, amount, lp });
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
            }
        }
    }

    pub fn lingering(&self) -> &[Lingering] {
        &self.lingering
    }

    // ----- queries scripts use -----

    pub fn on_field(&self, card: InstanceId) -> bool {
        matches!(
            self.instances[card.index()].location,
            Location::MonsterZone(_) | Location::SpellTrapZone(_)
        )
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
            Location::Deck => holder.deck.retain(|&c| c != card),
            Location::Extra => holder.extra.retain(|&c| c != card),
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
