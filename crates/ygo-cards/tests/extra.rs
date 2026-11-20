//! Rulings tests for the EXTRA cards (handoff update, 2026-11-20), batch 1:
//! one scenario or more per card, on the modern text.

use std::sync::Arc;

use ygo_cards::extra_monsters::*;
use ygo_cards::extra_spells::*;
use ygo_cards::extra_traps::*;
use ygo_cards::oneshot::SPARKS;
use ygo_cards::remaining::SPELLBINDING_CIRCLE;
use ygo_cards::ritual::WHITE_DRAGON_RITUAL;
use ygo_cards::spells::{DARK_HOLE, RAIGEKI};
use ygo_cards::traps::{MYSTICAL_SPACE_TYPHOON as MST, TRAP_HOLE};
use ygo_data::CardId;
use ygo_engine::testing::{
    CELTIC_GUARDIAN, MAMMOTH_GRAVEYARD, MYSTICAL_ELF, SEVEN_COLORED_FISH, answer, in_hand,
    next_turn, pass_all, play, shared_from, stacked, summon, with_rules,
};
use ygo_engine::{
    Action, DecisionKind, Event, Game, InstanceId, Location, PlayerId, Position, Shared,
    VisibleCard,
};

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const PAD: CardId = MAMMOTH_GRAVEYARD; // 1200/800 EARTH Dinosaur
const KOUMORI_DRAGON: CardId = CardId(67724379); // 1500/1200 DARK Dragon
const FERAL_IMP: CardId = CardId(41392891); // 1300/1400 DARK Fiend

/// Ten-card opening hands, five Normal Summons a turn and no hand limit
/// to speak of, so one turn can set up a board.
fn shared() -> Arc<Shared> {
    let base = shared_from(ygo_cards::registry());
    with_rules(&base, |r| {
        r.summon.normal_or_tribute_per_turn = 5;
        r.hand.opening_size = 10;
        r.hand.max_size = 30;
    })
}

fn game(p0: &[CardId], p1: &[CardId]) -> Game {
    Game::new(shared(), &stacked([p0, p1], PAD)).unwrap()
}

fn activations(g: &Game, id: CardId) -> Vec<Action> {
    g.legal_actions()
        .into_iter()
        .filter(|a| matches!(a, Action::Activate { card, .. } if g.stats(*card).id == id))
        .collect()
}

/// Passes every response window, collecting what happened.
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

/// Activates card `id` (the first activation offered) and lets the chain
/// resolve; returns everything that happened.
fn activate(g: &mut Game, id: CardId) -> Vec<Event> {
    let a = activations(g, id)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{id} not offered"));
    let mut events = play(g, a);
    events.extend(pass_collect(g));
    events
}

/// Activates card `id` with exactly these targets.
fn activate_on(g: &mut Game, id: CardId, targets: &[InstanceId]) {
    let a = activations(g, id)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets: t, .. } if t == targets))
        .unwrap_or_else(|| panic!("{id} not offered on {targets:?}"));
    play(g, a);
    pass_all(g);
}

fn set_card(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, Action::SetSpellTrap { card });
    pass_all(g);
    card
}

fn summon_now(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(g, summon(card));
    pass_all(g);
    card
}

fn set_monster(g: &mut Game, player: PlayerId, id: CardId) -> InstanceId {
    let card = in_hand(g, player, id);
    play(
        g,
        Action::Summon {
            card,
            tributes: vec![],
            face_down: true,
        },
    );
    pass_all(g);
    card
}

/// Answers every pending card choice with its first option.
fn choose_first(g: &mut Game) {
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Choose)
    {
        let a = g.legal_actions()[0].clone();
        play(g, a);
    }
    pass_all(g);
}

fn lp(g: &Game, p: PlayerId) -> u32 {
    g.player(p).lp
}

/// Two turns on: the same player's next Main Phase, Set Traps now usable.
fn next_own_turn(g: &mut Game) {
    next_turn(g);
    next_turn(g);
}

