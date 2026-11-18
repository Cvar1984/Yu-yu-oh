//! Test helpers: checking that text shown to a player names no card hidden
//! from them. Built only with the `test-support` feature.

use ygo_data::CardId;
use ygo_engine::{CardIndex, Observation, Shared, VisibleCard};

use crate::describe;

/// Names of every card the viewer can see right now.
pub fn visible_names<'a>(obs: &Observation, shared: &'a Shared) -> Vec<&'a str> {
    let mut ids: Vec<CardId> = Vec::new();
    let mut add = |c: &VisibleCard| {
        if let VisibleCard::Known { card, .. } = c {
            ids.push(*card);
        }
    };
    for side in &obs.sides {
        side.hand
            .iter()
            .chain(&side.graveyard)
            .chain(&side.banished)
            .chain(&side.extra)
            .for_each(&mut add);
        side.monster_zones
            .iter()
            .chain(&side.spell_trap_zones)
            .flatten()
            .for_each(&mut add);
        side.field_zone.iter().for_each(&mut add);
    }
    obs.revealed.iter().for_each(&mut add);
    ids.extend(obs.chain.iter().map(|l| l.card));
    ids.iter()
        .map(|&id| describe::card_name(&shared.pool, id))
        .collect()
}

/// Fails if `text` names any card except those `obs` shows.
pub fn assert_no_hidden_names(text: &str, obs: &Observation, shared: &Shared) {
    let mut visible = visible_names(obs, shared);
    visible.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut rest = text.to_string();
    for name in &visible {
        rest = rest.replace(name, "#");
    }
    for i in 0..shared.pool.len() as u16 {
        let name = &shared.pool.get(CardIndex(i)).name;
        assert!(
            !rest.contains(name.as_str()),
            "hidden card {name:?} named in:\n{text}"
        );
    }
}
