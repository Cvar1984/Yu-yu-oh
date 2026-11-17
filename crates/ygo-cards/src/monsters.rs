//! Monster scripts: summon restrictions, Graveyard triggers, FLIP effects,
//! quick effects and the Exodia win.

use ygo_data::CardId;
use ygo_engine::{
    CardScript, Effect, EffectKind, Event, Flow, Game, InstanceId, Location, Registry, Source, Tag,
    TargetSets, Window, WindowKind,
};

pub const SANGAN: CardId = CardId(26202165);
pub const WITCH_OF_THE_BLACK_FOREST: CardId = CardId(78010363);
pub const MAN_EATER_BUG: CardId = CardId(54652250);
pub const MAGICIAN_OF_FAITH: CardId = CardId(31560081);
pub const KURIBOH: CardId = CardId(40640057);
pub const EXODIA_THE_FORBIDDEN_ONE: CardId = CardId(33396948);
pub const EXODIA_LIMBS: [CardId; 4] = [
    CardId(70903634), // Right Arm of the Forbidden One
    CardId(7902349),  // Left Arm of the Forbidden One
    CardId(8124921),  // Right Leg of the Forbidden One
    CardId(44519536), // Left Leg of the Forbidden One
];

fn single(effect: Effect) -> CardScript {
    CardScript {
        tags: Vec::new(),
        effects: vec![effect],
    }
}

/// The source went from the field to the Graveyard in this window.
fn sent_from_field_this_window(_: &Game, s: Source, w: &Window) -> bool {
    w.events.iter().any(|e| {
        matches!(e,
        Event::SentToGraveyard { card, from: Location::MonsterZone(_) } if *card == s.card)
    })
}

/// Search the Deck for a monster passing `keep`; the controller picks,
/// the card is added to the hand and the Deck is shuffled.
fn search_deck(
    g: &mut Game,
    controller: ygo_engine::PlayerId,
    keep: fn(&Game, InstanceId) -> bool,
    then: fn(&mut Game, &mut ygo_engine::EffectCtx, InstanceId) -> Flow,
) -> Flow {
    let options: Vec<InstanceId> = g
        .deck(controller)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).is_monster() && keep(g, c))
        .collect();
    if options.is_empty() {
        return Flow::Done;
    }
    Flow::Choose { options, then }
}

fn add_and_shuffle(g: &mut Game, ctx: &mut ygo_engine::EffectCtx, picked: InstanceId) -> Flow {
    g.add_to_hand(picked);
    g.shuffle_deck(ctx.controller);
    Flow::Done
}

pub fn register(r: &mut Registry) {
    r.register(
        EXODIA_THE_FORBIDDEN_ONE,
        CardScript {
            tags: vec![Tag::ExodiaHead],
            effects: Vec::new(),
        },
    );
    for id in EXODIA_LIMBS {
        r.register(
            id,
            CardScript {
                tags: vec![Tag::ExodiaPiece],
                effects: Vec::new(),
            },
        );
    }
    // Modern text: "If this card is sent from the field to the GY: Add 1
    // monster with 1500 or less ATK from your Deck to your hand." The
    // 2002 printing says "When"; as a mandatory effect it never misses
    // its timing either way, so both versions play the same.
    r.register(
        SANGAN,
        single(Effect {
            timing: sent_from_field_this_window,
            ..Effect::trigger(true, |g, ctx| {
                search_deck(
                    g,
                    ctx.controller,
                    |g, c| g.stats(c).atk <= 1500,
                    add_and_shuffle,
                )
            })
        }),
    );
    r.register(
        WITCH_OF_THE_BLACK_FOREST,
        single(Effect {
            timing: sent_from_field_this_window,
            ..Effect::trigger(true, |g, ctx| {
                search_deck(
                    g,
                    ctx.controller,
                    |g, c| g.stats(c).def <= 1500,
                    add_and_shuffle,
                )
            })
        }),
    );
    // FLIP: Target 1 monster on the field; destroy that target.
    r.register(
        MAN_EATER_BUG,
        single(Effect {
            targets: Some(|g, s| {
                [s.controller, s.controller.opponent()]
                    .into_iter()
                    .flat_map(|p| g.monsters_controlled(p))
                    .map(|m| vec![m])
                    .collect::<TargetSets>()
            }),
            ..Effect::flip(|g, ctx| {
                g.destroy(ctx.targets[0], ctx.source);
                Flow::Done
            })
        }),
    );
    // FLIP: Target 1 Spell in your GY; add that target to your hand.
    r.register(
        MAGICIAN_OF_FAITH,
        single(Effect {
            targets: Some(|g, s| {
                g.graveyard(s.controller)
                    .iter()
                    .copied()
                    .filter(|&c| g.stats(c).kind == ygo_data::CardKind::Spell)
                    .map(|c| vec![c])
                    .collect()
            }),
            ..Effect::flip(|g, ctx| {
                g.add_to_hand(ctx.targets[0]);
                Flow::Done
            })
        }),
    );
    // During damage calculation, if your opponent's monster attacks (Quick
    // Effect): You can discard this card; you take no battle damage from
    // that battle.
    r.register(
        KURIBOH,
        single(Effect {
            kind: EffectKind::Quick,
            speed: 2,
            damage_step: true,
            timing: |g, s, w| {
                w.kind == WindowKind::DamageCalculation
                    && g.current_attack().is_some_and(|(attacker, _)| {
                        g.instance(attacker).controller != s.controller
                    })
                    && g.instance(s.card).location == Location::Hand
            },
            cost: Some(|g, ctx| g.discard(ctx.source)),
            ..Effect::activate(2, |g, ctx| {
                g.prevent_battle_damage(ctx.controller);
                Flow::Done
            })
        }),
    );
}