/// Passes the turn player's priority in the open window, so the opponent
/// may respond.
fn pass_turn_player(g: &mut Game) {
    while g
        .pending_decision()
        .is_some_and(|d| d.kind == DecisionKind::Respond && d.player == g.turn_player())
    {
        play(g, Action::Pass);
    }
}

// ----- monsters -----

#[test]
fn mysterious_puppeteer_gains_500_per_normal_or_flip_summon_by_either_player() {
    let mut g = game(&[MYSTERIOUS_PUPPETEER, CELTIC_GUARDIAN], &[CELTIC_GUARDIAN]);
    summon_now(&mut g, P0, MYSTERIOUS_PUPPETEER);
    assert_eq!(lp(&g, P0), 8500, "its own Normal Summon counts");
    let celtic = set_monster(&mut g, P0, CELTIC_GUARDIAN);
    assert_eq!(lp(&g, P0), 8500, "a Set is not a summon");
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    assert_eq!(lp(&g, P0), 9000, "the opponent's summon counts");
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: celtic });
    pass_all(&mut g);
    assert_eq!(lp(&g, P0), 9500, "a Flip Summon counts");
}

#[test]
fn the_wicked_worm_beast_returns_to_the_hand_in_its_controllers_end_phase() {
    let mut g = game(&[THE_WICKED_WORM_BEAST], &[]);
    let worm = summon_now(&mut g, P0, THE_WICKED_WORM_BEAST);
    next_turn(&mut g);
    assert_eq!(g.instance(worm).location, Location::Hand);
}

#[test]
fn penguin_soldier_may_return_up_to_two_monsters_or_nothing() {
    let mut g = game(&[PENGUIN_SOLDIER], &[CELTIC_GUARDIAN, MYSTICAL_ELF]);
    let penguin = set_monster(&mut g, P0, PENGUIN_SOLDIER);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let elf = summon_now(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    play(&mut g, Action::FlipSummon { card: penguin });
    pass_all(&mut g);
    let d = g.pending_decision().unwrap();
    assert_eq!((d.player, d.kind), (P0, DecisionKind::Triggers));
    assert!(g.legal_actions().contains(&Action::Pass), "it is optional");
    let both = g
        .legal_actions()
        .into_iter()
        .find(|a| {
            matches!(a, Action::Activate { targets, .. } if targets.len() == 2
            && targets.contains(&celtic) && targets.contains(&elf))
        })
        .expect("two targets offered");
    play(&mut g, both);
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Hand);
    assert_eq!(g.instance(elf).location, Location::Hand);
}

#[test]
fn trap_master_destroys_a_set_trap_but_leaves_a_set_spell() {
    for (pick_trap, destroyed) in [(true, true), (false, false)] {
        let mut g = game(&[TRAP_MASTER], &[TRAP_HOLE, DARK_HOLE]);
        let master = set_monster(&mut g, P0, TRAP_MASTER);
        next_turn(&mut g);
        let hole = set_card(&mut g, P1, TRAP_HOLE);
        let dark = set_card(&mut g, P1, DARK_HOLE);
        next_turn(&mut g);
        play(&mut g, Action::FlipSummon { card: master });
        pass_all(&mut g);
        let target = if pick_trap { hole } else { dark };
        let a = g
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &[target]))
            .expect("a Set card can be picked");
        play(&mut g, a);
        pass_all(&mut g);
        assert_eq!(g.on_field(target), !destroyed, "trap {pick_trap}");
    }
}

// ----- Spells -----

#[test]
fn ancient_telescope_shows_its_player_the_opponents_top_five_in_order() {
    let mut g = game(&[ANCIENT_TELESCOPE], &[]);
    let top = g.top_of_deck(P1, 5);
    let events = activate(&mut g, ANCIENT_TELESCOPE);
    let seen = events
        .iter()
        .find_map(|e| match e {
            Event::LookedAt { player, cards } if *player == P0 => Some(cards.clone()),
            _ => None,
        })
        .expect("a private look");
    assert_eq!(seen.iter().map(|(c, _)| *c).collect::<Vec<_>>(), top);
    assert_eq!(g.top_of_deck(P1, 5), top, "the Deck keeps its order");
}

