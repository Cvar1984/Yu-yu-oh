//! Effect and Flip monsters of Pharaoh's Servant (owner, 2026-11-21),
//! on the modern text; see `docs/psv-plan.md`.

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    BattleStep, CardScript, Effect, EffectCtx, Event, Flow, Game, InstanceId, Location, Phase,
    PlayerId, Position, Registry, Source, StatMod, Tag, TargetSets, Window, WindowKind,
};

use crate::positions::{FACE_UP, face_up as face_up_position};
use crate::ritual::{absorb_one, absorbed_instead, absorbed_stats};

pub const BITE_SHOES: CardId = CardId(50122883);
pub const BOMBARDMENT_BEETLE: CardId = CardId(57409948);
pub const BUBONIC_VERMIN: CardId = CardId(6104968);
pub const BUSTER_BLADER: CardId = CardId(78193831);
pub const DEEPSEA_WARRIOR: CardId = CardId(24128274);
pub const JINZO: CardId = CardId(77585513);
pub const THE_LEGENDARY_FISHERMAN: CardId = CardId(3643300);
pub const GOBLIN_ATTACK_FORCE: CardId = CardId(78658564);
pub const HAYABUSA_KNIGHT: CardId = CardId(21015833);
pub const MORPHING_JAR_2: CardId = CardId(79106360);
pub const SWORD_HUNTER: CardId = CardId(51345461);
pub const THOUSAND_EYES_RESTRICT: CardId = CardId(63519819);
pub const VAMPIRE_BABY: CardId = CardId(56387350);
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
        .filter(|&m| face_up(g, m) && g.race(m) == "Dragon")
        .count();
    let in_graveyard = g
        .graveyard(opponent)
        .iter()
        .filter(|&&c| g.stats(c).is_monster() && g.race(c) == "Dragon")
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

/// "Umi" is face-up on the field.
fn umi_on_field(g: &Game) -> bool {
    PlayerId::BOTH.into_iter().any(|p| {
        g.field_spell(p).is_some_and(|f| {
            g.stats(f).id == crate::continuous::UMI && g.instance(f).position.is_face_up()
        })
    })
}

/// The End of the Battle Phase was just entered.
fn battle_phase_ends(g: &Game, w: &Window) -> bool {
    let end = Phase::Battle(BattleStep::End);
    w.kind == WindowKind::Open
        && g.phase() == end
        && w.events.contains(&Event::PhaseEntered { phase: end })
}

/// Monsters `s.card` destroyed by battle this turn that are in a Graveyard.
fn destroyed_by(g: &Game, s: Source) -> Vec<InstanceId> {
    let turn = g.turn();
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.graveyard(p).iter().copied())
        .filter(|&c| g.instance(c).destroyed_by_battle == Some((s.card, turn)))
        .collect()
}

/// Morphing Jar #2: each player excavates from the top of their Deck
/// until `n` monsters are revealed; Level 4 or lower monsters among them
/// are Special Summoned face-down, the rest go to the Graveyard.
fn excavate(g: &mut Game, player: PlayerId, n: usize) {
    let mut cards = Vec::new();
    let mut monsters = 0;
    for c in g.top_of_deck(player, g.deck_size(player)) {
        if monsters == n {
            break;
        }
        if g.stats(c).is_monster() {
            monsters += 1;
        }
        cards.push(c);
    }
    if cards.is_empty() {
        return;
    }
    g.reveal(player, &cards);
    for c in cards {
        let stats = g.stats(c);
        let summonable = stats.is_monster() && stats.level <= 4 && g.special_summonable(c);
        if !(summonable && g.special_summon(c, player, Position::FaceDownDefense)) {
            g.mill(c);
        }
    }
}

fn vampire_baby_summon(g: &mut Game, ctx: &mut EffectCtx, option: usize) -> Flow {
    let target = ctx.targets[0];
    if g.instance(target).location == Location::Graveyard {
        g.special_summon(target, ctx.controller, face_up_position(option));
    }
    Flow::Done
}

