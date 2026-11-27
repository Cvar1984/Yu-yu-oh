//! Activations, response windows, the chain and its resolution.
//!
//! Work that must wait for the players (a response window, a summon
//! whose negation window is open, a declared attack, a chain to resolve)
//! sits on a stack of [`Pending`] items. `advance` works the top item;
//! a window becomes a decision only when the player with priority has
//! something to activate, otherwise the engine passes for them.

use serde::{Deserialize, Serialize};
use ygo_data::CardKind;

use crate::{
    Action, Effect, EffectCtx, EffectKind, Event, Flow, Game, InstanceId, Location, Phase,
    PlayerId, Position, Source, SpellTrapType, Tag, TargetSets, Window,
};

/// One activated effect waiting to resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    pub card: InstanceId,
    pub effect: u8,
    /// A Spell or Trap Card activated from the hand or from face-down, as
    /// opposed to the effect of a card already face-up.
    pub card_activation: bool,
    pub speed: u8,
    pub controller: PlayerId,
    pub targets: Vec<InstanceId>,
    pub cost: Vec<InstanceId>,
    /// A number the cost recorded for the resolution (`EffectCtx::value`).
    pub value: i32,
    pub negated: bool,
}

/// Why a response window is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowKind {
    /// After an action in open game state, or at a phase step.
    Open,
    /// A monster is about to be Normal, Tribute or Flip Summoned (or Set).
    SummonDeclared,
    /// A summon succeeded; the monster is on the field.
    SummonSuccess,
    AttackDeclared,
    /// Inside the Damage Step, right before damage is calculated.
    DamageCalculation,
    /// The Damage Step is over; its events are in the window.
    AfterBattle,
    /// A link was just added; responses chain to it.
    Chain,
    /// A chain just finished resolving.
    AfterChain,
}

/// How a summon was declared, for the cards that care.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SummonKind {
    /// A Normal or Tribute Summon or Set, in this position.
    Normal {
        position: Position,
    },
    Flip,
    /// A summon procedure from the hand, in this position.
    Procedure {
        position: Position,
    },
}

/// A summon between its declaration and its completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PendingSummon {
    pub player: PlayerId,
    pub card: InstanceId,
    pub kind: SummonKind,
    pub face_down: bool,
    pub negated: bool,
}

/// A trigger effect that fired and waits to be put on the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerRef {
    pub card: InstanceId,
    pub effect: u8,
    pub controller: PlayerId,
    pub mandatory: bool,
}

/// Work waiting on the players, worked from the top of the stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Pending {
    Window {
        kind: WindowKind,
        events: Vec<Event>,
        priority: PlayerId,
        passes: u8,
        /// Triggers that fired for this window, not yet on the chain.
        triggers: Vec<TriggerRef>,
    },
    ResolveChain,
    CompleteSummon(PendingSummon),
    /// After the attack-declaration window: start the Damage Step.
    /// `monsters` is how many monsters the defender had when the attack was
    /// declared: a different number by then means a replay.
    DamageStep {
        attacker: InstanceId,
        target: Option<InstanceId>,
        monsters: u8,
    },
    /// After the damage-calculation window: calculate and apply damage.
    DamageCalc {
        attacker: InstanceId,
        target: Option<InstanceId>,
    },
    /// The target left before the Damage Step: pick another or stop.
    Replay {
        attacker: InstanceId,
    },
}

/// What a paused resolution waits for.
#[derive(Clone)]
pub(crate) enum Prompt {
    Choose {
        chooser: PlayerId,
        options: Vec<InstanceId>,
        then: fn(&mut Game, &mut EffectCtx, InstanceId) -> Flow,
    },
    Ask {
        chooser: PlayerId,
        options: Vec<&'static str>,
        about: Option<InstanceId>,
        then: fn(&mut Game, &mut EffectCtx, usize) -> Flow,
    },
}

/// A resolution paused on a `Choose` or `Ask` decision.
#[derive(Clone)]
pub(crate) struct PendingChoice {
    pub link: ChainLink,
    pub ctx: EffectCtx,
    pub prompt: Prompt,
}

impl PendingChoice {
    /// Who answers: the controller, or the chooser of a `ChooseBy`.
    pub fn decider(&self) -> PlayerId {
        match &self.prompt {
            Prompt::Choose { chooser, .. } | Prompt::Ask { chooser, .. } => *chooser,
        }
    }

