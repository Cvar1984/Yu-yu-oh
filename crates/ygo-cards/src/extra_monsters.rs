//! Monsters among the 66 EXTRA cards (handoff update, 2026-11-20). Black
//! Luster Soldier lives with its Ritual Spell in `ritual`.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, Event, Flow, Game, InstanceId, Phase, PlayerId, Position, Registry, Source,
    TargetSets, Window, WindowKind,
};

use crate::flips::{destroy_if_kind, picks_of_kind};

pub const MYSTERIOUS_PUPPETEER: CardId = CardId(54098121);
pub const THE_WICKED_WORM_BEAST: CardId = CardId(6285791);
pub const PENGUIN_SOLDIER: CardId = CardId(93920745);
pub const TRAP_MASTER: CardId = CardId(46461247);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn face_up_on_field(g: &Game, s: Source) -> bool {
    g.on_field(s.card) && g.instance(s.card).position.is_face_up()
}

/// A monster was Normal Summoned (Tribute Summons included, Sets not) or
/// Flip Summoned in this window, by either player.
fn normal_or_flip_summon(w: &Window) -> bool {
    w.kind == WindowKind::SummonSuccess
        && w.events.iter().any(|e| {
            matches!(
                e,
                Event::Summoned { position, .. } if *position != Position::FaceDownDefense
            ) || matches!(e, Event::FlipSummoned { .. })
        })
}

/// Penguin Soldier: one or two monsters on the field.
fn up_to_two_monsters(g: &Game, _s: Source) -> TargetSets {
    let all: Vec<InstanceId> = PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .collect();
    let mut sets: TargetSets = all.iter().map(|&m| vec![m]).collect();
    for (i, &a) in all.iter().enumerate() {
        for &b in &all[i + 1..] {
            sets.push(vec![a, b]);
        }
    }
    sets
}

pub fn register(r: &mut Registry) {
    // Each time a monster is Normal or Flip Summoned: gain 500 LP.
    r.register(
        MYSTERIOUS_PUPPETEER,
        single(Effect {
            timing: |g, s, w| face_up_on_field(g, s) && normal_or_flip_summon(w),
            ..Effect::trigger(true, |g, ctx| {
                g.gain_lp(ctx.controller, 500);
                Flow::Done
            })
        }),
    );
    // Returned to its owner's hand during the End Phase of its
    // controller's turn, while face-up on the field.
    r.register(
        THE_WICKED_WORM_BEAST,
        single(Effect {
            timing: |g, s, w| {
                w.kind == WindowKind::Open
                    && g.turn_player() == s.controller
                    && w.events
                        .contains(&Event::PhaseEntered { phase: Phase::End })
                    && face_up_on_field(g, s)
            },
            ..Effect::trigger(true, |g, ctx| {
                if g.on_field(ctx.source) && g.instance(ctx.source).position.is_face_up() {
                    g.add_to_hand(ctx.source);
                }
                Flow::Done
            })
        }),
    );
    // FLIP: You can target up to 2 monsters on the field; return those
    // targets to the hand. Optional, so a trigger that may be passed.
    r.register(
        PENGUIN_SOLDIER,
        single(Effect {
            timing: ygo_engine::flipped_this_window,
            targets: Some(up_to_two_monsters),
            ..Effect::trigger(false, |g, ctx| {
                for &t in &ctx.targets.clone() {
                    if g.on_field(t) {
                        g.add_to_hand(t);
                    }
                }
                Flow::Done
            })
        }),
    );
    // FLIP: Target 1 Trap on the field; destroy it. A Set card can be
    // targeted; it is revealed, and destroyed only if it is a Trap.
    r.register(
        TRAP_MASTER,
        single(Effect {
            targets: Some(|g, _| picks_of_kind(g, CardKind::Trap)),
            ..Effect::flip(|g, ctx| destroy_if_kind(g, ctx, CardKind::Trap))
        }),
    );
}
