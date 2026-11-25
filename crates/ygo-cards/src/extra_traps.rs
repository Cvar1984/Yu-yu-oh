//! Traps among the 66 EXTRA cards (handoff update, 2026-11-20).

use ygo_data::{CardId, CardKind};
use ygo_engine::{
    CardScript, Effect, EffectCtx, EffectKind, Event, Flow, Game, InstanceId, Lingering,
    LingeringKind, Location, Phase, PlayerId, Registry, Source, StatMod, Tag, TargetSets, Window,
    WindowKind,
};

use crate::continuous::face_up_monsters;
use crate::remaining::{opposing_link, other_single_targets, single_monster_target};

pub const BACKUP_SOLDIER: CardId = CardId(36280194);
pub const BURST_BREATH: CardId = CardId(80163754);
pub const CASTLE_WALLS: CardId = CardId(44209392);
pub const DISAPPEAR: CardId = CardId(24623598);
pub const DUST_TORNADO: CardId = CardId(60082869);
pub const ENCHANTED_JAVELIN: CardId = CardId(96355986);
pub const GIFT_OF_THE_MYSTICAL_ELF: CardId = CardId(98299011);
pub const GRYPHON_WING: CardId = CardId(55608151);
pub const JAR_OF_GREED: CardId = CardId(83968380);
pub const JUST_DESSERTS: CardId = CardId(24068492);
pub const RAIGEKI_BREAK: CardId = CardId(4178474);
pub const REINFORCEMENTS: CardId = CardId(17814387);
pub const SHADOW_SPELL: CardId = CardId(29267084);
pub const THE_EYE_OF_TRUTH: CardId = CardId(34694160);
pub const WABOKU: CardId = CardId(12607053);
pub const D_TRIBE: CardId = CardId(2833249);
pub const RIRYOKU_FIELD: CardId = CardId(70344351);
pub const SHIFT: CardId = CardId(59560625);
pub const LIGHT_OF_INTERVENTION: CardId = CardId(62867251);
pub const REVERSE_TRAP: CardId = CardId(77622396);
pub const ULTIMATE_OFFERING: CardId = CardId(80604092);
/// Not in the pool: Gryphon Wing can never be activated here.
pub const HARPIES_FEATHER_DUSTER: CardId = CardId(18144506);

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

fn singles(cards: impl IntoIterator<Item = InstanceId>) -> TargetSets {
    cards.into_iter().map(|c| vec![c]).collect()
}

fn all_monsters(g: &Game) -> Vec<InstanceId> {
    PlayerId::BOTH
        .into_iter()
        .flat_map(|p| g.monsters_controlled(p))
        .collect()
}

fn hand_without(g: &Game, s: Source) -> Vec<InstanceId> {
    g.hand(s.controller)
        .iter()
        .copied()
        .filter(|&c| c != s.card)
        .collect()
}

/// A non-Effect monster: a Normal Monster, or a Fusion or Ritual Monster
/// whose script has no effects.
fn non_effect_monster(g: &Game, c: InstanceId) -> bool {
    match g.stats(c).kind {
        CardKind::Normal => true,
        CardKind::Fusion | CardKind::Ritual => g.script(c).is_none_or(|s| s.effects.is_empty()),
        _ => false,
    }
}

fn backup_options(g: &Game, player: PlayerId, taken: &[InstanceId]) -> Vec<InstanceId> {
    g.graveyard(player)
        .iter()
        .copied()
        .filter(|&c| non_effect_monster(g, c) && g.stats(c).atk <= 1500 && !taken.contains(&c))
        .collect()
}

/// Backup Soldier: picks up to three, one at a time, then adds them.
fn backup_next(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let options = backup_options(g, ctx.controller, &ctx.targets);
    if ctx.targets.len() >= 3 || options.is_empty() {
        for &c in &ctx.targets.clone() {
            if g.instance(c).location == Location::Graveyard {
                g.add_to_hand(c);
            }
        }
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |g, ctx, picked| {
            ctx.targets.push(picked);
            backup_next(g, ctx)
        },
    }
}

