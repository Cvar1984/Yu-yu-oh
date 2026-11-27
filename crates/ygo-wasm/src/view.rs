//! The screen for one moment of the game, as JSON for the page. Built only
//! from the human's observation and the public card pool.

use serde::Serialize;
use ygo_data::CardKind;
use ygo_engine::{
    Action, CardPool, DecisionKind, Observation, Position, Side, SpellTrapType, VisibleCard,
};
use ygo_play::describe;
use ygo_play::session::PlaySession;

/// How many of the latest log lines the page gets.
const LOG_LINES: usize = 300;

#[derive(Serialize)]
pub struct View {
    turn: u32,
    phase: &'static str,
    your_turn: bool,
    /// What the human is being asked, if anything.
    prompt: Option<&'static str>,
    you: SideView,
    opponent: SideView,
    hand: Vec<CardView>,
    chain: Vec<ChainView>,
    /// The attack being made, if any: who attacks whom (no target: direct).
    attack: Option<AttackView>,
    actions: Vec<ActionView>,
    /// The latest lines, oldest first, and how many there are in all.
    log: Vec<String>,
    log_total: usize,
    result: Option<String>,
}

#[derive(Serialize)]
struct SideView {
    lp: u32,
    hand: usize,
    deck: usize,
    extra: usize,
    monsters: Vec<Option<CardView>>,
    spells: Vec<Option<CardView>>,
    field: Option<CardView>,
    /// Top card last.
    graveyard: Vec<CardView>,
    banished: Vec<CardView>,
}

#[derive(Serialize)]
struct CardView {
    instance: u8,
    /// The card's id (its password), for its image; `None` when hidden.
    id: Option<u32>,
    /// `None` when the human cannot see the card.
    name: Option<String>,
    /// monster, effect, fusion, ritual, spell, trap, or hidden.
    kind: &'static str,
    /// attack, defense, set (face-down), or up (face-up Spell or Trap,
    /// or a card off the field).
    position: &'static str,
    /// Current ATK/DEF on the field, printed values elsewhere.
    atk: Option<i32>,
    def: Option<i32>,
    level: Option<u8>,
    /// Attribute and type for monsters, subtype for Spells and Traps.
    detail: Option<String>,
}

#[derive(Serialize)]
struct ChainView {
    name: String,
    yours: bool,
    /// The card that started the link (activations are public).
    instance: u8,
    id: u32,
}

#[derive(Serialize)]
struct AttackView {
    attacker: u8,
    target: Option<u8>,
    yours: bool,
}

#[derive(Serialize)]
struct ActionView {
    index: usize,
    text: String,
    /// The card it is about (the attacker for an attack), to offer it when
    /// that card is picked.
    card: Option<u8>,
    /// The cards it aims at: an attack's target, an activation's targets.
    /// Empty for a direct attack.
    targets: Vec<u8>,
    kind: &'static str,
    /// `card` as the human sees it, wherever it is (a Deck search's
    /// options too); hidden cards stay hidden.
    subject: Option<CardView>,
    /// Which of the card's effects an activation uses.
    effect: Option<u8>,
}

fn card_view(pool: &CardPool, c: &VisibleCard) -> CardView {
    let position = match c.position() {
        Position::FaceUpAttack => "attack",
        Position::FaceUpDefense => "defense",
        Position::FaceDownDefense | Position::FaceDown => "set",
        Position::FaceUp => "up",
    };
    let instance = c.instance().0;
    let VisibleCard::Known { card, stats, .. } = c else {
        return CardView {
            instance,
            id: None,
            name: None,
            kind: "hidden",
            position,
            atk: None,
            def: None,
            level: None,
            detail: None,
        };
    };
    let Some(s) = pool.index_of(*card).map(|i| pool.get(i)) else {
        return CardView {
            instance,
            id: None,
            name: None,
            kind: "hidden",
            position,
            atk: None,
            def: None,
            level: None,
            detail: None,
        };
    };
    let monster = s.is_monster();
    let (atk, def) = match stats {
        Some((a, d)) => (Some(*a), Some(*d)),
        None if monster => (Some(s.atk), Some(s.def)),
        None => (None, None),
    };
    let kind = match s.kind {
        CardKind::Normal => "monster",
        CardKind::Effect => "effect",
        CardKind::Fusion => "fusion",
        CardKind::Ritual => "ritual",
        CardKind::Spell => "spell",
        CardKind::Trap => "trap",
    };
    let detail = if monster {
        Some(
            format!("{} {}", s.attribute.as_deref().unwrap_or(""), s.race)
                .trim()
                .to_string(),
        )
    } else {
        Some(
            match s.subtype {
                SpellTrapType::QuickPlay => "Quick-Play",
                SpellTrapType::Continuous => "Continuous",
                SpellTrapType::Equip => "Equip",
                SpellTrapType::Field => "Field",
                SpellTrapType::Ritual => "Ritual",
                SpellTrapType::Counter => "Counter",
                SpellTrapType::Normal | SpellTrapType::None => "Normal",
            }
            .to_string(),
        )
    };
    CardView {
        instance,
        id: Some(card.0),
        name: Some(s.name.clone()),
        kind,
        position,
        atk,
        def,
        level: monster.then_some(s.level),
        detail,
    }
}

