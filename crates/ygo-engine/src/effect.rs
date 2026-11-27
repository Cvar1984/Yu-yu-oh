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

use crate::{Action, Event, Game, InstanceId, PlayerId, WindowKind};

/// Static facts about a card that are not effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    /// Cannot be Normal Summoned or Set (Toon monsters).
    CannotNormalSummon,
    /// One of the four limbs.
    ExodiaPiece,
    /// Exodia the Forbidden One itself.
    ExodiaHead,
    /// A Normal Spell that stays on the field after resolving (Swords).
    StaysOnField,
    /// May attack the opponent directly even when they control monsters.
    DirectAttacker,
    /// A Toon monster.
    Toon,
    /// While face-up on the field, both hands are revealed.
    RevealHands,
    /// While face-up on the field, cards sent to the Graveyard are
    /// banished instead (Banisher of the Light).
    BanishInsteadOfGraveyard,
    /// While face-up on the field, playing a card from the hand costs its
    /// player 500 LP (Chain Energy).
    HandPlayToll,
    /// An Equip that takes control: control returns to the owner however the
    /// Equip leaves the field (Snatch Steal).
    ControlWhileEquipped,
    /// Attached like an Equip, but stays on the field when its monster leaves
    /// (Spellbinding Circle, destroyed only with its monster's destruction).
    StaysWithoutTarget,
    /// "Can only be Special Summoned by" its own procedure: no other effect
    /// may ever Special Summon it, even after a proper summon (the Moths).
    OnlyByProcedure,
    /// While face-up on the field, its controller's opponent keeps their
    /// hand revealed (The Eye of Truth).
    RevealOpponentHand,
    /// While face-up on the field, each player shows their hand during
    /// their own turn (Respect Play).
    RevealTurnPlayerHand,
    /// May attack twice in each Battle Phase (Hayabusa Knight).
    SecondAttack,
    /// Its ATK/DEF changes set, swap or multiply values instead of adding
    /// or subtracting (Megamorph, Shield & Sword): Reverse Trap leaves them.
    StatsSetOrScaled,
    /// Cannot be Tributed for a Tribute Summon (Scapegoat's Sheep Tokens).
    NoTributeSummon,
    /// While face-up on the field, no monster can be Set or turned
    /// face-down; a Set becomes a Normal Summon in face-up Defense
    /// Position (Light of Intervention).
    NoFaceDownMonsters,
    /// Its `negate` hook negates effects only: the cards it negates may
    /// still be activated, and resolve without effect (Imperial Order, by
    /// the TCG rulings; Jinzo's Traps "cannot be activated").
    NegatesEffectsOnly,
}

impl Tag {
    /// Every tag, in declaration order. New tags go at the end, so a
    /// tag's place in this list (the neural encoding's slot) never moves.
    pub const ALL: [Tag; 19] = [
        Tag::CannotNormalSummon,
        Tag::ExodiaPiece,
        Tag::ExodiaHead,
        Tag::StaysOnField,
        Tag::DirectAttacker,
        Tag::Toon,
        Tag::RevealHands,
        Tag::BanishInsteadOfGraveyard,
        Tag::HandPlayToll,
        Tag::ControlWhileEquipped,
        Tag::StaysWithoutTarget,
        Tag::OnlyByProcedure,
        Tag::RevealOpponentHand,
        Tag::RevealTurnPlayerHand,
        Tag::SecondAttack,
        Tag::StatsSetOrScaled,
        Tag::NoTributeSummon,
        Tag::NoFaceDownMonsters,
        Tag::NegatesEffectsOnly,
    ];
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
    /// A summon procedure from the hand (Gate Guardian, the Moths, the
    /// Toons): not an effect. It never chains; its summon can be negated
    /// like a Normal Summon, and it counts as the monster's proper summon.
    Procedure,
}

/// Spell speed 1, 2 or 3.
pub type Speed = u8;

/// A continuous hook giving a monster another Type (DNA Surgery).
pub type RaceHook = fn(&Game, Source, InstanceId) -> Option<&'static str>;

/// A continuous hook about the source card itself (Deepsea Warrior).
pub type SelfHook = fn(&Game, Source) -> bool;

/// A continuous hook naming the only monster an attacker may attack.
pub type AttackLock = fn(&Game, Source, InstanceId) -> Option<InstanceId>;