fn monsters_in_graveyard(g: &Game, player: PlayerId) -> usize {
    g.graveyard(player)
        .iter()
        .filter(|&&c| g.stats(c).is_monster())
        .count()
}

/// The opponent's attack under way, while its declaration window is open.
fn opposing_attacker(g: &Game, s: Source, w: &Window) -> Option<InstanceId> {
    if w.kind != WindowKind::AttackDeclared {
        return None;
    }
    g.current_attack()
        .map(|(a, _)| a)
        .filter(|&a| g.on_field(a) && g.instance(a).controller != s.controller)
}

fn raigeki_break_targets(g: &Game, s: Source) -> TargetSets {
    singles(
        PlayerId::BOTH
            .into_iter()
            .flat_map(|p| {
                g.monsters_controlled(p)
                    .into_iter()
                    .chain(g.spells_traps_controlled(p))
            })
            .filter(|&c| c != s.card),
    )
}

fn shadow_spell_targets(g: &Game, s: Source) -> TargetSets {
    singles(
        g.monsters_controlled(s.controller.opponent())
            .into_iter()
            .filter(|&m| g.instance(m).position.is_face_up()),
    )
}

/// Gryphon Wing: the opponent's Harpie's Feather Duster on the chain.
fn feather_duster(g: &Game, s: Source) -> Option<InstanceId> {
    g.last_link()
        .filter(|l| {
            l.card_activation
                && !l.negated
                && l.controller != s.controller
                && g.stats(l.card).id == HARPIES_FEATHER_DUSTER
        })
        .map(|l| l.card)
}

/// Dust Tornado: Spells and Traps in the hand that may be Set now.
fn settable(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    let room = g.free_spell_trap_zone(player).is_some();
    g.hand(player)
        .iter()
        .copied()
        .filter(|&c| {
            let s = g.stats(c);
            !s.is_monster() && (room || s.subtype == ygo_engine::SpellTrapType::Field)
        })
        .collect()
}

/// Riryoku Field: the activation of a Spell Card that targets exactly 1
/// monster on the field, still waiting to resolve.
fn single_target_spell(g: &Game) -> Option<InstanceId> {
    g.last_link()
        .filter(|l| {
            !l.negated
                && l.card_activation
                && g.stats(l.card).kind == CardKind::Spell
                && single_monster_target(g, l).is_some()
        })
        .map(|l| l.card)
}

/// Shift: (the Spell or Trap, a new target) when the opponent's Spell or
/// Trap targets exactly 1 monster `s`'s controller controls; or the new
/// target alone when the opponent's attack does, with no chain open.
fn shift_targets(g: &Game, s: Source) -> TargetSets {
    let mine = |t: InstanceId| g.instance(t).controller == s.controller;
    if let Some(link) = opposing_link(g, s) {
        let spell_or_trap = matches!(g.stats(link.card).kind, CardKind::Spell | CardKind::Trap);
        if !spell_or_trap || !single_monster_target(g, &link).is_some_and(mine) {
            return Vec::new();
        }
        return other_single_targets(g, &link)
            .into_iter()
            .filter(|&t| mine(t) && g.on_field(t))
            .map(|t| vec![link.card, t])
            .collect();
    }
    if g.last_link().is_some() {
        return Vec::new();
    }
    match g.current_attack() {
        Some((attacker, Some(old))) if !mine(attacker) && mine(old) => g
            .attack_targets(attacker)
            .into_iter()
            .filter(|&t| t != old && mine(t))
            .map(|t| vec![t])
            .collect(),
        _ => Vec::new(),
    }
}

/// Ultimate Offering: monsters in the hand `player` could Normal Summon or
/// Set now with their own monsters as Tributes (Sheep Tokens excluded).
fn offering_monsters(g: &Game, player: PlayerId) -> Vec<InstanceId> {
    let tributes = offering_tributes(g, player, &[]).len();
    let free = g.free_monster_zone(player).is_some();
    g.hand(player)
        .iter()
        .copied()
        .filter(|&c| {
            let stats = g.stats(c);
            let needed = g.tributes_needed(stats.level);
            stats.can_normal_summon(g.script(c)) && needed <= tributes && (free || needed > 0)
        })
        .collect()
}

