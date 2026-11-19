//! Trap Card scripts, plus the Quick-Play Spell that lives among them.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, Event, Game, InstanceId, Position, Registry, Source, TargetSets, Window,
    WindowKind,
};

pub const MIRROR_FORCE: CardId = CardId(44095762);
pub const TRAP_HOLE: CardId = CardId(4206964);
pub const SOLEMN_JUDGMENT: CardId = CardId(41420027);
pub const MAGIC_JAMMER: CardId = CardId(77414722);
pub const SEVEN_TOOLS_OF_THE_BANDIT: CardId = CardId(3819470);
pub const MYSTICAL_SPACE_TYPHOON: CardId = CardId(5318639);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

/// The monster an opponent just Normal or Flip Summoned face-up with
/// 1000 or more ATK, if the window holds such a summon.
fn trap_hole_target(g: &Game, s: Source, w: &Window) -> Option<InstanceId> {
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
            if *player != s.controller && g.stats(*card).atk >= 1000 && g.on_field(*card) =>
        {
            Some(*card)
        }
        _ => None,
    })
}

/// What Solemn Judgment may negate right now: the pending summon, or the
/// last Spell or Trap activation on the chain.
fn solemn_target(g: &Game, _s: Source, w: &Window) -> Option<InstanceId> {
    if w.kind == WindowKind::SummonDeclared {
        return g.pending_summon().map(|ps| ps.card);
    }
    g.last_link()
        .filter(|l| !g.stats(l.card).is_monster())
        .map(|l| l.card)
}

fn last_link_of_kind(g: &Game, kind: CardKind) -> Option<InstanceId> {
    g.last_link()
        .filter(|l| g.stats(l.card).kind == kind && !l.negated)
        .map(|l| l.card)
}

fn one(target: Option<InstanceId>) -> TargetSets {
    target.map(|t| vec![t]).into_iter().collect()
}

pub fn register(r: &mut Registry) {
    r.register(
        MIRROR_FORCE,
        single(Effect {
            timing: |g, s, w| {
                w.kind == WindowKind::AttackDeclared
                    && w.events.iter().any(|e| {
                        matches!(e,
                        Event::AttackDeclared { attacker, .. }
                            if g.instance(*attacker).controller != s.controller)
                    })
            },
            ..Effect::activate(2, |g, ctx| {
                for m in g.monsters_controlled(ctx.controller.opponent()) {
                    if g.instance(m).position == Position::FaceUpAttack {
                        g.destroy(m, ctx.source);
                    }
                }
                ygo_engine::Flow::Done
            })
        }),
    );
    r.register(
        TRAP_HOLE,
        single(Effect {
            timing: |g, s, w| trap_hole_target(g, s, w).is_some(),
            targets: Some(|g, s| {
                let w = g.window().map(|(kind, events)| Window { kind, events });
                one(w.and_then(|w| trap_hole_target(g, s, &w)))
            }),
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                ygo_engine::Flow::Done
            })
        }),
    );
    r.register(
        SOLEMN_JUDGMENT,
        single(Effect {
            timing: |g, s, w| solemn_target(g, s, w).is_some(),
            targets: Some(|g, s| {
                let w = g.window().map(|(kind, events)| Window { kind, events });
                one(w.and_then(|w| solemn_target(g, s, &w)))
            }),
            cost: Some(|g, ctx| {
                let half = g.player(ctx.controller).lp / 2;
                g.pay_lp(ctx.controller, half);
            }),
            ..Effect::activate(3, |g, ctx| {
                let target = ctx.targets[0];
                if g.pending_summon().is_some_and(|ps| ps.card == target) {
                    g.negate_summon(ctx.source);
                } else {
                    g.negate_activation(target, ctx.source);
                }
                ygo_engine::Flow::Done
            })
        }),
    );
    r.register(
        MAGIC_JAMMER,
        single(Effect {
            timing: |g, s, _| {
                last_link_of_kind(g, CardKind::Spell).is_some()
                    && g.hand(s.controller).iter().any(|&c| c != s.card)
            },
            targets: Some(|g, _| one(last_link_of_kind(g, CardKind::Spell))),
            cost_choices: Some(|g, s| {
                g.hand(s.controller)
                    .iter()
                    .copied()
                    .filter(|&c| c != s.card)
                    .map(|c| vec![c])
                    .collect()
            }),
            cost: Some(|g, ctx| g.discard(ctx.cost[0])),
            ..Effect::activate(3, |g, ctx| {
                g.negate_activation(ctx.targets[0], ctx.source);
                ygo_engine::Flow::Done
            })
        }),
    );
    r.register(
        SEVEN_TOOLS_OF_THE_BANDIT,
        single(Effect {
            timing: |g, s, _| {
                last_link_of_kind(g, CardKind::Trap).is_some() && g.can_pay_lp(s.controller, 1000)
            },
            targets: Some(|g, _| one(last_link_of_kind(g, CardKind::Trap))),
            cost: Some(|g, ctx| g.pay_lp(ctx.controller, 1000)),
            ..Effect::activate(3, |g, ctx| {
                g.negate_activation(ctx.targets[0], ctx.source);
                ygo_engine::Flow::Done
            })
        }),
    );
    r.register(
        MYSTICAL_SPACE_TYPHOON,
        single(Effect {
            timing: |g, s, _| !mst_targets(g, s).is_empty(),
            targets: Some(mst_targets),
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                ygo_engine::Flow::Done
            })
        }),
    );
}

/// Any Spell or Trap on the field other than itself.
fn mst_targets(g: &Game, s: Source) -> TargetSets {
    [s.controller, s.controller.opponent()]
        .into_iter()
        .flat_map(|p| {
            g.player(p)
                .spell_trap_zones
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>()
        })
        .filter(|&c| c != s.card)
        .map(|c| vec![c])
        .collect()
}
