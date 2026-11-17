//! Words for what a player sees: card labels, action sentences, event log
//! lines and a plain-text board. Everything is built from one player's
//! `Observation`, so nothing hidden from that player is ever named.

use std::fmt::Write;

use ygo_data::CardId;
use ygo_engine::{
    Action, BattleStep, CardPool, DecisionKind, Event, GameResult, InstanceId, Observation, Phase,
    PlayerId, Position, Side, VisibleCard, WinReason,
};

/// Where a card sits, as the viewer sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Hand,
    Monster(usize),
    SpellTrap(usize),
    Field,
    Graveyard,
    Banished,
    Extra,
    /// Shown for a choice (a Deck search, a revealed hand).
    Revealed,
    Chain,
}

/// A card found in the observation: how it is seen, and where.
#[derive(Debug, Clone, Copy)]
pub struct Found<'a> {
    pub card: &'a VisibleCard,
    pub owner: Option<PlayerId>,
    pub place: Place,
}

/// Finds instance `id` in what the viewer can see.
pub fn find(obs: &Observation, id: InstanceId) -> Option<Found<'_>> {
    for p in PlayerId::BOTH {
        let side = &obs.sides[p.index()];
        let zones: [(&[VisibleCard], Place); 4] = [
            (&side.hand, Place::Hand),
            (&side.graveyard, Place::Graveyard),
            (&side.banished, Place::Banished),
            (&side.extra, Place::Extra),
        ];
        for (cards, place) in zones.iter() {
            if let Some(card) = cards.iter().find(|c| c.instance() == id) {
                return Some(Found {
                    card,
                    owner: Some(p),
                    place: *place,
                });
            }
        }
        for (i, z) in side.monster_zones.iter().enumerate() {
            if let Some(card) = z.as_ref().filter(|c| c.instance() == id) {
                return Some(Found {
                    card,
                    owner: Some(p),
                    place: Place::Monster(i),
                });
            }
        }
        for (i, z) in side.spell_trap_zones.iter().enumerate() {
            if let Some(card) = z.as_ref().filter(|c| c.instance() == id) {
                return Some(Found {
                    card,
                    owner: Some(p),
                    place: Place::SpellTrap(i),
                });
            }
        }
        if let Some(card) = side.field_zone.as_ref().filter(|c| c.instance() == id) {
            return Some(Found {
                card,
                owner: Some(p),
                place: Place::Field,
            });
        }
    }
    obs.revealed
        .iter()
        .find(|c| c.instance() == id)
        .map(|card| Found {
            card,
            owner: None,
            place: Place::Revealed,
        })
}

/// "You" or "Your opponent", from the viewer's side.
pub fn who(obs: &Observation, player: PlayerId) -> &'static str {
    if player == obs.viewer {
        "You"
    } else {
        "Your opponent"
    }
}

fn whose(obs: &Observation, player: PlayerId) -> &'static str {
    if player == obs.viewer {
        "your"
    } else {
        "your opponent's"
    }
}

pub fn card_name(pool: &CardPool, id: CardId) -> &str {
    pool.index_of(id)
        .map_or("an unknown card", |i| pool.get(i).name.as_str())
}

/// The name of instance `id` if the viewer can see it, or a description
/// of where the hidden card is.
pub fn card(obs: &Observation, pool: &CardPool, id: InstanceId) -> String {
    let chain = obs.chain.iter().find(|l| l.instance == id);
    match (find(obs, id), chain) {
        (
            Some(Found {
                card: VisibleCard::Known { card, .. },
                ..
            }),
            _,
        ) => card_name(pool, *card).to_string(),
        (_, Some(link)) => card_name(pool, link.card).to_string(),
        (Some(Found { owner, place, .. }), _) => {
            let owner = owner.map_or("a", |p| whose(obs, p));
            match place {
                Place::Hand => format!("a card in {owner} hand"),
                Place::Monster(i) => format!("{owner} face-down monster in zone {}", i + 1),
                Place::SpellTrap(i) => format!("{owner} face-down card in zone {}", i + 1),
                Place::Extra => format!("a card in {owner} Extra Deck"),
                _ => "a hidden card".to_string(),
            }
        }
        (None, None) => "a card".to_string(),
    }
}

