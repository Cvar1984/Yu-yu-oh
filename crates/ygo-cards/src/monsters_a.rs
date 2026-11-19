//! Effect Monsters, batch A: direct attackers, conditional stats,
//! recruiters, battle-damage effects, summon and discard triggers.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, EffectCtx, Event, Flow, Game, InstanceId, Location, PlayerId, Position,
    Registry, Source, StatMod, Tag, Window, WindowKind,
};

pub const DIRECT_ATTACKERS: [CardId; 6] = [
    CardId(98049915), // Mystic Lamp
    CardId(12472242), // Leghul
    CardId(58861941), // Ooguchi
    CardId(32809211), // Jinzo #7
    CardId(21347810), // Rainbow Flower
    CardId(5901497),  // Queen's Double
];
pub const SHADOW_GHOUL: CardId = CardId(30778711);
pub const MUKA_MUKA: CardId = CardId(46657337);
pub const FLASH_ASSAILANT: CardId = CardId(96890582);
pub const MAHA_VAILO: CardId = CardId(93013676);
pub const INSECT_SOLDIERS_OF_THE_SKY: CardId = CardId(7019529);
/// (recruiter, attribute of what it Special Summons)
pub const RECRUITERS: [(CardId, &str); 6] = [
    (CardId(97017120), "EARTH"), // Giant Rat
    (CardId(60806437), "FIRE"),  // UFO Turtle
    (CardId(95956346), "LIGHT"), // Shining Angel
    (CardId(57839750), "WATER"), // Mother Grizzly
    (CardId(84834865), "WIND"),  // Flying Kamakiri #1
    (CardId(83011277), "DARK"),  // Mystic Tomato
];
pub const GIANT_GERM: CardId = CardId(95178994);
pub const NIMBLE_MOMONGA: CardId = CardId(22567609);
pub const WHITE_MAGICAL_HAT: CardId = CardId(15150365);
pub const MASKED_SORCERER: CardId = CardId(10189126);
pub const THE_BISTRO_BUTCHER: CardId = CardId(71107816);
pub const SENJU_OF_THE_THOUSAND_HANDS: CardId = CardId(23401839);
pub const SONIC_BIRD: CardId = CardId(57617178);
pub const PENGUIN_KNIGHT: CardId = CardId(36039163);
pub const ELECTRIC_SNAKE: CardId = CardId(11324436);
pub const MINAR: CardId = CardId(32539892);
pub const AMEBA: CardId = CardId(95174353);
pub const GRIGGLE: CardId = CardId(95744531);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn self_mod(g: &Game, s: Source, target: InstanceId, f: fn(&Game, Source) -> StatMod) -> StatMod {
    if target == s.card {
        f(g, s)
    } else {
        StatMod::default()
    }
}

/// The source was destroyed by battle and sent to the Graveyard this window.
fn destroyed_by_battle(_: &Game, s: Source, w: &Window) -> bool {
    w.events
        .iter()
        .any(|e| matches!(e, Event::DestroyedByBattle { card } if *card == s.card))
        && w.events.iter().any(|e| {
            matches!(e,
            Event::SentToGraveyard { card, from: Location::MonsterZone(_) } if *card == s.card)
        })
}

/// The source was Normal Summoned face-up or Flip Summoned this window.
fn normal_or_flip_summoned(_: &Game, s: Source, w: &Window) -> bool {
    w.kind == WindowKind::SummonSuccess
        && w.events.iter().any(|e| {
            matches!(e,
            Event::Summoned { card, position: Position::FaceUpAttack, .. }
            | Event::FlipSummoned { card, .. } if *card == s.card)
        })
}

/// The source was discarded from the hand by the opponent's effect.
fn discarded_by_opponent(_: &Game, s: Source, w: &Window) -> bool {
    w.events
        .iter()
        .any(|e| matches!(e, Event::Discarded { card, .. } if *card == s.card))
        && w.events.iter().any(|e| {
            matches!(e,
            Event::ByEffect { card, controller } if *card == s.card && *controller != s.controller)
        })
}

fn recruit_attribute(g: &Game, s: Source) -> &'static str {
    let id = g.stats(s.card).id;
    RECRUITERS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, a)| *a)
        .expect("a recruiter row")
}

fn recruit_resolve(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let attribute = recruit_attribute(
        g,
        Source {
            card: ctx.source,
            controller: ctx.controller,
        },
    );
    let options: Vec<InstanceId> = g
        .deck(ctx.controller)
        .iter()
        .copied()
        .filter(|&c| {
            let st = g.stats(c);
            st.is_monster()
                && st.atk <= 1500
                && st.attribute.as_deref() == Some(attribute)
                && g.special_summonable(c)
        })
        .collect();
    if options.is_empty() || !g.can_special_summon(ctx.controller) {
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |g, ctx, picked| {
            g.special_summon(picked, ctx.controller, Position::FaceUpAttack);
            g.shuffle_deck(ctx.controller);
            Flow::Done
        },
    }
}

