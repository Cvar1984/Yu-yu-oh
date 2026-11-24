//! What each player remembers of cards they saw that are hidden again:
//! a monster bounced to the opponent's hand, a card turned face-down
//! (Book of Moon), a hand shown by The Eye of Truth or looked at by
//! Confiscation. A remembered card shows as known in that player's
//! observation, so bots and samples agree with what a player at the
//! table would know.
//!
//! Memory is cautious: it holds a card only while it stays where it
//! became hidden. It is dropped as soon as the card moves (a remembered
//! card in the hand that is Set could be any card of that hand), goes into
//! a Deck (its place there is unknown), or changes identity (Magical
//! Hats).

use crate::{CardIndex, Event, Game, InstanceId, Location, PlayerId};

/// A set of instance ids (they fit in a byte).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Ids([u64; 4]);

impl Ids {
    const ALL: Ids = Ids([u64::MAX; 4]);

    fn has(&self, id: usize) -> bool {
        self.0[id / 64] & (1 << (id % 64)) != 0
    }
    fn set(&mut self, id: usize, on: bool) {
        if on {
            self.0[id / 64] |= 1 << (id % 64);
        } else {
            self.0[id / 64] &= !(1 << (id % 64));
        }
    }
}

/// One player's memory. Cheap to copy with the game: a bit set and a
/// list that is usually empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Memory {
    /// Hidden from this player at the last update (everything, before the
    /// first one, so the opening deal is not "seen").
    hidden: Ids,
    /// Hidden since, at this place, with this card.
    kept: Vec<(InstanceId, Location, CardIndex)>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            hidden: Ids::ALL,
            kept: Vec::new(),
        }
    }
}

impl Memory {
    fn keep(&mut self, id: InstanceId, at: Option<(Location, CardIndex)>) {
        self.kept.retain(|k| k.0 != id);
        if let Some((location, card)) = at {
            self.kept.push((id, location, card));
        }
    }
}

impl Game {
    /// Whether `viewer` sees instance `id` now, by the rules of `observe`
    /// alone (not memory). `hand_revealed` and `declared` are the
    /// per-state facts `observe` computes once.
    pub(crate) fn sees_now(
        &self,
        viewer: PlayerId,
        id: InstanceId,
        hand_revealed: [bool; 2],
        declared: Option<InstanceId>,
    ) -> bool {
        let inst = &self.instances[id.index()];
        let own = inst.controller == viewer;
        match inst.location {
            Location::Hand => own || hand_revealed[inst.controller.index()] || declared == Some(id),
            Location::Graveyard => true,
            Location::Deck | Location::Gone => false,
            _ => inst.position.is_face_up() || own,
        }
    }

    /// Whether `viewer` remembers `id` as the card it is now.
    pub fn remembers(&self, viewer: PlayerId, id: InstanceId) -> bool {
        let inst = &self.instances[id.index()];
        self.memory[viewer.index()]
            .kept
            .iter()
            .any(|&(k, location, card)| k == id && location == inst.location && card == inst.card)
    }

    /// Brings both players' memory up to date at a decision. A card can
    /// only be hidden from a player in a Deck, in the opponent's hand
    /// (unless revealed) or face-down among the opponent's cards (field,
    /// banished, Extra Deck); one that
    /// was not hidden at the last update, or was glimpsed during this
    /// action (a reveal, a private look), and is hidden now is kept where
    /// it is (not in a Deck).
    pub(crate) fn update_memory(&mut self) {
        let declared = self
            .pending_summon()
            .filter(|p| !p.face_down)
            .map(|p| p.card);
        let mut on_chain = Ids::default();
        for link in &self.chain {
            on_chain.set(link.card.index(), true);
        }
        let mut in_decks = Ids::default();
        for state in &self.players {
            for c in &state.deck {
                in_decks.set(c.index(), true);
            }
        }
        for viewer in PlayerId::BOTH {
            let opponent = viewer.opponent();
            let mut hidden = in_decks;
            let state = &self.players[opponent.index()];
            if !self.hand_revealed(opponent) {
                for &c in &state.hand {
                    hidden.set(c.index(), Some(c) != declared);
                }
            }
            let face_down = state
                .monster_zones
                .iter()
                .chain(&state.spell_trap_zones)
                .flatten()
                .chain(&state.field_zone)
                .chain(&state.banished)
                .chain(&state.extra)
                .copied()
                .filter(|c| !self.instances[c.index()].position.is_face_up());
            for c in face_down {
                hidden.set(c.index(), true);
            }
            // Shown at this decision all the same: the chain, and the
            // options of the viewer's choice.
            let mut shown = on_chain;
            if let Some(pc) = &self.pending_choice
                && pc.decider() == viewer
            {
                for c in pc.card_options() {
                    shown.set(c.index(), true);
                }
            }
            let mut glimpsed = Ids::default();
            for event in &self.events {
                match event {
                    Event::Revealed { cards, .. } => {
                        cards.iter().for_each(|c| glimpsed.set(c.index(), true));
                    }
                    Event::LookedAt { player, cards } if *player == viewer => {
                        cards
                            .iter()
                            .for_each(|(c, _)| glimpsed.set(c.index(), true));
                    }
                    _ => {}
                }
            }
            let mut memory = std::mem::take(&mut self.memory[viewer.index()]);
            let mut newly = [0u64; 4];
            for (w, new) in newly.iter_mut().enumerate() {
                hidden.0[w] &= !shown.0[w];
                *new = hidden.0[w] & !in_decks.0[w] & (!memory.hidden.0[w] | glimpsed.0[w]);
            }
            for (w, word) in newly.iter().enumerate() {
                let mut bits = *word;
                while bits != 0 {
                    let i = w * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let inst = &self.instances[i];
                    memory.keep(InstanceId(i as u8), Some((inst.location, inst.card)));
                }
            }
            // A kept card that moved, changed, or is seen again is dropped.
            let instances = &self.instances;
            memory.kept.retain(|&(id, location, card)| {
                let inst = &instances[id.index()];
                hidden.has(id.index()) && inst.location == location && inst.card == card
            });
            memory.hidden = hidden;
            self.memory[viewer.index()] = memory;
        }
    }

    /// Both players forget `cards` (their identities were shuffled).
    pub(crate) fn forget(&mut self, cards: &[InstanceId]) {
        for memory in &mut self.memory {
            for &c in cards {
                memory.keep(c, None);
                // Not "newly hidden" afterwards: the shuffle hid them.
                memory.hidden.set(c.index(), true);
            }
        }
    }
}