fn list(obs: &Observation, pool: &CardPool, ids: &[InstanceId]) -> String {
    let names: Vec<String> = ids.iter().map(|&i| card(obs, pool, i)).collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// A sentence for an action the viewer may take.
pub fn action(obs: &Observation, pool: &CardPool, a: &Action) -> String {
    let c = |id: InstanceId| card(obs, pool, id);
    match a {
        Action::Discard { card } => format!("Discard {}", c(*card)),
        Action::Summon {
            card,
            tributes,
            face_down,
        } => {
            let verb = match (tributes.is_empty(), face_down) {
                (true, false) => "Normal Summon",
                (true, true) => "Set",
                (false, false) => "Tribute Summon",
                (false, true) => "Tribute Set",
            };
            if tributes.is_empty() {
                format!("{verb} {}", c(*card))
            } else {
                format!(
                    "{verb} {}, tributing {}",
                    c(*card),
                    list(obs, pool, tributes)
                )
            }
        }
        Action::FlipSummon { card } => format!("Flip Summon {}", c(*card)),
        Action::ChangePosition { card } => {
            let to = match find(obs, *card).map(|f| f.card) {
                Some(VisibleCard::Known {
                    position: Position::FaceUpAttack,
                    ..
                }) => "Defense",
                _ => "Attack",
            };
            format!("Change {} to {to} Position", c(*card))
        }
        Action::SetSpellTrap { card } => format!("Set {}", c(*card)),
        Action::Activate {
            card,
            effect,
            targets,
            cost,
        } => {
            let mut s = format!("Activate {}", c(*card));
            if *effect > 0 {
                let _ = write!(s, " (effect {})", effect + 1);
            }
            if !targets.is_empty() {
                let _ = write!(s, " on {}", list(obs, pool, targets));
            }
            if !cost.is_empty() {
                let _ = write!(s, ", paying with {}", list(obs, pool, cost));
            }
            s
        }
        Action::Choose { card } => format!("Choose {}", c(*card)),
        Action::Answer { option } => obs
            .question
            .get(usize::from(*option))
            .cloned()
            .unwrap_or_else(|| format!("Answer {}", option + 1)),
        Action::Pass => match obs.decision.map(|d| d.kind) {
            Some(DecisionKind::Triggers) => "Do not use the optional effect".to_string(),
            Some(DecisionKind::Replay) => "Stop the attack".to_string(),
            _ => "Do not respond".to_string(),
        },
        Action::EnterBattlePhase => "Go to the Battle Phase".to_string(),
        Action::Attack {
            attacker,
            target: None,
        } => format!("Attack directly with {}", c(*attacker)),
        Action::Attack {
            attacker,
            target: Some(t),
        } => format!("Attack {} with {}", c(*t), c(*attacker)),
        Action::EndBattlePhase => "End the Battle Phase".to_string(),
        Action::EndTurn => "End the turn".to_string(),
    }
}

pub fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Draw => "Draw Phase",
        Phase::Standby => "Standby Phase",
        Phase::Main1 => "Main Phase 1",
        Phase::Battle(BattleStep::Start) => "Battle Phase",
        Phase::Battle(BattleStep::Battle) => "Battle Phase",
        Phase::Battle(BattleStep::Damage) => "Damage Step",
        Phase::Battle(BattleStep::End) => "End of the Battle Phase",
        Phase::Main2 => "Main Phase 2",
        Phase::End => "End Phase",
    }
}