fn side_view(pool: &CardPool, side: &Side) -> SideView {
    let zone = |z: &Option<VisibleCard>| z.as_ref().map(|c| card_view(pool, c));
    SideView {
        lp: side.lp,
        hand: side.hand.len(),
        deck: side.deck_count,
        extra: side.extra_count,
        monsters: side.monster_zones.iter().map(zone).collect(),
        spells: side.spell_trap_zones.iter().map(zone).collect(),
        field: zone(&side.field_zone),
        graveyard: side.graveyard.iter().map(|c| card_view(pool, c)).collect(),
        banished: side.banished.iter().map(|c| card_view(pool, c)).collect(),
    }
}

fn action_kind(a: &Action) -> &'static str {
    match a {
        Action::Discard { .. } => "discard",
        Action::Summon {
            face_down: true, ..
        } => "set",
        Action::Summon { .. } => "summon",
        Action::ProcedureSummon { .. } => "summon",
        Action::FlipSummon { .. } => "flip",
        Action::ChangePosition { .. } => "position",
        Action::SetSpellTrap { .. } => "set",
        Action::Activate { .. } => "activate",
        Action::Choose { .. } => "choose",
        Action::Answer { .. } => "answer",
        Action::Pass => "pass",
        Action::EnterBattlePhase => "battle",
        Action::Attack { .. } => "attack",
        Action::EndBattlePhase => "end_battle",
        Action::EndTurn => "end_turn",
    }
}

fn prompt(obs: &Observation) -> Option<&'static str> {
    let d = obs.decision.filter(|d| d.player == obs.viewer)?;
    Some(match d.kind {
        DecisionKind::HandLimit => "Too many cards: choose one to send to the Graveyard",
        DecisionKind::MainPhase => "Your move",
        DecisionKind::BattlePhase => "Battle: attack or end the Battle Phase",
        DecisionKind::Choose if !obs.question.is_empty() => "Choose an option",
        DecisionKind::Choose => "Choose a card",
        DecisionKind::Respond => "Respond, or pass",
        DecisionKind::Triggers => "Choose which effect goes on the chain first",
        DecisionKind::Replay => "The attack target is gone: attack again or stop",
    })
}

pub fn build(session: &PlaySession) -> View {
    let obs = session.observation();
    let pool = &session.game().shared().pool;
    let texts = session.action_texts();
    let actions = session
        .actions()
        .iter()
        .zip(texts)
        .enumerate()
        .map(|(index, (a, text))| ActionView {
            index,
            text,
            card: PlaySession::subject(a).map(|i| i.0),
            targets: match a {
                Action::Attack {
                    target: Some(t), ..
                } => vec![t.0],
                Action::Activate { targets, .. } => targets.iter().map(|t| t.0).collect(),
                _ => Vec::new(),
            },
            kind: action_kind(a),
            subject: PlaySession::subject(a)
                .and_then(|i| obs.find(i))
                .map(|f| card_view(pool, f.card)),
            effect: match a {
                Action::Activate { effect, .. } => Some(*effect),
                _ => None,
            },
        })
        .collect();
    let log = session.log();
    View {
        turn: obs.turn,
        phase: describe::phase_name(obs.phase),
        your_turn: obs.turn_player == obs.viewer,
        prompt: prompt(&obs),
        you: side_view(pool, obs.me()),
        opponent: side_view(pool, obs.opponent()),
        hand: obs.me().hand.iter().map(|c| card_view(pool, c)).collect(),
        chain: obs
            .chain
            .iter()
            .map(|l| ChainView {
                name: describe::card_name(pool, l.card).to_string(),
                yours: l.controller == obs.viewer,
                instance: l.instance.0,
                id: l.card.0,
            })
            .collect(),
        attack: obs.attack.map(|(a, t)| AttackView {
            attacker: a.0,
            target: t.map(|t| t.0),
            // Your attack: the attacker is in your Monster Zones.
            yours: obs
                .me()
                .monster_zones
                .iter()
                .flatten()
                .any(|c| c.instance() == a),
        }),
        actions,
        log: log[log.len().saturating_sub(LOG_LINES)..].to_vec(),
        log_total: log.len(),
        result: session.result().map(|r| describe::result_sentence(&obs, r)),
    }
}
