//! Polymerization and the 26 Fusion Monsters. A Fusion Monster's script
//! is its material list; Polymerization reads it to find legal summons.

use ygo_data::CardId;

use crate::positions;
use ygo_engine::{CardScript, Effect, Flow, Game, InstanceId, Registry, Source, TargetSets};

pub const POLYMERIZATION: CardId = CardId(24094653);

/// (Fusion Monster, its materials by card id). Rabid Horseman and Cyber
/// Saurus need a material outside these three sets, so they can never be
/// summoned from this pool; their rows stay complete anyway.
pub const FUSIONS: [(CardId, &[CardId]); 26] = [
    (CardId(45231177), &[CardId(34460851), CardId(44287299)]), // Flame Swordsman
    (CardId(37421579), &[CardId(36121917), CardId(96851799)]), // Charubin the Fire Knight
    (CardId(17881964), &[CardId(53293545), CardId(75356564)]), // Darkfire Dragon
    (CardId(1641882), &[CardId(38142739), CardId(83464209)]),  // Fusionist
    (CardId(58528964), &[CardId(32274490), CardId(40826495)]), // Flame Ghost
    (CardId(54541900), &[CardId(56342351), CardId(92731455)]), // Karbonala Warrior
    (CardId(70681994), &[CardId(53153481), CardId(33064647)]), // Dragoness the Wicked Knight
    (CardId(9293977), &[CardId(29172562), CardId(55444629)]),  // Metal Dragon
    (CardId(95952802), &[CardId(90357090), CardId(43500484)]), // Flower Wolf
    (CardId(66889139), &[CardId(6368038), CardId(28279543)]),  // Gaia the Dragon Champion
    (CardId(11901678), &[CardId(70781052), CardId(74677422)]), // Black Skull Dragon
    (CardId(19066538), &[CardId(98049915), CardId(2118022)]),  // Roaring Ocean Snake
    (CardId(28593363), &[CardId(81386177), CardId(69572024)]), // Deepsea Shark
    (CardId(9653271), &[CardId(86088138), CardId(21817254)]),  // Kaminari Attack
    (CardId(15237615), &[CardId(5901497), CardId(64501875)]),  // Empress Judge
    (CardId(94905343), &[CardId(5053103), CardId(68516705)]), // Rabid Horseman (Battle Ox is not in the pool)
    (CardId(99551425), &[CardId(8471389), CardId(11384280)]), // Labyrinth Tank
    (CardId(25655502), &[CardId(93889755), CardId(13215230)]), // Bickuribox
    (CardId(51828629), &[CardId(89272878), CardId(10071456)]), // Giltia the D. Knight
    (CardId(74703140), &[CardId(41396436), CardId(7805359)]), // Punished Eagle
    (CardId(56907389), &[CardId(78010363), CardId(17358176)]), // Musician King
    (CardId(89112729), &[CardId(70138455), CardId(94119974)]), // Cyber Saurus (Two-Headed King Rex is not in the pool)
    (CardId(54752875), &[CardId(31786629), CardId(31786629)]), // Twin-Headed Thunder Dragon
    (CardId(2504891), &[CardId(28725004), CardId(42431843)]),  // Skull Knight
    (CardId(41462083), &[CardId(71625222), CardId(88819587)]), // Thousand Dragon
    (CardId(63519819), &[CardId(64631466), CardId(27125110)]), // Thousand-Eyes Restrict (PSV; its effects are in psv_monsters)
];

pub fn materials_of(fusion: CardId) -> Option<&'static [CardId]> {
    FUSIONS
        .iter()
        .find(|(id, _)| *id == fusion)
        .map(|(_, m)| *m)
}

/// Every legal Fusion Summon for `s.controller`: the Fusion Monster in the
/// Extra Deck first, then one instance per required material from the
/// hand or field, distinct instances.
fn polymerization_targets(g: &Game, s: Source) -> TargetSets {
    let player = s.controller;
    let pool: Vec<InstanceId> = g
        .hand(player)
        .iter()
        .copied()
        .filter(|&c| c != s.card)
        .chain(g.monsters_controlled(player))
        .collect();
    let mut sets = Vec::new();
    for &fusion in g.player(player).extra.iter() {
        let Some(required) = materials_of(g.stats(fusion).id) else {
            continue;
        };
        assign(g, &pool, required, &mut vec![fusion], &mut sets);
    }
    sets
}

