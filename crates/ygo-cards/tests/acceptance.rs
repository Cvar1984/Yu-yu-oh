//! The Python prototype's acceptance cases (`reference/python_prototype`,
//! `test_v8.py` and `test_v9.py`) that no rulings test covered yet, and the
//! two engine bugs the prototype found. Where the prototype's simplified
//! rules differ from the modern rules, the test follows the modern rules
//! and says so. `docs/acceptance.md` maps every case to its test.

use ygo_cards::extra_monsters::PENGUIN_SOLDIER;
use ygo_cards::extra_spells::{SCAPEGOAT, SHEEP_TOKEN};
use ygo_cards::extra_traps::RAIGEKI_BREAK;
use ygo_cards::ritual::{BLACK_LUSTER_RITUAL, BLACK_LUSTER_SOLDIER};
use ygo_cards::spells::{DARK_HOLE, MONSTER_REBORN, TRIBUTE_TO_THE_DOOMED};
use ygo_cards::traps::SOLEMN_JUDGMENT;
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MYSTICAL_ELF, check_invariants, in_hand, next_turn, play,
};
use ygo_engine::{Action, DecisionKind, Event, Game, InstanceId, Location};

mod psv_common;
use psv_common::*;

const LA_JINN: CardId = CardId(97590748);
const JUDGE_MAN: CardId = CardId(30113682);

/// Passes response windows, collecting what happened.
fn pass_collect(g: &mut Game) -> Vec<Event> {
    let mut events = Vec::new();
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Respond)
    {
        events.extend(play(g, Action::Pass));
    }
    events
}

fn reborn_targets(g: &Game) -> Vec<InstanceId> {
    activations(g, MONSTER_REBORN)
        .into_iter()
        .filter_map(|a| match a {
            Action::Activate { targets, .. } => targets.first().copied(),
            _ => None,
        })
        .collect()
}

// v8 "Normal Monster stats": the pool's stats for the prototype's checks.
#[test]
fn normal_monster_stats_match_the_prototype() {
    let g = game(&[], &[]);
    let pool = &g.shared().pool;
    let stats = |id| {
        let s = pool.get(pool.index_of(id).unwrap());
        (s.level, s.atk, s.def)
    };
    assert_eq!(stats(LA_JINN), (4, 1800, 1000));
    assert_eq!(stats(JUDGE_MAN), (6, 2200, 1500));
}

// v8 "Black Luster Ritual: Level 4+4 tributes" and "Black Luster Soldier
// revive only after proper Summon".
#[test]
fn black_luster_soldier_is_ritual_summoned_and_revived_only_after_that() {
    let mut g = game(
        &[
            TRIBUTE_TO_THE_DOOMED,
            BLACK_LUSTER_SOLDIER,
            MONSTER_REBORN,
            BLACK_LUSTER_RITUAL,
            BLACK_LUSTER_SOLDIER,
            CELTIC_GUARDIAN,
            CELTIC_GUARDIAN,
            DARK_HOLE,
        ],
        &[KOUMORI_DRAGON],
    );
    next_turn(&mut g);
    let koumori = summon_now(&mut g, P1, KOUMORI_DRAGON);
    next_turn(&mut g);
    let discarded = in_hand(&g, P0, BLACK_LUSTER_SOLDIER);
    let doomed = activations(&g, TRIBUTE_TO_THE_DOOMED)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { cost, .. } if cost == &vec![discarded]))
        .expect("discard a Black Luster Soldier");
    play(&mut g, doomed);
    pass_collect(&mut g);
    assert_eq!(g.instance(koumori).location, Location::Graveyard);
    assert_eq!(g.instance(discarded).location, Location::Graveyard);
    assert!(
        !reborn_targets(&g).contains(&discarded),
        "never properly summoned"
    );

    let soldier = in_hand(&g, P0, BLACK_LUSTER_SOLDIER);
    let guardians: Vec<InstanceId> = g
        .hand(P0)
        .iter()
        .copied()
        .filter(|&c| g.stats(c).id == CELTIC_GUARDIAN)
        .collect();
    let mut wanted = vec![soldier];
    wanted.extend(&guardians);
    activate_on(&mut g, BLACK_LUSTER_RITUAL, &wanted);
    answer(&mut g, 0);
    assert!(g.on_field(soldier));
    assert_eq!(g.atk(soldier), 3000);
    for c in guardians {
        assert_eq!(g.instance(c).location, Location::Graveyard, "4 + 4 = 8");
    }

    activate(&mut g, DARK_HOLE);
    assert_eq!(g.instance(soldier).location, Location::Graveyard);
    let targets = reborn_targets(&g);
    assert!(targets.contains(&soldier), "properly summoned once");
    assert!(!targets.contains(&discarded));
}

