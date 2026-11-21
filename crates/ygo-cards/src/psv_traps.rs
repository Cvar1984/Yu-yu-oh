//! Traps of Pharaoh's Servant (owner, 2026-11-21), on the modern text;
//! see `docs/psv-plan.md`.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, EffectCtx, Event, Flow, Game, InstanceId, Lingering, LingeringKind,
    Location, Phase, PlayerId, Position, Registry, Source, StatMod, TargetSets, Window,
};

use crate::psv_monsters::is_effect_monster;
use crate::psv_spells::standby_began;

pub const APPROPRIATE: CardId = CardId(48539234);
pub const ARMORED_GLASS: CardId = CardId(36868108);
pub const DNA_SURGERY: CardId = CardId(74701381);
pub const IMPERIAL_ORDER: CardId = CardId(61740673);
pub const METAL_DETECTOR: CardId = CardId(75646520);
pub const MYSTIC_PROBE: CardId = CardId(49251811);
pub const THE_REGULATION_OF_TRIBE: CardId = CardId(296499);
pub const WORLD_SUPPRESSION: CardId = CardId(12253117);
pub const ATTACK_AND_RECEIVE: CardId = CardId(63689843);
pub const CALL_OF_THE_HAUNTED: CardId = CardId(97077563);
pub const CEASEFIRE: CardId = CardId(36468556);
pub const CHAIN_DESTRUCTION: CardId = CardId(1248895);
pub const DRIVING_SNOW: CardId = CardId(473469);
pub const EARTHSHAKER: CardId = CardId(60866277);
pub const FORCED_REQUISITION: CardId = CardId(74923978);
pub const GAMBLE: CardId = CardId(37313786);
pub const MAJOR_RIOT: CardId = CardId(9074847);
pub const RESPECT_PLAY: CardId = CardId(8951260);
pub const SOLOMONS_LAWBOOK: CardId = CardId(23471572);
pub const TIME_SEAL: CardId = CardId(35316708);
pub const GRAVITY_BIND: CardId = CardId(85742772);
pub const GUST: CardId = CardId(73079365);
pub const INFINITE_DISMISSAL: CardId = CardId(54109233);
pub const MAGIC_DRAIN: CardId = CardId(59344077);
pub const MICHIZURE: CardId = CardId(37580756);
pub const MINOR_GOBLIN_OFFICIAL: CardId = CardId(1918087);
pub const MIRROR_WALL: CardId = CardId(22359980);
pub const NUMINOUS_HEALER: CardId = CardId(2130625);
pub const SHADOW_OF_EYES: CardId = CardId(58621589);
pub const SKULL_INVITATION: CardId = CardId(98139712);
pub const SOLEMN_WISHES: CardId = CardId(35346968);
pub const TYPE_ZERO_MAGIC_CRUSHER: CardId = CardId(21237481);

/// The monster Types a card may declare (every Type in the pool).
pub const RACES: [&str; 20] = [
    "Aqua",
    "Beast",
    "Beast-Warrior",
    "Dinosaur",
    "Dragon",
    "Fairy",
    "Fiend",
    "Fish",
    "Insect",
    "Machine",
    "Plant",
    "Pyro",
    "Reptile",
    "Rock",
    "Sea Serpent",
    "Spellcaster",
    "Thunder",
    "Warrior",
    "Winged Beast",
    "Zombie",
];

/// The Type a card declared (kept as index + 1 in its counter).
fn declared_race(g: &Game, card: InstanceId) -> Option<&'static str> {
    match g.counter(card) {
        0 => None,
        n => RACES.get(usize::from(n) - 1).copied(),
    }
}

/// Declares a Type on resolution and keeps it in the card's counter.
fn declare_race(_g: &mut Game, _ctx: &mut EffectCtx) -> Flow {
    Flow::Ask {
        options: RACES.to_vec(),
        then: |g, ctx, option| {
            if g.on_field(ctx.source) {
                g.set_counter(ctx.source, option as u8 + 1);
            }
            Flow::Done
        },
    }
}

/// The last chain link activates a card of this kind and subtype.
fn activates(g: &Game, kind: CardKind, subtype: ygo_engine::SpellTrapType) -> bool {
    g.last_link().is_some_and(|l| {
        let s = g.stats(l.card);
        l.card_activation && !l.negated && s.kind == kind && s.subtype == subtype
    })
}

