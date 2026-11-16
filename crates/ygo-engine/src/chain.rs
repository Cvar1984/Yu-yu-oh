//! Activations, the chain and its resolution. In step 2 a chain holds one
//! link and resolves at once; step 3 adds response windows and more links.

use crate::{
    Action, Effect, EffectCtx, EffectKind, Flow, Game, InstanceId, Location, Phase, PlayerId,
    Position, Source, SpellTrapType, TargetSets, Window,
};
use serde::{Deserialize, Serialize};
use ygo_data::CardKind;

/// One activated effect waiting to resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainLink {
    pub card: InstanceId,
    pub effect: u8,
    pub controller: PlayerId,
    pub targets: Vec<InstanceId>,
    pub cost: Vec<InstanceId>,
    pub negated: bool,
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

    /// Spell Cards the turn player may activate now: speed 1 Normal
    /// Spells from the hand (needs a free zone) or Set face-down.
    pub(crate) fn activation_actions(&self, player: PlayerId, actions: &mut Vec<Action>) {
        if !matches!(self.phase, Phase::Main1 | Phase::Main2) || !self.chain.is_empty() {
            return;
        }
        let state = &self.players[player.index()];
        let has_free_zone = self.free_spell_trap_zone(player).is_some();
        let mut candidates: Vec<InstanceId> = Vec::new();
        if has_free_zone {
            candidates.extend(state.hand.iter().copied());
        }
        candidates.extend(
            state
                .spell_trap_zones
                .iter()
                .flatten()
                .copied()
                .filter(|&c| self.instances[c.index()].position == Position::FaceDown),
        );
        for card in candidates {
            let stats = self.stats(card);
            if stats.kind != CardKind::Spell || stats.subtype != SpellTrapType::Normal {
                continue;
            }
            let Some(script) = self.script(card) else {
                continue;
            };
            let source = Source {
                card,
                controller: player,
            };
            for (i, effect) in script.effects.iter().enumerate() {
                if effect.kind != EffectKind::Activate || effect.speed != 1 {
                    continue;
                }
                if !(effect.timing)(self, source, &Window::default()) {
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
    }

    /// Puts the card face-up on the field if needed, pays the cost and
    /// adds the link to the chain.
    pub(crate) fn activate(
        &mut self,
        player: PlayerId,
        card: InstanceId,
        effect: u8,
        targets: Vec<InstanceId>,
        cost: Vec<InstanceId>,
    ) {
        match self.instances[card.index()].location {
            Location::Hand => {
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
        if let Some(pay) = self.effect(card, effect).cost {
            pay(self, &mut ctx);
        }
        self.chain.push(ChainLink {
            card,
            effect,
            controller: player,
            targets: ctx.targets,
            cost: ctx.cost,
            negated: false,
        });
        self.emit(crate::Event::Activated {
            card,
            controller: player,
            effect,
        });
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
                self.emit(crate::Event::Resolved {
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

    /// After a link resolves (or was negated): a Normal or Quick-Play
    /// Spell, or a Trap, still in its zone goes to the Graveyard.
    fn finish_link(&mut self, link: &ChainLink) {
        let inst = &self.instances[link.card.index()];
        if matches!(inst.location, Location::SpellTrapZone(_))
            && self.stats(link.card).subtype.leaves_after_resolving()
        {
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