#[test]
fn the_inexperienced_spy_shows_one_card_of_the_opponents_hand() {
    let mut g = game(&[THE_INEXPERIENCED_SPY], &[]);
    let events = activate(&mut g, THE_INEXPERIENCED_SPY);
    let seen = events
        .iter()
        .find_map(|e| match e {
            Event::LookedAt { player, cards } if *player == P0 => Some(cards.clone()),
            _ => None,
        })
        .expect("a private look");
    assert_eq!(seen.len(), 1);
    assert!(g.player(P1).hand.contains(&seen[0].0));
    // The opponent's hand stays hidden from the spy's player afterwards.
    let obs = g.observe(P0);
    assert!(
        obs.opponent()
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Hidden { .. }))
    );
}

#[test]
fn card_destruction_discards_both_hands_and_draws_as_many() {
    let mut g = game(&[CARD_DESTRUCTION], &[]);
    activate(&mut g, CARD_DESTRUCTION);
    assert_eq!(g.player(P0).hand.len(), 9);
    assert_eq!(g.player(P1).hand.len(), 10);
    assert_eq!(g.graveyard(P0).len(), 10, "nine discards and the card");
    assert_eq!(g.graveyard(P1).len(), 10);
}

#[test]
fn de_spell_destroys_a_spell_and_reveals_a_set_trap_without_destroying_it() {
    let mut g = game(&[DE_SPELL, DE_SPELL], &[TRAP_HOLE, DARK_HOLE]);
    next_turn(&mut g);
    let hole = set_card(&mut g, P1, TRAP_HOLE);
    let dark = set_card(&mut g, P1, DARK_HOLE);
    next_turn(&mut g);
    activate_on(&mut g, DE_SPELL, &[hole]);
    assert!(g.on_field(hole), "a Trap survives");
    activate_on(&mut g, DE_SPELL, &[dark]);
    assert!(!g.on_field(dark), "a Spell is destroyed");
}

#[test]
fn dian_keto_and_ookazi() {
    let mut g = game(&[DIAN_KETO_THE_CURE_MASTER, OOKAZI], &[]);
    activate(&mut g, DIAN_KETO_THE_CURE_MASTER);
    assert_eq!(lp(&g, P0), 9000);
    activate(&mut g, OOKAZI);
    assert_eq!(lp(&g, P1), 7200);
}

#[test]
fn fairy_meteor_crush_inflicts_piercing_damage() {
    let mut g = game(&[CELTIC_GUARDIAN, FAIRY_METEOR_CRUSH], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let pad = set_monster(&mut g, P1, PAD);
    next_turn(&mut g);
    activate_on(&mut g, FAIRY_METEOR_CRUSH, &[celtic]);
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: Some(pad),
        },
    );
    pass_all(&mut g);
    assert_eq!(g.instance(pad).location, Location::Graveyard);
    assert_eq!(lp(&g, P1), 8000 - 600, "1400 ATK through 800 DEF");
}

#[test]
fn graceful_charity_draws_three_then_discards_two() {
    let mut g = game(&[GRACEFUL_CHARITY], &[]);
    activate(&mut g, GRACEFUL_CHARITY);
    choose_first(&mut g);
    assert_eq!(g.player(P0).hand.len(), 10 - 1 + 3 - 2);
    assert_eq!(g.graveyard(P0).len(), 3);
}

