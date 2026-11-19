//! Effect Monsters, batch B: ignition effects, coins, position-change
//! and Standby Phase triggers, the Damage Step trio, attack
//! consequences, and the summon-restricted monsters with their enablers.

use ygo_data::CardId;

use crate::positions;
use ygo_engine::{
    CardScript, Effect, EffectCtx, EffectKind, Event, Flow, Game, InstanceId, Location, Phase,
    Position, Registry, Source, StatMod, Tag, TargetSets, Window, WindowKind,
};

pub const CATAPULT_TURTLE: CardId = CardId(95727991);
pub const CANNON_SOLDIER: CardId = CardId(11384280);
pub const TIME_WIZARD: CardId = CardId(71625222);
pub const BARREL_DRAGON: CardId = CardId(81480460);
pub const KARATE_MAN: CardId = CardId(23289281);
pub const THE_LITTLE_SWORDSMAN_OF_AILE: CardId = CardId(25109950);
pub const BLAST_JUGGLER: CardId = CardId(70138455);
pub const THUNDER_DRAGON: CardId = CardId(31786629);
pub const JIRAI_GUMO: CardId = CardId(94773007);
pub const DARK_ELF: CardId = CardId(21417692);
pub const CRASS_CLOWN: CardId = CardId(93889755);
pub const DREAM_CLOWN: CardId = CardId(13215230);
pub const YADO_KARU: CardId = CardId(29380133);
pub const TAINTED_WISDOM: CardId = CardId(28725004);
pub const PUMPKING_THE_KING_OF_GHOSTS: CardId = CardId(29155212);
pub use crate::flips::CASTLE_OF_DARK_ILLUSIONS;
pub const MUSHROOM_MAN_2: CardId = CardId(93900406);
pub const DARK_ZEBRA: CardId = CardId(59784896);
pub const SANGA_OF_THE_THUNDER: CardId = CardId(25955164);
pub const KAZEJIN: CardId = CardId(62340868);
pub const SUIJIN: CardId = CardId(98434877);
pub const STEEL_SCORPION: CardId = CardId(13599884);
pub const ELECTRIC_LIZARD: CardId = CardId(55875323);
pub const KOTODAMA: CardId = CardId(19406822);
pub const BOAR_SOLDIER: CardId = CardId(21340051);
pub const THE_UNHAPPY_MAIDEN: CardId = CardId(51275027);
pub const BANISHER_OF_THE_LIGHT: CardId = CardId(61528025);
pub const GATE_GUARDIAN: CardId = CardId(25833572);
pub const LARVAE_MOTH: CardId = CardId(87756343);
pub const GREAT_MOTH: CardId = CardId(14141448);
pub const COCOON_OF_EVOLUTION: CardId = CardId(40240595);
pub const PETIT_MOTH: CardId = CardId(58192742);
pub const HARPIE_LADY_SISTERS: CardId = CardId(12206212);
pub const ELEGANT_EGOTIST: CardId = CardId(90219263);
pub const HARPIE_LADY: CardId = CardId(76812113);
pub const WALL_SHADOW: CardId = CardId(63162310);
pub const MAGICAL_LABYRINTH: CardId = CardId(64389297);
pub const LABYRINTH_WALL: CardId = CardId(67284908);

fn script(effects: Vec<Effect>) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects,
    }
}