/// Negates every face-up card of this kind and subtype on the field for
/// the rest of the turn.
fn negate_all(
    g: &mut Game,
    source: InstanceId,
    kind: CardKind,
    subtype: ygo_engine::SpellTrapType,
) {
    let cards: Vec<_> = PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.spells_traps_controlled(p))
        .filter(|&c| {
            let s = g.stats(c);
            g.instance(c).position.is_face_up() && s.kind == kind && s.subtype == subtype
        })
        .collect();
    for c in cards {
        g.negate_until_end_of_turn(source, c);
    }
}

/// The Monster Card Attributes Earthshaker names.
pub const ATTRIBUTES: [&str; 6] = ["LIGHT", "DARK", "EARTH", "WATER", "FIRE", "WIND"];

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

/// A Continuous Trap: the activation does nothing, the rest are `more`.
fn continuous_trap(more: Vec<Effect>) -> CardScript {
    let mut effects = vec![Effect::activate(2, ygo_engine::nothing)];
    effects.extend(more);
    CardScript {
        tags: Vec::new(),
        effects,
    }
}

fn face_up_on_field(g: &Game, card: InstanceId) -> bool {
    g.on_field(card) && g.instance(card).position.is_face_up()
}

fn all_monsters(g: &Game) -> Vec<InstanceId> {
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .collect()
}

/// The open window, for `targets` functions that capture its events.
fn open_window(g: &Game) -> Window<'_> {
    g.window()
        .map(|(kind, events)| Window { kind, events })
        .unwrap_or_default()
}

/// Chain Destruction: monsters with 2000 or less ATK just summoned.
fn chain_destruction_targets(g: &Game, w: &Window) -> Vec<InstanceId> {
    w.events
        .iter()
        .filter_map(|e| match e {
            Event::Summoned { card, .. }
            | Event::SpecialSummoned { card, .. }
            | Event::FlipSummoned { card, .. } => Some(*card),
            _ => None,
        })
        .filter(|&c| face_up_on_field(g, c) && g.atk(c) <= 2000)
        .collect()
}

/// Gust and Driving Snow: one of your cards of `kind` was destroyed on the
/// field by a card your opponent controls and went to the Graveyard.
fn lost_to_opponent(g: &Game, s: Source, w: &Window, kind: CardKind) -> bool {
    w.events.iter().any(|e| match e {
        Event::DestroyedByEffect { card, source } => {
            let c = g.instance(*card);
            c.owner == s.controller
                && c.location == Location::Graveyard
                && g.stats(*card).kind == kind
                && g.instance(*source).controller != s.controller
                && w.events.iter().any(|e| {
                    matches!(e, Event::SentToGraveyard { card: x, from } if x == card && from.on_field())
                })
        }
        _ => false,
    })
}

fn other_spells_traps(g: &Game, s: Source) -> TargetSets {
    singles(
        PlayerId::BOTH
            .into_iter()
            .flat_map(|p| g.spells_traps_controlled(p))
            .filter(|&c| c != s.card),
    )
}

fn took_damage(s: Source, w: &Window) -> bool {
    w.events
        .iter()
        .any(|e| matches!(e, Event::DamageTaken { player, .. } if *player == s.controller))
}

fn copies_in_graveyard(g: &Game, player: PlayerId, id: CardId) -> u32 {
    g.graveyard(player)
        .iter()
        .filter(|&&c| g.stats(c).id == id)
        .count() as u32
}

/// The monster a card's `counter` remembers (stored as id + 1).
fn remembered(g: &Game, card: InstanceId) -> Option<InstanceId> {
    match g.counter(card) {
        0 => None,
        n => Some(InstanceId(n - 1)),
    }
}

/// Cards sent to the Graveyard in the window (Skull Invitation).
fn sent_to_graveyard(w: &Window) -> Vec<InstanceId> {
    w.events
        .iter()
        .filter_map(|e| match e {
            Event::SentToGraveyard { card, .. } => Some(*card),
            _ => None,
        })
        .collect()
}

/// Draws by `player` in the window that happened outside a Draw Phase.
/// The window may hold the start of a turn: phases entered in its events
/// say which phase each draw came in.
fn draws_outside_draw_phase(g: &Game, w: &Window, player: PlayerId) -> usize {
    let entered = w.events.iter().find_map(|e| match e {
        Event::PhaseEntered { phase } => Some(*phase),
        _ => None,
    });
    // Before the first phase change in the window, the phase is the one the
    // window started in: unknown here, so the current one unless a phase
    // was entered later.
    let mut phase = if entered.is_some() {
        None
    } else {
        Some(g.phase())
    };
    let mut n = 0;
    for e in w.events {
        match e {
            Event::PhaseEntered { phase: p } => phase = Some(*p),
            Event::Drew { player: p, .. } if *p == player && phase != Some(Phase::Draw) => n += 1,
            _ => {}
        }
    }
    n
}