/// A log line for an event, or `None` for bookkeeping not worth showing.
pub fn event(obs: &Observation, pool: &CardPool, e: &Event) -> Option<String> {
    let c = |id: InstanceId| card(obs, pool, id);
    Some(match e {
        Event::TurnStarted { turn, player } => {
            format!("Turn {turn}: {} turn", who_possessive(obs, *player))
        }
        Event::PhaseEntered {
            phase: phase @ (Phase::Main1 | Phase::Main2 | Phase::End | Phase::Standby),
        } => phase_name(*phase).to_string(),
        Event::PhaseEntered {
            phase: Phase::Battle(BattleStep::Start),
        } => "Battle Phase".to_string(),
        Event::Drew { player, card } => format!("{} drew {}", who(obs, *player), c(*card)),
        Event::DeckEmpty { player } => {
            format!("{} could not draw: the Deck is empty", who(obs, *player))
        }
        Event::Discarded { player, card } => {
            format!("{} discarded {}", who(obs, *player), c(*card))
        }
        Event::Tributed { player, card } => format!("{} tributed {}", who(obs, *player), c(*card)),
        Event::Summoned {
            player,
            card,
            position,
        } => {
            if position.is_face_up() {
                format!("{} summoned {}", who(obs, *player), c(*card))
            } else {
                format!("{} set a monster", who(obs, *player))
            }
        }
        Event::SpecialSummoned { player, card, .. } => {
            format!("{} Special Summoned {}", who(obs, *player), c(*card))
        }
        Event::FusionSummoned { player, card, .. } => {
            format!("{} Fusion Summoned {}", who(obs, *player), c(*card))
        }
        Event::RitualSummoned { player, card, .. } => {
            format!("{} Ritual Summoned {}", who(obs, *player), c(*card))
        }
        Event::FlipSummoned { player, card } => {
            format!("{} Flip Summoned {}", who(obs, *player), c(*card))
        }
        Event::Flipped { card } => format!("{} was flipped face-up", c(*card)),
        Event::PositionChanged { card, position } => format!(
            "{} changed to {} Position",
            c(*card),
            if *position == Position::FaceUpAttack {
                "Attack"
            } else {
                "Defense"
            }
        ),
        Event::SetCard { player, .. } => format!("{} set a card", who(obs, *player)),
        Event::Activated {
            card, controller, ..
        } => format!("{} activated {}", who(obs, *controller), c(*card)),
        Event::ActivationNegated { card, by } => format!("{} negated {}", c(*by), c(*card)),
        Event::SummonNegated { card, by } => {
            format!("{} negated the summon of {}", c(*by), c(*card))
        }
        Event::AttackDeclared {
            attacker,
            target: None,
        } => format!("{} attacks directly", c(*attacker)),
        Event::AttackDeclared {
            attacker,
            target: Some(t),
        } => format!("{} attacks {}", c(*attacker), c(*t)),
        Event::AttackCancelled { attacker } => format!("The attack of {} stopped", c(*attacker)),
        Event::AttackReplay { attacker } => format!("{} lost its target", c(*attacker)),
        Event::DestroyedByBattle { card } => format!("{} was destroyed by battle", c(*card)),
        Event::DestroyedByEffect { card, .. } => format!("{} was destroyed", c(*card)),
        Event::DamageTaken { player, amount, lp } => {
            format!("{} took {amount} damage ({lp} LP)", who(obs, *player))
        }
        Event::LifePointsGained { player, amount, lp } => {
            format!("{} gained {amount} LP ({lp} LP)", who(obs, *player))
        }
        Event::LifePointsPaid { player, amount, lp } => {
            format!("{} paid {amount} LP ({lp} LP)", who(obs, *player))
        }
        Event::BattleDamagePrevented { player } => {
            format!("{} took no battle damage", who(obs, *player))
        }
        Event::ControlChanged { card, to } => {
            format!("{} took control of {}", who(obs, *to), c(*card))
        }
        Event::AddedToHand { player, card, .. } => {
            format!("{} added {} to the hand", who(obs, *player), c(*card))
        }
        Event::Equipped { card, to } => format!("{} was equipped to {}", c(*card), c(*to)),
        Event::Banished { card, .. } => format!("{} was banished", c(*card)),
        Event::CoinTossed { player, heads } => format!(
            "{} tossed a coin: {}",
            who(obs, *player),
            if *heads { "heads" } else { "tails" }
        ),
        Event::Revealed { player, cards } => {
            format!("{} revealed {}", who(obs, *player), list(obs, pool, cards))
        }
        Event::DeckShuffled { player } => format!("{} shuffled the Deck", who(obs, *player)),
        Event::GameOver { result } => result_sentence(obs, *result),
        _ => return None,
    })
}

