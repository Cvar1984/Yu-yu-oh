//! FLIP monsters, and Dragon Capture Jar, which Dragon Piper answers.

use ygo_data::{CardId, CardKind};

use crate::positions;
use ygo_engine::{
    CardScript, Effect, EffectCtx, EffectKind, Event, Flow, Game, InstanceId, Location, Phase,
    PlayerId, Position, Registry, Source, StatMod, TargetSets, Window, WindowKind,
};

pub const REAPER_OF_THE_CARDS: CardId = CardId(33066139);
pub const ARMED_NINJA: CardId = CardId(9076207);
pub const HANE_HANE: CardId = CardId(7089711);
pub const MASK_OF_DARKNESS: CardId = CardId(28933734);
pub const DRAGON_PIPER: CardId = CardId(55763552);
pub const DRAGON_CAPTURE_JAR: CardId = CardId(50045299);
pub const CASTLE_OF_DARK_ILLUSIONS: CardId = CardId(62121);
pub const PRINCESS_OF_TSURUGI: CardId = CardId(51371017);
pub const THE_IMMORTAL_OF_THUNDER: CardId = CardId(84926738);
pub const WEATHER_REPORT: CardId = CardId(72053645);
pub const INVADER_OF_THE_THRONE: CardId = CardId(3056267);
pub const JIGEN_BAKUDAN: CardId = CardId(90020065);
pub const CYBER_JAR: CardId = CardId(34124316);
pub const SPEAR_CRETIN: CardId = CardId(58551308);
const SWORDS_OF_REVEALING_LIGHT: CardId = CardId(72302403);

fn script(effects: Vec<Effect>) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects,
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn all_monsters(g: &Game) -> Vec<InstanceId> {
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .collect()
}

fn all_spells_traps(g: &Game) -> Vec<InstanceId> {
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.spells_traps_controlled(p))
        .collect()
}

/// Spells and Traps a "destroy 1 <kind>" FLIP may pick: face-up ones of
/// that kind, and any Set card, which is revealed on resolution.
fn picks_of_kind(g: &Game, kind: CardKind) -> TargetSets {
    singles(all_spells_traps(g).into_iter().filter(|&c| {
        let face_up = g.instance(c).position.is_face_up();
        !face_up || g.stats(c).kind == kind
    }))
}

fn destroy_if_kind(g: &mut Game, ctx: &mut EffectCtx, kind: CardKind) -> Flow {
    let target = ctx.targets[0];
    if !g.on_field(target) {
        return Flow::Done;
    }
    if !g.instance(target).position.is_face_up() {
        let owner = g.instance(target).controller;
        g.reveal(owner, &[target]);
    }
    if g.stats(target).kind == kind {
        g.destroy(target, ctx.source);
    }
    Flow::Done
}

fn own_standby(g: &Game, s: Source, w: &Window) -> bool {
    w.kind == WindowKind::Open
        && g.phase() == Phase::Standby
        && g.turn_player() == s.controller
        && w.events.contains(&Event::PhaseEntered {
            phase: Phase::Standby,
        })
        && g.on_field(s.card)
        && g.instance(s.card).position.is_face_up()
}

fn is_dragon(g: &Game, m: InstanceId) -> bool {
    g.stats(m).race == "Dragon"
}

fn face_up_dragons(g: &Game) -> Vec<InstanceId> {
    all_monsters(g)
        .into_iter()
        .filter(|&m| is_dragon(g, m) && g.instance(m).position.is_face_up())
        .collect()
}

fn jar_active(g: &Game, s: Source) -> bool {
    g.on_field(s.card) && g.instance(s.card).position.is_face_up()
}

/// Cyber Jar: the revealed cards wait in `ctx.targets`; each Level 4 or
/// lower monster may be Special Summoned by its owner while the summon
/// settings allow, controller first, the picked one parked in `ctx.cost`.
fn cyber_jar_next(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    for player in [ctx.controller, ctx.controller.opponent()] {
        let options: Vec<InstanceId> = ctx
            .targets
            .iter()
            .copied()
            .filter(|&c| {
                let st = g.stats(c);
                g.instance(c).owner == player
                    && g.instance(c).location == Location::Deck
                    && st.is_monster()
                    && st.level <= 4
                    && g.special_summonable(c)
            })
            .collect();
        if !options.is_empty() && g.can_special_summon(player) {
            return Flow::ChooseBy {
                chooser: player,
                options,
                then: |g, ctx, picked| {
                    ctx.cost = vec![picked];
                    Flow::AskBy {
                        chooser: g.instance(picked).owner,
                        options: vec!["Face-up Attack Position", "Face-down Defense Position"],
                        then: |g, ctx, option| {
                            let card = ctx.cost[0];
                            let owner = g.instance(card).owner;
                            let position = if option == 0 {
                                Position::FaceUpAttack
                            } else {
                                Position::FaceDownDefense
                            };
                            g.special_summon(card, owner, position);
                            ctx.targets.retain(|&c| c != card);
                            cyber_jar_next(g, ctx)
                        },
                    }
                },
            };
        }
    }
    for c in ctx.targets.clone() {
        if g.instance(c).location == Location::Deck {
            g.add_to_hand(c);
        }
    }
    Flow::Done
}

