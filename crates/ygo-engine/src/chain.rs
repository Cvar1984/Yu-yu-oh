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
    PlayerId, Position, Source, SpellTrapType, TargetSets, Window,
};

/// One activated effect waiting to resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    pub card: InstanceId,
    pub effect: u8,
    pub speed: u8,
    pub controller: PlayerId,
    pub targets: Vec<InstanceId>,
    pub cost: Vec<InstanceId>,
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
    Normal,
    Flip,
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
    DamageStep {
        attacker: InstanceId,
        target: Option<InstanceId>,
    },
    /// After the damage-calculation window: calculate and apply damage.
    DamageCalc {
        attacker: InstanceId,
        target: Option<InstanceId>,
    },
}

/// A resolution paused on a `Choose` decision.
#[derive(Clone)]
pub(crate) struct PendingChoice {
    pub link: ChainLink,
    pub ctx: EffectCtx,
    pub options: Vec<InstanceId>,
    pub then: fn(&mut Game, &mut EffectCtx, InstanceId) -> Flow,
}

impl PartialEq for PendingChoice {
    fn eq(&self, other: &Self) -> bool {
        self.link == other.link && self.ctx == other.ctx && self.options == other.options
    }
}

impl std::fmt::Debug for PendingChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingChoice")
            .field("link", &self.link)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl Game {
    fn effect(&self, card: InstanceId, index: u8) -> Effect {
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
    fn collect_triggers(&self, kind: WindowKind, events: &[Event]) -> Vec<TriggerRef> {
        let window = Window {
            kind,
            events: events.to_vec(),
        };
        let mut found = Vec::new();
        for player in [self.turn_player, self.turn_player.opponent()] {
            let state = &self.players[player.index()];
            let cards: Vec<InstanceId> = state
                .monster_zones
                .iter()
                .chain(state.spell_trap_zones.iter())
                .flatten()
                .chain(state.graveyard.iter())
                .copied()
                .collect();
            for card in cards {
                let Some(script) = self.script(card) else {
                    continue;
                };
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
                    if (effect.timing)(self, source, &window) {
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
        let window = Window {
            kind,
            events: events.to_vec(),
        };
        for t in self.trigger_group() {
            self.push_activations(t.controller, t.card, &window, |_| true, &mut actions);
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

    /// The attack whose Damage Step is running, if any.
    pub fn current_attack(&self) -> Option<(InstanceId, Option<InstanceId>)> {
        self.pending.iter().rev().find_map(|p| match p {
            Pending::DamageCalc { attacker, target } => Some((*attacker, *target)),
            _ => None,
        })
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
            (Location::SpellTrapZone(_), CardKind::Trap, _) => {
                inst.position == Position::FaceDown
                    && (self.shared.rules.turn.trap_activation_turn_set
                        || inst.turn_set != Some(self.turn))
            }
            (Location::SpellTrapZone(_), CardKind::Spell, SpellTrapType::QuickPlay) => {
                inst.position == Position::FaceDown && inst.turn_set != Some(self.turn)
            }
            (Location::Hand, CardKind::Spell, SpellTrapType::QuickPlay) => {
                player == self.turn_player && self.free_spell_trap_zone(player).is_some()
            }
            (Location::MonsterZone(_), _, _) => {
                effect.kind == EffectKind::Quick && inst.position.is_face_up()
            }
            (Location::Hand, _, SpellTrapType::None) => effect.kind == EffectKind::Quick,
            _ => false,
        }
    }

    /// Pushes every activation of `card`'s effects that fit `filter`.
    fn push_activations(
        &self,
        player: PlayerId,
        card: InstanceId,
        window: &Window,
        filter: impl Fn(&Effect) -> bool,
        actions: &mut Vec<Action>,
    ) {
        let Some(script) = self.script(card) else {
            return;
        };
        let source = Source {
            card,
            controller: player,
        };
        for (i, effect) in script.effects.iter().enumerate() {
            if effect.kind == EffectKind::Continuous {
                continue;
            }
            if !filter(effect) || !(effect.timing)(self, source, window) {
                continue;
            }
            let costs: TargetSets = effect
                .cost_choices
                .map_or_else(|| vec![Vec::new()], |f| f(self, source));
            let targets: TargetSets = effect
                .targets
                .map_or_else(|| vec![Vec::new()], |f| f(self, source));
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
            if stats.kind != CardKind::Spell || !has_free_zone {
                continue;
            }
            match stats.subtype {
                SpellTrapType::Normal => {
                    self.push_activations(player, card, &window, |e| e.speed == 1, actions)
                }
                SpellTrapType::QuickPlay => {
                    self.push_activations(player, card, &window, |e| e.speed == 2, actions)
                }
                _ => {}
            }
        }
        for card in state.spell_trap_zones.iter().flatten().copied() {
            let inst = &self.instances[card.index()];
            let stats = self.stats(card);
            if inst.position != Position::FaceDown || stats.kind != CardKind::Spell {
                continue;
            }
            match stats.subtype {
                SpellTrapType::Normal => {
                    self.push_activations(player, card, &window, |e| e.speed == 1, actions)
                }
                SpellTrapType::QuickPlay if inst.turn_set != Some(self.turn) => {
                    self.push_activations(player, card, &window, |e| e.speed == 2, actions)
                }
                _ => {}
            }
        }
    }

    /// Fast effects `player` may activate in the open window, without `Pass`.
    pub(crate) fn response_actions(&self, player: PlayerId) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some((kind, events)) = self.window() else {
            return actions;
        };
        let window = Window {
            kind,
            events: events.to_vec(),
        };
        let min_speed = self.chain.last().map_or(2, |l| l.speed);
        let in_damage_step = kind == WindowKind::DamageCalculation;
        let state = &self.players[player.index()];
        let candidates: Vec<InstanceId> = state
            .hand
            .iter()
            .chain(state.spell_trap_zones.iter().flatten())
            .chain(state.monster_zones.iter().flatten())
            .copied()
            .collect();
        for card in candidates {
            self.push_activations(
                player,
                card,
                &window,
                |e| {
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
        match self.instances[card.index()].location {
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
        };
        if let Some(pay) = eff.cost {
            pay(self, &mut ctx);
        }
        self.chain.push(ChainLink {
            card,
            effect,
            speed: eff.speed,
            controller: player,
            targets: ctx.targets,
            cost: ctx.cost,
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
        let mut ctx = EffectCtx {
            source: link.card,
            controller: link.controller,
            targets: link.targets.clone(),
            cost: link.cost.clone(),
            event: None,
        };
        let flow = (self.effect(link.card, link.effect).resolve)(self, &mut ctx);
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
                self.pending_choice = Some(PendingChoice {
                    link,
                    ctx,
                    options,
                    then,
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
        let mut ctx = pc.ctx;
        let flow = (pc.then)(self, &mut ctx, picked);
        self.continue_flow(pc.link, ctx, flow);
    }

    /// After a link resolves or is skipped: a negated card is destroyed; a
    /// Normal, Quick-Play, Ritual or Counter card still in its zone goes to
    /// the Graveyard.
    fn finish_link(&mut self, link: &ChainLink) {
        let inst = &self.instances[link.card.index()];
        if !matches!(inst.location, Location::SpellTrapZone(_)) {
            return;
        }
        if link.negated || self.stats(link.card).subtype.leaves_after_resolving() {
            self.send_to_graveyard(link.card);
        }
    }

    pub(crate) fn choose_actions(&self, actions: &mut Vec<Action>) {
        if let Some(pc) = &self.pending_choice {
            for &card in &pc.options {
                actions.push(Action::Choose { card });
            }
        }
    }
}