fn who_possessive(obs: &Observation, player: PlayerId) -> &'static str {
    if player == obs.viewer {
        "your"
    } else {
        "your opponent's"
    }
}

pub fn result_sentence(obs: &Observation, result: GameResult) -> String {
    match result {
        GameResult::Win { winner, reason } => {
            let how = match reason {
                WinReason::LifePoints => "Life Points",
                WinReason::DeckOut => "deck-out",
                WinReason::Exodia => "Exodia",
            };
            if winner == obs.viewer {
                format!("You win by {how}!")
            } else {
                format!("Your opponent wins by {how}.")
            }
        }
        GameResult::Draw { .. } => "The game is a draw.".to_string(),
    }
}

pub fn zone_text(pool: &CardPool, z: &Option<VisibleCard>) -> String {
    match z {
        None => "-".to_string(),
        Some(VisibleCard::Hidden { position, .. }) => match position {
            Position::FaceDownDefense => "[set monster]".to_string(),
            _ => "[set card]".to_string(),
        },
        Some(VisibleCard::Known {
            card,
            position,
            stats,
            ..
        }) => {
            let name = card_name(pool, *card);
            match (stats, position) {
                (Some((atk, def)), Position::FaceUpAttack) => format!("{name} {atk}/{def} ATK"),
                (Some((atk, def)), Position::FaceUpDefense) => format!("{name} {atk}/{def} DEF"),
                (Some((atk, def)), Position::FaceDownDefense) => format!("{name} {atk}/{def} set"),
                (_, Position::FaceDown) => format!("{name} (set)"),
                _ => name.to_string(),
            }
        }
    }
}

fn side_text(
    out: &mut String,
    obs: &Observation,
    pool: &CardPool,
    label: &str,
    side: &Side,
    mine: bool,
) {
    let _ = writeln!(
        out,
        "{label}: {} LP | hand {} | deck {} | extra {} | graveyard {} | banished {}",
        side.lp,
        side.hand.len(),
        side.deck_count,
        side.extra_count,
        side.graveyard.len(),
        side.banished.len()
    );
    let monsters: Vec<String> = side
        .monster_zones
        .iter()
        .map(|z| zone_text(pool, z))
        .collect();
    let spells: Vec<String> = side
        .spell_trap_zones
        .iter()
        .map(|z| zone_text(pool, z))
        .collect();
    let _ = writeln!(out, "  Monsters: {}", monsters.join(" | "));
    let _ = writeln!(
        out,
        "  Spells/Traps: {} | Field: {}",
        spells.join(" | "),
        zone_text(pool, &side.field_zone)
    );
    if let Some(top) = side.graveyard.last() {
        let _ = writeln!(out, "  Graveyard top: {}", card(obs, pool, top.instance()));
    }
    if mine
        || side
            .hand
            .iter()
            .any(|c| matches!(c, VisibleCard::Known { .. }))
    {
        let hand: Vec<String> = side
            .hand
            .iter()
            .map(|c| card(obs, pool, c.instance()))
            .collect();
        let _ = writeln!(out, "  Hand: {}", hand.join(", "));
    }
}

/// The whole board as plain text, for logs, replays and tests.
pub fn board(obs: &Observation, pool: &CardPool) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Turn {}, {} turn, {}",
        obs.turn,
        who_possessive(obs, obs.turn_player),
        phase_name(obs.phase)
    );
    side_text(&mut out, obs, pool, "Opponent", obs.opponent(), false);
    side_text(&mut out, obs, pool, "You", obs.me(), true);
    if !obs.chain.is_empty() {
        let links: Vec<String> = obs
            .chain
            .iter()
            .enumerate()
            .map(|(i, l)| {
                format!(
                    "{}. {} ({})",
                    i + 1,
                    card_name(pool, l.card),
                    who(obs, l.controller)
                )
            })
            .collect();
        let _ = writeln!(out, "Chain: {}", links.join(", "));
    }
    out
}