/// Giant Germ and Nimble Momonga: after their fixed effect, Special Summon
/// any number of copies from the Deck, one question per copy.
fn copies_loop(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let id = g.stats(ctx.source).id;
    let next = g
        .deck(ctx.controller)
        .iter()
        .copied()
        .find(|&c| g.stats(c).id == id);
    match next {
        Some(_) if g.can_special_summon(ctx.controller) => Flow::Ask {
            options: vec!["Special Summon a copy from the Deck", "Stop"],
            then: |g, ctx, option| {
                if option != 0 {
                    g.shuffle_deck(ctx.controller);
                    return Flow::Done;
                }
                let id = g.stats(ctx.source).id;
                let position = if id == NIMBLE_MOMONGA {
                    Position::FaceDownDefense
                } else {
                    Position::FaceUpAttack
                };
                if let Some(copy) = g
                    .deck(ctx.controller)
                    .iter()
                    .copied()
                    .find(|&c| g.stats(c).id == id)
                {
                    g.special_summon(copy, ctx.controller, position);
                }
                copies_loop(g, ctx)
            },
        },
        _ => {
            g.shuffle_deck(ctx.controller);
            Flow::Done
        }
    }
}

/// This card's battle dealt battle damage to its controller's opponent in
/// this window: "When this card inflicts Battle Damage to your opponent".
fn inflicted_damage(_: &Game, s: Source, w: &Window) -> bool {
    w.events.iter().any(|e| {
        matches!(e,
        Event::BattleDamageDealt { attacker, target, damaged, .. }
            if *damaged != s.controller && (*attacker == s.card || *target == Some(s.card)))
    })
}

fn search_deck(g: &mut Game, ctx: &mut EffectCtx, keep: fn(&Game, InstanceId) -> bool) -> Flow {
    let options: Vec<InstanceId> = g
        .deck(ctx.controller)
        .iter()
        .copied()
        .filter(|&c| keep(g, c))
        .collect();
    if options.is_empty() {
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |g, ctx, picked| {
            g.add_to_hand(picked);
            g.shuffle_deck(ctx.controller);
            Flow::Done
        },
    }
}