#[test]
fn invigoration_and_sword_of_dark_destruction_fit_their_attribute_only() {
    let mut g = game(
        &[
            CELTIC_GUARDIAN,
            FERAL_IMP,
            MYSTICAL_ELF,
            INVIGORATION,
            SWORD_OF_DARK_DESTRUCTION,
        ],
        &[],
    );
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    let imp = summon_now(&mut g, P0, FERAL_IMP);
    let elf = summon_now(&mut g, P0, MYSTICAL_ELF);
    let targets = |g: &Game, id: CardId| -> Vec<InstanceId> {
        activations(g, id)
            .into_iter()
            .filter_map(|a| match a {
                Action::Activate { targets, .. } => Some(targets[0]),
                _ => None,
            })
            .collect()
    };
    assert_eq!(targets(&g, INVIGORATION), vec![celtic], "EARTH only");
    assert_eq!(
        targets(&g, SWORD_OF_DARK_DESTRUCTION),
        vec![imp],
        "DARK only"
    );
    activate_on(&mut g, INVIGORATION, &[celtic]);
    activate_on(&mut g, SWORD_OF_DARK_DESTRUCTION, &[imp]);
    assert_eq!((g.atk(celtic), g.def(celtic)), (1800, 1000));
    assert_eq!((g.atk(imp), g.def(imp)), (1700, 1200));
    assert_eq!((g.atk(elf), g.def(elf)), (800, 2000));
}

#[test]
fn last_will_summons_from_the_deck_after_a_monster_of_yours_went_to_the_graveyard() {
    let mut g = game(&[CELTIC_GUARDIAN, DARK_HOLE, LAST_WILL], &[]);
    assert!(activations(&g, LAST_WILL).is_empty(), "nothing went yet");
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    activate(&mut g, DARK_HOLE);
    let deck = g.deck_size(P0);
    activate(&mut g, LAST_WILL);
    // The monster, then its position (Attack, the first option).
    choose_first(&mut g);
    assert_eq!(g.deck_size(P0), deck - 1);
    let summoned: Vec<InstanceId> = g.player(P0).monsters().collect();
    assert_eq!(summoned.len(), 1);
    assert!(g.stats(summoned[0]).atk <= 1500);
    next_own_turn(&mut g);
    assert!(
        activations(&g, LAST_WILL).is_empty(),
        "the count resets each turn"
    );
}

#[test]
fn nobleman_of_crossout_banishes_a_flip_monster_and_every_copy_in_both_decks() {
    let mut p1 = vec![PENGUIN_SOLDIER];
    p1.extend([PAD; 9]);
    p1.extend([PENGUIN_SOLDIER, PENGUIN_SOLDIER]);
    let mut g = game(&[NOBLEMAN_OF_CROSSOUT, NOBLEMAN_OF_CROSSOUT], &p1);
    next_turn(&mut g);
    let penguin = set_monster(&mut g, P1, PENGUIN_SOLDIER);
    let pad = set_monster(&mut g, P1, PAD);
    next_turn(&mut g);
    activate_on(&mut g, NOBLEMAN_OF_CROSSOUT, &[penguin]);
    assert_eq!(g.instance(penguin).location, Location::Banished);
    assert_eq!(g.banished(P1).len(), 3, "both copies in the Deck too");
    assert!(g.deck(P1).iter().all(|&c| g.stats(c).id != PENGUIN_SOLDIER));
    let deck = g.deck_size(P1);
    activate_on(&mut g, NOBLEMAN_OF_CROSSOUT, &[pad]);
    assert_eq!(g.instance(pad).location, Location::Banished);
    assert_eq!(g.deck_size(P1), deck, "not a Flip monster: the Deck stays");
}

