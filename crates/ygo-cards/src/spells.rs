//! Spell Card scripts.

use ygo_data::CardId;
use ygo_engine::{
    CardScript, Effect, EffectCtx, Flow, Game, InstanceId, Lingering, LingeringKind, Position,
    Registry, Source, TargetSets, Window,
};

pub const POT_OF_GREED: CardId = CardId(55144522);
pub const RAIGEKI: CardId = CardId(12580477);
pub const DARK_HOLE: CardId = CardId(53129443);
pub const FISSURE: CardId = CardId(66788016);
pub const UPSTART_GOBLIN: CardId = CardId(70368879);
pub const MONSTER_REBORN: CardId = CardId(83764718);
pub const CHANGE_OF_HEART: CardId = CardId(4031928);
pub const TRIBUTE_TO_THE_DOOMED: CardId = CardId(79759861);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

pub fn register(r: &mut Registry) {
    r.register(
        POT_OF_GREED,
        single(Effect {
            timing: |g, s, _| g.deck_size(s.controller) >= 2,
            ..Effect::activate(1, |g, ctx| {
                g.draw(ctx.controller, 2);
                Flow::Done
            })
        }),
    );
    r.register(
        RAIGEKI,
        single(Effect {
            timing: |g, s, _| !g.monsters_controlled(s.controller.opponent()).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                for m in g.monsters_controlled(ctx.controller.opponent()) {
                    g.destroy(m, ctx.source);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        DARK_HOLE,
        single(Effect {
            timing: |g, s, _| {
                !g.monsters_controlled(s.controller).is_empty()
                    || !g.monsters_controlled(s.controller.opponent()).is_empty()
            },
            ..Effect::activate(1, |g, ctx| {
                for p in [ctx.controller, ctx.controller.opponent()] {
                    for m in g.monsters_controlled(p) {
                        g.destroy(m, ctx.source);
                    }
                }
                Flow::Done
            })
        }),
    );
    r.register(
        FISSURE,
        single(Effect {
            timing: |g, s, _| !fissure_candidates(g, s).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                let lowest = fissure_candidates(
                    g,
                    Source {
                        card: ctx.source,
                        controller: ctx.controller,
                    },
                );
                match lowest.as_slice() {
                    [] => Flow::Done,
                    [one] => {
                        g.destroy(*one, ctx.source);
                        Flow::Done
                    }
                    _ => Flow::Choose {
                        options: lowest,
                        then: |g, ctx, picked| {
                            g.destroy(picked, ctx.source);
                            Flow::Done
                        },
                    },
                }
            })
        }),
    );
    r.register(
        UPSTART_GOBLIN,
        single(Effect {
            timing: |g, s, _| g.deck_size(s.controller) >= 1,
            ..Effect::activate(1, |g, ctx| {
                g.draw(ctx.controller, 1);
                g.gain_lp(ctx.controller.opponent(), 1000);
                Flow::Done
            })
        }),
    );
    r.register(
        MONSTER_REBORN,
        single(Effect {
            timing: |g, s, _| g.can_special_summon(s.controller),
            targets: Some(|g, s| {
                let mut sets: TargetSets = Vec::new();
                for p in [s.controller, s.controller.opponent()] {
                    for &c in g.graveyard(p) {
                        if g.stats(c).is_monster() {
                            sets.push(vec![c]);
                        }
                    }
                }
                sets
            }),
            ..Effect::activate(1, |g, ctx| {
                let target = ctx.targets[0];
                if g.instance(target).location == ygo_engine::Location::Graveyard {
                    g.special_summon(target, ctx.controller, Position::FaceUpAttack);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        CHANGE_OF_HEART,
        single(Effect {
            timing: |g, s, _| g.free_monster_zone(s.controller).is_some(),
            targets: Some(|g, s| {
                g.monsters_controlled(s.controller.opponent())
                    .into_iter()
                    .filter(|&m| g.spell_targetable(m))
                    .map(|m| vec![m])
                    .collect()
            }),
            ..Effect::activate(1, |g, ctx| {
                let target = ctx.targets[0];
                let from = g.instance(target).controller;
                if from != ctx.controller && g.change_control(target, ctx.controller) {
                    g.add_lingering(Lingering {
                        source: ctx.source,
                        kind: LingeringKind::ReturnControl {
                            card: target,
                            to: from,
                        },
                        expires_turn: g.turn(),
                    });
                }
                Flow::Done
            })
        }),
    );
    r.register(
        TRIBUTE_TO_THE_DOOMED,
        single(Effect {
            timing: |g, s, _| {
                g.hand(s.controller).iter().any(|&c| c != s.card)
                    && !doomed_targets(g, s).is_empty()
            },
            cost_choices: Some(|g, s| {
                g.hand(s.controller)
                    .iter()
                    .copied()
                    .filter(|&c| c != s.card)
                    .map(|c| vec![c])
                    .collect()
            }),
            cost: Some(|g, ctx| g.discard(ctx.cost[0])),
            targets: Some(doomed_targets),
            ..Effect::activate(1, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
}

/// Fissure: the opponent's face-up monsters with the lowest ATK.
fn fissure_candidates(g: &Game, s: Source) -> Vec<InstanceId> {
    let face_up: Vec<InstanceId> = g
        .monsters_controlled(s.controller.opponent())
        .into_iter()
        .filter(|&m| g.instance(m).position.is_face_up())
        .collect();
    let Some(min) = face_up.iter().map(|&m| g.stats(m).atk).min() else {
        return Vec::new();
    };
    face_up
        .into_iter()
        .filter(|&m| g.stats(m).atk == min)
        .collect()
}

/// Tribute to the Doomed: any monster on the field a Spell may target.
fn doomed_targets(g: &Game, s: Source) -> TargetSets {
    [s.controller, s.controller.opponent()]
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .filter(|&m| g.spell_targetable(m))
        .map(|m| vec![m])
        .collect()
}

// Keep the unused-import lint honest for `EffectCtx` and `Window`, which
// script signatures name only through inference.
#[allow(dead_code)]
fn _signatures(_: &EffectCtx, _: &Window) {}
