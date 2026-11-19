//! One-shot Normal Spells, Quick-Play Spells and Traps: burn, Life
//! Points, position changes, mass removal, discards and banishing.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, EffectCtx, Flow, Game, InstanceId, Location, Phase, PlayerId, Position,
    Registry, Source, TargetSets, WindowKind,
};

pub const SPARKS: CardId = CardId(76103675);
pub const HINOTAMA: CardId = CardId(46130346);
pub const FINAL_FLAME: CardId = CardId(73134081);
pub const TREMENDOUS_FIRE: CardId = CardId(46918794);
pub const RED_MEDICINE: CardId = CardId(38199696);
pub const GOBLINS_SECRET_REMEDY: CardId = CardId(11868825);
pub const REMOVE_TRAP: CardId = CardId(51482758);
pub const STOP_DEFENSE: CardId = CardId(63102017);
pub const BLOCK_ATTACK: CardId = CardId(25880422);
pub const HEAVY_STORM: CardId = CardId(19613556);
pub const GIANT_TRUNADE: CardId = CardId(42703248);
pub const ETERNAL_REST: CardId = CardId(95051344);
pub const TWO_PRONGED_ATTACK: CardId = CardId(83887306);
pub const SNAKE_FANG: CardId = CardId(596051);
pub const RUSH_RECKLESSLY: CardId = CardId(70046172);
pub const THE_RELIABLE_GUARDIAN: CardId = CardId(16430187);
pub const SHIELD_AND_SWORD: CardId = CardId(52097679);
pub const CURSE_OF_FIEND: CardId = CardId(12470447);
pub const DARKNESS_APPROACHES: CardId = CardId(80168720);
pub const THE_CHEERFUL_COFFIN: CardId = CardId(41142615);
pub const SHARE_THE_PAIN: CardId = CardId(56830749);
pub const FINAL_DESTINY: CardId = CardId(18591904);
pub const SOUL_RELEASE: CardId = CardId(5758500);
pub const GRAVEDIGGER_GHOUL: CardId = CardId(82542267);
pub const TAILOR_OF_THE_FICKLE: CardId = CardId(43641473);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

/// Burn: damage to the opponent, read from the card's own row.
const BURN: [(CardId, u32, u32); 4] = [
    (SPARKS, 200, 0),
    (HINOTAMA, 500, 0),
    (FINAL_FLAME, 600, 0),
    (TREMENDOUS_FIRE, 1000, 500),
];

const HEAL: [(CardId, u32); 2] = [(RED_MEDICINE, 500), (GOBLINS_SECRET_REMEDY, 600)];

fn burn_resolve(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let id = g.stats(ctx.source).id;
    let (_, to_opponent, to_self) = BURN.iter().find(|(i, _, _)| *i == id).expect("a burn row");
    g.take_damage(ctx.controller.opponent(), *to_opponent);
    if *to_self > 0 {
        g.take_damage(ctx.controller, *to_self);
    }
    Flow::Done
}

