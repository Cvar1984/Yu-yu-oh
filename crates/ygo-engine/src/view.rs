//! What a player may use to decide: their observation, the legal actions,
//! and sampled games that agree with everything they can see. Search bots
//! play forward in the samples; none of them ever sees the real game.

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

use crate::{Action, CardIndex, Game, InstanceId, Location, Observation, PlayerId, SpellTrapType};

/// The pending decision as the deciding player sees it. The real game is
/// private: a bot reads the observation, or plays in a [`sample`](Self::sample).
pub struct PlayerView<'a> {
    game: &'a Game,
    observation: Observation,
}

impl<'a> PlayerView<'a> {
    /// The view of whoever must decide now; `None` once the game is over.
    pub fn new(game: &'a Game) -> Option<Self> {
        let decision = game.pending_decision()?;
        Some(Self {
            game,
            observation: game.observe(decision.player),
        })
    }

    pub fn player(&self) -> PlayerId {
        self.observation.viewer
    }

    pub fn observation(&self) -> &Observation {
        &self.observation
    }

    /// The legal actions, never empty.
    pub fn actions(&self) -> &'a [Action] {
        self.game.actions()
    }

    /// A whole game that looks exactly like this one to the deciding
    /// player, with every card they cannot see dealt again at random and
    /// a fresh random generator, so coin tosses and shuffles to come are
    /// unknown too. Different seeds give different samples.
    ///
    /// Each player's hidden cards are redealt among that player's own
    /// hidden places: their Deck, their hand when it is hidden, their
    /// face-down cards (a monster only to a Monster Zone, a Spell or Trap
    /// to a Spell & Trap Zone, a Field Spell to the Field Zone) and their
    /// Extra Deck. The cards themselves are those of the real decks: the
    /// sample assumes the opponent's deck list is known. A redeal in which
    /// a hidden hand would already hold all of Exodia is drawn again,
    /// because that player would have won.
    ///
    /// The sample's legal actions are worked out again; they are the same
    /// as [`actions`](Self::actions) unless a card's legality depends on
    /// a card the decider cannot see.
    pub fn sample(&self, seed: u64) -> Game {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let hidden = self.hidden();
        let mut game = self.game.clone();
        // A few redeals are enough: a complete Exodia is rare.
        for _ in 0..16 {
            redeal(&mut game, &hidden, &mut rng);
            if !PlayerId::BOTH
                .iter()
                .any(|&p| p != self.player() && game.exodia_complete(p))
            {
                break;
            }
        }
        game.rng = ChaCha8Rng::seed_from_u64(rng.next_u64());
        game.advance();
        game
    }

    /// Instances whose identity the decider cannot see.
    fn hidden(&self) -> Vec<InstanceId> {
        let mut known = vec![false; self.game.instances.len()];
        let obs = &self.observation;
        let mut mark = |c: &crate::VisibleCard| {
            if c.card().is_some() {
                known[c.instance().index()] = true;
            }
        };
        for side in &obs.sides {
            side.hand
                .iter()
                .chain(&side.extra)
                .chain(&side.graveyard)
                .chain(&side.banished)
                .chain(side.monster_zones.iter().flatten())
                .chain(side.spell_trap_zones.iter().flatten())
                .chain(side.field_zone.iter())
                .for_each(&mut mark);
        }
        obs.revealed.iter().for_each(&mut mark);
        // A card on the chain, or one whose resolution waits for an answer,
        // was activated in the open: it keeps its identity even if it has
        // since gone back to the hand or the Deck.
        for link in &obs.chain {
            known[link.instance.index()] = true;
        }
        if let Some(card) = self.game.resolving_card() {
            known[card.index()] = true;
        }
        (0..known.len())
            .filter(|&i| !known[i])
            .map(|i| InstanceId(i as u8))
            .collect()
    }
}

/// Where a hidden card sits, which decides what may be dealt there.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Monster,
    SpellTrap,
    Field,
    Extra,
    /// Hand or Deck: any Main Deck card.
    Loose,
}

impl Slot {
    fn fits(self, game: &Game, card: CardIndex) -> bool {
        let stats = game.shared.pool.get(card);
        match self {
            Slot::Monster => stats.is_monster(),
            Slot::SpellTrap => !stats.is_monster() && stats.subtype != SpellTrapType::Field,
            Slot::Field => stats.subtype == SpellTrapType::Field,
            Slot::Extra | Slot::Loose => true,
        }
    }
}

/// Deals the identities of `hidden` again, per owner, each card staying
/// with its owner and each slot getting a card that may sit there. Every
/// instance keeps its place, position and history; only its card changes.
fn redeal(game: &mut Game, hidden: &[InstanceId], rng: &mut ChaCha8Rng) {
    for owner in PlayerId::BOTH {
        // The Extra Deck is dealt among itself; the rest share one pile.
        let mut main: Vec<(InstanceId, Slot)> = Vec::new();
        let mut extra: Vec<(InstanceId, Slot)> = Vec::new();
        for &id in hidden {
            let inst = &game.instances[id.index()];
            if inst.owner != owner {
                continue;
            }
            let extra_card = game.shared.pool.get(inst.card).is_extra_deck();
            let slot = match inst.location {
                Location::Extra => Slot::Extra,
                // A face-down Fusion Monster on the field keeps its card.
                _ if extra_card => continue,
                Location::MonsterZone(_) => Slot::Monster,
                Location::SpellTrapZone(_) => Slot::SpellTrap,
                Location::FieldZone => Slot::Field,
                // Hand, Deck, or banished face-down (Lightforce Sword).
                Location::Hand | Location::Deck | Location::Banished => Slot::Loose,
                Location::Graveyard => continue,
            };
            if slot == Slot::Extra {
                extra.push((id, slot));
            } else {
                main.push((id, slot));
            }
        }
        deal(game, &extra, rng);
        // Restricted slots first, so the Hand and Deck take what is left.
        main.sort_by_key(|&(_, slot)| slot == Slot::Loose);
        deal(game, &main, rng);
    }
}

/// Gives each slot in order a card drawn at random from the pile of all
/// the slots' cards that fits it. Monsters, Field Spells and the other
/// Spells and Traps are disjoint, and the real deal fits, so a fitting
/// card is always left.
fn deal(game: &mut Game, slots: &[(InstanceId, Slot)], rng: &mut ChaCha8Rng) {
    let mut pile: Vec<CardIndex> = slots
        .iter()
        .map(|&(id, _)| game.instances[id.index()].card)
        .collect();
    for &(id, slot) in slots {
        let pick = if matches!(slot, Slot::Extra | Slot::Loose) {
            (rng.next_u64() % pile.len() as u64) as usize
        } else {
            let fitting: Vec<usize> = (0..pile.len())
                .filter(|&i| slot.fits(game, pile[i]))
                .collect();
            fitting[(rng.next_u64() % fitting.len() as u64) as usize]
        };
        game.instances[id.index()].card = pile.swap_remove(pick);
    }
}