    pub fn card_options(&self) -> &[InstanceId] {
        match &self.prompt {
            Prompt::Choose { options, .. } => options,
            Prompt::Ask { .. } => &[],
        }
    }

    /// The card a question is about, if it names one.
    pub fn about(&self) -> Option<InstanceId> {
        match &self.prompt {
            Prompt::Ask { about, .. } => *about,
            Prompt::Choose { .. } => None,
        }
    }

    pub fn question(&self) -> &[&'static str] {
        match &self.prompt {
            Prompt::Ask { options, .. } => options,
            Prompt::Choose { .. } => &[],
        }
    }
}

impl PartialEq for PendingChoice {
    fn eq(&self, other: &Self) -> bool {
        self.link == other.link
            && self.ctx == other.ctx
            && self.card_options() == other.card_options()
            && self.question() == other.question()
    }
}

impl std::fmt::Debug for PendingChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingChoice")
            .field("link", &self.link)
            .field("options", &self.card_options())
            .field("question", &self.question())
            .finish_non_exhaustive()
    }
}

impl Game {
    pub(crate) fn effect(&self, card: InstanceId, index: u8) -> Effect {
        self.script(card)
            .and_then(|s| s.effects.get(usize::from(index)))
            .copied()
            .expect("an activated effect exists on its card")
    }

    // ----- windows -----

    /// The open response window, if the top pending item is one.
    pub fn window(&self) -> Option<(WindowKind, &[Event])> {
        match self.pending.last() {
            Some(Pending::Window { kind, events, .. }) => Some((*kind, events)),
            _ => None,
        }
    }

    /// The summon between declaration and completion, if any.
    pub fn pending_summon(&self) -> Option<PendingSummon> {
        self.pending.iter().rev().find_map(|p| match p {
            Pending::CompleteSummon(ps) => Some(*ps),
            _ => None,
        })
    }

    /// The card whose resolution waits for a `Choose` or `Ask` answer, if
    /// any (players see it too: `Observation::resolving`).
    pub fn resolving_card(&self) -> Option<InstanceId> {
        self.pending_choice.as_ref().map(|pc| pc.link.card)
    }

    /// The link responses would chain to.
    pub fn last_link(&self) -> Option<&ChainLink> {
        self.chain.last()
    }

    /// Opens a window holding everything emitted since the last one and
    /// collects the triggers it fires. A summon-declaration or
    /// damage-calculation window leaves the events for the window that
    /// follows it (summon success, after battle), where triggers belong.
    pub(crate) fn open_window(&mut self, kind: WindowKind, priority: PlayerId) {
        let keeps = matches!(
            kind,
            WindowKind::SummonDeclared | WindowKind::DamageCalculation
        );
        let events = if keeps {
            self.recent.clone()
        } else {
            std::mem::take(&mut self.recent)
        };
        let triggers = if keeps || kind == WindowKind::Chain {
            Vec::new()
        } else {
            self.collect_triggers(kind, &events)
        };
        self.pending.push(Pending::Window {
            kind,
            events,
            priority,
            passes: 0,
            triggers,
        });
    }

