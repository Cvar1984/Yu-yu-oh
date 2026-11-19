//! The last cards: special Equips, Continuous Spells and Traps, and the
//! Traps that answer summons, attacks and other cards' effects.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, EffectCtx, Event, Flow, Game, InstanceId, Location, Phase, PlayerId,
    Position, Registry, Source, StatMod, Tag, TargetSets, Window, WindowKind,
};

use crate::continuous::{equip_resolve, equipped_bonus, face_up_monsters};

pub const SWORD_OF_DEEP_SEATED: CardId = CardId(98495314);
pub const GERM_INFECTION: CardId = CardId(24668830);
pub const PARALYZING_POTION: CardId = CardId(50152549);
pub const RING_OF_MAGNETISM: CardId = CardId(20436034);
pub const STIM_PACK: CardId = CardId(83225447);
pub const AXE_OF_DESPAIR: CardId = CardId(40619825);
pub const BLACK_PENDANT: CardId = CardId(65169794);
pub const HORN_OF_LIGHT: CardId = CardId(38552107);
pub const HORN_OF_THE_UNICORN: CardId = CardId(64047146);
pub const MEGAMORPH: CardId = CardId(22046459);
pub const SNATCH_STEAL: CardId = CardId(45986603);
pub const SPELLBINDING_CIRCLE: CardId = CardId(18807108);
pub const ROBBIN_GOBLIN: CardId = CardId(88279736);
pub const GRAVEKEEPERS_SERVANT: CardId = CardId(16762927);
pub const TOLL: CardId = CardId(82003859);
pub const CHAIN_ENERGY: CardId = CardId(79323590);
pub const CHORUS_OF_SANCTUARY: CardId = CardId(81380218);
pub const FAIRYS_HAND_MIRROR: CardId = CardId(17653779);
pub const FAKE_TRAP: CardId = CardId(3027001);
pub const HORN_OF_HEAVEN: CardId = CardId(98069388);
pub const HOUSE_OF_ADHESIVE_TAPE: CardId = CardId(15083728);
pub const EATGABOON: CardId = CardId(42578427);
const HEAVY_STORM: CardId = CardId(19613556);
const FINAL_DESTINY: CardId = CardId(18591904);
use crate::flips::{DRAGON_CAPTURE_JAR, DRAGON_PIPER};

fn script(effects: Vec<Effect>) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects,
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn active(g: &Game, s: Source) -> bool {
    g.on_field(s.card) && g.instance(s.card).position.is_face_up()
}

fn sent_from_spell_trap_zone(s: Source, w: &Window) -> bool {
    w.events.iter().any(|e| {
        matches!(e,
        Event::SentToGraveyard { card, from: Location::SpellTrapZone(_) } if *card == s.card)
    })
}

fn standby_of(g: &Game, player: PlayerId, w: &Window) -> bool {
    w.kind == WindowKind::Open
        && g.phase() == Phase::Standby
        && g.turn_player() == player
        && w.events.contains(&Event::PhaseEntered {
            phase: Phase::Standby,
        })
}

/// An Equip Spell: activation targets `targets` and runs `resolve`, then
/// a continuous effect and any further effects.
fn equip_with(
    targets: fn(&Game, Source) -> TargetSets,
    resolve: fn(&mut Game, &mut EffectCtx) -> Flow,
    continuous: Effect,
    rest: Vec<Effect>,
) -> CardScript {
    let mut effects = vec![
        Effect {
            targets: Some(targets),
            timing: |g, s, _| {
                let f = g
                    .script(s.card)
                    .and_then(|sc| sc.effects[0].targets)
                    .expect("equip targets");
                !f(g, s).is_empty()
            },
            ..Effect::activate(1, resolve)
        },
        continuous,
    ];
    effects.extend(rest);
    script(effects)
}

fn equip(
    targets: fn(&Game, Source) -> TargetSets,
    continuous: Effect,
    rest: Vec<Effect>,
) -> CardScript {
    equip_with(targets, equip_resolve, continuous, rest)
}

/// Snatch Steal: equip and take control.
fn snatch_resolve(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let target = ctx.targets[0];
    if !(g.equip(ctx.source, target) && g.change_control(target, ctx.controller))
        && g.on_field(ctx.source)
    {
        g.destroy(ctx.source, ctx.source);
    }
    Flow::Done
}

