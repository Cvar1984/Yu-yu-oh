//! How good a position is: Life Points and the worth of each side's cards
//! (`card_values.toml`), plus the monsters a held revive card could bring
//! back. The search bot scores its playouts with it, and the heuristic
//! bot's lookahead compares the options of one decision.

use ygo_data::ReviveFrom;
use ygo_engine::{
    DecisionKind, Game, GameResult, InstanceId, Location, PlayerId, PlayerView, Position,
};

use crate::{HeuristicPlayer, Player};

/// Weights of a position's parts; the defaults are the search bot's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Worth {
    /// Life Points worth one point of card value (values run 0 to 100).
    pub lp_per_point: f64,
    /// A card in the hand is worth this share of its value on the field.
    pub hand_worth: f64,
    /// A monster a held revive card could bring back from a Graveyard is
    /// worth this share of its value (the style's `revive_worth`).
    pub revive_worth: f64,
}

impl Default for Worth {
    fn default() -> Self {
        Self {
            lp_per_point: 50.0,
            hand_worth: 0.5,
            revive_worth: 0.0,
        }
    }
}

/// What `p` has: Life Points and their cards on the field (fully, plus a
/// face-up attacker's ATK boosts) and in the hand (by `hand_worth`), and
/// what their revive cards could bring back (`revive_potential`). Card
/// values and revive cards are `bot`'s. Cards in `ignore` count for nothing.
pub(crate) fn side_worth(
    g: &Game,
    p: PlayerId,
    bot: &HeuristicPlayer,
    w: Worth,
    ignore: &[InstanceId],
) -> f64 {
    let value = |c| bot.card_value(c);
    let mut worth = f64::from(g.player(p).lp) / w.lp_per_point;
    for (i, inst) in g.instances().iter().enumerate() {
        let id = InstanceId(i as u8);
        if inst.controller != p || ignore.contains(&id) {
            continue;
        }
        let v = value(inst.card);
        worth += match inst.location {
            Location::MonsterZone(_) => {
                let boost = if inst.position == Position::FaceUpAttack {
                    f64::from(g.atk(id) - g.stats(id).atk) / w.lp_per_point
                } else {
                    0.0
                };
                v + boost
            }
            Location::SpellTrapZone(_) | Location::FieldZone => v,
            Location::Hand => v * w.hand_worth,
            _ => 0.0,
        };
    }
    if w.revive_worth > 0.0 {
        worth += w.revive_worth * revive_potential(g, p, bot, ignore);
    }
    worth
}

/// The worth of the monsters `p`'s revive cards could bring back: each
/// revive card `p` holds (in the hand, or Set) takes the best monster
/// left that it can reach and that may be Special Summoned. Both
/// Graveyards are public, so a card that revives from either player's
/// looks at both.
fn revive_potential(g: &Game, p: PlayerId, bot: &HeuristicPlayer, ignore: &[InstanceId]) -> f64 {
    let held = |inst: &ygo_engine::CardInstance| {
        inst.controller == p
            && (inst.location == Location::Hand
                || (matches!(inst.location, Location::SpellTrapZone(_))
                    && inst.position == Position::FaceDown))
    };
    let mut revivers: Vec<ReviveFrom> = g
        .instances()
        .iter()
        .enumerate()
        .filter(|&(i, inst)| held(inst) && !ignore.contains(&InstanceId(i as u8)))
        .filter_map(|(_, inst)| bot.revive_from(inst.card))
        .collect();
    if revivers.is_empty() {
        return 0.0;
    }
    // Cards that reach only their own Graveyard pick first.
    revivers.sort_by_key(|r| *r == ReviveFrom::Either);
    let mut targets: Vec<(f64, PlayerId)> = PlayerId::BOTH
        .into_iter()
        .flat_map(|owner| g.graveyard(owner).iter().map(move |&c| (c, owner)))
        .filter(|&(c, _)| g.stats(c).is_monster() && g.special_summonable(c))
        .map(|(c, owner)| (bot.card_value(g.instance(c).card), owner))
        .collect();
    targets.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut total = 0.0;
    for from in revivers {
        let pick = targets
            .iter()
            .position(|&(_, owner)| from == ReviveFrom::Either || owner == p);
        if let Some(i) = pick {
            total += targets.remove(i).0;
        }
    }
    total
}

/// A won or lost game against any lead in points.
const DECIDED: f64 = 1e6;

/// `p`'s lead over the opponent in points; a finished game is worth
/// `DECIDED` either way.
pub(crate) fn lead(
    g: &Game,
    p: PlayerId,
    bot: &HeuristicPlayer,
    w: Worth,
    ignore: &[InstanceId],
) -> f64 {
    match g.result() {
        Some(GameResult::Win { winner, .. }) => {
            if winner == p {
                DECIDED
            } else {
                -DECIDED
            }
        }
        Some(GameResult::Draw { .. }) => 0.0,
        None => side_worth(g, p, bot, w, ignore) - side_worth(g, p.opponent(), bot, w, ignore),
    }
}

/// Plays on with `bot` answering for both sides until an open decision
/// (a Main or Battle Phase action, a hand-limit discard), the end of the
/// game, or `cap` actions: what one action led to, once its chain and
/// the answers to it are over.
pub(crate) fn settle(g: &mut Game, bot: &mut HeuristicPlayer, cap: usize) {
    for _ in 0..cap {
        let Some(d) = g.pending_decision() else {
            return;
        };
        if matches!(
            d.kind,
            DecisionKind::MainPhase | DecisionKind::BattlePhase | DecisionKind::HandLimit
        ) {
            return;
        }
        let action = if g.actions().len() == 1 {
            g.actions()[0].clone()
        } else {
            let view = PlayerView::new(g).expect("a pending decision");
            bot.choose(&view)
        };
        g.apply(&action).expect("bots choose legal actions");
    }
}