fn discards_by(w: &Window, player: PlayerId) -> Vec<InstanceId> {
    w.events
        .iter()
        .filter_map(|e| match e {
            Event::Discarded { player: p, card } if *p == player => Some(*card),
            _ => None,
        })
        .collect()
}

/// Forced Requisition: the opponent discards one card per step until
/// `ctx.value` cards are gone or their hand is empty.
fn requisition_next(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let opponent = ctx.controller.opponent();
    let hand = g.hand(opponent).to_vec();
    if ctx.value <= 0 || hand.is_empty() {
        return Flow::Done;
    }
    Flow::ChooseBy {
        chooser: opponent,
        options: hand,
        then: |g, ctx, card| {
            g.discard(card);
            ctx.value -= 1;
            requisition_next(g, ctx)
        },
    }
}

/// Infinite Dismissal: Level 3 or lower monsters just Normal or Flip Summoned.
fn dismissed(g: &Game, w: &Window) -> Vec<InstanceId> {
    w.events
        .iter()
        .filter_map(|e| match e {
            Event::Summoned { card, position, .. } if position.is_face_up() => Some(*card),
            Event::FlipSummoned { card, .. } => Some(*card),
            _ => None,
        })
        .filter(|&c| face_up_on_field(g, c) && g.stats(c).level <= 3)
        .collect()
}

fn set_by_opponent(g: &Game, s: Source, w: &Window) -> Vec<InstanceId> {
    w.events
        .iter()
        .filter_map(|e| match e {
            Event::Summoned {
                player,
                card,
                position: Position::FaceDownDefense,
            } if *player != s.controller => Some(*card),
            _ => None,
        })
        .filter(|&c| g.instance(c).position == Position::FaceDownDefense && g.on_field(c))
        .collect()
}

/// Magic Drain: the opponent's Spell Card activation on the chain.
fn spell_to_drain(g: &Game, s: Source) -> Option<InstanceId> {
    g.last_link()
        .filter(|l| {
            l.card_activation
                && !l.negated
                && l.controller != s.controller
                && g.stats(l.card).kind == CardKind::Spell
        })
        .map(|l| l.card)
}

fn spells_in_hand(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    g.hand(player)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).kind == CardKind::Spell)
        .collect()
}

/// Earthshaker: `value` holds the first Attribute's index, then the
/// second's in bits 8 and up.
fn earthshaker_second(ctx: &mut EffectCtx) -> Flow {
    let first = ctx.value as usize;
    Flow::Ask {
        options: ATTRIBUTES
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != first)
            .map(|(_, &a)| a)
            .collect(),
        then: |_g, ctx, option| {
            let first = (ctx.value & 0xff) as usize;
            let second = (0..ATTRIBUTES.len())
                .filter(|&i| i != first)
                .nth(option)
                .expect("an offered Attribute");
            ctx.value = (first | second << 8) as i32;
            Flow::AskBy {
                chooser: ctx.controller.opponent(),
                options: vec![ATTRIBUTES[first], ATTRIBUTES[second]],
                then: |g, ctx, pick| {
                    let index = if pick == 0 {
                        ctx.value & 0xff
                    } else {
                        ctx.value >> 8
                    };
                    let picked = ATTRIBUTES[index as usize];
                    for m in all_monsters(g) {
                        if face_up_on_field(g, m) && g.stats(m).attribute.as_deref() == Some(picked)
                        {
                            g.destroy(m, ctx.source);
                        }
                    }
                    Flow::Done
                },
            }
        },
    }
}