/// Spear Cretin: each player picks one monster from their own Graveyard
/// (controller first; `ctx.cost` counts the players asked), then all
/// picks are Special Summoned together.
fn cretin_step(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let stage = ctx.cost.len();
    if stage >= 2 {
        // Summon the picks one at a time; each owner picks the position
        // ("face-up Attack Position or face-down Defense Position"). The
        // card being asked about is the last entry of `ctx.cost`.
        while let Some(c) = ctx.targets.pop() {
            let owner = g.instance(c).owner;
            if g.instance(c).location == Location::Graveyard && g.can_special_summon(owner) {
                ctx.cost.push(c);
                return Flow::AskBy {
                    chooser: owner,
                    options: positions::ATTACK_OR_SET.to_vec(),
                    then: |g, ctx, option| {
                        let c = *ctx.cost.last().expect("the card being summoned");
                        let owner = g.instance(c).owner;
                        g.special_summon(c, owner, positions::attack_or_set(option));
                        cretin_step(g, ctx)
                    },
                };
            }
        }
        return Flow::Done;
    }
    let player = if stage == 0 {
        ctx.controller
    } else {
        ctx.controller.opponent()
    };
    ctx.cost.push(ctx.source);
    let options: Vec<InstanceId> = g
        .graveyard(player)
        .iter()
        .copied()
        .filter(|&c| g.special_summonable(c))
        .collect();
    if options.is_empty() {
        return cretin_step(g, ctx);
    }
    Flow::ChooseBy {
        chooser: player,
        options,
        then: |g, ctx, picked| {
            ctx.targets.push(picked);
            cretin_step(g, ctx)
        },
    }
}

