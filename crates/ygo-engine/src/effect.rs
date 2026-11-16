//! The effect API: what a card script can be. `ygo-cards` fills a
//! [`Registry`] with one [`CardScript`] per non-Normal card; the engine
//! never knows a card by name, only through these function pointers.
//!
//! Scripts hold no state. Everything an effect needs while it runs is in
//! its [`EffectCtx`], and a resolution that needs a choice returns
//! [`Flow::Choose`] with a continuation instead of blocking.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ygo_data::CardId;

use crate::{Event, Game, InstanceId, PlayerId, WindowKind};

/// Static facts about a card that are not effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    /// Cannot be Normal Summoned or Set (Toon monsters).
    CannotNormalSummon,
    /// One of the four limbs.
    ExodiaPiece,
    /// Exodia the Forbidden One itself.
    ExodiaHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    /// Main Phase effect of a monster on the field, speed 1.
    Ignition,
    /// Activation of a Spell or Trap Card.
    Activate,
    /// Starts in response to an event.
    Trigger { mandatory: bool },
    /// Trigger when the monster is flipped face-up.
    Flip,
    /// Fast monster effect, speed 2.
    Quick,
    /// Always on while the card is face-up on the field; never chains.
    Continuous,
}

/// Spell speed 1, 2 or 3.
pub type Speed = u8;

/// The card an effect belongs to and who controls it right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Source {
    pub card: InstanceId,
    pub controller: PlayerId,
}

/// A point where fast effects may start, with what just happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub kind: WindowKind,
    pub events: Vec<Event>,
}

impl Default for Window {
    /// Open game state with nothing to respond to.
    fn default() -> Self {
        Self {
            kind: WindowKind::Open,
            events: Vec::new(),
        }
    }
}

impl Effect {
    /// An effect with no targets, no cost and nothing continuous; fill in
    /// the rest with struct update syntax.
    pub const fn activate(speed: Speed, resolve: fn(&mut Game, &mut EffectCtx) -> Flow) -> Self {
        Self {
            kind: EffectKind::Activate,
            speed,
            timing: always,
            targets: None,
            cost_choices: None,
            cost: None,
            resolve,
            modify: None,
        }
    }
}

/// A timing that is always met: the card's own activation conditions
/// are enough.
pub fn always(_: &Game, _: Source, _: &Window) -> bool {
    true
}

/// Everything an effect has while it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectCtx {
    pub source: InstanceId,
    pub controller: PlayerId,
    /// Chosen when the effect was activated.
    pub targets: Vec<InstanceId>,
    /// Cards chosen for the cost when the effect was activated.
    pub cost: Vec<InstanceId>,
    /// The event that triggered the effect, if any.
    pub event: Option<Event>,
}

/// What a resolution step asks the engine to do next.
pub enum Flow {
    Done,
    /// The controller must pick one of `options`; the engine then calls
    /// `then` with the pick.
    Choose {
        options: Vec<InstanceId>,
        then: fn(&mut Game, &mut EffectCtx, InstanceId) -> Flow,
    },
}

/// Every legal set of targets for an effect, each set in the order the
/// card text names them.
pub type TargetSets = Vec<Vec<InstanceId>>;

/// A change to a monster's ATK and DEF from a continuous effect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatMod {
    pub atk: i32,
    pub def: i32,
}

/// One effect of a card. Function pointers only: nothing to clone with
/// the game and nothing that cannot build to WebAssembly.
#[derive(Clone, Copy)]
pub struct Effect {
    pub kind: EffectKind,
    pub speed: Speed,
    /// May this effect start now, given the window's events?
    pub timing: fn(&Game, Source, &Window) -> bool,
    /// Every legal target set; `None` for effects without targets.
    pub targets: Option<fn(&Game, Source) -> TargetSets>,
    /// Every legal choice of cards to pay the cost with (a discard, a
    /// tribute); `None` when the cost needs no choice.
    pub cost_choices: Option<fn(&Game, Source) -> TargetSets>,
    /// Paid when the effect is activated, before anything can respond.
    pub cost: Option<fn(&mut Game, &mut EffectCtx)>,
    pub resolve: fn(&mut Game, &mut EffectCtx) -> Flow,
    /// Continuous modifier applied to `target` while the source is active.
    pub modify: Option<fn(&Game, Source, InstanceId) -> StatMod>,
}

impl std::fmt::Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Effect")
            .field("kind", &self.kind)
            .field("speed", &self.speed)
            .finish_non_exhaustive()
    }
}

/// Everything the engine knows about how one card plays.
#[derive(Debug, Clone, Default)]
pub struct CardScript {
    pub tags: Vec<Tag>,
    pub effects: Vec<Effect>,
}

impl CardScript {
    pub fn has(&self, tag: Tag) -> bool {
        self.tags.contains(&tag)
    }
}

/// Scripts by card id. Built once by `ygo-cards`, copied into `Shared`.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    scripts: HashMap<CardId, CardScript>,
}

impl Registry {
    pub fn register(&mut self, id: CardId, script: CardScript) {
        let previous = self.scripts.insert(id, script);
        assert!(previous.is_none(), "card {id} scripted twice");
    }

    pub fn get(&self, id: CardId) -> Option<&CardScript> {
        self.scripts.get(&id)
    }

    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = CardId> + '_ {
        self.scripts.keys().copied()
    }
}