pub fn register(r: &mut Registry) {
    for id in DIRECT_ATTACKERS {
        r.register(
            id,
            CardScript {
                tags: vec![Tag::DirectAttacker],
                effects: Vec::new(),
            },
        );
    }
    r.register(
        SHADOW_GHOUL,
        single(Effect {
            modify: Some(|g, s, t| {
                self_mod(g, s, t, |g, s| StatMod {
                    atk: 100
                        * g.graveyard(s.controller)
                            .iter()
                            .filter(|&&c| g.stats(c).is_monster())
                            .count() as i32,
                    def: 0,
                })
            }),
            ..Effect::continuous()
        }),
    );
    r.register(
        MUKA_MUKA,
        single(Effect {
            modify: Some(|g, s, t| {
                self_mod(g, s, t, |g, s| {
                    let n = 300 * g.hand(s.controller).len() as i32;
                    StatMod { atk: n, def: n }
                })
            }),
            ..Effect::continuous()
        }),
    );
    r.register(
        FLASH_ASSAILANT,
        single(Effect {
            modify: Some(|g, s, t| {
                self_mod(g, s, t, |g, s| {
                    let n = -400 * g.hand(s.controller).len() as i32;
                    StatMod { atk: n, def: n }
                })
            }),
            ..Effect::continuous()
        }),
    );
    r.register(
        MAHA_VAILO,
        single(Effect {
            modify: Some(|g, s, t| {
                self_mod(g, s, t, |g, s| StatMod {
                    atk: 500
                        * g.spells_traps_controlled(s.controller)
                            .iter()
                            .filter(|&&e| g.equip_target(e) == Some(s.card))
                            .count() as i32,
                    def: 0,
                })
            }),
            ..Effect::continuous()
        }),
    );
    r.register(
        INSECT_SOLDIERS_OF_THE_SKY,
        single(Effect {
            modify: Some(|g, s, t| {
                self_mod(g, s, t, |g, s| {
                    let attacking_wind = g.current_attack().is_some_and(|(attacker, target)| {
                        attacker == s.card
                            && target
                                .is_some_and(|t| g.stats(t).attribute.as_deref() == Some("WIND"))
                    });
                    StatMod {
                        atk: if attacking_wind { 1000 } else { 0 },
                        def: 0,
                    }
                })
            }),
            ..Effect::continuous()
        }),
    );
    for (id, _) in RECRUITERS {
        r.register(
            id,
            single(Effect {
                timing: destroyed_by_battle,
                misses_timing: true,
                ..Effect::trigger(false, recruit_resolve)
            }),
        );
    }
    r.register(
        GIANT_GERM,
        single(Effect {
            timing: destroyed_by_battle,
            ..Effect::trigger(true, |g, ctx| {
                g.take_damage(ctx.controller.opponent(), 500);
                copies_loop(g, ctx)
            })
        }),
    );
    r.register(
        NIMBLE_MOMONGA,
        single(Effect {
            timing: destroyed_by_battle,
            ..Effect::trigger(true, |g, ctx| {
                g.gain_lp(ctx.controller, 1000);
                copies_loop(g, ctx)
            })
        }),
    );
    r.register(
        WHITE_MAGICAL_HAT,
        single(Effect {
            timing: inflicted_damage,
            ..Effect::trigger(true, |g, ctx| {
                let hand = g.hand(ctx.controller.opponent()).to_vec();
                if !hand.is_empty() {
                    let pick = hand[g.random_index(hand.len())];
                    g.discard(pick);
                }
                Flow::Done
            })
        }),
    );
    r.register(
        MASKED_SORCERER,
        single(Effect {
            timing: inflicted_damage,
            ..Effect::trigger(true, |g, ctx| {
                g.draw(ctx.controller, 1);
                Flow::Done
            })
        }),
    );
    r.register(
        THE_BISTRO_BUTCHER,
        single(Effect {
            timing: inflicted_damage,
            ..Effect::trigger(true, |g, ctx| {
                g.draw(ctx.controller.opponent(), 2);
                Flow::Done
            })
        }),
    );
    r.register(
        SENJU_OF_THE_THOUSAND_HANDS,
        single(Effect {
            timing: normal_or_flip_summoned,
            misses_timing: true,
            ..Effect::trigger(false, |g, ctx| {
                search_deck(g, ctx, |g, c| matches!(g.stats(c).kind, CardKind::Ritual))
            })
        }),
    );
    r.register(
        SONIC_BIRD,
        single(Effect {
            timing: normal_or_flip_summoned,
            misses_timing: true,
            ..Effect::trigger(false, |g, ctx| {
                search_deck(g, ctx, |g, c| {
                    g.stats(c).subtype == ygo_engine::SpellTrapType::Ritual
                })
            })
        }),
    );
    r.register(
        PENGUIN_KNIGHT,
        single(Effect {
            timing: |_, s, w| {
                w.events.iter().any(|e| matches!(e,
                    Event::SentToGraveyard { card, from: Location::Deck } if *card == s.card))
                    && w.events.iter().any(|e| matches!(e,
                        Event::ByEffect { card, controller } if *card == s.card && *controller != s.controller))
            },
            ..Effect::trigger(true, |g, ctx| {
                for c in g.graveyard(ctx.controller).to_vec() {
                    g.return_to_deck(c);
                }
                g.shuffle_deck(ctx.controller);
                Flow::Done
            })
        }),
    );
    r.register(
        ELECTRIC_SNAKE,
        single(Effect {
            timing: discarded_by_opponent,
            ..Effect::trigger(true, |g, ctx| {
                g.draw(ctx.controller, 2);
                Flow::Done
            })
        }),
    );
    r.register(
        MINAR,
        single(Effect {
            timing: discarded_by_opponent,
            ..Effect::trigger(true, |g, ctx| {
                g.take_damage(ctx.controller.opponent(), 1000);
                Flow::Done
            })
        }),
    );
    // Ameba and Griggle: the player who takes control suffers or gives. The
    // trigger belongs to the new controller, so "your opponent" in the text
    // is that player, and the counter keeps it to once while face-up.
    r.register(
        AMEBA,
        single(Effect {
            timing: |g, s, w| {
                g.counter(s.card) == 0
                    && w.events.iter().any(|e| matches!(e,
                        Event::ControlChanged { card, to } if *card == s.card && *to == s.controller))
            },
            ..Effect::trigger(true, |g, ctx| {
                g.set_counter(ctx.source, 1);
                g.take_damage(ctx.controller, 2000);
                Flow::Done
            })
        }),
    );
    r.register(
        GRIGGLE,
        single(Effect {
            timing: |g, s, w| {
                g.counter(s.card) == 0
                    && w.events.iter().any(|e| matches!(e,
                        Event::ControlChanged { card, to } if *card == s.card && *to == s.controller))
            },
            ..Effect::trigger(true, |g, ctx| {
                g.set_counter(ctx.source, 1);
                g.gain_lp(ctx.controller.opponent(), 3000);
                Flow::Done
            })
        }),
    );
}

#[allow(dead_code)]
fn _unused(_: PlayerId) {}