#[test]
fn premature_burial_revives_in_attack_position_and_takes_its_monster_down_with_it() {
    let mut g = game(&[CELTIC_GUARDIAN, DARK_HOLE, PREMATURE_BURIAL], &[MST]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    activate(&mut g, DARK_HOLE);
    activate_on(&mut g, PREMATURE_BURIAL, &[celtic]);
    assert_eq!(lp(&g, P0), 7200, "800 LP paid");
    assert_eq!(g.instance(celtic).position, Position::FaceUpAttack);
    let burial = g
        .player(P0)
        .spell_trap_zones
        .iter()
        .flatten()
        .copied()
        .find(|&c| g.stats(c).id == PREMATURE_BURIAL)
        .unwrap();
    assert_eq!(g.equip_target(burial), Some(celtic));
    next_turn(&mut g);
    activate_on(&mut g, MST, &[burial]);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
}

#[test]
fn premature_burial_goes_when_its_monster_leaves() {
    let mut g = game(&[CELTIC_GUARDIAN, DARK_HOLE, PREMATURE_BURIAL], &[RAIGEKI]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    activate(&mut g, DARK_HOLE);
    activate_on(&mut g, PREMATURE_BURIAL, &[celtic]);
    next_turn(&mut g);
    activate(&mut g, RAIGEKI);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert!(
        g.graveyard(P0)
            .iter()
            .any(|&c| g.stats(c).id == PREMATURE_BURIAL)
    );
}

#[test]
fn cards_needing_a_card_outside_the_pool_are_never_offered() {
    let g = game(
        &[
            THE_FLUTE_OF_SUMMONING_DRAGON,
            KOUMORI_DRAGON,
            WHITE_DRAGON_RITUAL,
            CELTIC_GUARDIAN,
            MYSTICAL_ELF,
        ],
        &[],
    );
    assert!(activations(&g, THE_FLUTE_OF_SUMMONING_DRAGON).is_empty());
    assert!(activations(&g, WHITE_DRAGON_RITUAL).is_empty());
    let mut g = game(&[GRYPHON_WING], &[SPARKS]);
    set_card(&mut g, P0, GRYPHON_WING);
    next_turn(&mut g);
    let sparks = in_hand(&g, P1, SPARKS);
    let a = g
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Action::Activate { card, .. } if *card == sparks))
        .unwrap();
    play(&mut g, a);
    pass_turn_player(&mut g);
    assert!(
        activations(&g, GRYPHON_WING).is_empty(),
        "only Harpie's Feather Duster"
    );
}

// ----- Traps -----

#[test]
fn backup_soldier_returns_three_non_effect_monsters_of_1500_or_less() {
    let mut g = game(
        &[
            BACKUP_SOLDIER,
            DARK_HOLE,
            PAD,
            CELTIC_GUARDIAN,
            MYSTICAL_ELF,
            SEVEN_COLORED_FISH,
            MYSTERIOUS_PUPPETEER,
        ],
        &[],
    );
    set_card(&mut g, P0, BACKUP_SOLDIER);
    assert!(activations(&g, BACKUP_SOLDIER).is_empty());
    for id in [
        PAD,
        CELTIC_GUARDIAN,
        MYSTICAL_ELF,
        SEVEN_COLORED_FISH,
        MYSTERIOUS_PUPPETEER,
    ] {
        summon_now(&mut g, P0, id);
    }
    activate(&mut g, DARK_HOLE);
    next_own_turn(&mut g);
    activate(&mut g, BACKUP_SOLDIER);
    choose_first(&mut g);
    let mut left: Vec<CardId> = g.graveyard(P0).iter().map(|&c| g.stats(c).id).collect();
    left.sort();
    let mut expected = vec![
        DARK_HOLE,
        BACKUP_SOLDIER,
        SEVEN_COLORED_FISH,   // 1800 ATK
        MYSTERIOUS_PUPPETEER, // an Effect Monster
    ];
    expected.sort();
    assert_eq!(left, expected, "Mammoth, Celtic and Elf went to the hand");
}

#[test]
fn burst_breath_destroys_face_up_monsters_with_def_up_to_the_tributed_atk() {
    let mut g = game(
        &[KOUMORI_DRAGON, BURST_BREATH],
        &[CELTIC_GUARDIAN, MYSTICAL_ELF],
    );
    let dragon = summon_now(&mut g, P0, KOUMORI_DRAGON);
    set_card(&mut g, P0, BURST_BREATH);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    let elf = summon_now(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    activate(&mut g, BURST_BREATH);
    assert_eq!(g.instance(dragon).location, Location::Graveyard);
    assert_eq!(g.instance(celtic).location, Location::Graveyard, "DEF 1200");
    assert!(g.on_field(elf), "DEF 2000 is above 1500");
}

#[test]
fn castle_walls_and_reinforcements_last_until_the_end_of_the_turn() {
    let mut g = game(&[CELTIC_GUARDIAN, CASTLE_WALLS, REINFORCEMENTS], &[]);
    let celtic = summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, CASTLE_WALLS);
    set_card(&mut g, P0, REINFORCEMENTS);
    next_own_turn(&mut g);
    activate_on(&mut g, CASTLE_WALLS, &[celtic]);
    activate_on(&mut g, REINFORCEMENTS, &[celtic]);
    assert_eq!((g.atk(celtic), g.def(celtic)), (1900, 1700));
    next_turn(&mut g);
    assert_eq!((g.atk(celtic), g.def(celtic)), (1400, 1200));
}