fn any_face_up(g: &Game, _s: Source) -> TargetSets {
    face_up_monsters(g, |g, m| g.spell_targetable(m))
}

fn non_machine(g: &Game, _s: Source) -> TargetSets {
    face_up_monsters(g, |g, m| {
        g.spell_targetable(m) && g.stats(m).race != "Machine"
    })
}

fn bonus(atk: i32, def: i32) -> Effect {
    Effect {
        modify: Some(match (atk, def) {
            (500, 500) => |g, s, t| equipped_bonus(g, s, t, 500, 500),
            (700, 700) => |g, s, t| equipped_bonus(g, s, t, 700, 700),
            (1000, 0) => |g, s, t| equipped_bonus(g, s, t, 1000, 0),
            (500, 0) => |g, s, t| equipped_bonus(g, s, t, 500, 0),
            (0, 800) => |g, s, t| equipped_bonus(g, s, t, 0, 800),
            _ => unreachable!("add a row for {atk}/{def}"),
        }),
        ..Effect::continuous()
    }
}

fn to_top_of_deck(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    if g.instance(ctx.source).location == Location::Graveyard {
        g.place_on_top_of_deck(ctx.source);
    }
    Flow::Done
}

/// The last chain link, if the opponent of `s` activated it.
fn opposing_link(g: &Game, s: Source) -> Option<ygo_engine::ChainLink> {
    g.last_link()
        .filter(|l| l.controller != s.controller && !l.negated)
        .cloned()
}

/// Fairy's Hand Mirror: (the Spell, a new single target) pairs the Spell
/// could legally have chosen instead.
fn mirror_targets(g: &Game, s: Source) -> TargetSets {
    let Some(link) = opposing_link(g, s) else {
        return Vec::new();
    };
    let is_spell = g.stats(link.card).kind == CardKind::Spell;
    let one_monster = link.targets.len() == 1
        && g.stats(link.targets[0]).is_monster()
        && g.on_field(link.targets[0]);
    if !is_spell || !one_monster {
        return Vec::new();
    }
    let Some(f) = g
        .script(link.card)
        .and_then(|sc| sc.effects.get(usize::from(link.effect)))
        .and_then(|e| e.targets)
    else {
        return Vec::new();
    };
    let spell = Source {
        card: link.card,
        controller: link.controller,
    };
    f(g, spell)
        .into_iter()
        .filter(|set| set.len() == 1 && set[0] != link.targets[0])
        .map(|set| vec![link.card, set[0]])
        .collect()
}

/// Fake Trap answers an opposing effect that targets one of the
/// controller's Traps, or that destroys every Spell and Trap.
fn fake_trap_needed(g: &Game, s: Source) -> bool {
    let Some(link) = opposing_link(g, s) else {
        return false;
    };
    let mine = |c: &InstanceId| {
        g.instance(*c).controller == s.controller
            && g.on_field(*c)
            && g.stats(*c).kind == CardKind::Trap
    };
    // Effects that destroy Traps without targeting: Heavy Storm and Final
    // Destiny, and Dragon Piper when a face-up Dragon Capture Jar is ours.
    let jar_at_risk = g
        .spells_traps_controlled(s.controller)
        .into_iter()
        .any(|c| g.stats(c).id == DRAGON_CAPTURE_JAR && g.instance(c).position.is_face_up());
    let mass = matches!(g.stats(link.card).id, HEAVY_STORM | FINAL_DESTINY)
        || (g.stats(link.card).id == DRAGON_PIPER && jar_at_risk);
    let traps_at_risk = g
        .spells_traps_controlled(s.controller)
        .into_iter()
        .any(|c| c != s.card && g.stats(c).kind == CardKind::Trap);
    (link.targets.iter().any(mine) || mass) && traps_at_risk
}

/// Trap Hole's pattern for House of Adhesive Tape and Eatgaboon: the
/// opponent's face-up Normal, Tribute or Flip Summon of a monster whose
/// stat passes `weak`.
fn weak_summon(
    g: &Game,
    s: Source,
    w: &Window,
    weak: fn(&Game, InstanceId) -> bool,
) -> Option<InstanceId> {
    if w.kind != WindowKind::SummonSuccess {
        return None;
    }
    w.events.iter().find_map(|e| match e {
        Event::Summoned {
            player,
            card,
            position: Position::FaceUpAttack,
        }
        | Event::FlipSummoned { player, card }
            if *player != s.controller && g.on_field(*card) && weak(g, *card) =>
        {
            Some(*card)
        }
        _ => None,
    })
}