    /// Trigger and Flip effects on the field or in a Graveyard whose timing
    /// accepts this window, in SEGOC order: turn player's mandatory, the
    /// opponent's mandatory, turn player's optional, the opponent's optional.
    ///
    /// Cards on the field are always checked. A card in the Graveyard is
    /// checked only if the window's events moved it there, which is what
    /// every Graveyard trigger in these sets triggers on.
    fn collect_triggers(&self, kind: WindowKind, events: &[Event]) -> Vec<TriggerRef> {
        let window = Window { kind, events };
        // What happened last: the events from the last summon declaration or
        // card activation on. A trigger that can miss the timing must find
        // its event there.
        let last_start = events
            .iter()
            .rposition(|e| matches!(e, Event::SummonDeclared { .. } | Event::Activated { .. }))
            .unwrap_or(0);
        let last = Window {
            kind,
            events: &events[last_start..],
        };
        let mut found = Vec::new();
        let moved: Vec<InstanceId> = events
            .iter()
            .filter_map(|e| match e {
                Event::SentToGraveyard { card, .. } | Event::Drew { card, .. } => Some(*card),
                _ => None,
            })
            .collect();
        let negators = self.negators();
        for player in [self.turn_player, self.turn_player.opponent()] {
            let state = &self.players[player.index()];
            let cards = state
                .monster_zones
                .iter()
                .chain(state.spell_trap_zones.iter())
                .flatten()
                .copied()
                .chain(state.field_zone)
                .chain(moved.iter().copied().filter(|c| {
                    let inst = &self.instances[c.index()];
                    (inst.location == Location::Graveyard && inst.owner == player)
                        || (inst.location == Location::Hand && inst.controller == player)
                }));
            for card in cards {
                let Some(script) = self.script(card) else {
                    continue;
                };
                if self.instances[card.index()].location.on_field()
                    && self.negated_with(&negators, card)
                {
                    continue;
                }
                let controller = self.instances[card.index()].controller;
                if controller != player {
                    continue;
                }
                for (i, effect) in script.effects.iter().enumerate() {
                    let mandatory = match effect.kind {
                        EffectKind::Trigger { mandatory } => mandatory,
                        EffectKind::Flip => true,
                        _ => continue,
                    };
                    let source = Source { card, controller };
                    let seen = if effect.misses_timing && !mandatory {
                        &last
                    } else {
                        &window
                    };
                    if (effect.timing)(self, source, seen) {
                        found.push(TriggerRef {
                            card,
                            effect: i as u8,
                            controller,
                            mandatory,
                        });
                    }
                }
            }
        }
        found.sort_by_key(|t| (!t.mandatory, t.controller != self.turn_player));
        found
    }

    /// The triggers of the window's first SEGOC group still waiting.
    pub(crate) fn trigger_group(&self) -> Vec<TriggerRef> {
        let Some(Pending::Window { triggers, .. }) = self.pending.last() else {
            return Vec::new();
        };
        let Some(first) = triggers.first() else {
            return Vec::new();
        };
        triggers
            .iter()
            .filter(|t| t.mandatory == first.mandatory && t.controller == first.controller)
            .copied()
            .collect()
    }