pub fn register(r: &mut Registry) {
    // If this card attacks, it changes to Defense Position at the end of
    // the Battle Phase, and its position cannot be changed until the End
    // Phase of your next turn.
    r.register(
        GOBLIN_ATTACK_FORCE,
        single(Effect {
            timing: |g, s, w| {
                battle_phase_ends(g, w)
                    && g.instance(s.card).attacks_this_turn > 0
                    && g.instance(s.card).position == Position::FaceUpAttack
            },
            ..Effect::trigger(true, |g, ctx| {
                if g.instance(ctx.source).position == Position::FaceUpAttack {
                    g.set_position(ctx.source, Position::FaceUpDefense);
                }
                let turn = g.turn() + 2;
                g.lock_position_until(ctx.source, ctx.source, turn);
                Flow::Done
            })
        }),
    );
    // While "Umi" is face-up on the field, this card is unaffected by
    // Spell Cards.
    r.register(
        DEEPSEA_WARRIOR,
        single(Effect {
            spell_immune: Some(|g, _| umi_on_field(g)),
            ..Effect::continuous()
        }),
    );
    // Trap Cards and their effects on the field cannot be activated;
    // negate all Trap effects on the field.
    r.register(
        JINZO,
        single(Effect {
            negate: Some(|g, _, card| g.stats(card).kind == CardKind::Trap),
            ..Effect::continuous()
        }),
    );
    // FLIP: put this card face-up into your opponent's Deck. When they draw
    // it, it is Special Summoned to their field in face-up Defense Position
    // and inflicts 1000 damage to them; then, while it is face-up, every
    // monster they control is an Insect.
    r.register(
        PARASITE_PARACIDE,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::flip(|g, ctx| {
                    if g.on_field(ctx.source) {
                        g.put_face_up_in_deck(ctx.source, ctx.controller.opponent());
                    }
                    Flow::Done
                }),
                Effect {
                    timing: |g, s, w| {
                        g.instance(s.card).location == Location::Hand
                            && g.instance(s.card).owner != s.controller
                            && w.events
                                .iter()
                                .any(|e| matches!(e, Event::Drew { card, .. } if *card == s.card))
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        if g.instance(ctx.source).location == Location::Hand
                            && g.special_summon(ctx.source, ctx.controller, Position::FaceUpDefense)
                        {
                            g.take_damage(ctx.controller, 1000);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    race: Some(|g, s, t| {
                        (g.instance(s.card).owner != s.controller
                            && g.instance(t).controller == s.controller)
                            .then_some("Insect")
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // While "Umi" is on the field, this card is unaffected by Spell
    // effects and cannot be targeted for attacks (an attack may go
    // directly to you instead).
    r.register(
        THE_LEGENDARY_FISHERMAN,
        single(Effect {
            spell_immune: Some(|g, _| umi_on_field(g)),
            unattackable: Some(|g, _| umi_on_field(g)),
            ..Effect::continuous()
        }),
    );
    r.register(
        HAYABUSA_KNIGHT,
        CardScript {
            tags: vec![Tag::SecondAttack],
            effects: Vec::new(),
        },
    );
    // FLIP: shuffle every monster on the field into the Deck; then each
    // player excavates until as many monsters as they shuffled into their
    // Main Deck, Special Summons the Level 4 or lower ones among them face-
    // down and sends the rest to the Graveyard.
    r.register(
        MORPHING_JAR_2,
        single(Effect::flip(|g, _ctx| {
            let mut counts = [0usize; 2];
            for m in all_monsters(g) {
                let owner = g.instance(m).owner;
                if !g.stats(m).is_extra_deck() {
                    counts[owner.index()] += 1;
                }
                g.return_to_deck(m);
            }
            for p in PlayerId::BOTH {
                g.shuffle_deck(p);
            }
            let first = g.turn_player();
            for p in [first, first.opponent()] {
                excavate(g, p, counts[p.index()]);
            }
            Flow::Done
        })),
    );
    // At the end of the Battle Phase, equip every monster this card
    // destroyed by battle this turn from the Graveyard to it; it gains 200
    // ATK for each.
    r.register(
        SWORD_HUNTER,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    timing: |g, s, w| {
                        battle_phase_ends(g, w)
                            && face_up(g, s.card)
                            && !destroyed_by(g, s).is_empty()
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        let s = Source {
                            card: ctx.source,
                            controller: ctx.controller,
                        };
                        for m in destroyed_by(g, s) {
                            if !face_up(g, ctx.source)
                                || !g.absorb_as_equip(m, ctx.controller, ctx.source)
                            {
                                break;
                            }
                        }
                        Flow::Done
                    })
                },
                Effect {
                    modify: Some(|g, s, t| {
                        if t != s.card {
                            return StatMod::default();
                        }
                        let equipped = PlayerId::BOTH
                            .into_iter()
                            .flat_map(|p| g.spells_traps_controlled(p))
                            .filter(|&c| {
                                g.equip_target(c) == Some(s.card) && g.stats(c).is_monster()
                            })
                            .count() as i32;
                        StatMod {
                            atk: 200 * equipped,
                            def: 0,
                        }
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // "Relinquished" + "Thousand-Eyes Idol". Other monsters cannot change
    // their positions or attack. Once per turn, it equips 1 opponent's
    // monster (one at most) and takes its ATK and DEF; if it would be
    // destroyed by battle, the equipped monster is destroyed instead.
    r.register(
        THOUSAND_EYES_RESTRICT,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                absorb_one(),
                Effect {
                    modify: Some(absorbed_stats),
                    replace_battle_destruction: Some(absorbed_instead),
                    restrict: Some(|_, s, t| t != s.card),
                    lock_position: Some(|_, s, t| t != s.card),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // At the end of the Battle Phase, if this card destroyed a monster by
    // battle this turn and sent it to the Graveyard: you can Special Summon
    // that monster to your field.
    r.register(
        VAMPIRE_BABY,
        single(Effect {
            timing: |g, s, w| {
                battle_phase_ends(g, w)
                    && face_up(g, s.card)
                    && g.can_special_summon(s.controller)
                    && destroyed_by(g, s).iter().any(|&c| g.special_summonable(c))
            },
            targets: Some(|g, s| {
                singles(
                    destroyed_by(g, s)
                        .into_iter()
                        .filter(|&c| g.special_summonable(c)),
                )
            }),
            ..Effect::trigger(false, |_g, _ctx| Flow::Ask {
                options: FACE_UP.to_vec(),
                then: vampire_baby_summon,
            })
        }),
    );
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
