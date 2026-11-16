//! Equip, Field and Continuous Spells, and Swords of Revealing Light.

use ygo_data::CardId;
use ygo_engine::{
    CardScript, Effect, EffectCtx, Event, Flow, Game, InstanceId, Location, Phase, Position,
    Registry, Source, StatMod, Tag, TargetSets, Window, WindowKind,
};

pub const MALEVOLENT_NUZZLER: CardId = CardId(99597615);
pub const LEGENDARY_SWORD: CardId = CardId(61854111);
pub const UMI: CardId = CardId(22702055);
pub const MESSENGER_OF_PEACE: CardId = CardId(44656491);
pub const SWORDS_OF_REVEALING_LIGHT: CardId = CardId(72302403);

/// Face-up monsters on the field that `keep` accepts, as target sets.
fn face_up_monsters(g: &Game, keep: impl Fn(&Game, InstanceId) -> bool) -> TargetSets {
    ygo_engine::PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .filter(|&m| g.instance(m).position.is_face_up() && keep(g, m))
        .map(|m| vec![m])
        .collect()
}

/// Resolving an Equip Spell: attach to the target if it is still a
/// face-up monster, otherwise the Equip is destroyed.
fn equip_resolve(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    if !g.equip(ctx.source, ctx.targets[0]) {
        g.destroy(ctx.source, ctx.source);
    }
    Flow::Done
}

fn equipped_bonus(g: &Game, s: Source, target: InstanceId, atk: i32, def: i32) -> StatMod {
    if g.equip_target(s.card) == Some(target) {
        StatMod { atk, def }
    } else {
        StatMod::default()
    }
}

/// Opponent's End Phase from the point of view of `s`'s controller.
fn opponents_end_phase(g: &Game, s: Source, w: &Window) -> bool {
    w.kind == WindowKind::Open
        && g.turn_player() != s.controller
        && w.events
            .contains(&Event::PhaseEntered { phase: Phase::End })
        && g.on_field(s.card)
}

pub fn register(r: &mut Registry) {
    // Equip only to a face-up monster. It gains 700 ATK. When this card is
    // sent from the field to the GY: You can pay 500 LP; place it on top
    // of the Deck.
    r.register(
        MALEVOLENT_NUZZLER,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    targets: Some(|g, _| face_up_monsters(g, |_, _| true)),
                    timing: |g, _, _| !face_up_monsters(g, |_, _| true).is_empty(),
                    ..Effect::activate(1, equip_resolve)
                },
                Effect {
                    modify: Some(|g, s, t| equipped_bonus(g, s, t, 700, 0)),
                    ..Effect::continuous()
                },
                Effect {
                    timing: |g, s, w| {
                        g.player(s.controller).lp > 500
                            && w.events.iter().any(|e| {
                                matches!(e,
                                Event::SentToGraveyard { card, from: Location::SpellTrapZone(_) }
                                    if *card == s.card)
                            })
                    },
                    cost: Some(|g, ctx| g.pay_lp(ctx.controller, 500)),
                    ..Effect::trigger(false, |g, ctx| {
                        if g.instance(ctx.source).location == Location::Graveyard {
                            g.place_on_top_of_deck(ctx.source);
                        }
                        Flow::Done
                    })
                },
            ],
        },
    );
    // A Warrior monster equipped with this card gains 300 ATK and DEF.
    r.register(
        LEGENDARY_SWORD,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    targets: Some(|g, _| face_up_monsters(g, |g, m| g.stats(m).race == "Warrior")),
                    timing: |g, _, _| {
                        !face_up_monsters(g, |g, m| g.stats(m).race == "Warrior").is_empty()
                    },
                    ..Effect::activate(1, equip_resolve)
                },
                Effect {
                    modify: Some(|g, s, t| equipped_bonus(g, s, t, 300, 300)),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // All Fish, Sea Serpent, Thunder and Aqua monsters gain 200 ATK/DEF;
    // all Machine and Pyro monsters lose 200 ATK/DEF.
    r.register(
        UMI,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(1, ygo_engine::nothing),
                Effect {
                    modify: Some(|g, _, t| match g.stats(t).race.as_str() {
                        "Fish" | "Sea Serpent" | "Thunder" | "Aqua" => {
                            StatMod { atk: 200, def: 200 }
                        }
                        "Machine" | "Pyro" => StatMod {
                            atk: -200,
                            def: -200,
                        },
                        _ => StatMod::default(),
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // Monsters with 1500 or more ATK cannot declare an attack. During each
    // of your Standby Phases, pay 100 LP or destroy this card.
    r.register(
        MESSENGER_OF_PEACE,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(1, ygo_engine::nothing),
                Effect {
                    restrict: Some(|g, _, t| g.atk(t) >= 1500),
                    ..Effect::continuous()
                },
                Effect {
                    timing: |g, s, w| {
                        w.kind == WindowKind::Open
                            && g.turn_player() == s.controller
                            && g.on_field(s.card)
                            && w.events.contains(&Event::PhaseEntered {
                                phase: Phase::Standby,
                            })
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        if g.player(ctx.controller).lp <= 100 {
                            g.destroy(ctx.source, ctx.source);
                            return Flow::Done;
                        }
                        Flow::Ask {
                            options: vec!["Pay 100 LP", "Destroy Messenger of Peace"],
                            then: |g, ctx, option| {
                                if option == 0 {
                                    g.pay_lp(ctx.controller, 100);
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
    // When this card is activated: If your opponent controls any face-down
    // monsters, flip them face-up. Your opponent's monsters cannot declare
    // an attack. Destroy this card during your opponent's 3rd End Phase
    // after activation.
    r.register(
        SWORDS_OF_REVEALING_LIGHT,
        CardScript {
            tags: vec![Tag::StaysOnField],
            effects: vec![
                Effect::activate(1, |g, ctx| {
                    for m in g.monsters_controlled(ctx.controller.opponent()) {
                        if g.instance(m).position == Position::FaceDownDefense {
                            g.flip_face_up(m);
                        }
                    }
                    Flow::Done
                }),
                Effect {
                    restrict: Some(|g, s, t| g.instance(t).controller != s.controller),
                    ..Effect::continuous()
                },
                Effect {
                    timing: opponents_end_phase,
                    ..Effect::trigger(true, |g, ctx| {
                        let n = g.counter(ctx.source) + 1;
                        g.set_counter(ctx.source, n);
                        if n >= 3 {
                            g.destroy(ctx.source, ctx.source);
                        }
                        Flow::Done
                    })
                },
            ],
        },
    );
}