/// A continuous hook run when an attack is declared.
pub type AttackHook = fn(&mut Game, Source, InstanceId);

/// A battle that inflicted damage, for `on_battle_damage` hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleDamage {
    pub attacker: InstanceId,
    pub target: Option<InstanceId>,
    pub damaged: PlayerId,
    pub amount: u32,
}

/// A continuous hook run after battle damage is dealt.
pub type BattleDamageHook = fn(&mut Game, Source, BattleDamage);

/// A continuous hook naming the card destroyed instead of `card` when
/// `card` would be destroyed by battle.
pub type DestructionReplacement = fn(&Game, Source, InstanceId) -> Option<InstanceId>;

/// The card an effect belongs to and who controls it right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Source {
    pub card: InstanceId,
    pub controller: PlayerId,
}

/// A point where fast effects may start, with what just happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window<'a> {
    pub kind: WindowKind,
    pub events: &'a [Event],
}

impl Default for Window<'_> {
    /// Open game state with nothing to respond to.
    fn default() -> Self {
        Self {
            kind: WindowKind::Open,
            events: &[],
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
            damage_step: false,
            restrict: None,
            on_attack: None,
            attack_lock: None,
            once_per_turn: false,
            in_standby: false,
            on_battle_damage: None,
            replace_battle_destruction: None,
            lock_position: None,
            misses_timing: false,
            pierce: None,
            negate: None,
            forbid: None,
            race: None,
            spell_immune: None,
            unattackable: None,
        }
    }

    /// An Ignition effect: a Main Phase action of the turn player, speed 1.
    pub const fn ignition(resolve: fn(&mut Game, &mut EffectCtx) -> Flow) -> Self {
        Self {
            kind: EffectKind::Ignition,
            ..Self::activate(1, resolve)
        }
    }

    /// A summon procedure: `timing` says when it is allowed and `cost` pays
    /// the tributes; the engine offers it in each face-up position.
    pub const fn procedure() -> Self {
        Self {
            kind: EffectKind::Procedure,
            ..Self::activate(1, nothing)
        }
    }

    /// A continuous effect that never chains: only `modify` and `restrict`
    /// matter.
    pub const fn continuous() -> Self {
        Self {
            kind: EffectKind::Continuous,
            ..Self::activate(1, nothing)
        }
    }

    /// A trigger effect: starts by itself when `timing` accepts a window.
    pub const fn trigger(mandatory: bool, resolve: fn(&mut Game, &mut EffectCtx) -> Flow) -> Self {
        Self {
            kind: EffectKind::Trigger { mandatory },
            ..Self::activate(1, resolve)
        }
    }

    /// A FLIP effect: a mandatory trigger when the monster is turned face-up.
    pub const fn flip(resolve: fn(&mut Game, &mut EffectCtx) -> Flow) -> Self {
        Self {
            kind: EffectKind::Flip,
            timing: flipped_this_window,
            ..Self::activate(1, resolve)
        }
    }
}

/// The source was turned face-up in this window, by a Flip Summon, an
/// attack or an effect.
pub fn flipped_this_window(_: &Game, s: Source, w: &Window) -> bool {
    w.events.iter().any(|e| {
        matches!(e,
        Event::FlipSummoned { card, .. } | Event::Flipped { card } if *card == s.card)
    })
}