#[test]
fn disappear_banishes_a_card_from_the_opponents_graveyard() {
    let mut g = game(&[DISAPPEAR], &[SPARKS]);
    set_card(&mut g, P0, DISAPPEAR);
    next_turn(&mut g);
    activate(&mut g, SPARKS);
    let sparks = g.graveyard(P1)[0];
    next_turn(&mut g);
    activate_on(&mut g, DISAPPEAR, &[sparks]);
    assert_eq!(g.instance(sparks).location, Location::Banished);
}

#[test]
fn dust_tornado_destroys_then_may_set_a_card_from_the_hand() {
    let mut g = game(&[DUST_TORNADO, TRAP_HOLE], &[TRAP_HOLE]);
    set_card(&mut g, P0, DUST_TORNADO);
    next_turn(&mut g);
    let theirs = set_card(&mut g, P1, TRAP_HOLE);
    next_turn(&mut g);
    let mine = in_hand(&g, P0, TRAP_HOLE);
    activate_on(&mut g, DUST_TORNADO, &[theirs]);
    assert!(!g.on_field(theirs));
    answer(&mut g, 0);
    choose_first(&mut g);
    assert_eq!(g.instance(mine).position, Position::FaceDown);
    assert!(g.on_field(mine), "Set from the hand");
}

#[test]
fn enchanted_javelin_gains_the_attackers_atk() {
    let mut g = game(&[ENCHANTED_JAVELIN], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, ENCHANTED_JAVELIN);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    play(
        &mut g,
        Action::Attack {
            attacker: celtic,
            target: None,
        },
    );
    pass_turn_player(&mut g);
    activate_on(&mut g, ENCHANTED_JAVELIN, &[celtic]);
    assert_eq!(lp(&g, P0), 8000 + 1400 - 1400, "gained, then hit");
}

