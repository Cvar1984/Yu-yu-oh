//! Spells of Pharaoh's Servant (owner, 2026-11-21), on the modern text;
//! see `docs/psv-plan.md`.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    Action, CardScript, Effect, EffectCtx, Event, Flow, Game, InstanceId, Location, Phase,
    PlayerId, Position, Registry, Source, StatMod, TargetSets, WindowKind,
};

use crate::positions::{FACE_UP, face_up};

pub const BURNING_LAND: CardId = CardId(24294108);
pub const COLD_WAVE: CardId = CardId(60682203);
pub const INSECT_BARRIER: CardId = CardId(23615409);
pub const INSECT_IMITATION: CardId = CardId(96965364);
pub const INSPECTION: CardId = CardId(16227556);
pub const LIMITER_REMOVAL: CardId = CardId(23171610);
pub const MONSTER_RECOVERY: CardId = CardId(93108433);
pub const NOBLEMAN_OF_EXTERMINATION: CardId = CardId(17449108);
pub const PROHIBITION: CardId = CardId(43711255);
pub const RAIN_OF_MERCY: CardId = CardId(66719324);
pub const SEVEN_COMPLETED: CardId = CardId(86198326);
pub const THE_SHALLOW_GRAVE: CardId = CardId(43434803);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn face_up_on_field(g: &Game, card: InstanceId) -> bool {
    g.on_field(card) && g.instance(card).position.is_face_up()
}

/// The Standby Phase just began, for "during each Standby Phase" effects.
pub(crate) fn standby_began(g: &Game, events: &[Event], kind: WindowKind) -> bool {
    kind == WindowKind::Open
        && g.phase() == Phase::Standby
        && events.contains(&Event::PhaseEntered {
            phase: Phase::Standby,
        })
}

fn face_up_machines(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    g.monsters_controlled(player)
        .into_iter()
        .filter(|&m| face_up_on_field(g, m) && g.race(m) == "Machine")
        .collect()
}

/// Insect Imitation: Insects in the Deck one Level above `level`.
fn imitations(g: &Game, player: PlayerId, level: u8) -> Vec<InstanceId> {
    g.deck(player)
        .iter()
        .copied()
        .filter(|&c| {
            let s = g.stats(c);
            s.is_monster() && s.race == "Insect" && s.level == level + 1 && g.special_summonable(c)
        })
        .collect()
}

/// Prohibition: one card for each name the activating player can see
/// now (face-up on the field, in a Graveyard, banished, or in their own
/// hand), so declaring never reveals a hidden card.
fn declarable_names(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for p in PlayerId::BOTH {
        let visible = g
            .monsters_controlled(p)
            .into_iter()
            .chain(g.spells_traps_controlled(p))
            .filter(|&c| g.instance(c).position.is_face_up())
            .chain(g.graveyard(p).iter().copied())
            .chain(g.banished(p).iter().copied())
            .chain(if p == player {
                g.hand(p).to_vec()
            } else {
                Vec::new()
            });
        for c in visible {
            let id = g.stats(c).id;
            if id != PROHIBITION && !seen.contains(&id) {
                seen.push(id);
                out.push(c);
            }
        }
    }
    out
}

/// The card whose name Prohibition declared (kept as id + 1 in its counter).
fn declared(g: &Game, prohibition: InstanceId) -> Option<CardId> {
    match g.counter(prohibition) {
        0 => None,
        n => Some(g.stats(InstanceId(n - 1)).id),
    }
}

fn revivable(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    g.graveyard(player)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).is_monster() && g.special_summonable(c))
        .collect()
}

fn set_spells_traps(g: &Game, s: Source) -> TargetSets {
    singles(
        PlayerId::BOTH
            .into_iter()
            .flat_map(|p| g.spells_traps_controlled(p))
            .filter(|&c| c != s.card && g.instance(c).position == Position::FaceDown),
    )
}

/// 7 Completed: the choice is kept in the counter, 1 for ATK, 2 for DEF.
fn seven_completed_equip(g: &mut Game, ctx: &mut EffectCtx, choice: u8) -> Flow {
    if g.instance(ctx.targets[0]).position.is_face_up() && g.equip(ctx.source, ctx.targets[0]) {
        g.set_counter(ctx.source, choice);
    } else if g.on_field(ctx.source) {
        g.destroy(ctx.source, ctx.source);
    }
    Flow::Done
}

