use std::sync::Arc;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_data::CardId;

use crate::{
    CardInstance, CardStats, GameResult, GameSetup, InstanceId, Location, Phase, PlayerId,
    PlayerState, SetupError, Shared,
};

/// A whole game. Cheap to clone: flat vectors plus one `Arc` to the shared
/// rules and card pool.
#[derive(Debug, Clone)]
pub struct Game {
    shared: Arc<Shared>,
    /// Kept in the state so effects that shuffle (M2) stay deterministic.
    #[allow(dead_code)]
    rng: ChaCha8Rng,
    instances: Vec<CardInstance>,
    players: [PlayerState; 2],
    turn: u32,
    turn_player: PlayerId,
    phase: Phase,
    max_turns: u32,
    result: Option<GameResult>,
}

impl Game {
    /// Builds the instance table, shuffles both decks and deals the
    /// opening hands. The game starts in player `first_player`'s Draw Phase
    /// of turn 1.
    pub fn new(shared: Arc<Shared>, setup: &GameSetup) -> Result<Self, SetupError> {
        let rules = &shared.rules;
        let mut rng = ChaCha8Rng::seed_from_u64(setup.seed);

        let total: usize = setup
            .decks
            .iter()
            .map(|d| d.main.len() + d.extra.len())
            .sum();
        let max = usize::from(u8::MAX) + 1;
        if total > max {
            return Err(SetupError::TooManyCards { total, max });
        }
        if rules.hand.opening_size > rules.deck.main_min {
            return Err(SetupError::OpeningHandTooLarge {
                opening: rules.hand.opening_size,
                min: rules.deck.main_min,
            });
        }

        let mut instances = Vec::with_capacity(total);
        let mut players = [
            PlayerState::new(
                rules.life.starting_lp,
                rules.field.monster_zones,
                rules.field.spell_trap_zones,
            ),
            PlayerState::new(
                rules.life.starting_lp,
                rules.field.monster_zones,
                rules.field.spell_trap_zones,
            ),
        ];

        for player in PlayerId::BOTH {
            let deck = &setup.decks[player.index()];
            let state = &mut players[player.index()];
            let (min, max) = (rules.deck.main_min, rules.deck.main_max);
            if deck.main.len() < min || deck.main.len() > max {
                return Err(SetupError::MainDeckSize {
                    player,
                    size: deck.main.len(),
                    min,
                    max,
                });
            }
            if deck.extra.len() > rules.deck.extra_max {
                return Err(SetupError::ExtraDeckSize {
                    player,
                    size: deck.extra.len(),
                    max: rules.deck.extra_max,
                });
            }
            for (ids, extra) in [(&deck.main, false), (&deck.extra, true)] {
                for &id in ids {
                    let (index, stats) = lookup(&shared, player, id)?;
                    if stats.is_extra_deck() != extra {
                        return Err(SetupError::WrongDeckList {
                            player,
                            id,
                            name: stats.name.clone(),
                        });
                    }
                    let instance = InstanceId(instances.len() as u8);
                    let mut card = CardInstance::in_deck(index, player);
                    if extra {
                        card.location = Location::Extra;
                        state.extra.push(instance);
                    } else {
                        state.deck.push(instance);
                    }
                    instances.push(card);
                }
            }
            shuffle(&mut rng, &mut state.deck);
        }

        let turn_player = setup
            .first_player
            .unwrap_or_else(|| PlayerId((rng.next_u32() & 1) as u8));

        let mut game = Self {
            shared,
            rng,
            instances,
            players,
            turn: 1,
            turn_player,
            phase: Phase::Draw,
            max_turns: setup.max_turns,
            result: None,
        };
        for player in PlayerId::BOTH {
            for _ in 0..game.shared.rules.hand.opening_size {
                game.draw_card(player);
            }
        }
        Ok(game)
    }

    /// Moves the top card of `player`'s Deck to their hand. Returns `false`
    /// when the Deck is empty.
    fn draw_card(&mut self, player: PlayerId) -> bool {
        let state = &mut self.players[player.index()];
        let Some(id) = state.deck.pop() else {
            return false;
        };
        state.hand.push(id);
        self.instances[id.index()].location = Location::Hand;
        true
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    pub fn instance(&self, id: InstanceId) -> &CardInstance {
        &self.instances[id.index()]
    }

    pub fn instances(&self) -> &[CardInstance] {
        &self.instances
    }

    pub fn stats(&self, id: InstanceId) -> &CardStats {
        self.shared.pool.get(self.instances[id.index()].card)
    }

    pub fn player(&self, player: PlayerId) -> &PlayerState {
        &self.players[player.index()]
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn turn_player(&self) -> PlayerId {
        self.turn_player
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn max_turns(&self) -> u32 {
        self.max_turns
    }

    pub fn result(&self) -> Option<GameResult> {
        self.result
    }
}

fn lookup(
    shared: &Shared,
    player: PlayerId,
    id: CardId,
) -> Result<(crate::CardIndex, &CardStats), SetupError> {
    let index = shared
        .pool
        .index_of(id)
        .ok_or(SetupError::UnknownCard { player, id })?;
    Ok((index, shared.pool.get(index)))
}

/// Fisher-Yates with the game's own RNG, so no `rand` dependency and the
/// same seed shuffles the same way on every platform.
fn shuffle(rng: &mut ChaCha8Rng, cards: &mut [InstanceId]) {
    for i in (1..cards.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        cards.swap(i, j);
    }
}
