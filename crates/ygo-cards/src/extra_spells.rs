//! Spells among the 66 EXTRA cards (handoff update, 2026-11-20). Black
//! Luster Ritual and White Dragon Ritual live in `ritual`.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, EffectCtx, Flow, Game, InstanceId, Lingering, LingeringKind, Location,
    Phase, PlayerId, Position, Registry, Source, Tag, TargetSets,
};

use crate::continuous::{equip_resolve, equipped_bonus, face_up_monsters};
use crate::positions;

pub const ANCIENT_TELESCOPE: CardId = CardId(17092736);
pub const SOUL_EXCHANGE: CardId = CardId(68005187);
pub const SCAPEGOAT: CardId = CardId(73915051);
/// Scapegoat's Token (`data/tokens.json`).
pub const SHEEP_TOKEN: CardId = CardId(73915052);
pub const CARD_DESTRUCTION: CardId = CardId(72892473);
pub const DE_SPELL: CardId = CardId(19159413);
pub const DIAN_KETO_THE_CURE_MASTER: CardId = CardId(84257639);
pub const FAIRY_METEOR_CRUSH: CardId = CardId(97687912);
pub const GRACEFUL_CHARITY: CardId = CardId(79571449);
pub const INVIGORATION: CardId = CardId(98374133);
pub const LAST_WILL: CardId = CardId(85602018);
pub const NOBLEMAN_OF_CROSSOUT: CardId = CardId(71044499);
pub const OOKAZI: CardId = CardId(19523799);
pub const PREMATURE_BURIAL: CardId = CardId(70828912);
pub const SWORD_OF_DARK_DESTRUCTION: CardId = CardId(37120512);
pub const THE_FLUTE_OF_SUMMONING_DRAGON: CardId = CardId(43973174);
pub const THE_INEXPERIENCED_SPY: CardId = CardId(81820689);
/// Not in the pool: The Flute of Summoning Dragon can never be activated here.
pub const LORD_OF_D: CardId = CardId(17985575);

/// "An <attribute> monster equipped with this card gains 400 ATK and
/// loses 200 DEF."
pub const ATTRIBUTE_EQUIPS: [(CardId, &str); 2] =
    [(INVIGORATION, "EARTH"), (SWORD_OF_DARK_DESTRUCTION, "DARK")];

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn attribute_of_equip(g: &Game, s: Source) -> &'static str {
    let id = g.stats(s.card).id;
    ATTRIBUTE_EQUIPS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, a)| *a)
        .expect("an attribute Equip's row")
}

fn attribute_equip_targets(g: &Game, s: Source) -> TargetSets {
    let attribute = attribute_of_equip(g, s);
    face_up_monsters(g, |g, m| {
        g.spell_targetable(m) && g.stats(m).attribute.as_deref() == Some(attribute)
    })
}

/// De-Spell: a face-up Spell or any Set Spell/Trap, other than itself.
fn de_spell_targets(g: &Game, s: Source) -> TargetSets {
    singles(
        PlayerId::BOTH
            .into_iter()
            .flat_map(|p| g.spells_traps_controlled(p))
            .filter(|&c| {
                c != s.card
                    && (!g.instance(c).position.is_face_up() || g.stats(c).kind == CardKind::Spell)
            }),
    )
}

/// Graceful Charity: discard one card at a time, two in all.
fn charity_discard(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let hand = g.hand(ctx.controller).to_vec();
    if ctx.value >= 2 || hand.is_empty() {
        return Flow::Done;
    }
    Flow::Choose {
        options: hand,
        then: |g, ctx, picked| {
            g.discard(picked);
            ctx.value += 1;
            charity_discard(g, ctx)
        },
    }
}

/// Last Will: Deck monsters with 1500 or less original ATK that may be
/// Special Summoned.
fn last_will_options(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    g.deck(player)
        .iter()
        .copied()
        .filter(|&c| {
            let s = g.stats(c);
            s.is_monster() && s.atk <= 1500 && g.special_summonable(c)
        })
        .collect()
}

/// The Flute: Dragons in the hand that may be Special Summoned.
fn flute_dragons(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    g.hand(player)
        .iter()
        .copied()
        .filter(|&c| g.race(c) == "Dragon" && g.special_summonable(c))
        .collect()
}

fn lord_of_d_on_field(g: &Game) -> bool {
    PlayerId::BOTH.into_iter().any(|p| {
        g.monsters_controlled(p)
            .into_iter()
            .any(|m| g.stats(m).id == LORD_OF_D && g.instance(m).position.is_face_up())
    })
}