pub fn register(r: &mut Registry) {
    // On activation, destroy every Field Spell on the field. During each
    // player's Standby Phase, the turn player takes 500 damage.
    r.register(
        BURNING_LAND,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(1, |g, ctx| {
                    for p in PlayerId::BOTH {
                        if let Some(f) = g.field_spell(p) {
                            g.destroy(f, ctx.source);
                        }
                    }
                    Flow::Done
                }),
                Effect {
                    timing: |g, s, w| {
                        standby_began(g, w.events, w.kind) && face_up_on_field(g, s.card)
                    },
                    ..Effect::trigger(true, |g, _ctx| {
                        let player = g.turn_player();
                        g.take_damage(player, 500);
                        Flow::Done
                    })
                },
            ],
        },
    );
    // At the start of Main Phase 1: until your next turn, neither player
    // can activate or Set Spell or Trap Cards. "At the start" is read as
    // "in Main Phase 1, with no chain".
    r.register(
        COLD_WAVE,
        single(Effect {
            timing: |g, s, _| g.phase() == Phase::Main1 && g.turn_player() == s.controller,
            ..Effect::activate(1, |g, ctx| {
                let until = g.turn() + 1;
                g.forbid_spells_and_traps_until(ctx.source, until);
                Flow::Done
            })
        }),
    );
    // Insect monsters your opponent controls cannot declare an attack.
    r.register(
        INSECT_BARRIER,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(1, ygo_engine::nothing),
                Effect {
                    restrict: Some(|g, s, t| {
                        g.instance(t).controller != s.controller && g.race(t) == "Insect"
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // Tribute 1 monster (only one with a matching Insect in the Deck);
    // Special Summon from your Deck 1 Insect whose
    // Level is 1 higher than the Tributed monster's.
    r.register(
        INSECT_IMITATION,
        single(Effect {
            cost_choices: Some(|g, s| {
                singles(
                    g.monsters_controlled(s.controller)
                        .into_iter()
                        .filter(|&m| !imitations(g, s.controller, g.stats(m).level).is_empty()),
                )
            }),
            cost: Some(|g, ctx| {
                ctx.value = i32::from(g.stats(ctx.cost[0]).level);
                g.tribute(ctx.cost[0]);
            }),
            ..Effect::activate(1, |g, ctx| {
                let options = imitations(g, ctx.controller, ctx.value as u8);
                if options.is_empty() || !g.can_special_summon(ctx.controller) {
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |_g, ctx, insect| {
                        ctx.targets = vec![insect];
                        Flow::Ask {
                            options: FACE_UP.to_vec(),
                            then: |g, ctx, option| {
                                g.special_summon(ctx.targets[0], ctx.controller, face_up(option));
                                g.shuffle_deck(ctx.controller);
                                Flow::Done
                            },
                        }
                    },
                }
            })
        }),
    );
    // During your opponent's Standby Phase, you can pay 500 LP to look at
    // 1 random card in their hand.
    r.register(
        INSPECTION,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(1, ygo_engine::nothing),
                Effect {
                    timing: |g, s, w| {
                        standby_began(g, w.events, w.kind)
                            && g.turn_player() != s.controller
                            && face_up_on_field(g, s.card)
                            && g.can_pay_lp(s.controller, 500)
                            && !g.hand(s.controller.opponent()).is_empty()
                    },
                    cost: Some(|g, ctx| g.pay_lp(ctx.controller, 500)),
                    ..Effect::trigger(false, |g, ctx| {
                        let hand = g.hand(ctx.controller.opponent()).to_vec();
                        if !hand.is_empty() {
                            let card = hand[g.random_index(hand.len())];
                            g.look_at(ctx.controller, &[card]);
                        }
                        Flow::Done
                    })
                },
            ],
        },
    );
    // Double the ATK of every Machine you control until the end of this
    // turn; destroy them in this turn's End Phase.
    r.register(
        LIMITER_REMOVAL,
        single(Effect {
            timing: |g, s, _| !face_up_machines(g, s.controller).is_empty(),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                let turn = g.turn();
                for m in face_up_machines(g, ctx.controller) {
                    let atk = g.atk(m);
                    g.modify_until_end_phase(ctx.source, m, atk, 0);
                    g.destroy_at_end_of_turn(ctx.source, m, turn);
                }
                Flow::Done
            })
        }),
    );
    // Target 1 monster you control that you own; shuffle it and your whole
    // hand into the Deck, then draw as many cards as went from the hand.
    // Not while you hold a card your opponent owns.
    r.register(
        MONSTER_RECOVERY,
        single(Effect {
            timing: |g, s, _| {
                g.hand(s.controller)
                    .iter()
                    .all(|&c| g.instance(c).owner == s.controller)
                    && g.monsters_controlled(s.controller)
                        .iter()
                        .any(|&m| g.instance(m).owner == s.controller)
            },
            targets: Some(|g, s| {
                singles(
                    g.monsters_controlled(s.controller)
                        .into_iter()
                        .filter(|&m| g.instance(m).owner == s.controller),
                )
            }),
            ..Effect::activate(2, |g, ctx| {
                let target = ctx.targets[0];
                if !g.on_field(target) || g.instance(target).controller != ctx.controller {
                    return Flow::Done;
                }
                g.return_to_deck(target);
                let hand = g.hand(ctx.controller).to_vec();
                for &c in &hand {
                    g.return_to_deck(c);
                }
                g.shuffle_deck(ctx.controller);
                g.draw(ctx.controller, hand.len());
                Flow::Done
            })
        }),
    );
    // Target 1 Set Spell/Trap on the field; destroy and banish it, and if
    // it was a Trap, banish every card with its name from both Decks.
    r.register(
        NOBLEMAN_OF_EXTERMINATION,
        single(Effect {
            timing: |g, s, _| !set_spells_traps(g, s).is_empty(),
            targets: Some(set_spells_traps),
            ..Effect::activate(1, |g, ctx| {
                let target = ctx.targets[0];
                if !g.on_field(target) || g.instance(target).position != Position::FaceDown {
                    return Flow::Done;
                }
                g.destroy(target, ctx.source);
                if g.instance(target).location != Location::Graveyard {
                    return Flow::Done;
                }
                g.banish(target);
                let stats = g.stats(target);
                if stats.kind == CardKind::Trap {
                    let id = stats.id;
                    for p in PlayerId::BOTH {
                        let copies: Vec<_> = g
                            .deck(p)
                            .iter()
                            .copied()
                            .filter(|&c| g.stats(c).id == id)
                            .collect();
                        for c in copies {
                            g.banish(c);
                        }
                    }
                }
                Flow::Done
            })
        }),
    );
    // Declare 1 card name; cards with that name cannot be used, except
    // those already on the field. The name is declared on resolution,
    // among the names you can see.
    r.register(
        PROHIBITION,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(1, |g, ctx| {
                    let options = declarable_names(g, ctx.controller);
                    if options.is_empty() {
                        return Flow::Done;
                    }
                    Flow::Choose {
                        options,
                        then: |g, ctx, card| {
                            if g.on_field(ctx.source) {
                                g.set_counter(ctx.source, card.0.wrapping_add(1));
                            }
                            Flow::Done
                        },
                    }
                }),
                Effect {
                    forbid: Some(|g, s, action| {
                        let card = match action {
                            Action::Summon { card, .. }
                            | Action::SetSpellTrap { card }
                            | Action::Activate { card, .. }
                            | Action::ProcedureSummon { card, .. } => *card,
                            _ => return false,
                        };
                        !g.on_field(card) && declared(g, s.card) == Some(g.stats(card).id)
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
    r.register(
        RAIN_OF_MERCY,
        single(Effect::activate(1, |g, _ctx| {
            for p in PlayerId::BOTH {
                g.gain_lp(p, 1000);
            }
            Flow::Done
        })),
    );
    // Equip only to a Machine; it gains 700 ATK or 700 DEF, as chosen.
    r.register(
        SEVEN_COMPLETED,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    targets: Some(|g, _| {
                        singles(
                            PlayerId::BOTH
                                .into_iter()
                                .flat_map(|p| face_up_machines(g, p))
                                .filter(|&m| g.spell_targetable(m)),
                        )
                    }),
                    ..Effect::activate(1, |_g, _ctx| Flow::Ask {
                        options: vec!["Gain 700 ATK", "Gain 700 DEF"],
                        then: |g, ctx, option| {
                            seven_completed_equip(g, ctx, if option == 0 { 1 } else { 2 })
                        },
                    })
                },
                Effect {
                    modify: Some(|g, s, t| {
                        if g.equip_target(s.card) != Some(t) {
                            return StatMod::default();
                        }
                        match g.counter(s.card) {
                            1 => StatMod { atk: 700, def: 0 },
                            2 => StatMod { atk: 0, def: 700 },
                            _ => StatMod::default(),
                        }
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // Each player targets 1 monster in their own Graveyard and Special
    // Summons it in face-down Defense Position. You target yours on
    // activation; your opponent picks theirs on resolution.
    r.register(
        THE_SHALLOW_GRAVE,
        single(Effect {
            timing: |g, s, _| {
                g.can_special_summon(s.controller) && !revivable(g, s.controller).is_empty()
            },
            targets: Some(|g, s| singles(revivable(g, s.controller))),
            ..Effect::activate(1, |g, ctx| {
                let mine = ctx.targets[0];
                if g.instance(mine).location == Location::Graveyard {
                    g.special_summon(mine, ctx.controller, Position::FaceDownDefense);
                }
                let opponent = ctx.controller.opponent();
                let options = revivable(g, opponent);
                if options.is_empty() || !g.can_special_summon(opponent) {
                    return Flow::Done;
                }
                Flow::ChooseBy {
                    chooser: opponent,
                    options,
                    then: |g, ctx, theirs| {
                        g.special_summon(
                            theirs,
                            ctx.controller.opponent(),
                            Position::FaceDownDefense,
                        );
                        Flow::Done
                    },
                }
            })
        }),
    );
}