#[test]
fn gift_of_the_mystical_elf_counts_every_monster_on_the_field() {
    let mut g = game(
        &[CELTIC_GUARDIAN, GIFT_OF_THE_MYSTICAL_ELF],
        &[CELTIC_GUARDIAN, MYSTICAL_ELF],
    );
    summon_now(&mut g, P0, CELTIC_GUARDIAN);
    set_card(&mut g, P0, GIFT_OF_THE_MYSTICAL_ELF);
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    set_monster(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    activate(&mut g, GIFT_OF_THE_MYSTICAL_ELF);
    assert_eq!(lp(&g, P0), 8000 + 900);
}

#[test]
fn jar_of_greed_draws_one_and_just_desserts_burns_per_monster() {
    let mut g = game(
        &[JAR_OF_GREED, JUST_DESSERTS],
        &[CELTIC_GUARDIAN, MYSTICAL_ELF],
    );
    set_card(&mut g, P0, JAR_OF_GREED);
    set_card(&mut g, P0, JUST_DESSERTS);
    next_turn(&mut g);
    summon_now(&mut g, P1, CELTIC_GUARDIAN);
    summon_now(&mut g, P1, MYSTICAL_ELF);
    next_turn(&mut g);
    let hand = g.player(P0).hand.len();
    activate(&mut g, JAR_OF_GREED);
    assert_eq!(g.player(P0).hand.len(), hand + 1);
    activate(&mut g, JUST_DESSERTS);
    assert_eq!(lp(&g, P1), 7000);
}

#[test]
fn raigeki_break_discards_then_destroys_any_card() {
    let mut g = game(&[RAIGEKI_BREAK], &[CELTIC_GUARDIAN]);
    set_card(&mut g, P0, RAIGEKI_BREAK);
    next_turn(&mut g);
    let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
    next_turn(&mut g);
    let hand = g.player(P0).hand.len();
    let a = activations(&g, RAIGEKI_BREAK)
        .into_iter()
        .find(|a| matches!(a, Action::Activate { targets, .. } if targets == &[celtic]))
        .unwrap();
    play(&mut g, a);
    pass_all(&mut g);
    assert_eq!(g.instance(celtic).location, Location::Graveyard);
    assert_eq!(g.player(P0).hand.len(), hand - 1, "one card discarded");
}

#[test]
fn shadow_spell_and_spellbinding_circle_stop_the_attack_they_answer() {
    for trap in [SHADOW_SPELL, SPELLBINDING_CIRCLE] {
        let mut g = game(&[trap, RAIGEKI], &[CELTIC_GUARDIAN]);
        let card = set_card(&mut g, P0, trap);
        next_turn(&mut g);
        let celtic = summon_now(&mut g, P1, CELTIC_GUARDIAN);
        play(&mut g, Action::EnterBattlePhase);
        pass_all(&mut g);
        play(
            &mut g,
            Action::Attack {
                attacker: celtic,
                target: None,
            },
        );
        pass_turn_player(&mut g);
        activate_on(&mut g, trap, &[celtic]);
        assert_eq!(lp(&g, P0), 8000, "{trap}: the attack stopped");
        if trap == SHADOW_SPELL {
            assert_eq!(g.atk(celtic), 700);
        }
        play(&mut g, Action::EndBattlePhase);
        next_own_turn(&mut g);
        play(&mut g, Action::EnterBattlePhase);
        pass_all(&mut g);
        assert!(
            !g.legal_actions()
                .iter()
                .any(|a| matches!(a, Action::Attack { attacker, .. } if *attacker == celtic)),
            "{trap}: it cannot attack"
        );
        play(&mut g, Action::EndBattlePhase);
        next_turn(&mut g);
        activate(&mut g, RAIGEKI);
        assert_eq!(
            g.instance(card).location,
            Location::Graveyard,
            "{trap} goes with its monster"
        );
    }
}

#[test]
fn the_eye_of_truth_reveals_only_the_opponents_hand_and_pays_them_for_spells() {
    let mut g = game(&[THE_EYE_OF_TRUTH], &[SPARKS]);
    set_card(&mut g, P0, THE_EYE_OF_TRUTH);
    next_own_turn(&mut g);
    activate(&mut g, THE_EYE_OF_TRUTH);
    let mine = g.observe(P0);
    assert!(
        mine.opponent()
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Known { .. }))
    );
    let theirs = g.observe(P1);
    assert!(
        theirs
            .opponent()
            .hand
            .iter()
            .all(|c| matches!(c, VisibleCard::Hidden { .. }))
    );
    next_turn(&mut g);
    assert_eq!(lp(&g, P1), 9000, "Sparks in hand at their Standby Phase");
}

#[test]
fn waboku_stops_battle_damage_and_battle_destruction_for_the_turn() {
    let mut g = game(&[PAD, WABOKU], &[SEVEN_COLORED_FISH]);
    let pad = summon_now(&mut g, P0, PAD);
    set_card(&mut g, P0, WABOKU);
    next_turn(&mut g);
    let fish = summon_now(&mut g, P1, SEVEN_COLORED_FISH);
    play(&mut g, Action::EnterBattlePhase);
    pass_all(&mut g);
    play(
        &mut g,
        Action::Attack {
            attacker: fish,
            target: Some(pad),
        },
    );
    pass_turn_player(&mut g);
    activate(&mut g, WABOKU);
    assert!(g.on_field(pad), "not destroyed by battle");
    assert_eq!(lp(&g, P0), 8000, "no battle damage");
}