pub fn register(r: &mut Registry) {
    // Activate only when your opponent draws outside a Draw Phase. Then,
    // each time they do, you draw 2 cards.
    r.register(
        APPROPRIATE,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, w| draws_outside_draw_phase(g, w, s.controller.opponent()) > 0,
                    ..Effect::activate(2, ygo_engine::nothing)
                },
                Effect {
                    timing: |g, s, w| {
                        face_up_on_field(g, s.card)
                            && draws_outside_draw_phase(g, w, s.controller.opponent()) > 0
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        g.draw(ctx.controller, 2);
                        Flow::Done
                    })
                },
            ],
        },
    );
    // When a monster is equipped with an Equip Card: negate the effects of
    // every Equip Card on the field this turn.
    r.register(
        ARMORED_GLASS,
        single(Effect {
            timing: |_, _, w| w.events.iter().any(|e| matches!(e, Event::Equipped { .. })),
            ..Effect::activate(2, |g, ctx| {
                let equips: Vec<_> = PlayerId::BOTH
                    .into_iter()
                    .flat_map(|p| g.spells_traps_controlled(p))
                    .filter(|&c| g.equip_target(c).is_some())
                    .collect();
                for e in equips {
                    g.negate_until_end_of_turn(ctx.source, e);
                }
                Flow::Done
            })
        }),
    );
    // Declare 1 Type: every face-up monster on the field becomes that Type.
    r.register(
        DNA_SURGERY,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(2, declare_race),
                Effect {
                    race: Some(|g, s, _| declared_race(g, s.card)),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // Negate all Spell effects on the field (Spells cannot be activated
    // either, see docs/psv-plan.md). In each Standby Phase pay 700 LP, or
    // this card is destroyed.
    r.register(
        IMPERIAL_ORDER,
        continuous_trap(vec![
            Effect {
                negate: Some(|g, _, card| g.stats(card).kind == CardKind::Spell),
                ..Effect::continuous()
            },
            Effect {
                timing: |g, s, w| {
                    standby_began(g, w.events, w.kind)
                        && g.turn_player() == s.controller
                        && face_up_on_field(g, s.card)
                },
                ..Effect::trigger(true, |g, ctx| {
                    if g.can_pay_lp(ctx.controller, 700) {
                        g.pay_lp(ctx.controller, 700);
                    } else {
                        g.destroy(ctx.source, ctx.source);
                    }
                    Flow::Done
                })
            },
        ]),
    );
    // When a Continuous Trap Card is activated: negate all Continuous Trap
    // Cards this turn.
    r.register(
        METAL_DETECTOR,
        single(Effect {
            timing: |g, _, _| activates(g, CardKind::Trap, ygo_engine::SpellTrapType::Continuous),
            ..Effect::activate(2, |g, ctx| {
                negate_all(
                    g,
                    ctx.source,
                    CardKind::Trap,
                    ygo_engine::SpellTrapType::Continuous,
                );
                Flow::Done
            })
        }),
    );
    // When a Continuous Spell Card is activated: negate all Continuous
    // Spell Cards this turn.
    r.register(
        MYSTIC_PROBE,
        single(Effect {
            timing: |g, _, _| activates(g, CardKind::Spell, ygo_engine::SpellTrapType::Continuous),
            ..Effect::activate(2, |g, ctx| {
                negate_all(
                    g,
                    ctx.source,
                    CardKind::Spell,
                    ygo_engine::SpellTrapType::Continuous,
                );
                Flow::Done
            })
        }),
    );
    // Declare 1 Type; monsters of that Type cannot attack. In each of your
    // Standby Phases, Tribute 1 monster or this card is destroyed.
    r.register(
        THE_REGULATION_OF_TRIBE,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(2, declare_race),
                Effect {
                    restrict: Some(|g, s, t| declared_race(g, s.card) == Some(g.race(t))),
                    ..Effect::continuous()
                },
                Effect {
                    timing: |g, s, w| {
                        standby_began(g, w.events, w.kind)
                            && g.turn_player() == s.controller
                            && face_up_on_field(g, s.card)
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        if g.monsters_controlled(ctx.controller).is_empty() {
                            g.destroy(ctx.source, ctx.source);
                            return Flow::Done;
                        }
                        Flow::Ask {
                            options: vec!["Tribute a monster", "Destroy this card"],
                            then: |g, ctx, option| {
                                if option != 0 {
                                    g.destroy(ctx.source, ctx.source);
                                    return Flow::Done;
                                }
                                Flow::Choose {
                                    options: g.monsters_controlled(ctx.controller),
                                    then: |g, _ctx, m| {
                                        g.tribute(m);
                                        Flow::Done
                                    },
                                }
                            },
                        }
                    })
                },
            ],
        },
    );
    // When a Field Spell Card is activated: negate it this turn.
    r.register(
        WORLD_SUPPRESSION,
        single(Effect {
            timing: |g, _, _| activates(g, CardKind::Spell, ygo_engine::SpellTrapType::Field),
            targets: Some(|g, _| g.last_link().map(|l| vec![l.card]).into_iter().collect()),
            ..Effect::activate(2, |g, ctx| {
                g.negate_until_end_of_turn(ctx.source, ctx.targets[0]);
                Flow::Done
            })
        }),
    );
    // When you take damage: 700 damage to your opponent, plus 300 for each
    // "Attack and Receive" in your Graveyard.
    r.register(
        ATTACK_AND_RECEIVE,
        single(Effect {
            timing: |_, s, w| took_damage(s, w),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                let n = copies_in_graveyard(g, ctx.controller, ATTACK_AND_RECEIVE);
                g.take_damage(ctx.controller.opponent(), 700 + 300 * n);
                Flow::Done
            })
        }),
    );
    // Target 1 monster in your Graveyard; Special Summon it in Attack
    // Position. When this card leaves the field, destroy that monster;
    // when that monster is destroyed, destroy this card.
    r.register(
        CALL_OF_THE_HAUNTED,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, _| g.can_special_summon(s.controller),
                    targets: Some(|g, s| {
                        singles(
                            g.graveyard(s.controller)
                                .iter()
                                .copied()
                                .filter(|&c| g.stats(c).is_monster() && g.special_summonable(c)),
                        )
                    }),
                    ..Effect::activate(2, |g, ctx| {
                        let target = ctx.targets[0];
                        if g.on_field(ctx.source)
                            && g.instance(target).location == Location::Graveyard
                            && g.special_summon(target, ctx.controller, Position::FaceUpAttack)
                        {
                            g.set_counter(ctx.source, target.0.wrapping_add(1));
                        }
                        Flow::Done
                    })
                },
                Effect {
                    timing: |g, s, w| {
                        remembered(g, s.card).is_some_and(|m| g.on_field(m))
                            && w.events.iter().any(|e| {
                                matches!(e, Event::SentToGraveyard { card, from }
                                    if *card == s.card && from.on_field())
                            })
                    },
                    damage_step: true,
                    ..Effect::trigger(true, |g, ctx| {
                        if let Some(m) = remembered(g, ctx.source) {
                            g.destroy(m, ctx.source);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    timing: |g, s, w| {
                        face_up_on_field(g, s.card)
                            && remembered(g, s.card).is_some_and(|m| {
                                w.events.iter().any(|e| {
                                    matches!(e, Event::DestroyedByBattle { card }
                                        | Event::DestroyedByEffect { card, .. } if *card == m)
                                })
                            })
                    },
                    damage_step: true,
                    ..Effect::trigger(true, |g, ctx| {
                        g.destroy(ctx.source, ctx.source);
                        Flow::Done
                    })
                },
            ],
        },
    );
    // If a face-down Defense Position monster or an Effect Monster is on
    // the field: turn every face-down Defense Position monster face-up in
    // Defense Position (no FLIP effects), and inflict 500 damage to your
    // opponent for each Effect Monster on the field.
    r.register(
        CEASEFIRE,
        single(Effect {
            timing: |g, _, _| {
                all_monsters(g).into_iter().any(|m| {
                    g.instance(m).position == Position::FaceDownDefense || is_effect_monster(g, m)
                })
            },
            ..Effect::activate(2, |g, ctx| {
                for m in all_monsters(g) {
                    g.turn_face_up_quietly(m, Position::FaceUpDefense);
                }
                let n = all_monsters(g)
                    .into_iter()
                    .filter(|&m| face_up_on_field(g, m) && is_effect_monster(g, m))
                    .count() as u32;
                g.take_damage(ctx.controller.opponent(), 500 * n);
                Flow::Done
            })
        }),
    );
    // When a monster with 2000 or less ATK is summoned: target it; destroy
    // every card with its name in its controller's hand and Deck.
    r.register(
        CHAIN_DESTRUCTION,
        single(Effect {
            timing: |g, _, w| !chain_destruction_targets(g, w).is_empty(),
            targets: Some(|g, _| singles(chain_destruction_targets(g, &open_window(g)))),
            ..Effect::activate(2, |g, ctx| {
                let target = ctx.targets[0];
                let id = g.stats(target).id;
                let player = g.instance(target).controller;
                let cards: Vec<_> = g
                    .hand(player)
                    .iter()
                    .chain(g.deck(player))
                    .copied()
                    .filter(|&c| g.stats(c).id == id)
                    .collect();
                for c in cards {
                    g.mill(c);
                }
                Flow::Done
            })
        }),
    );
    // When your Trap Card is destroyed on the field by your opponent's card
    // effect and sent to your Graveyard: destroy 1 Spell/Trap on the field.
    r.register(
        DRIVING_SNOW,
        single(Effect {
            timing: |g, s, w| {
                lost_to_opponent(g, s, w, CardKind::Trap) && !other_spells_traps(g, s).is_empty()
            },
            targets: Some(other_spells_traps),
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    // Choose 2 Attributes; your opponent picks 1 of them, and every face-up
    // monster of that Attribute is destroyed.
    r.register(
        EARTHSHAKER,
        single(Effect::activate(2, |_g, _ctx| Flow::Ask {
            options: ATTRIBUTES.to_vec(),
            then: |_g, ctx, first| {
                ctx.value = first as i32;
                earthshaker_second(ctx)
            },
        })),
    );
    // Activate when you discard. Then, each time you discard, your
    // opponent discards as many cards.
    r.register(
        FORCED_REQUISITION,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |_, s, w| !discards_by(w, s.controller).is_empty(),
                    ..Effect::activate(2, ygo_engine::nothing)
                },
                Effect {
                    timing: |g, s, w| {
                        face_up_on_field(g, s.card) && !discards_by(w, s.controller).is_empty()
                    },
                    targets: Some(|g, s| vec![discards_by(&open_window(g), s.controller)]),
                    ..Effect::trigger(true, |g, ctx| {
                        ctx.value = ctx.targets.len() as i32;
                        requisition_next(g, ctx)
                    })
                },
            ],
        },
    );
    // Level 4 or higher monsters cannot attack.
    r.register(
        GRAVITY_BIND,
        continuous_trap(vec![Effect {
            restrict: Some(|g, _, t| g.stats(t).level >= 4),
            ..Effect::continuous()
        }]),
    );
    // When your Spell Card is destroyed on the field by your opponent's
    // card effect and sent to your Graveyard: destroy 1 Spell/Trap on the
    // field.
    r.register(
        GUST,
        single(Effect {
            timing: |g, s, w| {
                lost_to_opponent(g, s, w, CardKind::Spell) && !other_spells_traps(g, s).is_empty()
            },
            targets: Some(other_spells_traps),
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    // Level 3 or lower monsters are destroyed in the End Phase of the turn
    // they are Normal or Flip Summoned. Each such summon is noted when it
    // happens and the destruction scheduled for that End Phase.
    r.register(
        INFINITE_DISMISSAL,
        continuous_trap(vec![Effect {
            timing: |g, s, w| face_up_on_field(g, s.card) && !dismissed(g, w).is_empty(),
            targets: Some(|g, _| vec![dismissed(g, &open_window(g))]),
            ..Effect::trigger(true, |g, ctx| {
                let turn = g.turn();
                for &m in &ctx.targets.clone() {
                    if g.on_field(m) {
                        g.destroy_at_end_of_turn(ctx.source, m, turn);
                    }
                }
                Flow::Done
            })
        }]),
    );
    // When your opponent activates a Spell Card: they may discard 1 Spell
    // to negate this card's effect; otherwise negate the activation and
    // destroy the Spell.
    r.register(
        MAGIC_DRAIN,
        single(Effect {
            timing: |g, s, _| spell_to_drain(g, s).is_some(),
            targets: Some(|g, s| spell_to_drain(g, s).map(|c| vec![c]).into_iter().collect()),
            damage_step: true,
            ..Effect::activate(3, |g, ctx| {
                let opponent = ctx.controller.opponent();
                if spells_in_hand(g, opponent).is_empty() {
                    g.negate_activation(ctx.targets[0], ctx.source);
                    return Flow::Done;
                }
                Flow::AskBy {
                    chooser: opponent,
                    options: vec!["Discard a Spell to negate Magic Drain", "Do not"],
                    then: |g, ctx, option| {
                        let opponent = ctx.controller.opponent();
                        if option != 0 {
                            g.negate_activation(ctx.targets[0], ctx.source);
                            return Flow::Done;
                        }
                        Flow::ChooseBy {
                            chooser: opponent,
                            options: spells_in_hand(g, opponent),
                            then: |g, _ctx, spell| {
                                g.discard(spell);
                                Flow::Done
                            },
                        }
                    },
                }
            })
        }),
    );
    // When a monster is sent from the field to your Graveyard, even in the
    // Damage Step: target 1 monster on the field; destroy it.
    r.register(
        MICHIZURE,
        single(Effect {
            timing: |g, s, w| {
                !all_monsters(g).is_empty()
                    && w.events.iter().any(|e| {
                        matches!(e, Event::SentToGraveyard { card, from: Location::MonsterZone(_) }
                            if g.instance(*card).owner == s.controller
                                && g.instance(*card).location == Location::Graveyard)
                    })
            },
            targets: Some(|g, _| singles(all_monsters(g))),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    // Only while your opponent has 6 or more cards in hand and you have 2
    // or less: call a coin toss; if right, draw until you hold 5 cards, if
    // wrong, skip your next turn.
    r.register(
        GAMBLE,
        single(Effect {
            timing: |g, s, _| {
                g.hand(s.controller.opponent()).len() >= 6 && g.hand(s.controller).len() <= 2
            },
            ..Effect::activate(2, |_g, _ctx| Flow::Ask {
                options: vec!["Call heads", "Call tails"],
                then: |g, ctx, option| {
                    let heads = g.coin(ctx.controller);
                    if heads == (option == 0) {
                        let n = 5usize.saturating_sub(g.hand(ctx.controller).len());
                        g.draw(ctx.controller, n);
                    } else {
                        g.skip_next_turn(ctx.controller);
                    }
                    Flow::Done
                },
            })
        }),
    );
    // When your monster is returned from the field to your hand by your
    // opponent's effect: return every monster on the field to the hand;
    // then each player Special Summons from their hand, face-down, as many
    // monsters as they had on the field (as many as they can).
    r.register(
        MAJOR_RIOT,
        single(Effect {
            timing: bounced_by_opponent,
            ..Effect::activate(2, |g, ctx| {
                let mine = g.monsters_controlled(ctx.controller).len() as i32;
                let theirs = g.monsters_controlled(ctx.controller.opponent()).len() as i32;
                for m in all_monsters(g) {
                    g.add_to_hand(m);
                }
                ctx.value = mine.min(0xff) | theirs.min(0xff) << 8;
                riot_next(g, ctx)
            })
        }),
    );
    // Activate only while your opponent has 3000 or less LP. During each
    // of their Standby Phases, inflict 500 damage to them.
    r.register(
        MINOR_GOBLIN_OFFICIAL,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, _| g.player(s.controller.opponent()).lp <= 3000,
                    ..Effect::activate(2, ygo_engine::nothing)
                },
                Effect {
                    timing: |g, s, w| {
                        standby_began(g, w.events, w.kind)
                            && g.turn_player() != s.controller
                            && face_up_on_field(g, s.card)
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        g.take_damage(ctx.controller.opponent(), 500);
                        Flow::Done
                    })
                },
            ],
        },
    );
    // Each opponent's monster that attacks while this card is face-up has
    // its ATK halved while this card stays (half its printed ATK: the
    // marks are on this card). During each of your Standby Phases, pay
    // 2000 LP or destroy this card.
    r.register(
        MIRROR_WALL,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(2, |g, ctx| {
                    g.clear_marks(ctx.source);
                    if let Some((attacker, _)) = g.current_attack()
                        && g.instance(attacker).controller != ctx.controller
                    {
                        mark(g, ctx.source, attacker);
                    }
                    Flow::Done
                }),
                Effect {
                    on_attack: Some(|g, s, attacker| {
                        if g.instance(attacker).controller != s.controller {
                            mark(g, s.card, attacker);
                        }
                    }),
                    modify: Some(|g, s, t| {
                        if g.marked_by(s.card).contains(&t) {
                            StatMod {
                                atk: -(g.stats(t).atk / 2),
                                def: 0,
                            }
                        } else {
                            StatMod::default()
                        }
                    }),
                    ..Effect::continuous()
                },
                Effect {
                    timing: |g, s, w| {
                        standby_began(g, w.events, w.kind)
                            && g.turn_player() == s.controller
                            && face_up_on_field(g, s.card)
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        if !g.can_pay_lp(ctx.controller, 2000) {
                            g.destroy(ctx.source, ctx.source);
                            return Flow::Done;
                        }
                        Flow::Ask {
                            options: vec!["Pay 2000 LP", "Destroy Mirror Wall"],
                            then: |g, ctx, option| {
                                if option == 0 {
                                    g.pay_lp(ctx.controller, 2000);
                                } else {
                                    g.destroy(ctx.source, ctx.source);
                                }
                                Flow::Done
                            },
                        }
                    })
                },
            ],
        },
    );
    // When you take damage: gain 1000 LP, plus 500 for each "Numinous
    // Healer" in your Graveyard.
    r.register(
        NUMINOUS_HEALER,
        single(Effect {
            timing: |_, s, w| took_damage(s, w),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                let n = copies_in_graveyard(g, ctx.controller, NUMINOUS_HEALER);
                g.gain_lp(ctx.controller, 1000 + 500 * n);
                Flow::Done
            })
        }),
    );
    // When a monster is Set on your opponent's field: target 1 of them;
    // change it to face-up Attack Position (no FLIP effects).
    r.register(
        SHADOW_OF_EYES,
        single(Effect {
            timing: |g, s, w| !set_by_opponent(g, s, w).is_empty(),
            targets: Some(|g, s| singles(set_by_opponent(g, s, &open_window(g)))),
            ..Effect::activate(2, |g, ctx| {
                g.turn_face_up_quietly(ctx.targets[0], Position::FaceUpAttack);
                Flow::Done
            })
        }),
    );
    // Each time a card is sent to the Graveyard, inflict 300 damage to its
    // owner for each card. The cards of one window are counted together.
    r.register(
        SKULL_INVITATION,
        continuous_trap(vec![Effect {
            timing: |g, s, w| face_up_on_field(g, s.card) && !sent_to_graveyard(w).is_empty(),
            targets: Some(|g, _| vec![sent_to_graveyard(&open_window(g))]),
            damage_step: true,
            ..Effect::trigger(true, |g, ctx| {
                for p in PlayerId::BOTH {
                    let n = ctx
                        .targets
                        .iter()
                        .filter(|&&c| g.instance(c).owner == p)
                        .count() as u32;
                    if n > 0 {
                        g.take_damage(p, 300 * n);
                    }
                }
                Flow::Done
            })
        }]),
    );
    // Each player shows their hand to the opponent during their own turn.
    r.register(
        RESPECT_PLAY,
        CardScript {
            tags: vec![ygo_engine::Tag::RevealTurnPlayerHand],
            effects: vec![Effect::activate(2, ygo_engine::nothing)],
        },
    );
    // Skip your next Standby Phase.
    r.register(
        SOLOMONS_LAWBOOK,
        single(Effect::activate(2, |g, ctx| {
            g.skip_next_standby(ctx.controller);
            Flow::Done
        })),
    );
    // Skip the Draw Phase of your opponent's next turn (its draw).
    r.register(
        TIME_SEAL,
        single(Effect::activate(2, |g, ctx| {
            g.skip_next_draw(ctx.controller.opponent());
            Flow::Done
        })),
    );
    // Each time you draw a card (or cards), gain 500 LP.
    r.register(
        SOLEMN_WISHES,
        continuous_trap(vec![Effect {
            timing: |g, s, w| {
                face_up_on_field(g, s.card)
                    && w.events
                        .iter()
                        .any(|e| matches!(e, Event::Drew { player, .. } if *player == s.controller))
            },
            ..Effect::trigger(true, |g, ctx| {
                g.gain_lp(ctx.controller, 500);
                Flow::Done
            })
        }]),
    );
    // Discard 1 Spell to inflict 500 damage to your opponent. Used as a
    // Main Phase action of the face-up card.
    r.register(
        TYPE_ZERO_MAGIC_CRUSHER,
        continuous_trap(vec![Effect {
            timing: |g, s, _| face_up_on_field(g, s.card),
            cost_choices: Some(|g, s| singles(spells_in_hand(g, s.controller))),
            cost: Some(|g, ctx| g.discard(ctx.cost[0])),
            ..Effect::ignition(|g, ctx| {
                g.take_damage(ctx.controller.opponent(), 500);
                Flow::Done
            })
        }]),
    );
}