/// A resolution that does nothing: for cards whose whole effect is
/// continuous.
pub fn nothing(_: &mut Game, _: &mut EffectCtx) -> Flow {
    Flow::Done
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
    /// A number the cost leaves for the resolution (Catapult Turtle: the
    /// Tributed monster's ATK on the field).
    pub value: i32,
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
    /// Like `Choose`, but `chooser` picks (the opponent, for Painful
    /// Choice or Confiscation); the options are revealed to them.
    ChooseBy {
        chooser: PlayerId,
        options: Vec<InstanceId>,
        then: fn(&mut Game, &mut EffectCtx, InstanceId) -> Flow,
    },
    /// The controller answers a question with one of the labelled
    /// options; the engine then calls `then` with the index.
    Ask {
        options: Vec<&'static str>,
        then: fn(&mut Game, &mut EffectCtx, usize) -> Flow,
    },
    /// Like `Ask`, but `chooser` answers.
    AskBy {
        chooser: PlayerId,
        options: Vec<&'static str>,
        then: fn(&mut Game, &mut EffectCtx, usize) -> Flow,
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
    /// May start inside the Damage Step (counter traps, Kuriboh, ATK/DEF changes).
    pub damage_step: bool,
    /// Continuous restriction: while the source is active, `target`
    /// cannot declare an attack.
    pub restrict: Option<fn(&Game, Source, InstanceId) -> bool>,
    /// Continuous: paid or done when `attacker` declares an attack
    /// (Toll, Gravekeeper's Servant, Dark Elf).
    pub on_attack: Option<AttackHook>,
    /// Continuous: the only monster `attacker` may attack, if any
    /// (Ring of Magnetism).
    pub attack_lock: Option<AttackLock>,
    /// An Ignition or Quick effect usable once per turn per card.
    pub once_per_turn: bool,
    /// A Normal Spell activated in its controller's Standby Phase instead
    /// of a Main Phase (Curse of Fiend).
    pub in_standby: bool,
    /// Continuous: run after battle damage is dealt.
    pub on_battle_damage: Option<BattleDamageHook>,
    /// Continuous: a card destroyed by battle in another's place.
    pub replace_battle_destruction: Option<DestructionReplacement>,
    /// Continuous: `target` cannot change its battle position by its
    /// controller's choice (Dragon Capture Jar).
    pub lock_position: Option<fn(&Game, Source, InstanceId) -> bool>,
    /// An optional "When ... you can" trigger: it misses the timing when its
    /// event was not the last thing to happen, that is when a summon or a
    /// card activation came after it in the same window (an equipped
    /// monster Tributed for a Tribute Summon).
    pub misses_timing: bool,
    /// Continuous: `attacker` inflicts piercing battle damage when it
    /// attacks a Defense Position monster (Fairy Meteor Crush).
    pub pierce: Option<fn(&Game, Source, InstanceId) -> bool>,
    /// Continuous: while the source is active, `card`'s effects are negated
    /// and it cannot be activated (Jinzo, Imperial Order).
    pub negate: Option<fn(&Game, Source, InstanceId) -> bool>,
    /// Continuous: while the source is active, this action is illegal
    /// (Prohibition).
    pub forbid: Option<fn(&Game, Source, &Action) -> bool>,
    /// Continuous: the Type `card` has instead of its own, if this effect
    /// changes it (DNA Surgery, Parasite Paracide).
    pub race: Option<RaceHook>,
    /// Continuous: the source itself is unaffected by Spell effects while
    /// this holds (Deepsea Warrior, The Legendary Fisherman).
    pub spell_immune: Option<SelfHook>,
    /// Continuous: the source cannot be targeted for attacks while this
    /// holds (The Legendary Fisherman).
    pub unattackable: Option<SelfHook>,
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

    /// The same script with one more tag.
    pub fn with_tag(mut self, tag: Tag) -> Self {
        self.tags.push(tag);
        self
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

#[cfg(test)]
mod tests {
    use super::Tag;

    /// Exhaustive on purpose: a new tag fails to compile here until it is
    /// added at the end of `Tag::ALL`.
    fn place(tag: Tag) -> usize {
        match tag {
            Tag::CannotNormalSummon => 0,
            Tag::ExodiaPiece => 1,
            Tag::ExodiaHead => 2,
            Tag::StaysOnField => 3,
            Tag::DirectAttacker => 4,
            Tag::Toon => 5,
            Tag::RevealHands => 6,
            Tag::BanishInsteadOfGraveyard => 7,
            Tag::HandPlayToll => 8,
            Tag::ControlWhileEquipped => 9,
            Tag::StaysWithoutTarget => 10,
            Tag::OnlyByProcedure => 11,
            Tag::RevealOpponentHand => 12,
            Tag::RevealTurnPlayerHand => 13,
            Tag::SecondAttack => 14,
            Tag::StatsSetOrScaled => 15,
            Tag::NoTributeSummon => 16,
            Tag::NoFaceDownMonsters => 17,
            Tag::NegatesEffectsOnly => 18,
        }
    }

    #[test]
    fn all_tags_lists_every_tag_in_order() {
        for (i, tag) in Tag::ALL.iter().enumerate() {
            assert_eq!(place(*tag), i, "{tag:?}");
        }
    }
}
