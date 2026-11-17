//! One assertion per row of the M3 tables: type Equips, type and
//! attribute Field Spells, attribute boosters and the Battleguards.

use std::sync::Arc;

use ygo_cards::tables::*;
use ygo_data::{CardId, CardKind};
use ygo_engine::testing::{in_hand, play, shared_from, stacked, summon, with_rules};
use ygo_engine::{Action, CardIndex, Game, PlayerId, Shared};

const P0: PlayerId = PlayerId(0);

fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| r.summon.normal_or_tribute_per_turn = 3)
}

/// The lowest-id Normal Monster of Level 4 or lower that `keep` accepts.
fn normal_where(shared: &Shared, keep: impl Fn(&ygo_engine::CardStats) -> bool) -> Option<CardId> {
    (0..shared.pool.len() as u16)
        .map(CardIndex)
        .map(|i| shared.pool.get(i))
        .filter(|c| c.kind == CardKind::Normal && c.level <= 4 && keep(c))
        .map(|c| c.id)
        .min()
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

fn base(g: &Game, card: ygo_engine::InstanceId) -> (i32, i32) {
    (g.stats(card).atk, g.stats(card).def)
}

#[test]
fn every_type_equip_boosts_only_its_type() {
    let shared = shared();
    for (equip, race) in TYPE_EQUIPS {
        let Some(matching) = normal_where(&shared, |c| c.race == race) else {
            // No Level 4 Normal Monster of this type exists (Sea Serpent); the
            // Equip still registers and can only ever target that type.
            continue;
        };
        let other = normal_where(&shared, |c| c.race != race).unwrap();
        let mut g = Game::new(
            shared.clone(),
            &stacked([&[equip, matching, other], &[]], other),
        )
        .unwrap();
        let m = in_hand(&g, P0, matching);
        let o = in_hand(&g, P0, other);
        play(&mut g, summon(o));
        assert!(
            activations(&g, equip).is_empty(),
            "{equip}: {race} only, not {}",
            g.stats(o).race
        );
        play(&mut g, summon(m));
        let offered = activations(&g, equip);
        assert!(
            matches!(offered.as_slice(), [Action::Activate { targets, .. }] if targets == &vec![m]),
            "{equip}: exactly the {race} monster: {offered:?}"
        );
        play(&mut g, offered[0].clone());
        let (atk, def) = base(&g, m);
        assert_eq!(
            (g.atk(m), g.def(m)),
            (atk + 300, def + 300),
            "{equip} on {race}"
        );
        let (atk, def) = base(&g, o);
        assert_eq!(
            (g.atk(o), g.def(o)),
            (atk, def),
            "{equip} leaves the other monster alone"
        );
    }
}

#[test]
fn every_type_field_spell_changes_the_named_types() {
    let shared = shared();
    for (field, gains, loses) in TYPE_FIELDS {
        let up = normal_where(&shared, |c| gains.contains(&c.race.as_str())).unwrap();
        let neutral = normal_where(&shared, |c| {
            !gains.contains(&c.race.as_str()) && !loses.contains(&c.race.as_str())
        })
        .unwrap();
        let down = loses
            .first()
            .and_then(|_| normal_where(&shared, |c| loses.contains(&c.race.as_str())));
        let mut top = vec![field, up, neutral];
        top.extend(down);
        let mut g = Game::new(shared.clone(), &stacked([&top, &[]], neutral)).unwrap();
        let u = in_hand(&g, P0, up);
        let n = in_hand(&g, P0, neutral);
        play(&mut g, summon(u));
        play(&mut g, summon(n));
        let d = down.map(|d| {
            let d = in_hand(&g, P0, d);
            play(&mut g, summon(d));
            d
        });
        let a = activations(&g, field);
        assert_eq!(a.len(), 1, "{field}");
        play(&mut g, a[0].clone());
        let (atk, def) = base(&g, u);
        assert_eq!(
            (g.atk(u), g.def(u)),
            (atk + 200, def + 200),
            "{field} gains"
        );
        let (atk, def) = base(&g, n);
        assert_eq!((g.atk(n), g.def(n)), (atk, def), "{field} neutral");
        if let Some(d) = d {
            let (atk, def) = base(&g, d);
            assert_eq!(
                (g.atk(d), g.def(d)),
                (atk - 200, def - 200),
                "{field} loses"
            );
        }
    }
}

#[test]
fn every_attribute_field_spell_gives_500_atk_and_takes_400_def() {
    let shared = shared();
    for (field, attribute) in ATTRIBUTE_FIELDS {
        let up = normal_where(&shared, |c| c.attribute.as_deref() == Some(attribute)).unwrap();
        let other = normal_where(&shared, |c| c.attribute.as_deref() != Some(attribute)).unwrap();
        let mut g = Game::new(shared.clone(), &stacked([&[field, up, other], &[]], other)).unwrap();
        let u = in_hand(&g, P0, up);
        let o = in_hand(&g, P0, other);
        play(&mut g, summon(u));
        play(&mut g, summon(o));
        let a = activations(&g, field);
        play(&mut g, a[0].clone());
        let (atk, def) = base(&g, u);
        assert_eq!(
            (g.atk(u), g.def(u)),
            (atk + 500, def - 400),
            "{field} {attribute}"
        );
        let (atk, def) = base(&g, o);
        assert_eq!((g.atk(o), g.def(o)), (atk, def), "{field} other attribute");
    }
}

#[test]
fn every_attribute_booster_raises_one_attribute_and_lowers_another() {
    let shared = shared();
    for (booster, up, down) in ATTRIBUTE_BOOSTERS {
        let u = normal_where(&shared, |c| c.attribute.as_deref() == Some(up)).unwrap();
        let d = normal_where(&shared, |c| c.attribute.as_deref() == Some(down)).unwrap();
        let mut g = Game::new(shared.clone(), &stacked([&[booster, u, d], &[]], u)).unwrap();
        let b = in_hand(&g, P0, booster);
        let u = in_hand(&g, P0, u);
        let d = in_hand(&g, P0, d);
        assert!(
            g.stats(b).level <= 4,
            "{booster} is a Level 4 or lower monster"
        );
        play(&mut g, summon(u));
        play(&mut g, summon(d));
        let (u_atk, u_def) = base(&g, u);
        let (d_atk, d_def) = base(&g, d);
        play(&mut g, summon(b));
        assert_eq!(
            (g.atk(u), g.def(u)),
            (u_atk + 500, u_def),
            "{booster} +500 {up}"
        );
        assert_eq!(
            (g.atk(d), g.def(d)),
            (d_atk - 400, d_def),
            "{booster} -400 {down}"
        );
        let (b_atk, _) = base(&g, b);
        let expected = if g.stats(b).attribute.as_deref() == Some(up) {
            b_atk + 500
        } else {
            b_atk
        };
        assert_eq!(
            g.atk(b),
            expected,
            "{booster} counts itself when it shares the attribute"
        );
    }
}

#[test]
fn the_battleguards_gain_500_atk_for_each_other() {
    use ygo_engine::testing::MYSTICAL_ELF;
    let base = shared_from(ygo_cards::registry());
    let shared = with_rules(&base, |r| r.summon.normal_or_tribute_per_turn = 5);
    let mut g = Game::new(
        shared,
        &stacked(
            [
                &[
                    MYSTICAL_ELF,
                    MYSTICAL_ELF,
                    SWAMP_BATTLEGUARD,
                    LAVA_BATTLEGUARD,
                ],
                &[],
            ],
            MYSTICAL_ELF,
        ),
    )
    .unwrap();
    let swamp = in_hand(&g, P0, SWAMP_BATTLEGUARD);
    let lava = in_hand(&g, P0, LAVA_BATTLEGUARD);
    assert!(
        g.stats(swamp).level == 5 && g.stats(lava).level == 5,
        "both need one tribute"
    );
    let elf1 = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf1));
    play(
        &mut g,
        Action::Summon {
            card: swamp,
            tributes: vec![elf1],
            face_down: false,
        },
    );
    assert_eq!(g.atk(swamp), 1800, "alone");
    let elf2 = in_hand(&g, P0, MYSTICAL_ELF);
    play(&mut g, summon(elf2));
    play(
        &mut g,
        Action::Summon {
            card: lava,
            tributes: vec![elf2],
            face_down: false,
        },
    );
    assert_eq!(g.atk(swamp), 2300, "one Lava Battleguard");
    assert_eq!(g.atk(lava), 2050, "one Swamp Battleguard");
}
