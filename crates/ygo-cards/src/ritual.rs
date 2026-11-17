//! The four Ritual Spells, their Ritual Monsters, and Relinquished.

use ygo_data::CardId;
use ygo_engine::{
    CardScript, Effect, Flow, Game, InstanceId, Location, Position, Registry, Source, StatMod,
    TargetSets,
};

pub const BLACK_ILLUSION_RITUAL: CardId = CardId(41426869);
pub const COMMENCEMENT_DANCE: CardId = CardId(43417563);
pub const HAMBURGER_RECIPE: CardId = CardId(80811661);
pub const TURTLE_OATH: CardId = CardId(76806714);
pub const RELINQUISHED: CardId = CardId(64631466);
pub const PERFORMANCE_OF_SWORD: CardId = CardId(4849037);
pub const HUNGRY_BURGER: CardId = CardId(30243636);
pub const CRAB_TURTLE: CardId = CardId(91782219);

/// (Ritual Spell, the monster it summons, the Levels the tributes must reach).
pub const RITUALS: [(CardId, CardId, u8); 4] = [
    (BLACK_ILLUSION_RITUAL, RELINQUISHED, 1),
    (COMMENCEMENT_DANCE, PERFORMANCE_OF_SWORD, 6),
    (HAMBURGER_RECIPE, HUNGRY_BURGER, 6),
    (TURTLE_OATH, CRAB_TURTLE, 8),
];

fn ritual_row(g: &Game, s: Source) -> (CardId, u8) {
    let id = g.stats(s.card).id;
    let (_, monster, levels) = RITUALS
        .iter()
        .find(|(spell, _, _)| *spell == id)
        .expect("a Ritual row");
    (*monster, *levels)
}

/// Every legal summon: the Ritual Monster from the hand first, then a set
/// of up to three tributes from the hand or field whose Levels reach the
/// requirement.
fn ritual_targets(g: &Game, s: Source) -> TargetSets {
    let (monster_id, levels) = ritual_row(g, s);
    let player = s.controller;
    let monsters: Vec<InstanceId> = g
        .hand(player)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).id == monster_id)
        .collect();
    let pool: Vec<InstanceId> = g
        .hand(player)
        .iter()
        .copied()
        .filter(|&c| c != s.card && g.stats(c).is_monster())
        .chain(g.monsters_controlled(player))
        .collect();
    let mut sets = Vec::new();
    for &monster in &monsters {
        let candidates: Vec<InstanceId> = pool.iter().copied().filter(|&c| c != monster).collect();
        for k in 1..=3.min(candidates.len()) {
            for tributes in subsets(&candidates, k) {
                let total: u32 = tributes.iter().map(|&t| u32::from(g.stats(t).level)).sum();
                if total >= u32::from(levels) {
                    let mut set = vec![monster];
                    set.extend(tributes);
                    sets.push(set);
                }
            }
        }
    }
    sets
}

fn subsets(items: &[InstanceId], k: usize) -> TargetSets {
    fn go(
        items: &[InstanceId],
        k: usize,
        start: usize,
        cur: &mut Vec<InstanceId>,
        out: &mut TargetSets,
    ) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..items.len() {
            cur.push(items[i]);
            go(items, k, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(items, k, 0, &mut Vec::new(), &mut out);
    out
}

/// The monster Relinquished has absorbed, if any.
fn absorbed(g: &Game, relinquished: InstanceId) -> Option<InstanceId> {
    let holder = g.instance(relinquished).controller;
    g.spells_traps_controlled(holder)
        .into_iter()
        .find(|&c| g.equip_target(c) == Some(relinquished) && g.stats(c).is_monster())
}

pub fn register(r: &mut Registry) {
    for (spell, _, _) in RITUALS {
        r.register(
            spell,
            CardScript {
                tags: Vec::new(),
                effects: vec![Effect {
                    timing: |g, s, _| {
                        g.ritual_summons_allowed()
                            && g.can_special_summon(s.controller)
                            && !ritual_targets(g, s).is_empty()
                    },
                    targets: Some(ritual_targets),
                    ..Effect::activate(1, |g, ctx| {
                        let monster = ctx.targets[0];
                        let tributes: Vec<InstanceId> = ctx.targets[1..].to_vec();
                        let still_there = g.instance(monster).location == Location::Hand
                            && tributes.iter().all(|&t| {
                                g.instance(t).location == Location::Hand || g.on_field(t)
                            });
                        if still_there {
                            g.ritual_summon(
                                ctx.controller,
                                monster,
                                &tributes,
                                Position::FaceUpAttack,
                            );
                        }
                        Flow::Done
                    })
                }],
            },
        );
    }
    for id in [PERFORMANCE_OF_SWORD, HUNGRY_BURGER, CRAB_TURTLE] {
        r.register(
            id,
            CardScript {
                tags: Vec::new(),
                effects: Vec::new(),
            },
        );
    }
    // Relinquished: once per turn, absorb one opposing monster as an Equip;
    // its stats become the absorbed monster's; the absorbed monster is
    // destroyed in its place by battle; battle damage its controller takes
    // from its battles is dealt to the opponent too.
    r.register(
        RELINQUISHED,
        CardScript {
            tags: Vec::new(),
            effects: vec![
                Effect {
                    once_per_turn: true,
                    timing: |g, s, _| {
                        g.on_field(s.card)
                            && absorbed(g, s.card).is_none()
                            && g.free_spell_trap_zone(s.controller).is_some()
                            && !g.monsters_controlled(s.controller.opponent()).is_empty()
                    },
                    targets: Some(|g, s| {
                        g.monsters_controlled(s.controller.opponent())
                            .into_iter()
                            .map(|m| vec![m])
                            .collect()
                    }),
                    ..Effect::ignition(|g, ctx| {
                        let target = ctx.targets[0];
                        if g.on_field(target) && g.on_field(ctx.source) {
                            g.absorb_as_equip(target, ctx.controller, ctx.source);
                        }
                        Flow::Done
                    })
                },
                Effect {
                    modify: Some(|g, s, t| {
                        if t != s.card {
                            return StatMod::default();
                        }
                        match absorbed(g, s.card) {
                            Some(a) => StatMod {
                                atk: g.stats(a).atk - g.stats(s.card).atk,
                                def: g.stats(a).def - g.stats(s.card).def,
                            },
                            None => StatMod::default(),
                        }
                    }),
                    replace_battle_destruction: Some(|g, s, card| {
                        if card == s.card {
                            absorbed(g, s.card)
                        } else {
                            None
                        }
                    }),
                    on_battle_damage: Some(|g, s, d| {
                        let involved = d.attacker == s.card || d.target == Some(s.card);
                        if involved && d.damaged == s.controller && absorbed(g, s.card).is_some() {
                            g.take_damage(s.controller.opponent(), d.amount);
                        }
                    }),
                    ..Effect::continuous()
                },
            ],
        },
    );
}