pub fn register(r: &mut Registry) {
    r.register(
        REAPER_OF_THE_CARDS,
        script(vec![Effect {
            targets: Some(|g, _| picks_of_kind(g, CardKind::Trap)),
            ..Effect::flip(|g, ctx| destroy_if_kind(g, ctx, CardKind::Trap))
        }]),
    );
    r.register(
        ARMED_NINJA,
        script(vec![Effect {
            targets: Some(|g, _| picks_of_kind(g, CardKind::Spell)),
            ..Effect::flip(|g, ctx| destroy_if_kind(g, ctx, CardKind::Spell))
        }]),
    );
    r.register(
        HANE_HANE,
        script(vec![Effect {
            targets: Some(|g, _| singles(all_monsters(g))),
            ..Effect::flip(|g, ctx| {
                if g.on_field(ctx.targets[0]) {
                    g.add_to_hand(ctx.targets[0]);
                }
                Flow::Done
            })
        }]),
    );
    r.register(
        MASK_OF_DARKNESS,
        script(vec![Effect {
            targets: Some(|g, s| {
                singles(
                    g.graveyard(s.controller)
                        .iter()
                        .copied()
                        .filter(|&c| g.stats(c).kind == CardKind::Trap),
                )
            }),
            ..Effect::flip(|g, ctx| {
                g.add_to_hand(ctx.targets[0]);
                Flow::Done
            })
        }]),
    );
    r.register(
        DRAGON_PIPER,
        script(vec![Effect::flip(|g, ctx| {
            let jars: Vec<InstanceId> = all_spells_traps(g)
                .into_iter()
                .filter(|&c| {
                    g.stats(c).id == DRAGON_CAPTURE_JAR && g.instance(c).position.is_face_up()
                })
                .collect();
            if jars.is_empty() {
                return Flow::Done;
            }
            for j in jars {
                g.destroy(j, ctx.source);
            }
            for d in face_up_dragons(g) {
                g.set_position(d, Position::FaceUpAttack);
            }
            Flow::Done
        })]),
    );
    r.register(
        DRAGON_CAPTURE_JAR,
        script(vec![
            Effect::activate(2, |g, _ctx| {
                for d in face_up_dragons(g) {
                    g.set_position(d, Position::FaceUpDefense);
                }
                Flow::Done
            }),
            Effect {
                lock_position: Some(|g, s, t| jar_active(g, s) && is_dragon(g, t)),
                restrict: Some(|g, s, t| jar_active(g, s) && is_dragon(g, t)),
                ..Effect::continuous()
            },
            // A Dragon that arrives face-up later goes to Defense Position too.
            Effect {
                timing: |g, s, w| {
                    jar_active(g, s)
                        && w.kind != WindowKind::Chain
                        && face_up_dragons(g)
                            .iter()
                            .any(|&d| g.instance(d).position == Position::FaceUpAttack)
                },
                ..Effect::trigger(true, |g, _ctx| {
                    for d in face_up_dragons(g) {
                        g.set_position(d, Position::FaceUpDefense);
                    }
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        CASTLE_OF_DARK_ILLUSIONS,
        script(vec![
            Effect::flip(|g, ctx| {
                g.set_counter(ctx.source, 1);
                Flow::Done
            }),
            Effect {
                modify: Some(|g, s, t| {
                    let n = i32::from(g.counter(s.card));
                    if n > 0 && g.stats(t).race == "Zombie" {
                        StatMod {
                            atk: 200 * n,
                            def: 200 * n,
                        }
                    } else {
                        StatMod::default()
                    }
                }),
                ..Effect::continuous()
            },
            Effect {
                timing: |g, s, w| own_standby(g, s, w) && (1..5).contains(&g.counter(s.card)),
                ..Effect::trigger(true, |g, ctx| {
                    let n = g.counter(ctx.source) + 1;
                    g.set_counter(ctx.source, n);
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        PRINCESS_OF_TSURUGI,
        script(vec![Effect::flip(|g, ctx| {
            let n = g.spells_traps_controlled(ctx.controller.opponent()).len() as u32;
            if n > 0 {
                g.take_damage(ctx.controller.opponent(), 500 * n);
            }
            Flow::Done
        })]),
    );
    r.register(
        THE_IMMORTAL_OF_THUNDER,
        script(vec![
            Effect::flip(|g, ctx| {
                g.gain_lp(ctx.controller, 3000);
                Flow::Done
            }),
            Effect {
                timing: |_, s, w| {
                    w.events.iter().any(|e| matches!(e,
                        Event::SentToGraveyard { card, from: Location::MonsterZone(_) } if *card == s.card))
                },
                ..Effect::trigger(true, |g, ctx| {
                    g.pay_lp(ctx.controller, 5000);
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        WEATHER_REPORT,
        script(vec![Effect::flip(|g, ctx| {
            let swords: Vec<InstanceId> = g
                .spells_traps_controlled(ctx.controller.opponent())
                .into_iter()
                .filter(|&c| {
                    g.stats(c).id == SWORDS_OF_REVEALING_LIGHT
                        && g.instance(c).position.is_face_up()
                })
                .collect();
            if swords.is_empty() {
                return Flow::Done;
            }
            for c in swords {
                g.destroy(c, ctx.source);
            }
            let turn = if g.turn_player() == ctx.controller {
                g.turn()
            } else {
                g.turn() + 1
            };
            g.grant_extra_battle_phase(ctx.controller, turn);
            Flow::Done
        })]),
    );
    r.register(
        INVADER_OF_THE_THRONE,
        script(vec![Effect {
            timing: |g, s, w| {
                ygo_engine::flipped_this_window(g, s, w) && !matches!(g.phase(), Phase::Battle(_))
            },
            targets: Some(|g, s| singles(g.monsters_controlled(s.controller.opponent()))),
            ..Effect::flip(|g, ctx| {
                if g.on_field(ctx.source) && g.on_field(ctx.targets[0]) {
                    g.swap_control(ctx.source, ctx.targets[0]);
                }
                Flow::Done
            })
        }]),
    );
    r.register(
        JIGEN_BAKUDAN,
        script(vec![
            Effect::flip(|g, ctx| {
                g.set_counter(ctx.source, 1);
                Flow::Done
            }),
            Effect {
                kind: EffectKind::Quick,
                speed: 1,
                timing: |g, s, w| own_standby(g, s, w) && g.counter(s.card) == 1,
                cost: Some(|g, ctx| g.tribute(ctx.source)),
                ..Effect::activate(1, |g, ctx| {
                    let mine = g.monsters_controlled(ctx.controller);
                    let total: i32 = mine.iter().map(|&m| g.atk(m)).sum();
                    for m in mine {
                        g.destroy(m, ctx.source);
                    }
                    g.take_damage(ctx.controller.opponent(), (total / 2).max(0) as u32);
                    Flow::Done
                })
            },
        ]),
    );
    r.register(
        CYBER_JAR,
        script(vec![Effect::flip(|g, ctx| {
            for m in all_monsters(g) {
                g.destroy(m, ctx.source);
            }
            let mut revealed = Vec::new();
            for p in [ctx.controller, ctx.controller.opponent()] {
                let top = g.top_of_deck(p, 5);
                g.reveal(p, &top);
                revealed.extend(top);
            }
            ctx.targets = revealed;
            ctx.cost.clear();
            cyber_jar_next(g, ctx)
        })]),
    );
    r.register(
        SPEAR_CRETIN,
        script(vec![
            // Remembers a flip that happened while it stayed on the field.
            Effect {
                timing: |g, s, w| ygo_engine::flipped_this_window(g, s, w) && g.on_field(s.card),
                ..Effect::flip(|g, ctx| {
                    g.set_counter(ctx.source, 1);
                    Flow::Done
                })
            },
            Effect {
                timing: |g, s, w| {
                    let to_graveyard = w.events.iter().any(|e| matches!(e,
                        Event::SentToGraveyard { card, from: Location::MonsterZone(_) } if *card == s.card));
                    to_graveyard && (g.counter(s.card) == 1 || ygo_engine::flipped_this_window(g, s, w))
                },
                ..Effect::trigger(true, |g, ctx| {
                    ctx.targets.clear();
                    ctx.cost.clear();
                    cretin_step(g, ctx)
                })
            },
        ]),
    );
}