/// The Flute: summon one chosen Dragon, then perhaps a second.
fn flute_next(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let options = flute_dragons(g, ctx.controller);
    if ctx.value >= 2
        || options.is_empty()
        || !g.can_special_summon(ctx.controller)
        || !lord_of_d_on_field(g)
    {
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |_, ctx, picked| {
            ctx.targets = vec![picked];
            Flow::Ask {
                options: positions::FACE_UP.to_vec(),
                then: |g, ctx, option| {
                    g.special_summon(ctx.targets[0], ctx.controller, positions::face_up(option));
                    ctx.value += 1;
                    flute_next(g, ctx)
                },
            }
        },
    }
}

/// Free Monster Zones `player` may use.
fn free_monster_zones(g: &Game, player: PlayerId) -> usize {
    g.player(player)
        .monster_zones
        .iter()
        .enumerate()
        .filter(|&(i, z)| z.is_none() && !g.zone_locked(player, i))
        .count()
}

pub fn register(r: &mut Registry) {
    // Special Summon 4 Sheep Tokens in Defense Position; they cannot be
    // Tributed for a Tribute Summon. You cannot Summon other monsters the
    // turn you activate this card (you can Normal Set), so not after a
    // summon either.
    r.register(
        SCAPEGOAT,
        CardScript {
            tags: Vec::new(),
            effects: vec![Effect {
                timing: |g, s, _| {
                    !g.player(s.controller).summoned
                        && free_monster_zones(g, s.controller) >= 4
                        && g.can_special_summon(s.controller)
                        && g.shared().pool.index_of(SHEEP_TOKEN).is_some()
                },
                ..Effect::activate(2, |g, ctx| {
                    if free_monster_zones(g, ctx.controller) >= 4
                        && g.can_special_summon(ctx.controller)
                    {
                        for _ in 0..4 {
                            g.special_summon_token(
                                SHEEP_TOKEN,
                                ctx.controller,
                                Position::FaceUpDefense,
                            );
                        }
                    }
                    let turn = g.turn();
                    g.add_lingering(Lingering {
                        source: ctx.source,
                        kind: LingeringKind::NoSummons {
                            player: ctx.controller,
                        },
                        expires_turn: turn,
                    });
                    Flow::Done
                })
            }],
        },
    );
    r.register(
        SHEEP_TOKEN,
        CardScript {
            tags: vec![Tag::NoTributeSummon],
            effects: Vec::new(),
        },
    );
    // Target 1 monster your opponent controls: this turn your Tribute
    // Summons must Tribute it, as if you controlled it. No Battle Phase the
    // turn you activate this card, so only before one (Main Phase 1).
    r.register(
        SOUL_EXCHANGE,
        CardScript {
            tags: Vec::new(),
            effects: vec![Effect {
                timing: |g, s, _| {
                    g.phase() == Phase::Main1
                        && !g.monsters_controlled(s.controller.opponent()).is_empty()
                },
                targets: Some(|g, s| {
                    g.monsters_controlled(s.controller.opponent())
                        .into_iter()
                        .map(|m| vec![m])
                        .collect()
                }),
                cost: Some(|g, ctx| {
                    let turn = g.turn();
                    g.add_lingering(Lingering {
                        source: ctx.source,
                        kind: LingeringKind::NoBattlePhase {
                            player: ctx.controller,
                        },
                        expires_turn: turn,
                    });
                }),
                ..Effect::activate(1, |g, ctx| {
                    let target = ctx.targets[0];
                    if g.on_field(target) && g.instance(target).controller != ctx.controller {
                        let turn = g.turn();
                        g.add_lingering(Lingering {
                            source: ctx.source,
                            kind: LingeringKind::MustTribute {
                                player: ctx.controller,
                                card: target,
                            },
                            expires_turn: turn,
                        });
                    }
                    Flow::Done
                })
            }],
        },
    );
    // See the top 5 cards of your opponent's Deck; they stay in order.
    r.register(
        ANCIENT_TELESCOPE,
        single(Effect {
            timing: |g, s, _| g.deck_size(s.controller.opponent()) > 0,
            ..Effect::activate(1, |g, ctx| {
                let top = g.top_of_deck(ctx.controller.opponent(), 5);
                g.look_at(ctx.controller, &top);
                Flow::Done
            })
        }),
    );
    // Both players discard their whole hands, then each draws as many.
    r.register(
        CARD_DESTRUCTION,
        single(Effect {
            timing: |g, s, _| {
                g.hand(s.controller).iter().any(|&c| c != s.card)
                    || !g.hand(s.controller.opponent()).is_empty()
            },
            ..Effect::activate(1, |g, ctx| {
                let mut counts = [0; 2];
                for p in [ctx.controller, ctx.controller.opponent()] {
                    for c in g.hand(p).to_vec() {
                        g.discard(c);
                        counts[p.index()] += 1;
                    }
                }
                for p in [ctx.controller, ctx.controller.opponent()] {
                    g.draw(p, counts[p.index()]);
                }
                Flow::Done
            })
        }),
    );
    // Target 1 face-up Spell, or 1 Set Spell/Trap; destroy it if it is a
    // Spell (a Set target is revealed).
    r.register(
        DE_SPELL,
        single(Effect {
            timing: |g, s, _| !de_spell_targets(g, s).is_empty(),
            targets: Some(de_spell_targets),
            ..Effect::activate(1, |g, ctx| {
                crate::flips::destroy_if_kind(g, ctx, CardKind::Spell)
            })
        }),
    );
    r.register(
        DIAN_KETO_THE_CURE_MASTER,
        single(Effect::activate(1, |g, ctx| {
            g.gain_lp(ctx.controller, 1000);
            Flow::Done
        })),
    );
    // If the equipped monster attacks a Defense Position monster, inflict
    // piercing battle damage.
    r.register(
        FAIRY_METEOR_CRUSH,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    targets: Some(|g, _| face_up_monsters(g, |g, m| g.spell_targetable(m))),
                    timing: |g, _, _| !face_up_monsters(g, |g, m| g.spell_targetable(m)).is_empty(),
                    ..Effect::activate(1, equip_resolve)
                },
                Effect {
                    pierce: Some(|g, s, attacker| g.equip_target(s.card) == Some(attacker)),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // Draw 3 cards, then discard 2.
    r.register(
        GRACEFUL_CHARITY,
        single(Effect {
            timing: |g, s, _| g.deck_size(s.controller) >= 3,
            ..Effect::activate(1, |g, ctx| {
                g.draw(ctx.controller, 3);
                ctx.value = 0;
                charity_discard(g, ctx)
            })
        }),
    );
    for (id, _) in ATTRIBUTE_EQUIPS {
        r.register(
            id,
            CardScript {
                tags: Vec::new(),
                effects: vec![
                    Effect {
                        targets: Some(attribute_equip_targets),
                        timing: |g, s, _| !attribute_equip_targets(g, s).is_empty(),
                        ..Effect::activate(1, equip_resolve)
                    },
                    Effect {
                        modify: Some(|g, s, t| equipped_bonus(g, s, t, 400, -200)),
                        ..Effect::continuous()
                    },
                ],
            },
        );
    }
    // If a monster you controlled went to your Graveyard this turn, Special
    // Summon 1 monster with 1500 or less ATK from your Deck, then shuffle.
    // Simplified: the condition is checked on activation and the summon
    // happens on resolution, not "once during this turn" later.
    r.register(
        LAST_WILL,
        single(Effect {
            timing: |g, s, _| {
                g.player(s.controller).monsters_to_graveyard > 0
                    && g.can_special_summon(s.controller)
                    && !last_will_options(g, s.controller).is_empty()
            },
            ..Effect::activate(1, |g, ctx| {
                let options = last_will_options(g, ctx.controller);
                if options.is_empty() || !g.can_special_summon(ctx.controller) {
                    g.shuffle_deck(ctx.controller);
                    return Flow::Done;
                }
                Flow::Choose {
                    options,
                    then: |_, ctx, picked| {
                        ctx.targets = vec![picked];
                        Flow::Ask {
                            options: positions::FACE_UP.to_vec(),
                            then: |g, ctx, option| {
                                g.special_summon(
                                    ctx.targets[0],
                                    ctx.controller,
                                    positions::face_up(option),
                                );
                                g.shuffle_deck(ctx.controller);
                                Flow::Done
                            },
                        }
                    },
                }
            })
        }),
    );
    // Target 1 face-down monster; destroy and banish it; if it was a Flip
    // monster, both players reveal their Decks and banish every card with
    // its name.
    r.register(
        NOBLEMAN_OF_CROSSOUT,
        single(Effect {
            timing: |g, s, _| !nobleman_targets(g, s).is_empty(),
            targets: Some(nobleman_targets),
            ..Effect::activate(1, |g, ctx| {
                let target = ctx.targets[0];
                if !g.on_field(target) {
                    return Flow::Done;
                }
                let (flip, name) = (g.stats(target).flip, g.stats(target).id);
                g.destroy(target, ctx.source);
                if g.instance(target).location != Location::Graveyard {
                    return Flow::Done;
                }
                g.banish(target);
                if flip {
                    for p in [ctx.controller, ctx.controller.opponent()] {
                        let deck = g.deck(p).to_vec();
                        g.reveal(p, &deck);
                        for c in deck {
                            if g.stats(c).id == name {
                                g.banish(c);
                            }
                        }
                    }
                }
                Flow::Done
            })
        }),
    );
    r.register(
        OOKAZI,
        single(Effect::activate(1, |g, ctx| {
            g.take_damage(ctx.controller.opponent(), 800);
            Flow::Done
        })),
    );
    // Pay 800 LP, target 1 monster in your Graveyard; Special Summon it in
    // Attack Position and equip it with this card. When this card is
    // destroyed, destroy that monster. The monster is kept in the counter
    // (plus one), since the link is needed after this card left the field.
    r.register(
        PREMATURE_BURIAL,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, _| {
                        g.can_pay_lp(s.controller, 800)
                            && g.can_special_summon(s.controller)
                            && !burial_targets(g, s).is_empty()
                    },
                    cost: Some(|g, ctx| g.pay_lp(ctx.controller, 800)),
                    targets: Some(burial_targets),
                    ..Effect::activate(1, |g, ctx| {
                        let target = ctx.targets[0];
                        let revived = g.instance(target).location == Location::Graveyard
                            && g.special_summon(target, ctx.controller, Position::FaceUpAttack);
                        if revived && g.equip(ctx.source, target) {
                            g.set_counter(ctx.source, target.0.wrapping_add(1));
                        } else if g.on_field(ctx.source) {
                            g.destroy(ctx.source, ctx.source);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    timing: |g, s, w| {
                        let n = g.counter(s.card);
                        n > 0
                            && w.events.iter().any(|e| {
                                matches!(e,
                                ygo_engine::Event::DestroyedByEffect { card, .. } if *card == s.card)
                            })
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        let monster = InstanceId(g.counter(ctx.source).wrapping_sub(1));
                        g.destroy(monster, ctx.source);
                        Flow::Done
                    })
                },
            ],
        },
    );
    // Special Summon up to 2 Dragons from your hand while "Lord of D." is
    // face-up on the field. Lord of D. is not in this pool.
    r.register(
        THE_FLUTE_OF_SUMMONING_DRAGON,
        single(Effect {
            timing: |g, s, _| {
                lord_of_d_on_field(g)
                    && g.can_special_summon(s.controller)
                    && !flute_dragons(g, s.controller).is_empty()
            },
            ..Effect::activate(1, |g, ctx| {
                ctx.value = 0;
                flute_next(g, ctx)
            })
        }),
    );
    // Select and see 1 card in your opponent's hand. The pick is blind, so
    // a random card stands for it.
    r.register(
        THE_INEXPERIENCED_SPY,
        single(Effect {
            timing: |g, s, _| !g.hand(s.controller.opponent()).is_empty(),
            ..Effect::activate(1, |g, ctx| {
                let hand = g.hand(ctx.controller.opponent()).to_vec();
                if !hand.is_empty() {
                    let card = hand[g.random_index(hand.len())];
                    g.look_at(ctx.controller, &[card]);
                }
                Flow::Done
            })
        }),
    );
}

/// Nobleman of Crossout: a face-down monster on the field.
fn nobleman_targets(g: &Game, _s: Source) -> TargetSets {
    singles(
        PlayerId::BOTH
            .into_iter()
            .flat_map(|p| g.monsters_controlled(p))
            .filter(|&m| {
                g.instance(m).position == Position::FaceDownDefense && g.spell_targetable(m)
            }),
    )
}

/// Premature Burial: a monster in your Graveyard that may be revived.
fn burial_targets(g: &Game, s: Source) -> TargetSets {
    singles(
        g.graveyard(s.controller)
            .iter()
            .copied()
            .filter(|&c| g.stats(c).is_monster() && g.special_summonable(c)),
    )
}