fn single(effect: Effect) -> CardScript {
    script(vec![effect])
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn own_monsters_except(g: &Game, s: Source) -> Vec<InstanceId> {
    g.monsters_controlled(s.controller)
        .into_iter()
        .filter(|&m| m != s.card)
        .collect()
}

fn on_field_face_up(g: &Game, s: Source) -> bool {
    g.on_field(s.card) && g.instance(s.card).position.is_face_up()
}

fn position_changed_to(s: Source, w: &Window, position: Position) -> bool {
    w.events.iter().any(|e| {
        matches!(e,
        Event::PositionChanged { card, position: p } if *card == s.card && *p == position)
    })
}

/// The controller's own Standby Phase window.
fn own_standby(g: &Game, s: Source, w: &Window) -> bool {
    w.kind == WindowKind::Open
        && g.phase() == Phase::Standby
        && g.turn_player() == s.controller
        && w.events.contains(&Event::PhaseEntered {
            phase: Phase::Standby,
        })
        && on_field_face_up(g, s)
}

fn own_end_phase(g: &Game, s: Source, w: &Window) -> bool {
    w.kind == WindowKind::Open
        && g.phase() == Phase::End
        && g.turn_player() == s.controller
        && w.events
            .contains(&Event::PhaseEntered { phase: Phase::End })
        && on_field_face_up(g, s)
}

fn castle_on_field(g: &Game) -> bool {
    ygo_engine::PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .any(|m| g.stats(m).id == CASTLE_OF_DARK_ILLUSIONS && g.instance(m).position.is_face_up())
}

/// Sanga, Kazejin and Suijin: once while face-up, during damage
/// calculation of an attack against them, the attacker's ATK becomes 0.
fn guardian() -> CardScript {
    single(Effect {
        kind: EffectKind::Quick,
        speed: 2,
        damage_step: true,
        timing: |g, s, w| {
            w.kind == WindowKind::DamageCalculation
                && g.counter(s.card) == 0
                && g.turn_player() != s.controller
                && g.current_attack()
                    .is_some_and(|(_, target)| target == Some(s.card))
        },
        targets: Some(|g, _| singles(g.current_attack().map(|(a, _)| a))),
        ..Effect::activate(2, |g, ctx| {
            g.set_counter(ctx.source, 1);
            let attacker = ctx.targets[0];
            let atk = g.atk(attacker);
            g.modify_during_damage_calculation(ctx.source, attacker, -atk, 0);
            Flow::Done
        })
    })
}

/// Thunder Dragon's search: up to two copies, one pick at a time.
fn thunder_search(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let options: Vec<InstanceId> = g
        .deck(ctx.controller)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).id == THUNDER_DRAGON)
        .collect();
    if options.is_empty() || ctx.targets.len() >= 2 {
        g.shuffle_deck(ctx.controller);
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |g, ctx, picked| {
            g.add_to_hand(picked);
            ctx.targets.push(picked);
            thunder_search(g, ctx)
        },
    }
}

/// Yado Karu: any number of hand cards to the bottom of the Deck.
fn yado_loop(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let hand = g.hand(ctx.controller).to_vec();
    if hand.is_empty() {
        return Flow::Done;
    }
    Flow::Ask {
        options: vec!["Place a card at the bottom of the Deck", "Done"],
        then: |g, ctx, option| {
            if option != 0 {
                return Flow::Done;
            }
            let hand = g.hand(ctx.controller).to_vec();
            Flow::Choose {
                options: hand,
                then: |g, ctx, picked| {
                    g.return_to_deck(picked);
                    yado_loop(g, ctx)
                },
            }
        },
    }
}

/// A Petit Moth the controller has had under Cocoon of Evolution for at
/// least `turns` turns.
fn evolved_moth(g: &Game, s: Source, turns: u32) -> Option<InstanceId> {
    g.monsters_controlled(s.controller).into_iter().find(|&m| {
        g.stats(m).id == PETIT_MOTH
            && g.spells_traps_controlled(s.controller).iter().any(|&e| {
                g.stats(e).id == COCOON_OF_EVOLUTION
                    && g.equip_target(e) == Some(m)
                    && g.instance(e)
                        .turn_entered_field
                        .is_some_and(|t| g.turn() >= t + turns)
            })
    })
}

/// Turns a Petit Moth must have spent under Cocoon of Evolution before
/// this Moth may be summoned: 2 of its controller's turns for Larvae Moth,
/// 4 for Great Moth (counted in game turns).
fn moth_turns(g: &Game, s: Source) -> u32 {
    if g.stats(s.card).id == LARVAE_MOTH {
        4
    } else {
        8
    }
}

/// Larvae Moth and Great Moth: a summon procedure from the hand that
/// Tributes the evolved Petit Moth.
fn moth_script() -> CardScript {
    CardScript {
        tags: vec![Tag::CannotNormalSummon],
        effects: vec![Effect {
            timing: |g, s, _| {
                g.instance(s.card).location == Location::Hand
                    && evolved_moth(g, s, moth_turns(g, s)).is_some()
            },
            cost_choices: Some(|g, s| singles(evolved_moth(g, s, moth_turns(g, s)))),
            cost: Some(|g, ctx| g.tribute(ctx.cost[0])),
            ..Effect::procedure()
        }],
    }
}