/// Major Riot: one of your monsters went from the field to your hand
/// while a card your opponent controls resolved.
fn bounced_by_opponent(g: &Game, s: Source, w: &Window) -> bool {
    let bounced = w.events.iter().any(|e| {
        matches!(e, Event::AddedToHand { player, from: Location::MonsterZone(_), .. }
            if *player == s.controller)
    });
    bounced
        && w.events.iter().any(|e| {
            matches!(e, Event::Resolved { card, .. } if g.instance(*card).controller != s.controller)
        })
}

fn riot_options(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    if !g.can_special_summon(player) {
        return Vec::new();
    }
    g.hand(player)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).is_monster() && g.special_summonable(c))
        .collect()
}

/// Major Riot: `value` holds how many monsters each player still Special
/// Summons, yours in the low byte and your opponent's in the next.
fn riot_next(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    for (shift, player) in [(0, ctx.controller), (8, ctx.controller.opponent())] {
        let left = (ctx.value >> shift) & 0xff;
        if left == 0 {
            continue;
        }
        let options = riot_options(g, player);
        if options.is_empty() {
            ctx.value &= !(0xff << shift);
            continue;
        }
        let then = |g: &mut Game, ctx: &mut EffectCtx, card: InstanceId| {
            let player = g.instance(card).controller;
            let shift = if player == ctx.controller { 0 } else { 8 };
            g.special_summon(card, player, Position::FaceDownDefense);
            ctx.value -= 1 << shift;
            riot_next(g, ctx)
        };
        return if player == ctx.controller {
            Flow::Choose { options, then }
        } else {
            Flow::ChooseBy {
                chooser: player,
                options,
                then,
            }
        };
    }
    Flow::Done
}

/// Mirror Wall remembers each attacker it halves.
fn mark(g: &mut Game, source: InstanceId, card: InstanceId) {
    if !g.marked_by(source).contains(&card) {
        g.add_lingering(Lingering {
            source,
            kind: LingeringKind::Marked { card },
            expires_turn: u32::MAX,
        });
    }
}
