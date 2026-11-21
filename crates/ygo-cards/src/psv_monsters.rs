//! Effect and Flip monsters of Pharaoh's Servant (owner, 2026-11-21),
//! on the modern text; see `docs/psv-plan.md`.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, Event, Flow, Game, InstanceId, Location, Phase, PlayerId, Position,
    Registry, Source, StatMod, TargetSets, WindowKind,
};

pub const BITE_SHOES: CardId = CardId(50122883);
pub const BOMBARDMENT_BEETLE: CardId = CardId(57409948);
pub const BUBONIC_VERMIN: CardId = CardId(6104968);
pub const BUSTER_BLADER: CardId = CardId(78193831);
pub const DRILL_BUG: CardId = CardId(88733579);
pub const FOUR_STARRED_LADYBUG_OF_DOOM: CardId = CardId(83994646);
pub const GEARFRIED_THE_IRON_KNIGHT: CardId = CardId(423705);
pub const INVITATION_TO_A_DARK_SLEEP: CardId = CardId(52675689);
pub const KISEITAI: CardId = CardId(4266839);
pub const MAD_SWORD_BEAST: CardId = CardId(79870141);
pub const PARASITE_PARACIDE: CardId = CardId(27911549);
pub const THE_FIEND_MEGACYBER: CardId = CardId(66362965);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn face_up(g: &Game, card: InstanceId) -> bool {
    g.on_field(card) && g.instance(card).position.is_face_up()
}

fn all_monsters(g: &Game) -> Vec<InstanceId> {
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .collect()
}

/// An Effect Monster: an Effect, Flip or Toon monster, or a Fusion or
/// Ritual Monster whose script has effects.
pub(crate) fn is_effect_monster(g: &Game, c: InstanceId) -> bool {
    match g.stats(c).kind {
        CardKind::Effect => true,
        CardKind::Fusion | CardKind::Ritual => g.script(c).is_some_and(|s| !s.effects.is_empty()),
        _ => false,
    }
}

fn dragons_for_buster_blader(g: &Game, s: Source) -> i32 {
    let opponent = s.controller.opponent();
    let on_field = g
        .monsters_controlled(opponent)
        .into_iter()
        .filter(|&m| face_up(g, m) && g.stats(m).race == "Dragon")
        .count();
    let in_graveyard = g
        .graveyard(opponent)
        .iter()
        .filter(|&&c| g.stats(c).is_monster() && g.stats(c).race == "Dragon")
        .count();
    (on_field + in_graveyard) as i32
}

/// The monster a card's `counter` remembers (stored as id + 1).
fn remembered(g: &Game, card: InstanceId) -> Option<InstanceId> {
    match g.counter(card) {
        0 => None,
        n => Some(InstanceId(n - 1)),
    }
}

/// Gearfried: the Equip Cards just equipped to it, among `events`.
fn equipped_to_gearfried(g: &Game, s: Source, events: &[Event]) -> Vec<InstanceId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Equipped { card, to } if *to == s.card && g.on_field(*card) => Some(*card),
            _ => None,
        })
        .collect()
}

fn opponent_face_down(g: &Game, s: Source) -> Vec<InstanceId> {
    g.monsters_controlled(s.controller.opponent())
        .into_iter()
        .filter(|&m| g.instance(m).position == Position::FaceDownDefense)
        .collect()
}

fn vermin_in_deck(g: &Game, player: PlayerId) -> Option<InstanceId> {
    g.deck(player)
        .iter()
        .copied()
        .find(|&c| g.stats(c).id == BUBONIC_VERMIN)
}