fn assign(
    g: &Game,
    pool: &[InstanceId],
    required: &[CardId],
    chosen: &mut Vec<InstanceId>,
    out: &mut TargetSets,
) {
    let Some(&need) = required.first() else {
        out.push(chosen.clone());
        return;
    };
    for &candidate in pool {
        if g.stats(candidate).id == need && !chosen.contains(&candidate) {
            chosen.push(candidate);
            assign(g, pool, &required[1..], chosen, out);
            chosen.pop();
        }
    }
}

pub fn register(r: &mut Registry) {
    for (id, _) in FUSIONS {
        if id == crate::psv_monsters::THOUSAND_EYES_RESTRICT {
            continue;
        }
        r.register(
            id,
            CardScript {
                tags: Vec::new(),
                effects: Vec::new(),
            },
        );
    }
    // Fusion Summon 1 Fusion Monster from the Extra Deck, using monsters
    // from the hand or field: the Fusion Monster, then each material, are
    // chosen on resolution; then its position.
    r.register(
        POLYMERIZATION,
        CardScript {
            tags: Vec::new(),
            effects: vec![Effect {
                timing: |g, s, _| {
                    g.fusion_summons_allowed()
                        && polymerization_targets(g, s)
                            .iter()
                            .any(|set| g.room_after_leaving(s.controller, &set[1..]))
                },
                ..Effect::activate(1, |g, ctx| {
                    ctx.targets.clear();
                    poly_step(g, ctx)
                })
            }],
        },
    );
}

/// Monsters in the hand or on the field that may serve as materials.
fn material_pool(g: &Game, player: ygo_engine::PlayerId, poly: InstanceId) -> Vec<InstanceId> {
    g.hand(player)
        .iter()
        .copied()
        .filter(|&c| c != poly)
        .chain(g.monsters_controlled(player))
        .collect()
}

/// Whether `chosen` (the Fusion Monster, then the materials so far) can be
/// completed from `pool`.
fn completes(g: &Game, pool: &[InstanceId], chosen: &[InstanceId]) -> bool {
    let Some(required) = materials_of(g.stats(chosen[0]).id) else {
        return false;
    };
    let mut out = Vec::new();
    assign(
        g,
        pool,
        &required[chosen.len() - 1..],
        &mut chosen.to_vec(),
        &mut out,
    );
    out.iter()
        .any(|set| g.room_after_leaving(g.instance(chosen[0]).owner, &set[1..]))
}

/// One step of Polymerization's resolution; `ctx.targets` holds the Fusion
/// Monster and the materials picked so far.
fn poly_step(g: &mut Game, ctx: &mut ygo_engine::EffectCtx) -> Flow {
    let pool = material_pool(g, ctx.controller, ctx.source);
    if ctx.targets.is_empty() {
        let fusions: Vec<InstanceId> = g
            .player(ctx.controller)
            .extra
            .iter()
            .copied()
            .filter(|&f| completes(g, &pool, &[f]))
            .collect();
        if fusions.is_empty() {
            return Flow::Done;
        }
        return Flow::Choose {
            options: fusions,
            then: |g, ctx, picked| {
                ctx.targets.push(picked);
                poly_step(g, ctx)
            },
        };
    }
    let required = materials_of(g.stats(ctx.targets[0]).id).unwrap_or(&[]);
    let picked = ctx.targets.len() - 1;
    if picked == required.len() {
        return Flow::Ask {
            options: positions::FACE_UP.to_vec(),
            then: |g, ctx, option| {
                let (fusion, materials) = (ctx.targets[0], ctx.targets[1..].to_vec());
                g.fusion_summon(
                    ctx.controller,
                    fusion,
                    &materials,
                    positions::face_up(option),
                );
                Flow::Done
            },
        };
    }
    let need = required[picked];
    let options: Vec<InstanceId> = pool
        .iter()
        .copied()
        .filter(|&c| g.stats(c).id == need && !ctx.targets.contains(&c))
        .filter(|&c| {
            let mut next = ctx.targets.clone();
            next.push(c);
            completes(g, &pool, &next)
        })
        .collect();
    if options.is_empty() {
        return Flow::Done;
    }
    Flow::Choose {
        options,
        then: |g, ctx, picked| {
            ctx.targets.push(picked);
            poly_step(g, ctx)
        },
    }
}