// v8 "Penguin Soldier flipped by attack bounces 2, no battle". Modern rules:
// a face-down monster attacked is flipped before damage calculation, and
// its FLIP effect resolves after it; the battle happens first.
#[test]
fn penguin_soldier_attacked_face_down_battles_then_bounces_two() {
    let mut g = game(&[CELTIC_GUARDIAN, MYSTICAL_ELF], &[PENGUIN_SOLDIER]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let elf = summon_now(&mut g, P0, MYSTICAL_ELF);
    next_turn(&mut g);
    let penguin = set_monster(&mut g, P1, PENGUIN_SOLDIER);
    next_turn(&mut g);
    attack(&mut g, celtic, Some(penguin));
    loop {
        let d = g.pending_decision().expect("the game goes on");
        if d.kind == DecisionKind::Triggers && d.player == P1 {
            break;
        }
        assert_ne!(
            d.kind,
            DecisionKind::BattlePhase,
            "Penguin's effect was offered"
        );
        play(&mut g, Action::Pass);
    }
    assert_eq!(
        g.instance(penguin).location,
        Location::Graveyard,
        "1400 ATK beat 500 DEF before the FLIP effect"
    );
    let both = g
        .legal_actions()
        .into_iter()
        .find(|a| {
            matches!(a, Action::Activate { targets, .. }
                if targets.len() == 2 && targets.contains(&celtic) && targets.contains(&elf))
        })
        .expect("both attackers' side monsters offered");
    play(&mut g, both);
    pass_collect(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Hand);
    assert_eq!(g.instance(elf).location, Location::Hand);
    assert_eq!((lp(&g, P0), lp(&g, P1)), (8000, 8000));
    check_invariants(&g);
}

// v8 "Scapegoat on a direct attack". Modern rules: the Sheep Tokens change
// the defender's number of monsters, so the attack replays; it cannot go
// on directly.
#[test]
fn scapegoat_turns_a_direct_attack_into_a_replay() {
    let mut g = game(&[CELTIC_GUARDIAN], &[SCAPEGOAT]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    set_card(&mut g, P1, SCAPEGOAT);
    next_turn(&mut g);
    attack(&mut g, celtic, None);
    respond(&mut g, P1, SCAPEGOAT, None);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Replay));
    let options = g.legal_actions();
    assert!(!options.contains(&Action::Attack {
        attacker: celtic,
        target: None
    }));
    let sheep = options
        .iter()
        .find_map(|a| match a {
            Action::Attack {
                target: Some(t), ..
            } if g.stats(*t).id == SHEEP_TOKEN => Some(*t),
            _ => None,
        })
        .expect("a Sheep Token can be attacked");
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: Some(sheep),
        },
    );
    pass_collect(&mut g);
    assert_eq!(g.instance(sheep).location, Location::Gone);
    assert_eq!(lp(&g, P1), 8000);
    check_invariants(&g);
}

// Prototype bug 1: removing the attacker before damage calculation ends
// the battle with no damage calculation.
#[test]
fn an_attacker_removed_in_the_battle_step_deals_no_damage() {
    let mut g = game(&[CELTIC_GUARDIAN], &[RAIGEKI_BREAK, MYSTICAL_ELF]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    set_card(&mut g, P1, RAIGEKI_BREAK);
    next_turn(&mut g);
    attack(&mut g, celtic, None);
    let mut events = Vec::new();
    loop {
        let d = g.pending_decision().unwrap();
        if d.kind != DecisionKind::Respond {
            break;
        }
        let offer = activations(&g, RAIGEKI_BREAK)
            .into_iter()
            .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &vec![celtic]));
        match offer {
            Some(a) if d.player == P1 => events.extend(play(&mut g, a)),
            _ => events.extend(play(&mut g, Action::Pass)),
        }
    }
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::BattleDamageDealt { .. })),
        "{events:?}"
    );
    assert_eq!(lp(&g, P1), 8000);
}

// Prototype bug 2: a negated Spell goes to the Graveyard exactly once.
#[test]
fn a_negated_spell_goes_to_the_graveyard_once() {
    let mut g = game(&[DARK_HOLE, CELTIC_GUARDIAN], &[SOLEMN_JUDGMENT]);
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    set_card(&mut g, P1, SOLEMN_JUDGMENT);
    next_turn(&mut g);
    let hole = in_hand(&g, P0, DARK_HOLE);
    let a = activations(&g, DARK_HOLE).remove(0);
    let mut events = play(&mut g, a);
    loop {
        let d = g.pending_decision().unwrap();
        if d.kind != DecisionKind::Respond {
            break;
        }
        let offer = activations(&g, SOLEMN_JUDGMENT).into_iter().next();
        match offer {
            Some(a) if d.player == P1 => events.extend(play(&mut g, a)),
            _ => events.extend(play(&mut g, Action::Pass)),
        }
    }
    let sent = events
        .iter()
        .filter(|e| matches!(e, Event::SentToGraveyard { card, .. } if *card == hole))
        .count();
    assert_eq!(sent, 1, "{events:?}");
    assert_eq!(g.graveyard(P0).iter().filter(|&&c| c == hole).count(), 1);
    assert!(
        g.monsters_controlled(P0).len() == 1,
        "Dark Hole was negated"
    );
    check_invariants(&g);
}
