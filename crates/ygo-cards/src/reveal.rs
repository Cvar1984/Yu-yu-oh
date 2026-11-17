//! Cards that look at hidden cards or let the other player choose.

use ygo_data::{CardId, CardKind};
use ygo_engine::{CardScript, Effect, EffectCtx, Flow, Game, InstanceId, Registry, Tag};

pub const CONFISCATION: CardId = CardId(17375316);
pub const THE_FORCEFUL_SENTRY: CardId = CardId(42829885);
pub const DELINQUENT_DUO: CardId = CardId(44763025);
pub const PAINFUL_CHOICE: CardId = CardId(74191942);
pub const CEREMONIAL_BELL: CardId = CardId(20228463);
pub const HIROS_SHADOW_SCOUT: CardId = CardId(81863068);
pub const BIG_EYE: CardId = CardId(16768387);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

/// Painful Choice: the controller picks five from the Deck one at a time
/// (`ctx.targets` accumulates them), then the opponent picks one of them.
fn painful_pick(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    if ctx.targets.len() < 5 {
        let options: Vec<InstanceId> = g
            .deck(ctx.controller)
            .iter()
            .copied()
            .filter(|c| !ctx.targets.contains(c))
            .collect();
        if options.is_empty() {
            return painful_resolve(g, ctx);
        }
        return Flow::Choose {
            options,
            then: |g, ctx, picked| {
                ctx.targets.push(picked);
                painful_pick(g, ctx)
            },
        };
    }
    painful_resolve(g, ctx)
}

fn painful_resolve(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    if ctx.targets.is_empty() {
        return Flow::Done;
    }
    g.reveal(ctx.controller, &ctx.targets);
    Flow::ChooseBy {
        chooser: ctx.controller.opponent(),
        options: ctx.targets.clone(),
        then: |g, ctx, picked| {
            g.add_to_hand(picked);
            for &c in &ctx.targets.clone() {
                if c != picked {
                    g.mill(c);
                }
            }
            g.shuffle_deck(ctx.controller);
            Flow::Done
        },
    }
}

/// Big Eye: the five looked-at cards go back in the order chosen, the
/// first pick on top.
fn big_eye_pick(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let top = g.top_of_deck(ctx.controller, 5);
    let remaining: Vec<InstanceId> = top
        .iter()
        .copied()
        .filter(|c| !ctx.targets.contains(c))
        .collect();
    if remaining.is_empty() {
        let order = ctx.targets.clone();
        g.reorder_top(ctx.controller, &order);
        return Flow::Done;
    }
    Flow::Choose {
        options: remaining,
        then: |g, ctx, picked| {
            ctx.targets.push(picked);
            big_eye_pick(g, ctx)
        },
    }
}

pub fn register(r: &mut Registry) {
    r.register(
        CONFISCATION,
        single(Effect {
            timing: |g, s, _| {
                g.player(s.controller).lp > 1000 && !g.hand(s.controller.opponent()).is_empty()
            },
            cost: Some(|g, ctx| g.pay_lp(ctx.controller, 1000)),
            ..Effect::activate(1, |g, ctx| {
                let options = g.hand(ctx.controller.opponent()).to_vec();
                if options.is_empty() {
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |g, _ctx, picked| {
                        g.discard(picked);
                        Flow::Done
                    },
                }
            })
        }),
    );
    r.register(
        THE_FORCEFUL_SENTRY,
        single(Effect {
            timing: |g, s, _| !g.hand(s.controller.opponent()).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                let options = g.hand(ctx.controller.opponent()).to_vec();
                if options.is_empty() {
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |g, ctx, picked| {
                        g.place_on_top_of_deck(picked);
                        g.shuffle_deck(ctx.controller.opponent());
                        Flow::Done
                    },
                }
            })
        }),
    );
    r.register(
        DELINQUENT_DUO,
        single(Effect {
            timing: |g, s, _| {
                g.player(s.controller).lp > 1000 && !g.hand(s.controller.opponent()).is_empty()
            },
            cost: Some(|g, ctx| g.pay_lp(ctx.controller, 1000)),
            ..Effect::activate(1, |g, ctx| {
                let opponent = ctx.controller.opponent();
                let hand = g.hand(opponent).to_vec();
                if hand.is_empty() {
                    return Flow::Done;
                }
                let random = hand[g.random_index(hand.len())];
                g.discard(random);
                let rest = g.hand(opponent).to_vec();
                if rest.is_empty() {
                    return Flow::Done;
                }
                Flow::ChooseBy {
                    chooser: opponent,
                    options: rest,
                    then: |g, _ctx, picked| {
                        g.discard(picked);
                        Flow::Done
                    },
                }
            })
        }),
    );
    r.register(
        PAINFUL_CHOICE,
        single(Effect {
            timing: |g, s, _| g.deck_size(s.controller) >= 5,
            ..Effect::activate(1, painful_pick)
        }),
    );
    r.register(
        CEREMONIAL_BELL,
        CardScript {
            tags: vec![Tag::RevealHands],
            effects: Vec::new(),
        },
    );
    r.register(
        HIROS_SHADOW_SCOUT,
        single(Effect::flip(|g, ctx| {
            let opponent = ctx.controller.opponent();
            let before = g.hand(opponent).len();
            g.draw(opponent, 3);
            let drawn: Vec<InstanceId> = g.hand(opponent)[before..].to_vec();
            g.reveal(opponent, &drawn);
            for c in drawn {
                if g.stats(c).kind == CardKind::Spell {
                    g.discard(c);
                }
            }
            Flow::Done
        })),
    );
    r.register(
        BIG_EYE,
        single(Effect {
            timing: |g, s, w| {
                ygo_engine::flipped_this_window(g, s, w) && g.deck_size(s.controller) > 0
            },
            ..Effect::flip(big_eye_pick)
        }),
    );
}