    /// Activations of the current trigger group (one per target choice).
    pub(crate) fn trigger_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some((kind, events)) = self.window() else {
            return actions;
        };
        let window = Window { kind, events };
        for t in self.trigger_group() {
            self.push_activations(
                t.controller,
                t.card,
                &window,
                |i, _| i == usize::from(t.effect),
                &mut actions,
            );
        }
        actions
    }

    /// Removes the trigger of `card` from the open window.
    fn take_trigger(&mut self, card: InstanceId, effect: u8) {
        if let Some(Pending::Window { triggers, .. }) = self.pending.last_mut() {
            triggers.retain(|t| !(t.card == card && t.effect == effect));
        }
    }

    /// Drops the whole current trigger group (declined optional triggers,
    /// or mandatory ones with nothing legal to do).
    pub(crate) fn drop_trigger_group(&mut self) {
        let group = self.trigger_group();
        if let Some(Pending::Window { triggers, .. }) = self.pending.last_mut() {
            triggers.retain(|t| !group.contains(t));
        }
    }

    fn window_mut(&mut self) -> (&mut PlayerId, &mut u8) {
        match self.pending.last_mut() {
            Some(Pending::Window {
                priority, passes, ..
            }) => (priority, passes),
            _ => unreachable!("called with an open window"),
        }
    }

    /// The attack in progress, from its declaration to the end of its
    /// Damage Step, if any.
    pub fn current_attack(&self) -> Option<(InstanceId, Option<InstanceId>)> {
        self.current_attack
    }

    /// The player with priority passes. Two passes in a row close the
    /// window; a chain built in it then resolves.
    pub(crate) fn pass(&mut self) {
        let (priority, passes) = self.window_mut();
        *passes += 1;
        *priority = priority.opponent();
        if *passes >= 2 {
            self.pending.pop();
            if !self.chain.is_empty() {
                self.pending.push(Pending::ResolveChain);
            }
        }
    }

    // ----- what may be activated -----

    fn fast_effect_allowed(&self, card: InstanceId, effect: &Effect, player: PlayerId) -> bool {
        let inst = &self.instances[card.index()];
        let stats = self.stats(card);
        match (inst.location, stats.kind, stats.subtype) {
            // The fast effect of a face-up Spell or Trap (Ultimate Offering).
            (Location::SpellTrapZone(_), _, _) if effect.kind == EffectKind::Quick => {
                inst.position.is_face_up()
            }
            (Location::SpellTrapZone(_), CardKind::Trap, _) => {
                inst.position == Position::FaceDown
                    && (self.shared.rules.turn.trap_activation_turn_set
                        || inst.turn_set != Some(self.turn))
            }
            (Location::SpellTrapZone(_), CardKind::Spell, SpellTrapType::QuickPlay) => {
                inst.position == Position::FaceDown && inst.turn_set != Some(self.turn)
            }
            (Location::SpellTrapZone(_), CardKind::Spell, SpellTrapType::Normal) => {
                effect.in_standby
                    && inst.position == Position::FaceDown
                    && self.phase == Phase::Standby
                    && player == self.turn_player
                    && self.chain.is_empty()
            }
            (Location::Hand, CardKind::Spell, SpellTrapType::Normal) => {
                effect.in_standby
                    && self.phase == Phase::Standby
                    && player == self.turn_player
                    && self.chain.is_empty()
                    && self.free_spell_trap_zone(player).is_some()
            }
            (Location::Hand, CardKind::Spell, SpellTrapType::QuickPlay) => {
                player == self.turn_player && self.free_spell_trap_zone(player).is_some()
            }
            (Location::MonsterZone(_), _, _) => {
                effect.kind == EffectKind::Quick && inst.position.is_face_up()
            }
            (Location::Hand, _, SpellTrapType::None) => effect.kind == EffectKind::Quick,
            (Location::Graveyard, _, _) => effect.kind == EffectKind::Quick,
            _ => false,
        }
    }

    /// Pushes every activation of `card`'s effects that fit `filter`.
    fn push_activations(
        &self,
        player: PlayerId,
        card: InstanceId,
        window: &Window,
        filter: impl Fn(usize, &Effect) -> bool,
        actions: &mut Vec<Action>,
    ) {
        let Some(script) = self.script(card) else {
            return;
        };
        if self.negated(card) && !self.activation_allowed_while_negated(card) {
            return;
        }
        let source = Source {
            card,
            controller: player,
        };
        let uses = self.instances[card.index()].effect_uses;
        // A name locked for this player cannot be activated this turn (but a
        // summon procedure is not an activation).
        let locked = self.players[player.index()]
            .names_locked
            .contains(&self.stats(card).id);
        for (i, effect) in script.effects.iter().enumerate() {
            if locked && effect.kind != EffectKind::Procedure {
                continue;
            }
            if effect.kind == EffectKind::Continuous {
                continue;
            }
            if effect.once_per_turn && uses & (1 << i) != 0 {
                continue;
            }
            if !filter(i, effect) || !(effect.timing)(self, source, window) {
                continue;
            }
            let costs: TargetSets = effect
                .cost_choices
                .map_or_else(|| vec![Vec::new()], |f| f(self, source));
            let targets: TargetSets = effect
                .targets
                .map_or_else(|| vec![Vec::new()], |f| f(self, source));
            if effect.kind == EffectKind::Procedure {
                if self.summons_locked(source.controller) {
                    continue;
                }
                for cost in &costs {
                    for position in [Position::FaceUpAttack, Position::FaceUpDefense] {
                        actions.push(Action::ProcedureSummon {
                            card,
                            effect: i as u8,
                            cost: cost.clone(),
                            position,
                        });
                    }
                }
                continue;
            }
            for cost in &costs {
                for t in &targets {
                    actions.push(Action::Activate {
                        card,
                        effect: i as u8,
                        targets: t.clone(),
                        cost: cost.clone(),
                    });
                }
            }
        }
    }

    /// Activations the turn player may start in open game state: Normal
    /// Spells (speed 1) and Quick-Play Spells from the hand or Set.
    pub(crate) fn activation_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !matches!(self.phase, Phase::Main1 | Phase::Main2) || !self.pending.is_empty() {
            return;
        }
        let state = &self.players[player.index()];
        let has_free_zone = self.free_spell_trap_zone(player).is_some();
        let window = Window::default();
        for &card in &state.hand {
            let stats = self.stats(card);
            if stats.kind != CardKind::Spell {
                continue;
            }
            match stats.subtype {
                SpellTrapType::Normal
                | SpellTrapType::Equip
                | SpellTrapType::Continuous
                | SpellTrapType::Ritual
                    if has_free_zone =>
                {
                    self.push_activations(
                        player,
                        card,
                        &window,
                        |_, e| e.speed == 1 && !e.in_standby,
                        actions,
                    )
                }
                SpellTrapType::QuickPlay if has_free_zone => {
                    self.push_activations(player, card, &window, |_, e| e.speed == 2, actions)
                }
                SpellTrapType::Field => {
                    self.push_activations(player, card, &window, |_, e| e.speed == 1, actions)
                }
                _ => {}
            }
        }
        // A Set Field Spell is activated from the Field Zone.
        if let Some(field) = state.field_zone
            && self.instances[field.index()].position == Position::FaceDown
        {
            self.push_activations(player, field, &window, |_, e| e.speed == 1, actions);
        }
        for card in state.spell_trap_zones.iter().flatten().copied() {
            let inst = &self.instances[card.index()];
            let stats = self.stats(card);
            if inst.position == Position::FaceDown && stats.kind == CardKind::Trap {
                self.push_activations(
                    player,
                    card,
                    &window,
                    |_, e| {
                        e.kind == EffectKind::Activate && self.fast_effect_allowed(card, e, player)
                    },
                    actions,
                );
                continue;
            }
            if inst.position != Position::FaceDown || stats.kind != CardKind::Spell {
                continue;
            }
            match stats.subtype {
                SpellTrapType::Normal
                | SpellTrapType::Equip
                | SpellTrapType::Continuous
                | SpellTrapType::Ritual => self.push_activations(
                    player,
                    card,
                    &window,
                    |_, e| e.speed == 1 && !e.in_standby,
                    actions,
                ),
                SpellTrapType::QuickPlay if inst.turn_set != Some(self.turn) => {
                    self.push_activations(player, card, &window, |_, e| e.speed == 2, actions)
                }
                _ => {}
            }
        }
    }

    /// Ignition effects of the turn player's face-up monsters, and of cards
    /// in their hand or Graveyard that say so, as Main Phase actions; also
    /// the fast effects of their face-up Spells and Traps.
    pub(crate) fn ignition_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !matches!(self.phase, Phase::Main1 | Phase::Main2) || !self.pending.is_empty() {
            return;
        }
        let state = &self.players[player.index()];
        let window = Window::default();
        let candidates = state
            .monster_zones
            .iter()
            .chain(state.spell_trap_zones.iter())
            .flatten()
            .copied()
            .chain(state.field_zone)
            .chain(state.hand.iter().copied())
            .chain(state.graveyard.iter().copied());
        for card in candidates {
            let inst = &self.instances[card.index()];
            if inst.location.on_field() && !inst.position.is_face_up() {
                continue;
            }
            // A face-up Spell or Trap's fast effect may be used as a Main
            // Phase action too (Ultimate Offering).
            let spell_trap = matches!(inst.location, Location::SpellTrapZone(_));
            self.push_activations(
                player,
                card,
                &window,
                |_, e| {
                    matches!(e.kind, EffectKind::Ignition | EffectKind::Procedure)
                        || (spell_trap && e.kind == EffectKind::Quick)
                },
                actions,
            );
        }
    }

    /// Fast effects `player` may activate in the open window, without `Pass`.
    pub(crate) fn response_actions(&self, player: PlayerId) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some((kind, events)) = self.window() else {
            return actions;
        };
        let window = Window { kind, events };
        // With no chain, speed 1 is left to `fast_effect_allowed` (Standby Spells).
        // Between a summon's declaration and its success only Counter Traps
        // (speed 3) may start a chain.
        let min_speed = match (self.chain.last(), kind) {
            (Some(l), _) => l.speed,
            (None, WindowKind::SummonDeclared) => 3,
            (None, _) => 1,
        };
        // Every window of the Damage Step counts, chain windows included:
        // only effects that may start there can respond to a Damage Step chain.
        let in_damage_step = self.phase == crate::Phase::Battle(crate::BattleStep::Damage);
        let state = &self.players[player.index()];
        let candidates = state
            .hand
            .iter()
            .chain(state.spell_trap_zones.iter().flatten())
            .chain(state.monster_zones.iter().flatten())
            .copied();
        for card in candidates {
            self.push_activations(
                player,
                card,
                &window,
                |_, e| {
                    matches!(e.kind, EffectKind::Activate | EffectKind::Quick)
                        && e.speed >= min_speed
                        && (!in_damage_step || e.damage_step)
                        && self.fast_effect_allowed(card, e, player)
                },
                &mut actions,
            );
        }
        actions
    }

    // ----- activation and resolution -----

    /// Puts the card face-up on the field if needed, pays the cost, adds
    /// the link and gives the opponent priority to respond.
    pub(crate) fn activate(
        &mut self,
        player: PlayerId,
        card: InstanceId,
        effect: u8,
        targets: Vec<InstanceId>,
        cost: Vec<InstanceId>,
    ) {
        let eff = self.effect(card, effect);
        if eff.once_per_turn {
            self.instances[card.index()].effect_uses |= 1 << effect;
        }
        let inst = &self.instances[card.index()];
        let card_activation = !self.stats(card).is_monster()
            && (inst.location == Location::Hand || inst.position == Position::FaceDown);
        match self.instances[card.index()].location {
            Location::Hand if self.stats(card).subtype == SpellTrapType::Field => {
                self.place_field_spell(player, card, Position::FaceUp);
            }
            Location::FieldZone if self.instances[card.index()].position == Position::FaceDown => {
                self.field_spell_turns_face_up(card);
                self.instances[card.index()].position = Position::FaceUp;
            }
            Location::Hand if !self.stats(card).is_monster() => {
                let zone = self
                    .free_spell_trap_zone(player)
                    .expect("checked when the action was offered");
                self.place_spell_trap(player, card, zone, Position::FaceUp);
            }
            Location::SpellTrapZone(_) => {
                self.instances[card.index()].position = Position::FaceUp;
            }
            _ => {}
        }
        let mut ctx = EffectCtx {
            source: card,
            controller: player,
            targets,
            cost,
            event: None,
            value: 0,
        };
        if let Some(pay) = eff.cost {
            self.resolving = Some(player);
            pay(self, &mut ctx);
            self.resolving = None;
        }
        self.chain.push(ChainLink {
            card,
            effect,
            card_activation,
            speed: eff.speed,
            controller: player,
            targets: ctx.targets,
            cost: ctx.cost,
            value: ctx.value,
            negated: false,
        });
        self.emit(Event::Activated {
            card,
            controller: player,
            effect,
        });
        let is_trigger = matches!(eff.kind, EffectKind::Trigger { .. } | EffectKind::Flip);
        if is_trigger {
            self.take_trigger(card, effect);
        }
        match self.pending.last_mut() {
            Some(Pending::Window {
                priority, passes, ..
            }) => {
                // Triggers go on the chain before anyone responds; the turn
                // player then gets priority as usual.
                *priority = if is_trigger {
                    self.turn_player
                } else {
                    player.opponent()
                };
                *passes = 0;
            }
            _ => self.open_window(WindowKind::Chain, player.opponent()),
        }
    }

    /// Marks the link activated by `card` as negated; its card is
    /// destroyed when the link is finished. Returns `false` if no such link.
    pub fn negate_activation(&mut self, card: InstanceId, by: InstanceId) -> bool {
        match self.chain.iter_mut().find(|l| l.card == card) {
            Some(link) => {
                link.negated = true;
                self.emit(Event::ActivationNegated { card, by });
                true
            }
            None => false,
        }
    }

    /// Negates the pending summon; the monster goes to the Graveyard
    /// instead of the field. Returns `false` if no summon is pending.
    pub fn negate_summon(&mut self, by: InstanceId) -> bool {
        let found = self.pending.iter_mut().rev().find_map(|p| match p {
            Pending::CompleteSummon(ps) => Some(ps),
            _ => None,
        });
        match found {
            Some(ps) => {
                ps.negated = true;
                let card = ps.card;
                self.emit(Event::SummonNegated { card, by });
                true
            }
            None => false,
        }
    }

    /// Resolves the top link of the chain, or pauses on a choice.
    pub(crate) fn resolve_top_link(&mut self) {
        let link = self.chain.pop().expect("called with a non-empty chain");
        if link.negated {
            self.finish_link(&link);
            return;
        }
        // An effect negated since its activation resolves without effect.
        if self.negated(link.card) {
            self.emit(Event::EffectNegated { card: link.card });
            self.finish_link(&link);
            return;
        }
        let mut ctx = EffectCtx {
            source: link.card,
            controller: link.controller,
            targets: link.targets.clone(),
            cost: link.cost.clone(),
            event: None,
            value: link.value,
        };
        self.resolving = Some(link.controller);
        let flow = (self.effect(link.card, link.effect).resolve)(self, &mut ctx);
        self.resolving = None;
        self.continue_flow(link, ctx, flow);
    }

    fn continue_flow(&mut self, link: ChainLink, ctx: EffectCtx, flow: Flow) {
        match flow {
            Flow::Done => {
                self.emit(Event::Resolved {
                    card: link.card,
                    effect: link.effect,
                });
                self.finish_link(&link);
            }
            Flow::Choose { options, then } => {
                assert!(!options.is_empty(), "a choice needs options");
                let chooser = ctx.controller;
                self.pending_choice = Some(PendingChoice {
                    link,
                    ctx,
                    prompt: Prompt::Choose {
                        chooser,
                        options,
                        then,
                    },
                });
            }
            Flow::ChooseBy {
                chooser,
                options,
                then,
            } => {
                assert!(!options.is_empty(), "a choice needs options");
                self.pending_choice = Some(PendingChoice {
                    link,
                    ctx,
                    prompt: Prompt::Choose {
                        chooser,
                        options,
                        then,
                    },
                });
            }
            Flow::Ask {
                options,
                about,
                then,
            } => {
                assert!(!options.is_empty(), "a question needs options");
                let chooser = ctx.controller;
                self.pending_choice = Some(PendingChoice {
                    link,
                    ctx,
                    prompt: Prompt::Ask {
                        chooser,
                        options,
                        about,
                        then,
                    },
                });
            }
            Flow::AskBy {
                chooser,
                options,
                about,
                then,
            } => {
                assert!(!options.is_empty(), "a question needs options");
                self.pending_choice = Some(PendingChoice {
                    link,
                    ctx,
                    prompt: Prompt::Ask {
                        chooser,
                        options,
                        about,
                        then,
                    },
                });
            }
        }
    }

    /// Answers the pending `Choose` decision and continues the resolution.
    pub(crate) fn choose(&mut self, picked: InstanceId) {
        let pc = self
            .pending_choice
            .take()
            .expect("a Choose action needs a pending choice");
        let Prompt::Choose { then, .. } = pc.prompt else {
            unreachable!("Choose answers a card prompt")
        };
        let mut ctx = pc.ctx;
        self.resolving = Some(ctx.controller);
        let flow = then(self, &mut ctx, picked);
        self.resolving = None;
        self.continue_flow(pc.link, ctx, flow);
    }

    /// Answers the pending `Ask` decision and continues the resolution.
    pub(crate) fn answer(&mut self, option: usize) {
        let pc = self
            .pending_choice
            .take()
            .expect("an Answer action needs a pending question");
        let Prompt::Ask { then, .. } = pc.prompt else {
            unreachable!("Answer answers a question prompt")
        };
        let mut ctx = pc.ctx;
        self.resolving = Some(ctx.controller);
        let flow = then(self, &mut ctx, option);
        self.resolving = None;
        self.continue_flow(pc.link, ctx, flow);
    }

    /// After a link resolves or is skipped: a negated card is destroyed; a
    /// Normal, Quick-Play, Ritual or Counter card still in its zone goes to
    /// the Graveyard.
    fn finish_link(&mut self, link: &ChainLink) {
        let inst = &self.instances[link.card.index()];
        if !matches!(
            inst.location,
            Location::SpellTrapZone(_) | Location::FieldZone
        ) {
            return;
        }
        let stays = self
            .script(link.card)
            .is_some_and(|s| s.has(Tag::StaysOnField));
        if link.negated || (self.stats(link.card).subtype.leaves_after_resolving() && !stays) {
            self.send_to_graveyard(link.card);
        }
    }

    pub(crate) fn choose_actions(&self, actions: &mut Vec<Action>) {
        if let Some(pc) = &self.pending_choice {
            for &card in pc.card_options() {
                actions.push(Action::Choose { card });
            }
            for i in 0..pc.question().len() {
                actions.push(Action::Answer { option: i as u8 });
            }
        }
    }
}