fn heal_resolve(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let id = g.stats(ctx.source).id;
    let (_, amount) = HEAL.iter().find(|(i, _)| *i == id).expect("a heal row");
    g.gain_lp(ctx.controller, *amount);
    Flow::Done
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

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

/// All `k`-subsets of `items`, in order.
fn subsets(items: &[InstanceId], k: usize) -> TargetSets {
    fn go(
        items: &[InstanceId],
        k: usize,
        start: usize,
        cur: &mut Vec<InstanceId>,
        out: &mut TargetSets,
    ) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..items.len() {
            cur.push(items[i]);
            go(items, k, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(items, k, 0, &mut Vec::new(), &mut out);
    out
}

fn face_up_traps(g: &Game, _s: Source) -> TargetSets {
    singles(
        all_spells_traps(g)
            .into_iter()
            .filter(|&c| g.stats(c).kind == CardKind::Trap && g.instance(c).position.is_face_up()),
    )
}

fn opponent_defenders(g: &Game, s: Source) -> TargetSets {
    singles(
        g.monsters_controlled(s.controller.opponent())
            .into_iter()
            .filter(|&m| {
                matches!(
                    g.instance(m).position,
                    Position::FaceUpDefense | Position::FaceDownDefense
                ) && g.spell_targetable(m)
            }),
    )
}

fn opponent_attackers(g: &Game, s: Source) -> TargetSets {
    singles(
        g.monsters_controlled(s.controller.opponent())
            .into_iter()
            .filter(|&m| g.instance(m).position == Position::FaceUpAttack && g.spell_targetable(m)),
    )
}

fn equipped_monsters(g: &Game) -> Vec<InstanceId> {
    all_monsters(g)
        .into_iter()
        .filter(|&m| {
            all_spells_traps(g)
                .iter()
                .any(|&e| g.equip_target(e) == Some(m))
        })
        .collect()
}

fn two_pronged_targets(g: &Game, s: Source) -> TargetSets {
    let own = g.monsters_controlled(s.controller);
    let opp = g.monsters_controlled(s.controller.opponent());
    let mut sets = Vec::new();
    for pair in subsets(&own, 2) {
        for &o in &opp {
            let mut set = pair.clone();
            set.push(o);
            sets.push(set);
        }
    }
    sets
}

fn face_up_monsters(g: &Game) -> Vec<InstanceId> {
    all_monsters(g)
        .into_iter()
        .filter(|&m| g.instance(m).position.is_face_up())
        .collect()
}

fn hand_without_self(g: &Game, s: Source) -> Vec<InstanceId> {
    g.hand(s.controller)
        .iter()
        .copied()
        .filter(|&c| c != s.card)
        .collect()
}

fn cheerful_coffin_targets(g: &Game, s: Source) -> TargetSets {
    let monsters: Vec<InstanceId> = hand_without_self(g, s)
        .into_iter()
        .filter(|&c| g.stats(c).is_monster())
        .collect();
    (1..=3.min(monsters.len()))
        .flat_map(|k| subsets(&monsters, k))
        .collect()
}

/// Banish picks one at a time, up to `max`, through Choose and Ask.
fn banish_loop(
    g: &mut Game,
    ctx: &mut EffectCtx,
    max: usize,
    from: fn(&Game, PlayerId) -> Vec<InstanceId>,
) -> Flow {
    let picked = ctx.targets.len();
    let options: Vec<InstanceId> = from(g, ctx.controller)
        .into_iter()
        .filter(|c| !ctx.targets.contains(c))
        .collect();
    if picked >= max || options.is_empty() {
        for &c in &ctx.targets {
            g.banish(c);
        }
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |g, ctx, picked| {
            ctx.targets.push(picked);
            let max = if g.stats(ctx.source).id == SOUL_RELEASE {
                5
            } else {
                2
            };
            let from = if g.stats(ctx.source).id == SOUL_RELEASE {
                both_graveyards
            } else {
                opponent_monsters_in_graveyard
            };
            if ctx.targets.len() >= max
                || from(g, ctx.controller)
                    .iter()
                    .all(|c| ctx.targets.contains(c))
            {
                return banish_loop(g, ctx, max, from);
            }
            Flow::Ask {
                options: vec!["Banish another", "Done"],
                then: |g, ctx, option| {
                    let max = if g.stats(ctx.source).id == SOUL_RELEASE {
                        5
                    } else {
                        2
                    };
                    let from = if g.stats(ctx.source).id == SOUL_RELEASE {
                        both_graveyards
                    } else {
                        opponent_monsters_in_graveyard
                    };
                    if option == 0 {
                        banish_loop(g, ctx, max, from)
                    } else {
                        banish_loop(g, ctx, 0, from)
                    }
                },
            }
        },
    }
}

fn both_graveyards(g: &Game, _: PlayerId) -> Vec<InstanceId> {
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.graveyard(p).to_vec())
        .collect()
}

fn opponent_monsters_in_graveyard(g: &Game, controller: PlayerId) -> Vec<InstanceId> {
    g.graveyard(controller.opponent())
        .iter()
        .copied()
        .filter(|&c| g.stats(c).is_monster())
        .collect()
}

/// Tailor of the Fickle: (Equip, new target) pairs the Equip could have
/// chosen itself.
fn tailor_targets(g: &Game, _s: Source) -> TargetSets {
    let mut sets = Vec::new();
    for e in all_spells_traps(g) {
        let Some(current) = g.equip_target(e) else {
            continue;
        };
        let Some(targets) = g
            .script(e)
            .and_then(|s| s.effects.first())
            .and_then(|eff| eff.targets)
        else {
            continue;
        };
        let source = Source {
            card: e,
            controller: g.instance(e).controller,
        };
        for set in targets(g, source) {
            if set.len() == 1 && set[0] != current {
                sets.push(vec![e, set[0]]);
            }
        }
    }
    sets
}

pub fn register(r: &mut Registry) {
    for (id, _, _) in BURN {
        r.register(id, single(Effect::activate(1, burn_resolve)));
    }
    for (id, _) in HEAL {
        r.register(id, single(Effect::activate(1, heal_resolve)));
    }
    r.register(
        REMOVE_TRAP,
        single(Effect {
            timing: |g, s, _| !face_up_traps(g, s).is_empty(),
            targets: Some(face_up_traps),
            ..Effect::activate(1, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    r.register(
        STOP_DEFENSE,
        single(Effect {
            timing: |g, s, _| !opponent_defenders(g, s).is_empty(),
            targets: Some(opponent_defenders),
            ..Effect::activate(1, |g, ctx| {
                g.set_position(ctx.targets[0], Position::FaceUpAttack);
                Flow::Done
            })
        }),
    );
    r.register(
        BLOCK_ATTACK,
        single(Effect {
            timing: |g, s, _| !opponent_attackers(g, s).is_empty(),
            targets: Some(opponent_attackers),
            ..Effect::activate(1, |g, ctx| {
                g.set_position(ctx.targets[0], Position::FaceUpDefense);
                Flow::Done
            })
        }),
    );
    r.register(
        HEAVY_STORM,
        single(Effect {
            timing: |g, s, _| all_spells_traps(g).iter().any(|&c| c != s.card),
            ..Effect::activate(1, |g, ctx| {
                for c in all_spells_traps(g) {
                    if c != ctx.source {
                        g.destroy(c, ctx.source);
                    }
                }
                Flow::Done
            })
        }),
    );
    r.register(
        GIANT_TRUNADE,
        single(Effect {
            timing: |g, s, _| all_spells_traps(g).iter().any(|&c| c != s.card),
            ..Effect::activate(1, |g, ctx| {
                for c in all_spells_traps(g) {
                    if c != ctx.source {
                        g.add_to_hand(c);
                    }
                }
                Flow::Done
            })
        }),
    );
    r.register(
        ETERNAL_REST,
        single(Effect {
            timing: |g, _, _| !equipped_monsters(g).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                for m in equipped_monsters(g) {
                    g.destroy(m, ctx.source);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        TWO_PRONGED_ATTACK,
        single(Effect {
            timing: |g, s, _| !two_pronged_targets(g, s).is_empty(),
            targets: Some(two_pronged_targets),
            ..Effect::activate(2, |g, ctx| {
                for &m in &ctx.targets {
                    g.destroy(m, ctx.source);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        SNAKE_FANG,
        single(Effect {
            timing: |g, _, _| !face_up_monsters(g).is_empty(),
            targets: Some(|g, _| singles(face_up_monsters(g))),
            ..Effect::activate(2, |g, ctx| {
                if g.on_field(ctx.targets[0]) {
                    g.modify_until_end_phase(ctx.source, ctx.targets[0], 0, -500);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        RUSH_RECKLESSLY,
        single(Effect {
            timing: |g, _, _| !face_up_monsters(g).is_empty(),
            targets: Some(|g, _| singles(face_up_monsters(g))),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                if g.on_field(ctx.targets[0]) {
                    g.modify_until_end_phase(ctx.source, ctx.targets[0], 700, 0);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        THE_RELIABLE_GUARDIAN,
        single(Effect {
            timing: |g, _, _| !face_up_monsters(g).is_empty(),
            targets: Some(|g, _| singles(face_up_monsters(g))),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                if g.on_field(ctx.targets[0]) {
                    g.modify_until_end_phase(ctx.source, ctx.targets[0], 0, 700);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        SHIELD_AND_SWORD,
        single(Effect {
            timing: |g, _, _| !face_up_monsters(g).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                for m in face_up_monsters(g) {
                    let (atk, def) = (g.stats(m).atk, g.stats(m).def);
                    g.modify_until_end_phase(ctx.source, m, def - atk, atk - def);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        CURSE_OF_FIEND,
        single(Effect {
            in_standby: true,
            timing: |g, _, w| {
                w.kind == WindowKind::Open
                    && g.phase() == Phase::Standby
                    && !face_up_monsters(g).is_empty()
            },
            ..Effect::activate(1, |g, _ctx| {
                for m in face_up_monsters(g) {
                    let next = match g.instance(m).position {
                        Position::FaceUpAttack => Position::FaceUpDefense,
                        Position::FaceUpDefense => Position::FaceUpAttack,
                        other => other,
                    };
                    g.set_position(m, next);
                    g.forbid_position_change_this_turn(m);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        DARKNESS_APPROACHES,
        single(Effect {
            timing: |g, s, _| hand_without_self(g, s).len() >= 2 && !face_up_monsters(g).is_empty(),
            cost_choices: Some(|g, s| subsets(&hand_without_self(g, s), 2)),
            cost: Some(|g, ctx| {
                for &c in &ctx.cost.clone() {
                    g.discard(c);
                }
            }),
            targets: Some(|g, _| singles(face_up_monsters(g))),
            ..Effect::activate(1, |g, ctx| {
                g.set_position(ctx.targets[0], Position::FaceDownDefense);
                Flow::Done
            })
        }),
    );
    r.register(
        THE_CHEERFUL_COFFIN,
        single(Effect {
            timing: |g, s, _| !cheerful_coffin_targets(g, s).is_empty(),
            targets: Some(cheerful_coffin_targets),
            ..Effect::activate(1, |g, ctx| {
                for &c in &ctx.targets.clone() {
                    g.discard(c);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        SHARE_THE_PAIN,
        single(Effect {
            timing: |g, s, _| {
                !g.monsters_controlled(s.controller).is_empty()
                    && !g.monsters_controlled(s.controller.opponent()).is_empty()
            },
            cost_choices: Some(|g, s| singles(g.monsters_controlled(s.controller))),
            cost: Some(|g, ctx| g.tribute(ctx.cost[0])),
            ..Effect::activate(1, |g, ctx| {
                let options = g.monsters_controlled(ctx.controller.opponent());
                if options.is_empty() {
                    return Flow::Done;
                }
                Flow::ChooseBy {
                    chooser: ctx.controller.opponent(),
                    options,
                    then: |g, _ctx, picked| {
                        g.tribute(picked);
                        Flow::Done
                    },
                }
            })
        }),
    );
    r.register(
        FINAL_DESTINY,
        single(Effect {
            timing: |g, s, _| hand_without_self(g, s).len() >= 5,
            cost_choices: Some(|g, s| subsets(&hand_without_self(g, s), 5)),
            cost: Some(|g, ctx| {
                for &c in &ctx.cost.clone() {
                    g.discard(c);
                }
            }),
            ..Effect::activate(1, |g, ctx| {
                let cards: Vec<InstanceId> = all_monsters(g)
                    .into_iter()
                    .chain(all_spells_traps(g))
                    .filter(|&c| c != ctx.source)
                    .collect();
                for c in cards {
                    g.destroy(c, ctx.source);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        SOUL_RELEASE,
        single(Effect {
            timing: |g, s, _| !both_graveyards(g, s.controller).is_empty(),
            ..Effect::activate(1, |g, ctx| banish_loop(g, ctx, 5, both_graveyards))
        }),
    );
    r.register(
        GRAVEDIGGER_GHOUL,
        single(Effect {
            timing: |g, s, _| !opponent_monsters_in_graveyard(g, s.controller).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                banish_loop(g, ctx, 2, opponent_monsters_in_graveyard)
            })
        }),
    );
    r.register(
        TAILOR_OF_THE_FICKLE,
        single(Effect {
            timing: |g, s, _| !tailor_targets(g, s).is_empty(),
            targets: Some(tailor_targets),
            ..Effect::activate(2, |g, ctx| {
                let (equip, to) = (ctx.targets[0], ctx.targets[1]);
                if g.instance(equip).location.on_field() {
                    g.equip(equip, to);
                }
                Flow::Done
            })
        }),
    );
}

#[allow(dead_code)]
fn _unused(_: Location) {}