fn offering_tributes(g: &Game, player: PlayerId, chosen: &[InstanceId]) -> Vec<InstanceId> {
    g.monsters_controlled(player)
        .into_iter()
        .filter(|m| !chosen.contains(m))
        .filter(|&m| !g.script(m).is_some_and(|s| s.has(Tag::NoTributeSummon)))
        .collect()
}

/// Ultimate Offering's summon: `ctx.cost` holds the monster, then its
/// Tributes as they are chosen; then Summon or Set.
fn offering_next(g: &mut Game, ctx: &mut EffectCtx) -> Flow {
    let card = ctx.cost[0];
    let needed = g.tributes_needed(g.stats(card).level);
    if ctx.cost.len() - 1 < needed {
        let options = offering_tributes(g, ctx.controller, &ctx.cost[1..]);
        if options.is_empty() {
            return Flow::Done;
        }
        return Flow::Choose {
            options,
            then: |g, ctx, t| {
                ctx.cost.push(t);
                offering_next(g, ctx)
            },
        };
    }
    let mut options = vec!["Set it (face-down Defense Position)"];
    if !g.summons_locked(ctx.controller) {
        options.insert(0, "Normal Summon it (face-up Attack Position)");
    }
    Flow::Ask {
        options,
        then: |g, ctx, option| {
            let set = option == 1 || g.summons_locked(ctx.controller);
            let (card, tributes) = (ctx.cost[0], ctx.cost[1..].to_vec());
            g.normal_summon_by_effect(card, &tributes, set);
            Flow::Done
        },
    }
}

