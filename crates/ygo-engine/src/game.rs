use std::sync::Arc;

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use ygo_data::CardId;

use crate::chain::{Pending, PendingChoice};
use crate::{
    Action, CardInstance, CardScript, CardStats, ChainLink, Decision, Event, GameResult, GameSetup,
    InstanceId, Lingering, Location, Phase, PlayerId, PlayerState, SetupError, Shared,
};

/// A whole game. Cheap to clone: flat vectors plus one `Arc` to the shared
/// rules and card pool.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub(crate) shared: Arc<Shared>,
    /// Shuffles by effects stay deterministic.
    pub(crate) rng: ChaCha8Rng,
    pub(crate) instances: Vec<CardInstance>,
    pub(crate) players: [PlayerState; 2],
    pub(crate) turn: u32,
    pub(crate) turn_player: PlayerId,
    pub(crate) phase: Phase,
    pub(crate) max_turns: u32,
    pub(crate) result: Option<GameResult>,
    /// Events of the action being applied; drained by `apply`.
    pub(crate) events: Vec<Event>,
    /// Events since the last window opened or closed: the next window's contents.
    pub(crate) recent: Vec<Event>,
    pub(crate) chain: Vec<ChainLink>,
    pub(crate) pending: Vec<Pending>,
    pub(crate) pending_choice: Option<PendingChoice>,
    pub(crate) lingering: Vec<Lingering>,
    /// Per player: battle damage prevented for the current battle.
    pub(crate) damage_prevented: [bool; 2],
    /// Controller of the effect currently paying a cost or resolving.
    pub(crate) resolving: Option<PlayerId>,
    /// The attack between its declaration and the end of its Damage Step.
    pub(crate) current_attack: Option<(InstanceId, Option<InstanceId>)>,
    /// A player allowed a second Battle Phase in this turn number.
    pub(crate) extra_battle_phase: Option<(PlayerId, u32)>,
    /// Cards that destruction cannot touch until the chain ends (Fake Trap).
    pub(crate) shielded: Vec<InstanceId>,
    /// Computed once by `advance` for the state it stopped in.
    pub(crate) decision: Option<Decision>,
    pub(crate) actions: Vec<Action>,
    /// The `HOOK_*` bits of every card in this game: a hook no card has is
    /// never looked for.
    pub(crate) hooks: u8,
    /// What each player remembers of cards hidden again (`memory.rs`).
    pub(crate) memory: [crate::memory::Memory; 2],
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
            // Instance numbers must not reveal a card's identity to the
            // opponent, who may know the deck list: number the Main Deck
            // in a seeded random order, then shuffle the draw order too.
            let mut main_order: Vec<usize> = (0..deck.main.len()).collect();
            if setup.shuffle {
                shuffle(&mut rng, &mut main_order);
            }
            for &slot in &main_order {
                let id = deck.main[slot];
                let (index, stats) = lookup(&shared, player, id)?;
                if stats.is_extra_deck() {
                    return Err(SetupError::WrongDeckList {
                        player,
                        id,
                        name: stats.name.clone(),
                    });
                }
                state.deck.push(InstanceId(instances.len() as u8));
                instances.push(CardInstance::in_deck(index, player));
            }
            for &id in &deck.extra {
                let (index, stats) = lookup(&shared, player, id)?;
                if !stats.is_extra_deck() {
                    return Err(SetupError::WrongDeckList {
                        player,
                        id,
                        name: stats.name.clone(),
                    });
                }
                let mut card = CardInstance::in_deck(index, player);
                card.location = Location::Extra;
                state.extra.push(InstanceId(instances.len() as u8));
                instances.push(card);
            }
            if setup.shuffle {
                shuffle(&mut rng, &mut state.deck);
            } else {
                state.deck.reverse();
            }
        }

        let turn_player = setup
            .first_player
            .unwrap_or_else(|| PlayerId((rng.next_u32() & 1) as u8));

        let mut game = Self {
            shared,
            rng,
            instances,
            players,
            turn: 0,
            turn_player,
            phase: Phase::End,
            max_turns: setup.max_turns,
            result: None,
            events: Vec::with_capacity(32),
            recent: Vec::with_capacity(32),
            chain: Vec::with_capacity(4),
            pending: Vec::with_capacity(8),
            pending_choice: None,
            lingering: Vec::new(),
            damage_prevented: [false, false],
            resolving: None,
            current_attack: None,
            extra_battle_phase: None,
            shielded: Vec::new(),
            decision: None,
            actions: Vec::with_capacity(64),
            hooks: 0,
            memory: Default::default(),
        };
        game.hooks = game
            .instances
            .iter()
            .fold(0, |bits, i| bits | game.shared.hooks[i.card.0 as usize]);
        for player in PlayerId::BOTH {
            for _ in 0..game.shared.rules.hand.opening_size {
                game.draw_card(player);
            }
        }
        for player in PlayerId::BOTH {
            game.check_exodia(player);
        }
        game.start_turn(1, turn_player);
        game.advance();
        game.events.clear();
        Ok(game)
    }

    /// Moves the top card of `player`'s Deck to their hand, or returns
    /// `None` when the Deck is empty.
    pub(crate) fn draw_card(&mut self, player: PlayerId) -> Option<InstanceId> {
        let state = &mut self.players[player.index()];
        let id = state.deck.pop()?;
        state.hand.push(id);
        self.instances[id.index()].location = Location::Hand;
        Some(id)
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

    pub fn script(&self, id: InstanceId) -> Option<&CardScript> {
        self.shared.script(self.instances[id.index()].card)
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

    /// The chain being built or resolved, first link first.
    pub fn chain(&self) -> &[ChainLink] {
        &self.chain
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
    let stats = shared.pool.get(index);
    if stats.token {
        return Err(SetupError::TokenInDeck { player, id });
    }
    Ok((index, stats))
}

/// Fisher-Yates with the game's own RNG, so no `rand` dependency and the
/// same seed shuffles the same way on every platform.
fn shuffle<T>(rng: &mut ChaCha8Rng, cards: &mut [T]) {
    for i in (1..cards.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        cards.swap(i, j);
    }
}