pub fn register(r: &mut Registry) {
    r.register(
        CATAPULT_TURTLE,
        single(Effect {
            once_per_turn: true,
            timing: |g, s, _| {
                on_field_face_up(g, s) && !g.monsters_controlled(s.controller).is_empty()
            },
            cost_choices: Some(|g, s| singles(g.monsters_controlled(s.controller))),
            // "half the Tributed monster's ATK on the field": read as it is
            // Tributed and kept for the resolution.
            cost: Some(|g, ctx| {
                ctx.value = g.atk(ctx.cost[0]);
                g.tribute(ctx.cost[0]);
            }),
            ..Effect::ignition(|g, ctx| {
                let half = (ctx.value / 2).max(0) as u32;
                g.take_damage(ctx.controller.opponent(), half);
                Flow::Done
            })
        }),
    );
    r.register(
        CANNON_SOLDIER,
        single(Effect {
            timing: |g, s, _| {
                on_field_face_up(g, s) && !g.monsters_controlled(s.controller).is_empty()
            },
            cost_choices: Some(|g, s| singles(g.monsters_controlled(s.controller))),
            cost: Some(|g, ctx| g.tribute(ctx.cost[0])),
            ..Effect::ignition(|g, ctx| {
                g.take_damage(ctx.controller.opponent(), 500);
                Flow::Done
            })
        }),
    );
    // Calling a coin is a 50/50 either way, so the call itself is not asked.
    r.register(
        TIME_WIZARD,
        single(Effect {
            once_per_turn: true,
            timing: |g, s, _| on_field_face_up(g, s),
            ..Effect::ignition(|g, ctx| {
                if g.coin(ctx.controller) {
                    for m in g.monsters_controlled(ctx.controller.opponent()) {
                        g.destroy(m, ctx.source);
                    }
                } else {
                    let mine = g.monsters_controlled(ctx.controller);
                    let total: i32 = mine
                        .iter()
                        .filter(|&&m| g.instance(m).position.is_face_up())
                        .map(|&m| g.atk(m))
                        .sum();
                    for m in mine {
                        g.destroy(m, ctx.source);
                    }
                    g.take_damage(ctx.controller, (total / 2).max(0) as u32);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        BARREL_DRAGON,
        single(Effect {
            once_per_turn: true,
            timing: |g, s, _| {
                on_field_face_up(g, s) && !g.monsters_controlled(s.controller.opponent()).is_empty()
            },
            targets: Some(|g, s| singles(g.monsters_controlled(s.controller.opponent()))),
            ..Effect::ignition(|g, ctx| {
                let heads = (0..3).filter(|_| g.coin(ctx.controller)).count();
                if heads >= 2 {
                    g.destroy(ctx.targets[0], ctx.source);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        KARATE_MAN,
        script(vec![
            Effect {
                once_per_turn: true,
                timing: |g, s, _| on_field_face_up(g, s) && g.counter(s.card) == 0,
                ..Effect::ignition(|g, ctx| {
                    let base = g.stats(ctx.source).atk;
                    g.modify_until_end_phase(ctx.source, ctx.source, base, 0);
                    g.set_counter(ctx.source, 1);
                    Flow::Done
                })
            },
            Effect {
                timing: |g, s, w| g.counter(s.card) == 1 && own_end_phase(g, s, w),
                ..Effect::trigger(true, |g, ctx| {
                    g.destroy(ctx.source, ctx.source);
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        THE_LITTLE_SWORDSMAN_OF_AILE,
        single(Effect {
            timing: |g, s, _| on_field_face_up(g, s) && !own_monsters_except(g, s).is_empty(),
            cost_choices: Some(|g, s| singles(own_monsters_except(g, s))),
            cost: Some(|g, ctx| g.tribute(ctx.cost[0])),
            ..Effect::ignition(|g, ctx| {
                g.modify_until_end_phase(ctx.source, ctx.source, 700, 0);
                Flow::Done
            })
        }),
    );
    r.register(
        BLAST_JUGGLER,
        single(Effect {
            kind: EffectKind::Quick,
            speed: 1,
            timing: |g, s, w| {
                own_standby(g, s, w)
                    && ygo_engine::PlayerId::BOTH
                        .into_iter()
                        .flat_map(|p| g.monsters_controlled(p))
                        .filter(|&m| {
                            m != s.card && g.instance(m).position.is_face_up() && g.atk(m) <= 1000
                        })
                        .count()
                        >= 2
            },
            targets: Some(|g, s| {
                let small: Vec<InstanceId> = ygo_engine::PlayerId::BOTH
                    .into_iter()
                    .flat_map(|p| g.monsters_controlled(p))
                    .filter(|&m| {
                        m != s.card && g.instance(m).position.is_face_up() && g.atk(m) <= 1000
                    })
                    .collect();
                let mut sets = Vec::new();
                for i in 0..small.len() {
                    for j in i + 1..small.len() {
                        sets.push(vec![small[i], small[j]]);
                    }
                }
                sets
            }),
            cost: Some(|g, ctx| g.tribute(ctx.source)),
            ..Effect::activate(1, |g, ctx| {
                for &m in &ctx.targets.clone() {
                    g.destroy(m, ctx.source);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        THUNDER_DRAGON,
        single(Effect {
            timing: |g, s, _| {
                g.instance(s.card).location == Location::Hand
                    && g.deck(s.controller)
                        .iter()
                        .any(|&c| g.stats(c).id == THUNDER_DRAGON)
            },
            cost: Some(|g, ctx| g.discard(ctx.source)),
            ..Effect::ignition(|g, ctx| {
                ctx.targets.clear();
                thunder_search(g, ctx)
            })
        }),
    );
    r.register(
        JIRAI_GUMO,
        // When this card declares an attack: toss a coin (the call is a
        // 50/50 either way); if wrong, lose half your LP.
        single(Effect {
            timing: |_, s, w| {
                w.kind == WindowKind::AttackDeclared
                    && w.events.iter().any(|e| {
                        matches!(e, Event::AttackDeclared { attacker, .. } if *attacker == s.card)
                    })
            },
            ..Effect::trigger(true, |g, ctx| {
                if !g.coin(ctx.controller) {
                    let half = g.player(ctx.controller).lp / 2;
                    g.pay_lp(ctx.controller, half);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        DARK_ELF,
        single(Effect {
            restrict: Some(|g, s, t| t == s.card && !g.can_pay_lp(s.controller, 1000)),
            on_attack: Some(|g, s, attacker| {
                if attacker == s.card {
                    g.pay_lp(s.controller, 1000);
                }
            }),
            ..Effect::continuous()
        }),
    );
    r.register(
        CRASS_CLOWN,
        single(Effect {
            timing: |g, s, w| {
                position_changed_to(s, w, Position::FaceUpAttack)
                    && !g.monsters_controlled(s.controller.opponent()).is_empty()
            },
            ..Effect::trigger(true, |g, ctx| {
                let options = g.monsters_controlled(ctx.controller.opponent());
                if options.is_empty() {
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |g, _ctx, picked| {
                        g.add_to_hand(picked);
                        Flow::Done
                    },
                }
            })
        }),
    );
    r.register(
        DREAM_CLOWN,
        single(Effect {
            timing: |g, s, w| {
                position_changed_to(s, w, Position::FaceUpDefense)
                    && !g.monsters_controlled(s.controller.opponent()).is_empty()
            },
            ..Effect::trigger(true, |g, ctx| {
                let options = g.monsters_controlled(ctx.controller.opponent());
                if options.is_empty() {
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |g, ctx, picked| {
                        g.destroy(picked, ctx.source);
                        Flow::Done
                    },
                }
            })
        }),
    );
    r.register(
        YADO_KARU,
        single(Effect {
            timing: |g, s, w| {
                position_changed_to(s, w, Position::FaceUpDefense)
                    && !g.hand(s.controller).is_empty()
            },
            misses_timing: true,
            ..Effect::trigger(false, yado_loop)
        }),
    );
    r.register(
        TAINTED_WISDOM,
        single(Effect {
            timing: |_, s, w| position_changed_to(s, w, Position::FaceUpDefense),
            ..Effect::trigger(true, |g, ctx| {
                g.shuffle_deck(ctx.controller);
                Flow::Done
            })
        }),
    );
    r.register(
        PUMPKING_THE_KING_OF_GHOSTS,
        script(vec![
            Effect {
                modify: Some(|g, s, t| {
                    if t != s.card || !castle_on_field(g) {
                        return StatMod::default();
                    }
                    let n = 100 * (1 + i32::from(g.counter(s.card)));
                    StatMod { atk: n, def: n }
                }),
                ..Effect::continuous()
            },
            Effect {
                timing: |g, s, w| {
                    own_standby(g, s, w) && castle_on_field(g) && g.counter(s.card) < 4
                },
                ..Effect::trigger(true, |g, ctx| {
                    let n = g.counter(ctx.source) + 1;
                    g.set_counter(ctx.source, n);
                    Flow::Done
                })
            },
            // The gains reset as soon as Castle of Dark Illusions is gone.
            Effect {
                timing: |g, s, _| {
                    on_field_face_up(g, s) && !castle_on_field(g) && g.counter(s.card) > 0
                },
                ..Effect::trigger(true, |g, ctx| {
                    g.set_counter(ctx.source, 0);
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        MUSHROOM_MAN_2,
        script(vec![
            Effect {
                timing: own_standby,
                ..Effect::trigger(true, |g, ctx| {
                    g.take_damage(ctx.controller, 300);
                    Flow::Done
                })
            },
            Effect {
                timing: |g, s, w| {
                    own_end_phase(g, s, w)
                        && g.can_pay_lp(s.controller, 500)
                        && g.free_monster_zone(s.controller.opponent()).is_some()
                },
                cost: Some(|g, ctx| g.pay_lp(ctx.controller, 500)),
                ..Effect::trigger(false, |g, ctx| {
                    g.change_control(ctx.source, ctx.controller.opponent());
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        DARK_ZEBRA,
        single(Effect {
            timing: |g, s, w| {
                own_standby(g, s, w)
                    && g.monsters_controlled(s.controller).len() == 1
                    && g.instance(s.card).position == Position::FaceUpAttack
            },
            ..Effect::trigger(true, |g, ctx| {
                g.set_position(ctx.source, Position::FaceUpDefense);
                g.forbid_position_change_this_turn(ctx.source);
                Flow::Done
            })
        }),
    );
    for id in [SANGA_OF_THE_THUNDER, KAZEJIN, SUIJIN] {
        r.register(id, guardian());
    }
    // Attacked by a non-Machine: that monster is destroyed in the End Phase
    // of its controller's 2nd turn after the attack (4 turns later).
    r.register(
        STEEL_SCORPION,
        single(Effect {
            on_attack: Some(|g, s, attacker| {
                let attacked = g.current_attack().is_some_and(|(_, t)| t == Some(s.card));
                if attacked && g.stats(attacker).race != "Machine" {
                    let turn = g.turn() + 4;
                    g.destroy_at_end_of_turn(s.card, attacker, turn);
                }
            }),
            ..Effect::continuous()
        }),
    );
    // Attacked by a non-Zombie: that monster cannot attack on its
    // controller's next turn.
    r.register(
        ELECTRIC_LIZARD,
        single(Effect {
            on_attack: Some(|g, s, attacker| {
                let attacked = g.current_attack().is_some_and(|(_, t)| t == Some(s.card));
                if attacked && g.stats(attacker).race != "Zombie" {
                    let turn = g.turn() + 2;
                    g.forbid_attacks_until(s.card, attacker, turn);
                }
            }),
            ..Effect::continuous()
        }),
    );
    // A monster summoned or flipped with the same name as a face-up one.
    r.register(
        KOTODAMA,
        single(Effect {
            timing: |g, s, w| {
                on_field_face_up(g, s)
                    && w.events.iter().any(|e| match e {
                        Event::Summoned { card, position, .. } if position.is_face_up() => {
                            same_name_elsewhere(g, *card)
                        }
                        Event::FlipSummoned { card, .. }
                        | Event::Flipped { card }
                        | Event::SpecialSummoned { card, .. } => same_name_elsewhere(g, *card),
                        _ => false,
                    })
            },
            // The newcomers are captured when the trigger goes on the chain,
            // while the summon window is still open.
            targets: Some(|g, _| {
                let newcomers: Vec<InstanceId> = g
                    .window()
                    .map(|(_, events)| {
                        events
                            .iter()
                            .filter_map(|e| match e {
                                Event::Summoned { card, .. }
                                | Event::FlipSummoned { card, .. }
                                | Event::Flipped { card }
                                | Event::SpecialSummoned { card, .. } => Some(*card),
                                _ => None,
                            })
                            .filter(|&c| g.on_field(c) && same_name_elsewhere(g, c))
                            .collect()
                    })
                    .unwrap_or_default();
                vec![newcomers]
            }),
            ..Effect::trigger(true, |g, ctx| {
                for &c in &ctx.targets.clone() {
                    if g.on_field(c) {
                        g.destroy(c, ctx.source);
                    }
                }
                Flow::Done
            })
        }),
    );
    r.register(
        BOAR_SOLDIER,
        script(vec![
            Effect {
                timing: |_, s, w| {
                    w.kind == WindowKind::SummonSuccess
                        && w.events.iter().any(|e| matches!(e,
                            Event::Summoned { card, position: Position::FaceUpAttack, .. } if *card == s.card))
                },
                ..Effect::trigger(true, |g, ctx| {
                    g.destroy(ctx.source, ctx.source);
                    Flow::Done
                })
            },
            Effect {
                modify: Some(|g, s, t| {
                    if t == s.card && !g.monsters_controlled(s.controller.opponent()).is_empty() {
                        StatMod { atk: -1000, def: 0 }
                    } else {
                        StatMod::default()
                    }
                }),
                ..Effect::continuous()
            },
        ]),
    );
    r.register(
        THE_UNHAPPY_MAIDEN,
        single(Effect {
            // "sent to the Graveyard as a result of battle": destroyed by
            // battle and in the Graveyard (not banished instead).
            timing: |g, s, w| {
                w.events
                    .iter()
                    .any(|e| matches!(e, Event::DestroyedByBattle { card } if *card == s.card))
                    && g.instance(s.card).location == Location::Graveyard
            },
            ..Effect::trigger(true, |g, _ctx| {
                g.end_battle_phase();
                Flow::Done
            })
        }),
    );
    r.register(
        BANISHER_OF_THE_LIGHT,
        CardScript {
            tags: vec![Tag::BanishInsteadOfGraveyard],
            effects: Vec::new(),
        },
    );
    r.register(
        GATE_GUARDIAN,
        CardScript {
            tags: vec![Tag::CannotNormalSummon],
            effects: vec![Effect {
                timing: |g, s, _| {
                    g.instance(s.card).location == Location::Hand
                        && !gate_guardian_tributes(g, s).is_empty()
                },
                cost_choices: Some(gate_guardian_tributes),
                cost: Some(|g, ctx| {
                    for &t in &ctx.cost.clone() {
                        g.tribute(t);
                    }
                }),
                ..Effect::procedure()
            }],
        },
    );
    r.register(LARVAE_MOTH, moth_script());
    r.register(GREAT_MOTH, moth_script());
    r.register(
        COCOON_OF_EVOLUTION,
        script(vec![
            Effect {
                timing: |g, s, _| {
                    g.instance(s.card).location == Location::Hand
                        && g.free_spell_trap_zone(s.controller).is_some()
                        && g.monsters_controlled(s.controller)
                            .iter()
                            .any(|&m| g.stats(m).id == PETIT_MOTH)
                },
                targets: Some(|g, s| {
                    singles(
                        g.monsters_controlled(s.controller)
                            .into_iter()
                            .filter(|&m| g.stats(m).id == PETIT_MOTH),
                    )
                }),
                ..Effect::ignition(|g, ctx| {
                    let petit = ctx.targets[0];
                    if g.on_field(petit) {
                        g.absorb_as_equip(ctx.source, ctx.controller, petit);
                    }
                    Flow::Done
                })
            },
            Effect {
                modify: Some(|g, s, t| {
                    if g.equip_target(s.card) == Some(t) {
                        StatMod {
                            atk: g.stats(s.card).atk - g.stats(t).atk,
                            def: g.stats(s.card).def - g.stats(t).def,
                        }
                    } else {
                        StatMod::default()
                    }
                }),
                ..Effect::continuous()
            },
        ]),
    );
    r.register(
        HARPIE_LADY_SISTERS,
        CardScript {
            tags: vec![Tag::CannotNormalSummon],
            effects: Vec::new(),
        },
    );
    r.register(
        ELEGANT_EGOTIST,
        single(Effect {
            timing: |g, s, _| {
                g.can_special_summon(s.controller)
                    && ygo_engine::PlayerId::BOTH
                        .into_iter()
                        .flat_map(|p| g.monsters_controlled(p))
                        .any(|m| {
                            g.stats(m).id == HARPIE_LADY && g.instance(m).position.is_face_up()
                        })
                    && !harpies_available(g, s.controller).is_empty()
            },
            ..Effect::activate(1, |g, ctx| {
                let options = harpies_available(g, ctx.controller);
                if options.is_empty() {
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |_g, ctx, picked| {
                        ctx.cost = vec![picked];
                        Flow::Ask {
                            options: positions::FACE_UP.to_vec(),
                            then: |g, ctx, option| {
                                let picked = ctx.cost[0];
                                let from_deck = g.instance(picked).location == Location::Deck;
                                g.special_summon_properly(
                                    picked,
                                    ctx.controller,
                                    positions::face_up(option),
                                );
                                if from_deck {
                                    g.shuffle_deck(ctx.controller);
                                }
                                Flow::Done
                            },
                        }
                    },
                }
            })
        }),
    );
    r.register(
        WALL_SHADOW,
        CardScript {
            tags: vec![Tag::CannotNormalSummon],
            effects: Vec::new(),
        },
    );
    r.register(
        MAGICAL_LABYRINTH,
        script(vec![
            Effect {
                timing: |g, s, _| !labyrinth_walls(g, s).is_empty(),
                targets: Some(labyrinth_walls),
                ..Effect::activate(1, crate::continuous::equip_resolve)
            },
            Effect {
                timing: |g, s, _| {
                    g.on_field(s.card)
                        && g.equip_target(s.card).is_some_and(|w| g.on_field(w))
                        && g.can_special_summon(s.controller)
                        && g.deck(s.controller)
                            .iter()
                            .any(|&c| g.stats(c).id == WALL_SHADOW)
                },
                ..Effect::ignition(|g, ctx| {
                    let Some(wall) = g.equip_target(ctx.source) else {
                        return Flow::Done;
                    };
                    g.tribute(wall);
                    let Some(shadow) = g
                        .deck(ctx.controller)
                        .iter()
                        .copied()
                        .find(|&c| g.stats(c).id == WALL_SHADOW)
                    else {
                        return Flow::Done;
                    };
                    ctx.cost = vec![shadow];
                    Flow::Ask {
                        options: positions::FACE_UP.to_vec(),
                        then: |g, ctx, option| {
                            g.special_summon_properly(
                                ctx.cost[0],
                                ctx.controller,
                                positions::face_up(option),
                            );
                            g.shuffle_deck(ctx.controller);
                            Flow::Done
                        },
                    }
                })
            },
        ]),
    );
}

/// One Sanga of the Thunder, one Kazejin and one Suijin its controller
/// controls, every way to pick them.
fn gate_guardian_tributes(g: &Game, s: Source) -> TargetSets {
    let of = |id| -> Vec<InstanceId> {
        g.monsters_controlled(s.controller)
            .into_iter()
            .filter(|&m| g.stats(m).id == id)
            .collect()
    };
    let mut sets = Vec::new();
    for a in of(SANGA_OF_THE_THUNDER) {
        for b in of(KAZEJIN) {
            for c in of(SUIJIN) {
                sets.push(vec![a, b, c]);
            }
        }
    }
    sets
}

fn same_name_elsewhere(g: &Game, card: InstanceId) -> bool {
    let id = g.stats(card).id;
    ygo_engine::PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .any(|m| m != card && g.stats(m).id == id && g.instance(m).position.is_face_up())
}

fn harpies_available(g: &Game, player: ygo_engine::PlayerId) -> Vec<InstanceId> {
    g.hand(player)
        .iter()
        .chain(g.deck(player).iter())
        .copied()
        .filter(|&c| matches!(g.stats(c).id, HARPIE_LADY | HARPIE_LADY_SISTERS))
        .collect()
}

fn labyrinth_walls(g: &Game, _s: Source) -> TargetSets {
    crate::continuous::face_up_monsters(g, |g, m| g.stats(m).id == LABYRINTH_WALL)
}