pub fn register(r: &mut Registry) {
    // Continuous Trap. During your Main Phase or your opponent's Battle
    // Phase: pay 500 LP; right after this effect resolves, Normal Summon or
    // Set 1 monster (an extra one).
    r.register(
        ULTIMATE_OFFERING,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect::activate(2, ygo_engine::nothing),
                Effect {
                    kind: EffectKind::Quick,
                    speed: 2,
                    timing: |g, s, _| {
                        let mine = g.turn_player() == s.controller;
                        let phase_ok = match g.phase() {
                            Phase::Main1 | Phase::Main2 => mine,
                            Phase::Battle(_) => !mine,
                            _ => false,
                        };
                        phase_ok
                            && g.on_field(s.card)
                            && g.instance(s.card).position.is_face_up()
                            && g.can_pay_lp(s.controller, 500)
                            && !offering_monsters(g, s.controller).is_empty()
                    },
                    cost: Some(|g, ctx| g.pay_lp(ctx.controller, 500)),
                    ..Effect::ignition(|g, ctx| {
                        let options = offering_monsters(g, ctx.controller);
                        if options.is_empty() {
                            return Flow::Done;
                        }
                        Flow::Choose {
                            options,
                            then: |g, ctx, card| {
                                ctx.cost = vec![card];
                                offering_next(g, ctx)
                            },
                        }
                    })
                },
            ],
        },
    );
    // Until the End Phase, every ATK/DEF addition subtracts and every
    // subtraction adds (multiplications and set values are untouched:
    // `Tag::StatsSetOrScaled`).
    r.register(
        REVERSE_TRAP,
        single(Effect {
            timing: |g, _, _| !all_monsters(g).is_empty(),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                let turn = g.turn();
                g.add_lingering(Lingering {
                    source: ctx.source,
                    kind: LingeringKind::StatChangesReversed,
                    expires_turn: turn,
                });
                Flow::Done
            })
        }),
    );
    // Continuous Trap: no monster can be Set or turned face-down; a Set is
    // a Normal Summon in face-up Defense Position.
    r.register(
        LIGHT_OF_INTERVENTION,
        CardScript {
            tags: vec![Tag::NoFaceDownMonsters],
            effects: vec![Effect::activate(2, ygo_engine::nothing)],
        },
    );
    // All face-up monsters you control at resolution become Dragon until
    // the end of this turn.
    r.register(
        D_TRIBE,
        single(Effect {
            timing: |g, s, _| {
                g.monsters_controlled(s.controller)
                    .into_iter()
                    .any(|m| g.instance(m).position.is_face_up())
            },
            race: Some(|_, _, _| Some("Dragon")),
            ..Effect::activate(2, |g, ctx| {
                for m in g.monsters_controlled(ctx.controller) {
                    if g.instance(m).position.is_face_up() {
                        g.change_race_until_end_phase(ctx.source, m);
                    }
                }
                Flow::Done
            })
        }),
    );
    // Counter Trap: when a Spell Card that targets exactly 1 monster on the
    // field is activated, negate it and destroy it.
    r.register(
        RIRYOKU_FIELD,
        single(Effect {
            timing: |g, _, _| single_target_spell(g).is_some(),
            targets: Some(|g, _| {
                single_target_spell(g)
                    .map(|c| vec![c])
                    .into_iter()
                    .collect()
            }),
            ..Effect::activate(3, |g, ctx| {
                g.negate_activation(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    // When the opponent targets exactly 1 monster you control for an
    // attack or with a Spell/Trap: move it to another monster you control
    // that would be a legal target.
    r.register(
        SHIFT,
        single(Effect {
            timing: |g, s, w| {
                (w.kind == WindowKind::AttackDeclared || g.last_link().is_some())
                    && !shift_targets(g, s).is_empty()
            },
            targets: Some(shift_targets),
            ..Effect::activate(2, |g, ctx| {
                if let &[card, new] = ctx.targets.as_slice() {
                    if g.on_field(new) {
                        g.retarget(card, vec![new]);
                    }
                } else {
                    let new = ctx.targets[0];
                    let legal = g
                        .current_attack()
                        .is_some_and(|(a, _)| g.attack_targets(a).contains(&new));
                    if legal && g.instance(new).controller == ctx.controller {
                        g.retarget_attack(new);
                    }
                }
                Flow::Done
            })
        }),
    );
    // While you have 5 or more monsters in your Graveyard: up to 3
    // non-Effect Monsters with 1500 or less ATK from your Graveyard to
    // your hand. Simplified like Soul Release: the cards are picked on
    // resolution (as many as possible, up to 3), not targeted.
    r.register(
        BACKUP_SOLDIER,
        single(Effect {
            timing: |g, s, _| {
                monsters_in_graveyard(g, s.controller) >= 5
                    && !backup_options(g, s.controller, &[]).is_empty()
            },
            ..Effect::activate(2, |g, ctx| {
                ctx.targets.clear();
                backup_next(g, ctx)
            })
        }),
    );
    // Tribute 1 Dragon; destroy all face-up monsters whose DEF is at most
    // the ATK the Tributed monster had on the field.
    r.register(
        BURST_BREATH,
        single(Effect {
            timing: |g, s, _| {
                g.monsters_controlled(s.controller)
                    .into_iter()
                    .any(|m| g.race(m) == "Dragon")
            },
            cost_choices: Some(|g, s| {
                singles(
                    g.monsters_controlled(s.controller)
                        .into_iter()
                        .filter(|&m| g.race(m) == "Dragon"),
                )
            }),
            cost: Some(|g, ctx| {
                ctx.value = g.atk(ctx.cost[0]);
                g.tribute(ctx.cost[0]);
            }),
            ..Effect::activate(2, |g, ctx| {
                for m in all_monsters(g) {
                    if g.instance(m).position.is_face_up() && g.def(m) <= ctx.value {
                        g.destroy(m, ctx.source);
                    }
                }
                Flow::Done
            })
        }),
    );
    // A face-up monster gains 500 DEF until the end of this turn.
    r.register(
        CASTLE_WALLS,
        single(Effect {
            timing: |g, _, _| !face_up_monsters(g, |_, _| true).is_empty(),
            targets: Some(|g, _| face_up_monsters(g, |_, _| true)),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                if g.on_field(ctx.targets[0]) {
                    g.modify_until_end_phase(ctx.source, ctx.targets[0], 0, 500);
                }
                Flow::Done
            })
        }),
    );
    // Banish 1 card from your opponent's Graveyard.
    r.register(
        DISAPPEAR,
        single(Effect {
            timing: |g, s, _| !g.graveyard(s.controller.opponent()).is_empty(),
            targets: Some(|g, s| singles(g.graveyard(s.controller.opponent()).iter().copied())),
            ..Effect::activate(2, |g, ctx| {
                if g.instance(ctx.targets[0]).location == Location::Graveyard {
                    g.banish(ctx.targets[0]);
                }
                Flow::Done
            })
        }),
    );
    // Target 1 Spell/Trap your opponent controls; destroy it, then you
    // can Set 1 Spell/Trap from your hand.
    r.register(
        DUST_TORNADO,
        single(Effect {
            timing: |g, s, _| {
                !g.spells_traps_controlled(s.controller.opponent())
                    .is_empty()
            },
            targets: Some(|g, s| singles(g.spells_traps_controlled(s.controller.opponent()))),
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                if settable(g, ctx.controller).is_empty() {
                    return Flow::Done;
                }
                Flow::Ask {
                    options: vec!["Set a Spell or Trap from your hand", "Set nothing"],
                    then: |g, ctx, option| {
                        let options = settable(g, ctx.controller);
                        if option != 0 || options.is_empty() {
                            return Flow::Done;
                        }
                        Flow::Choose {
                            options,
                            then: |g, ctx, picked| {
                                g.set_from_hand(ctx.controller, picked);
                                Flow::Done
                            },
                        }
                    },
                }
            })
        }),
    );
    // Select 1 attacking monster; gain LP equal to its ATK.
    r.register(
        ENCHANTED_JAVELIN,
        single(Effect {
            timing: |g, s, w| opposing_attacker(g, s, w).is_some(),
            targets: Some(|g, s| {
                let w = g.window().map(|(kind, events)| Window { kind, events });
                w.and_then(|w| opposing_attacker(g, s, &w))
                    .map(|a| vec![a])
                    .into_iter()
                    .collect()
            }),
            ..Effect::activate(2, |g, ctx| {
                let attacker = ctx.targets[0];
                if g.on_field(attacker) {
                    let atk = g.atk(attacker).max(0) as u32;
                    g.gain_lp(ctx.controller, atk);
                }
                Flow::Done
            })
        }),
    );
    // Gain 300 LP for each monster on the field.
    r.register(
        GIFT_OF_THE_MYSTICAL_ELF,
        single(Effect::activate(2, |g, ctx| {
            let n = all_monsters(g).len() as u32;
            g.gain_lp(ctx.controller, 300 * n);
            Flow::Done
        })),
    );
    // When your opponent activates Harpie's Feather Duster: negate it, and
    // if you do, destroy all their Spells and Traps. Harpie's Feather
    // Duster is not in this pool.
    r.register(
        GRYPHON_WING,
        single(Effect {
            timing: |g, s, _| feather_duster(g, s).is_some(),
            targets: Some(|g, s| feather_duster(g, s).map(|c| vec![c]).into_iter().collect()),
            ..Effect::activate(2, |g, ctx| {
                if g.negate_activation(ctx.targets[0], ctx.source) {
                    for c in g.spells_traps_controlled(ctx.controller.opponent()) {
                        g.destroy(c, ctx.source);
                    }
                }
                Flow::Done
            })
        }),
    );
    r.register(
        JAR_OF_GREED,
        single(Effect {
            timing: |g, s, _| g.deck_size(s.controller) >= 1,
            ..Effect::activate(2, |g, ctx| {
                g.draw(ctx.controller, 1);
                Flow::Done
            })
        }),
    );
    // 500 damage for each monster your opponent controls.
    r.register(
        JUST_DESSERTS,
        single(Effect {
            timing: |g, s, _| !g.monsters_controlled(s.controller.opponent()).is_empty(),
            ..Effect::activate(2, |g, ctx| {
                let n = g.monsters_controlled(ctx.controller.opponent()).len() as u32;
                g.take_damage(ctx.controller.opponent(), 500 * n);
                Flow::Done
            })
        }),
    );
    // Discard 1 card, then target 1 card on the field; destroy it.
    r.register(
        RAIGEKI_BREAK,
        single(Effect {
            timing: |g, s, _| {
                !hand_without(g, s).is_empty() && !raigeki_break_targets(g, s).is_empty()
            },
            cost_choices: Some(|g, s| singles(hand_without(g, s))),
            cost: Some(|g, ctx| g.discard(ctx.cost[0])),
            targets: Some(raigeki_break_targets),
            ..Effect::activate(2, |g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    // A face-up monster gains 500 ATK until the end of this turn.
    r.register(
        REINFORCEMENTS,
        single(Effect {
            timing: |g, _, _| !face_up_monsters(g, |_, _| true).is_empty(),
            targets: Some(|g, _| face_up_monsters(g, |_, _| true)),
            damage_step: true,
            ..Effect::activate(2, |g, ctx| {
                if g.on_field(ctx.targets[0]) {
                    g.modify_until_end_phase(ctx.source, ctx.targets[0], 500, 0);
                }
                Flow::Done
            })
        }),
    );
    // Target 1 face-up monster your opponent controls; it loses 700 ATK,
    // and cannot attack or change its battle position (an attack it has
    // declared stops). When it leaves the field, this card goes with it,
    // as an Equip would.
    r.register(
        SHADOW_SPELL,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    targets: Some(shadow_spell_targets),
                    timing: |g, s, _| !shadow_spell_targets(g, s).is_empty(),
                    ..Effect::activate(2, |g, ctx| {
                        let target = ctx.targets[0];
                        if g.instance(target).position.is_face_up() && g.attach(ctx.source, target)
                        {
                            g.cancel_attack(target);
                        } else if g.on_field(ctx.source) {
                            g.destroy(ctx.source, ctx.source);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    modify: Some(|g, s, t| {
                        if g.equip_target(s.card) == Some(t) {
                            StatMod { atk: -700, def: 0 }
                        } else {
                            StatMod::default()
                        }
                    }),
                    restrict: Some(|g, s, t| g.equip_target(s.card) == Some(t)),
                    lock_position: Some(|g, s, t| g.equip_target(s.card) == Some(t)),
                    ..Effect::continuous()
                },
            ],
        },
    );
    // Your opponent keeps their hand revealed. In their Standby Phase, if
    // they have a Spell in their hand, they gain 1000 LP.
    r.register(
        THE_EYE_OF_TRUTH,
        CardScript {
            tags: vec![Tag::RevealOpponentHand],
            effects: vec![
                Effect::activate(2, ygo_engine::nothing),
                Effect {
                    timing: |g, s, w| {
                        let opponent = s.controller.opponent();
                        w.kind == WindowKind::Open
                            && g.phase() == Phase::Standby
                            && g.turn_player() == opponent
                            && w.events.contains(&Event::PhaseEntered {
                                phase: Phase::Standby,
                            })
                            && g.on_field(s.card)
                            && g.instance(s.card).position.is_face_up()
                            && spell_in_hand(g, opponent)
                    },
                    ..Effect::trigger(true, |g, ctx| {
                        let opponent = ctx.controller.opponent();
                        if spell_in_hand(g, opponent) {
                            g.gain_lp(opponent, 1000);
                        }
                        Flow::Done
                    })
                },
            ],
        },
    );
    // You take no battle damage from your opponent's monsters this turn,
    // and your monsters cannot be destroyed by battle this turn.
    r.register(
        WABOKU,
        single(Effect::activate(2, |g, ctx| {
            g.shield_from_battle_this_turn(ctx.source, ctx.controller);
            Flow::Done
        })),
    );
}

fn spell_in_hand(g: &Game, player: PlayerId) -> bool {
    g.hand(player)
        .iter()
        .any(|&c| g.stats(c).kind == CardKind::Spell)
}