/// The weakness test is chosen by card id at run time, since effects are
/// plain function pointers.
fn summon_trap() -> CardScript {
    script(vec![Effect {
        timing: |g, s, w| weak_summon(g, s, w, weak_for(g, s)).is_some(),
        targets: Some(|g, s| {
            let found = g.window().and_then(|(kind, events)| {
                weak_summon(g, s, &Window { kind, events }, weak_for(g, s))
            });
            singles(found)
        }),
        ..Effect::activate(2, |g, ctx| {
            g.destroy(ctx.targets[0], ctx.source);
            Flow::Done
        })
    }])
}

fn weak_for(g: &Game, s: Source) -> fn(&Game, InstanceId) -> bool {
    if g.stats(s.card).id == EATGABOON {
        |g, m| g.atk(m) <= 500
    } else {
        |g, m| g.def(m) <= 500
    }
}

pub fn register(r: &mut Registry) {
    r.register(
        SWORD_OF_DEEP_SEATED,
        equip(
            any_face_up,
            bonus(500, 500),
            vec![Effect {
                timing: |_, s, w| {
                    w.events.iter().any(
                        |e| matches!(e, Event::SentToGraveyard { card, .. } if *card == s.card),
                    )
                },
                ..Effect::trigger(true, to_top_of_deck)
            }],
        ),
    );
    r.register(
        GERM_INFECTION,
        equip(
            non_machine,
            Effect {
                modify: Some(|g, s, t| {
                    if g.equip_target(s.card) == Some(t) {
                        StatMod {
                            atk: -300 * i32::from(g.counter(s.card)),
                            def: 0,
                        }
                    } else {
                        StatMod::default()
                    }
                }),
                ..Effect::continuous()
            },
            vec![Effect {
                timing: |g, s, w| {
                    active(g, s)
                        && g.equip_target(s.card)
                            .is_some_and(|t| standby_of(g, g.instance(t).controller, w))
                },
                ..Effect::trigger(true, |g, ctx| {
                    let n = g.counter(ctx.source).saturating_add(1);
                    g.set_counter(ctx.source, n);
                    Flow::Done
                })
            }],
        ),
    );
    r.register(
        PARALYZING_POTION,
        equip(
            non_machine,
            Effect {
                restrict: Some(|g, s, t| g.equip_target(s.card) == Some(t)),
                ..Effect::continuous()
            },
            Vec::new(),
        ),
    );
    r.register(
        RING_OF_MAGNETISM,
        equip(
            |g, s| {
                face_up_monsters(g, |g, m| {
                    g.instance(m).controller == s.controller && g.spell_targetable(m)
                })
            },
            Effect {
                modify: Some(|g, s, t| {
                    if g.equip_target(s.card) == Some(t) {
                        StatMod {
                            atk: -500,
                            def: -500,
                        }
                    } else {
                        StatMod::default()
                    }
                }),
                attack_lock: Some(|g, s, attacker| {
                    let target = g.equip_target(s.card)?;
                    (g.instance(attacker).controller != g.instance(target).controller)
                        .then_some(target)
                }),
                ..Effect::continuous()
            },
            Vec::new(),
        ),
    );
    r.register(
        STIM_PACK,
        equip(
            any_face_up,
            Effect {
                modify: Some(|g, s, t| {
                    if g.equip_target(s.card) == Some(t) {
                        StatMod {
                            atk: 700 - 200 * i32::from(g.counter(s.card)),
                            def: 0,
                        }
                    } else {
                        StatMod::default()
                    }
                }),
                ..Effect::continuous()
            },
            vec![Effect {
                timing: |g, s, w| active(g, s) && standby_of(g, s.controller, w),
                ..Effect::trigger(true, |g, ctx| {
                    let n = g.counter(ctx.source).saturating_add(1);
                    g.set_counter(ctx.source, n);
                    Flow::Done
                })
            }],
        ),
    );
    r.register(
        AXE_OF_DESPAIR,
        equip(
            any_face_up,
            bonus(1000, 0),
            vec![Effect {
                timing: |g, s, w| {
                    sent_from_spell_trap_zone(s, w)
                        && !g.monsters_controlled(s.controller).is_empty()
                },
                cost_choices: Some(|g, s| singles(g.monsters_controlled(s.controller))),
                cost: Some(|g, ctx| g.tribute(ctx.cost[0])),
                misses_timing: true,
                ..Effect::trigger(false, to_top_of_deck)
            }],
        ),
    );
    r.register(
        BLACK_PENDANT,
        equip(
            any_face_up,
            bonus(500, 0),
            vec![Effect {
                timing: |_, s, w| sent_from_spell_trap_zone(s, w),
                ..Effect::trigger(true, |g, ctx| {
                    g.take_damage(ctx.controller.opponent(), 500);
                    Flow::Done
                })
            }],
        ),
    );
    r.register(
        HORN_OF_LIGHT,
        equip(
            any_face_up,
            bonus(0, 800),
            vec![Effect {
                timing: |g, s, w| {
                    sent_from_spell_trap_zone(s, w) && g.can_pay_lp(s.controller, 500)
                },
                cost: Some(|g, ctx| g.pay_lp(ctx.controller, 500)),
                misses_timing: true,
                ..Effect::trigger(false, to_top_of_deck)
            }],
        ),
    );
    r.register(
        HORN_OF_THE_UNICORN,
        equip(
            any_face_up,
            bonus(700, 700),
            vec![Effect {
                timing: |_, s, w| sent_from_spell_trap_zone(s, w),
                ..Effect::trigger(true, to_top_of_deck)
            }],
        ),
    );
    r.register(
        MEGAMORPH,
        equip(
            any_face_up,
            Effect {
                modify: Some(|g, s, t| {
                    if g.equip_target(s.card) != Some(t) {
                        return StatMod::default();
                    }
                    let (mine, theirs) = (
                        g.player(s.controller).lp,
                        g.player(s.controller.opponent()).lp,
                    );
                    let base = g.stats(t).atk;
                    let atk = if mine < theirs {
                        base
                    } else if mine > theirs {
                        -(base / 2)
                    } else {
                        0
                    };
                    StatMod { atk, def: 0 }
                }),
                ..Effect::continuous()
            },
            Vec::new(),
        ),
    );
    // Control returns however Snatch Steal leaves the field (the engine
    // does it for Tag::ControlWhileEquipped).
    r.register(SNATCH_STEAL, {
        let mut script = equip_with(
            |g, s| {
                face_up_monsters(g, |g, m| {
                    g.instance(m).controller != s.controller && g.spell_targetable(m)
                })
            },
            snatch_resolve,
            Effect::continuous(),
            vec![Effect {
                timing: |g, s, w| active(g, s) && standby_of(g, s.controller.opponent(), w),
                ..Effect::trigger(true, |g, ctx| {
                    g.gain_lp(ctx.controller.opponent(), 1000);
                    Flow::Done
                })
            }],
        );
        script.tags = vec![Tag::ControlWhileEquipped];
        script
    });
    // Activate by targeting 1 monster your opponent controls; it cannot
    // attack or change its battle position. When that monster is
    // destroyed, destroy this card. Its monster is kept in the counter
    // (plus one), since the card stays if the monster leaves another way.
    r.register(
        SPELLBINDING_CIRCLE,
        CardScript {
            tags: vec![Tag::StaysWithoutTarget],
            effects: vec![
                Effect {
                    targets: Some(|g, s| singles(g.monsters_controlled(s.controller.opponent()))),
                    timing: |g, s, _| !g.monsters_controlled(s.controller.opponent()).is_empty(),
                    ..Effect::activate(2, |g, ctx| {
                        if g.attach(ctx.source, ctx.targets[0]) {
                            g.set_counter(ctx.source, ctx.targets[0].0.wrapping_add(1));
                        } else {
                            g.destroy(ctx.source, ctx.source);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    restrict: Some(|g, s, t| g.equip_target(s.card) == Some(t)),
                    lock_position: Some(|g, s, t| g.equip_target(s.card) == Some(t)),
                    ..Effect::continuous()
                },
                Effect {
                    timing: |g, s, w| {
                        let n = g.counter(s.card);
                        active(g, s)
                            && n > 0
                            && w.events.iter().any(|e| {
                                matches!(e,
                                Event::DestroyedByEffect { card, .. } | Event::DestroyedByBattle { card }
                                    if card.0.wrapping_add(1) == n)
                            })
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        g.destroy(ctx.source, ctx.source);
                        Flow::Done
                    })
                },
            ],
        },
    );
    r.register(
        ROBBIN_GOBLIN,
        script(vec![
            Effect::activate(2, ygo_engine::nothing),
            // Each time a monster you control inflicts battle damage to your
            // opponent: a trigger of this face-up Continuous Trap.
            Effect {
                timing: |g, s, w| {
                    active(g, s)
                        && w.events.iter().any(|e| {
                            matches!(e, Event::BattleDamageDealt { damaged, .. } if *damaged != s.controller)
                        })
                },
                ..Effect::trigger(true, |g, ctx| {
                    let hand = g.hand(ctx.controller.opponent()).to_vec();
                    if !hand.is_empty() {
                        let pick = hand[g.random_index(hand.len())];
                        g.discard(pick);
                    }
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        GRAVEKEEPERS_SERVANT,
        script(vec![
            Effect::activate(1, ygo_engine::nothing),
            Effect {
                restrict: Some(|g, s, t| {
                    let p = g.instance(t).controller;
                    p != s.controller && g.deck_size(p) == 0
                }),
                on_attack: Some(|g, s, attacker| {
                    let p = g.instance(attacker).controller;
                    if p != s.controller
                        && let Some(&top) = g.deck(p).last()
                    {
                        g.mill_by(top, s.controller);
                    }
                }),
                ..Effect::continuous()
            },
        ]),
    );
    r.register(
        TOLL,
        script(vec![
            Effect::activate(1, ygo_engine::nothing),
            Effect {
                restrict: Some(|g, _, t| !g.can_pay_lp(g.instance(t).controller, 500)),
                on_attack: Some(|g, _, attacker| {
                    let p = g.instance(attacker).controller;
                    g.pay_lp(p, 500);
                }),
                ..Effect::continuous()
            },
        ]),
    );
    r.register(
        CHAIN_ENERGY,
        CardScript {
            tags: vec![Tag::HandPlayToll],
            effects: vec![Effect::activate(1, ygo_engine::nothing)],
        },
    );
    r.register(
        CHORUS_OF_SANCTUARY,
        script(vec![
            Effect::activate(1, ygo_engine::nothing),
            Effect {
                modify: Some(|g, _, t| {
                    if matches!(
                        g.instance(t).position,
                        Position::FaceUpDefense | Position::FaceDownDefense
                    ) {
                        StatMod { atk: 0, def: 500 }
                    } else {
                        StatMod::default()
                    }
                }),
                ..Effect::continuous()
            },
        ]),
    );
    r.register(
        FAIRYS_HAND_MIRROR,
        script(vec![Effect {
            timing: |g, s, _| !mirror_targets(g, s).is_empty(),
            targets: Some(mirror_targets),
            ..Effect::activate(2, |g, ctx| {
                g.retarget(ctx.targets[0], vec![ctx.targets[1]]);
                Flow::Done
            })
        }]),
    );
    r.register(
        FAKE_TRAP,
        script(vec![Effect {
            timing: |g, s, _| fake_trap_needed(g, s),
            ..Effect::activate(2, |g, ctx| {
                for c in g.spells_traps_controlled(ctx.controller) {
                    if c != ctx.source && g.stats(c).kind == CardKind::Trap {
                        g.shield_until_chain_ends(c);
                    }
                }
                g.destroy(ctx.source, ctx.source);
                Flow::Done
            })
        }]),
    );
    r.register(
        HORN_OF_HEAVEN,
        script(vec![Effect {
            timing: |g, s, w| {
                w.kind == WindowKind::SummonDeclared
                    && g.pending_summon().is_some()
                    && !g.monsters_controlled(s.controller).is_empty()
            },
            cost_choices: Some(|g, s| singles(g.monsters_controlled(s.controller))),
            cost: Some(|g, ctx| g.tribute(ctx.cost[0])),
            ..Effect::activate(3, |g, ctx| {
                g.negate_summon(ctx.source);
                Flow::Done
            })
        }]),
    );
    r.register(HOUSE_OF_ADHESIVE_TAPE, summon_trap());
    r.register(EATGABOON, summon_trap());
}
