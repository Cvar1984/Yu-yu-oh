//! Toon World and the four Toon monsters, by the modern text.

use ygo_data::CardId;
use ygo_engine::{
    CardScript, Effect, Event, Flow, Game, InstanceId, Location, Position, Registry, Source, Tag,
    TargetSets,
};

pub const TOON_WORLD: CardId = CardId(15259703);
pub const TOON_MERMAID: CardId = CardId(65458948);
pub const TOON_SUMMONED_SKULL: CardId = CardId(91842653);
pub const BLUE_EYES_TOON_DRAGON: CardId = CardId(53183600);
pub const MANGA_RYU_RAN: CardId = CardId(38369349);
pub const TOONS: [CardId; 4] = [
    TOON_MERMAID,
    TOON_SUMMONED_SKULL,
    BLUE_EYES_TOON_DRAGON,
    MANGA_RYU_RAN,
];

fn controls_toon_world(g: &Game, player: ygo_engine::PlayerId) -> bool {
    g.spells_traps_controlled(player)
        .into_iter()
        .any(|c| g.stats(c).id == TOON_WORLD && g.instance(c).position.is_face_up())
}

/// Every set of the monsters to tribute; the count follows the summon
/// rules for the Toon's Level (1 for Level 5-6, 2 for Level 7+ by default).
fn tribute_sets(g: &Game, s: Source) -> TargetSets {
    let needed = g.tributes_needed(g.stats(s.card).level);
    let mine = g.monsters_controlled(s.controller);
    if needed == 0 {
        return vec![Vec::new()];
    }
    let mut sets = Vec::new();
    for i in 0..mine.len() {
        if needed == 1 {
            sets.push(vec![mine[i]]);
            continue;
        }
        for j in i + 1..mine.len() {
            sets.push(vec![mine[i], mine[j]]);
        }
    }
    sets
}

fn toon() -> CardScript {
    CardScript {
        tags: vec![Tag::CannotNormalSummon, Tag::Toon],
        effects: vec![
            // Special Summon from the hand while controlling Toon World.
            Effect {
                timing: |g, s, _| {
                    g.instance(s.card).location == Location::Hand
                        && controls_toon_world(g, s.controller)
                        && g.can_special_summon(s.controller)
                        && !tribute_sets(g, s).is_empty()
                },
                cost_choices: Some(tribute_sets),
                cost: Some(|g, ctx| {
                    for &t in &ctx.cost.clone() {
                        g.tribute(t);
                    }
                }),
                ..Effect::ignition(|g, ctx| {
                    g.special_summon(ctx.source, ctx.controller, Position::FaceUpAttack);
                    Flow::Done
                })
            },
            // No attack the turn it arrives; 500 LP to declare an attack.
            Effect {
                restrict: Some(|g, s, t| {
                    t == s.card
                        && (g.instance(s.card).turn_entered_field == Some(g.turn())
                            || !g.can_pay_lp(s.controller, 500))
                }),
                on_attack: Some(|g, s, attacker| {
                    if attacker == s.card {
                        g.pay_lp(s.controller, 500);
                    }
                }),
                ..Effect::continuous()
            },
            // Destroyed when a Toon World on the field is destroyed.
            Effect {
                timing: |g, s, w| {
                    g.on_field(s.card)
                        && w.events.iter().any(|e| {
                            matches!(e,
                            Event::SentToGraveyard { card, from: Location::SpellTrapZone(_) }
                                if g.stats(*card).id == TOON_WORLD)
                        })
                },
                ..Effect::trigger(true, |g, ctx| {
                    g.destroy(ctx.source, ctx.source);
                    Flow::Done
                })
            },
        ],
    }
}

pub fn register(r: &mut Registry) {
    r.register(
        TOON_WORLD,
        CardScript {
            tags: Vec::new(),
            effects: vec![Effect {
                timing: |g, s, _| g.can_pay_lp(s.controller, 1000),
                cost: Some(|g, ctx| g.pay_lp(ctx.controller, 1000)),
                ..Effect::activate(1, ygo_engine::nothing)
            }],
        },
    );
    for id in TOONS {
        r.register(id, toon());
    }
}

#[allow(dead_code)]
fn _unused(_: InstanceId) {}