pub fn register(r: &mut Registry) {
    // FLIP: change the battle position of 1 face-up monster on the field
    // (chosen on resolution: the text does not target).
    r.register(
        BITE_SHOES,
        single(Effect::flip(|g, _ctx| {
            let options: Vec<_> = all_monsters(g)
                .into_iter()
                .filter(|&m| face_up(g, m))
                .collect();
            if options.is_empty() {
                return Flow::Done;
            }
            Flow::Choose {
                options,
                then: |g, _ctx, m| {
                    let to = if g.instance(m).position == Position::FaceUpAttack {
                        Position::FaceUpDefense
                    } else {
                        Position::FaceUpAttack
                    };
                    g.set_position(m, to);
                    Flow::Done
                },
            }
        })),
    );
    // FLIP: look at 1 face-down Defense Position monster your opponent
    // controls; destroy it if it is an Effect Monster (it is not flipped),
    // otherwise it stays face-down.
    r.register(
        BOMBARDMENT_BEETLE,
        single(Effect::flip(|g, ctx| {
            let options = opponent_face_down(
                g,
                Source {
                    card: ctx.source,
                    controller: ctx.controller,
                },
            );
            if options.is_empty() {
                return Flow::Done;
            }
            Flow::Choose {
                options,
                then: |g, ctx, m| {
                    g.look_at(ctx.controller, &[m]);
                    if is_effect_monster(g, m) {
                        g.destroy(m, ctx.source);
                    }
                    Flow::Done
                },
            }
        })),
    );
    // FLIP: you can Special Summon 1 "Bubonic Vermin" from your Deck in
    // face-down Defense Position, then shuffle your Deck.
    r.register(
        BUBONIC_VERMIN,
        single(Effect::flip(|g, ctx| {
            if vermin_in_deck(g, ctx.controller).is_none() || !g.can_special_summon(ctx.controller)
            {
                return Flow::Done;
            }
            Flow::Ask {
                options: vec!["Special Summon a Bubonic Vermin from your Deck", "Do not"],
                then: |g, ctx, option| {
                    if option == 0
                        && let Some(v) = vermin_in_deck(g, ctx.controller)
                    {
                        g.special_summon(v, ctx.controller, Position::FaceDownDefense);
                        g.shuffle_deck(ctx.controller);
                    }
                    Flow::Done
                },
            }
        })),
    );
    // Gains 500 ATK for each Dragon your opponent controls or has in their
    // Graveyard (face-up Dragons: a face-down monster's Type is hidden).
    r.register(
        BUSTER_BLADER,
        single(Effect {
            modify: Some(|g, s, t| {
                if t == s.card {
                    StatMod {
                        atk: 500 * dragons_for_buster_blader(g, s),
                        def: 0,
                    }
                } else {
                    StatMod::default()
                }
            }),
            ..Effect::continuous()
        }),
    );
    // When this card inflicts battle damage to your opponent: you can put
    // 1 "Parasite Paracide" from your Deck on top of your Deck after
    // shuffling it.
    r.register(
        DRILL_BUG,
        single(Effect {
            timing: |g, s, w| {
                w.events.iter().any(|e| {
                    matches!(e, Event::BattleDamageDealt { attacker, damaged, .. }
                        if *attacker == s.card && *damaged != s.controller)
                }) && g
                    .deck(s.controller)
                    .iter()
                    .any(|&c| g.stats(c).id == PARASITE_PARACIDE)
            },
            damage_step: true,
            ..Effect::trigger(false, |g, ctx| {
                let paracide = g
                    .deck(ctx.controller)
                    .iter()
                    .copied()
                    .find(|&c| g.stats(c).id == PARASITE_PARACIDE);
                if let Some(p) = paracide {
                    g.shuffle_deck(ctx.controller);
                    g.reorder_top(ctx.controller, &[p]);
                }
                Flow::Done
            })
        }),
    );
    // FLIP: destroy all face-up Level 4 monsters your opponent controls.
    r.register(
        FOUR_STARRED_LADYBUG_OF_DOOM,
        single(Effect::flip(|g, ctx| {
            for m in g.monsters_controlled(ctx.controller.opponent()) {
                if face_up(g, m) && g.stats(m).level == 4 {
                    g.destroy(m, ctx.source);
                }
            }
            Flow::Done
        })),
    );
    // If an Equip Card is equipped to this card: destroy that Equip Card.
    r.register(
        GEARFRIED_THE_IRON_KNIGHT,
        single(Effect {
            timing: |g, s, w| !equipped_to_gearfried(g, s, w.events).is_empty(),
            targets: Some(|g, s| {
                let events = g.window().map_or(&[][..], |(_, e)| e);
                vec![equipped_to_gearfried(g, s, events)]
            }),
            damage_step: true,
            ..Effect::trigger(true, |g, ctx| {
                for &e in &ctx.targets.clone() {
                    if g.equip_target(e) == Some(ctx.source) {
                        g.destroy(e, ctx.source);
                    }
                }
                Flow::Done
            })
        }),
    );
    // When this card is Normal or Flip Summoned: target 1 monster your
    // opponent controls; while this card is face-up, that monster cannot
    // attack. The target is remembered in this card's counter.
    r.register(
        INVITATION_TO_A_DARK_SLEEP,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, w| {
                        w.events.iter().any(|e| match e {
                            Event::Summoned { card, position, .. } => {
                                *card == s.card && position.is_face_up()
                            }
                            Event::FlipSummoned { card, .. } => *card == s.card,
                            _ => false,
                        }) && !g.monsters_controlled(s.controller.opponent()).is_empty()
                    },
                    targets: Some(|g, s| singles(g.monsters_controlled(s.controller.opponent()))),
                    ..Effect::trigger(true, |g, ctx| {
                        let target = ctx.targets[0];
                        if face_up(g, ctx.source) && g.on_field(target) {
                            g.set_counter(ctx.source, target.0.wrapping_add(1));
                        }
                        Flow::Done
                    })
                },
                Effect {
                    restrict: Some(|g, s, t| remembered(g, s.card) == Some(t)),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // When your opponent's monster attacks this face-down Defense Position
    // card, it becomes an Equip Card equipped to the attacker, with no
    // damage calculation. In each of your opponent's Standby Phases, you
    // gain LP equal to half the equipped monster's ATK.
    r.register(
        KISEITAI,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, w| {
                        w.kind == WindowKind::AttackDeclared
                            && g.instance(s.card).position == Position::FaceDownDefense
                            && g.current_attack().is_some_and(|(a, t)| {
                                t == Some(s.card) && g.instance(a).controller != s.controller
                            })
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        let Some((attacker, Some(target))) = g.current_attack() else {
                            return Flow::Done;
                        };
                        if target == ctx.source
                            && g.on_field(attacker)
                            && g.instance(ctx.source).position == Position::FaceDownDefense
                            && g.absorb_as_equip(ctx.source, ctx.controller, attacker)
                        {
                            g.cancel_attack(attacker);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    timing: |g, s, w| {
                        w.kind == WindowKind::Open
                            && g.phase() == Phase::Standby
                            && g.turn_player() != s.controller
                            && w.events.contains(&Event::PhaseEntered {
                                phase: Phase::Standby,
                            })
                            && matches!(g.instance(s.card).location, Location::SpellTrapZone(_))
                            && g.equip_target(s.card).is_some_and(|m| g.on_field(m))
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        if let Some(m) = g.equip_target(ctx.source) {
                            let half = (g.atk(m) / 2).max(0) as u32;
                            g.gain_lp(ctx.controller, half);
                        }
                        Flow::Done
                    })
                },
            ],
        },
    );
    // Inflicts piercing battle damage.
    r.register(
        MAD_SWORD_BEAST,
        single(Effect {
            pierce: Some(|_, s, attacker| attacker == s.card),
            ..Effect::continuous()
        }),
    );
    // If your opponent controls at least 2 more monsters than you do, you
    // can Special Summon this card from your hand (a summon procedure).
    r.register(
        THE_FIEND_MEGACYBER,
        single(Effect {
            timing: |g, s, _| {
                let mine = g.monsters_controlled(s.controller).len();
                let theirs = g.monsters_controlled(s.controller.opponent()).len();
                g.instance(s.card).location == Location::Hand
                    && theirs >= mine + 2
                    && g.can_special_summon(s.controller)
            },
            ..Effect::procedure()
        }),
    );
}
