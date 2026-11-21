use std::fmt;

use serde::{Deserialize, Serialize};

use ygo_data::CardId;

use crate::CardIndex;

/// Player 0 or 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub u8);

impl PlayerId {
    pub const BOTH: [PlayerId; 2] = [PlayerId(0), PlayerId(1)];

    pub fn opponent(self) -> PlayerId {
        PlayerId(1 - self.0)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Index of a physical card in the game's instance table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(pub u8);

impl InstanceId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Location {
    Deck,
    Hand,
    MonsterZone(u8),
    SpellTrapZone(u8),
    FieldZone,
    Graveyard,
    Extra,
    Banished,
}

impl Location {
    pub fn on_field(self) -> bool {
        matches!(
            self,
            Location::MonsterZone(_) | Location::SpellTrapZone(_) | Location::FieldZone
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Position {
    FaceUpAttack,
    FaceUpDefense,
    FaceDownDefense,
    /// A Set Spell or Trap, or a card in the hand or Deck.
    FaceDown,
    /// A face-up Spell or Trap, or a card in the Graveyard.
    FaceUp,
}

impl Position {
    pub fn is_face_up(self) -> bool {
        matches!(
            self,
            Position::FaceUpAttack | Position::FaceUpDefense | Position::FaceUp
        )
    }
}

/// What a lingering effect does while it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LingeringKind {
    /// Give `card` back to `to` when the effect expires (Change of Heart).
    ReturnControl { card: InstanceId, to: PlayerId },
    /// `card` has these ATK and DEF changes until the effect expires.
    Stats {
        card: InstanceId,
        atk: i32,
        def: i32,
    },
    /// `card` is destroyed when the effect expires, if still on the field.
    Destroy { card: InstanceId },
    /// `card` cannot declare an attack until the effect expires.
    CannotAttack { card: InstanceId },
    /// `card` has these ATK and DEF changes until the current damage
    /// calculation ends.
    DamageCalcStats {
        card: InstanceId,
        atk: i32,
        def: i32,
    },
    /// The source remembers `card` (Castle of Dark Illusions' Zombies);
    /// no effect of its own.
    Marked { card: InstanceId },
    /// `player` takes no battle damage from the opponent's monsters, and
    /// `player`'s monsters cannot be destroyed by battle (Waboku).
    BattleShield { player: PlayerId },
    /// `card`'s controller cannot change its battle position (Goblin Attack
    /// Force, until the End Phase of its controller's next turn).
    PositionLocked { card: InstanceId },
    /// Neither player may activate or Set a Spell or Trap Card (Cold Wave).
    NoSpellsOrTraps,
}

/// An effect that outlives its chain link and ends at a named End Phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Lingering {
    /// The card whose effect this is.
    pub source: InstanceId,
    pub kind: LingeringKind,
    /// Expires in the End Phase of this turn number.
    pub expires_turn: u32,
}

/// One physical card and everything the rules track about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInstance {
    pub card: CardIndex,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub location: Location,
    pub position: Position,
    /// Turn in which the card was last Set, for rules that wait a turn.
    pub turn_set: Option<u32>,
    pub turn_entered_field: Option<u32>,
    pub position_changed_this_turn: bool,
    /// Attacks declared this turn (Hayabusa Knight may make two).
    pub attacks_this_turn: u8,
    /// The monster that destroyed this card by battle, and in which turn
    /// (Sword Hunter, Vampire Baby).
    pub destroyed_by_battle: Option<(InstanceId, u32)>,
    /// For an Equip Spell on the field: the monster it is equipped to.
    pub equipped_to: Option<InstanceId>,
    /// A script-owned counter (Swords of Revealing Light counts End Phases).
    pub counter: u8,
    /// Bit `i` set: effect `i` was used this turn (once-per-turn effects).
    pub effect_uses: u8,
    /// Summoned the way its text requires (Fusion, Ritual, a summon
    /// procedure); kept in the Graveyard and banished, cleared when the card
    /// goes back to the hand or a Deck.
    pub properly_summoned: bool,
}

impl CardInstance {
    pub(crate) fn in_deck(card: CardIndex, owner: PlayerId) -> Self {
        Self {
            card,
            owner,
            controller: owner,
            location: Location::Deck,
            position: Position::FaceDown,
            turn_set: None,
            turn_entered_field: None,
            position_changed_this_turn: false,
            attacks_this_turn: 0,
            destroyed_by_battle: None,
            equipped_to: None,
            counter: 0,
            properly_summoned: false,
            effect_uses: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BattleStep {
    Start,
    Battle,
    Damage,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Draw,
    Standby,
    Main1,
    Battle(BattleStep),
    Main2,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameResult {
    Win { winner: PlayerId, reason: WinReason },
    Draw { reason: DrawReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WinReason {
    LifePoints,
    DeckOut,
    Exodia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrawReason {
    BothLifePointsZero,
    TurnLimit,
}

/// One player's side of the table. Cards are referred to by [`InstanceId`];
/// `deck` holds its top card last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub lp: u32,
    pub deck: Vec<InstanceId>,
    pub hand: Vec<InstanceId>,
    pub graveyard: Vec<InstanceId>,
    pub extra: Vec<InstanceId>,
    pub banished: Vec<InstanceId>,
    pub monster_zones: Vec<Option<InstanceId>>,
    pub spell_trap_zones: Vec<Option<InstanceId>>,
    pub field_zone: Option<InstanceId>,
    /// Normal Summons, Tribute Summons and Sets used this turn.
    pub summons_used: u8,
    /// Monsters this player controlled that went from the field to their
    /// Graveyard this turn (Last Will).
    pub monsters_to_graveyard: u8,
    /// Monsters that entered the field this turn, for `max_monsters_entering_per_turn`.
    pub monsters_entered: u8,
    /// Card names whose "only once per turn" effect this player used this
    /// turn (Sangan, Witch of the Black Forest).
    pub names_used: Vec<CardId>,
    /// Card names this player may not activate, as cards or effects, for the
    /// rest of the turn (the card Sangan or Witch added).
    pub names_locked: Vec<CardId>,
    /// This player's next draw in a Draw Phase is skipped (Time Seal).
    pub skip_draw: bool,
    /// This player's next Standby Phase is skipped (Solomon's Lawbook).
    pub skip_standby: bool,
    /// This player's next turn is skipped (Gamble).
    pub skip_turn: bool,
}

impl PlayerState {
    pub(crate) fn new(lp: u32, monster_zones: usize, spell_trap_zones: usize) -> Self {
        Self {
            lp,
            deck: Vec::new(),
            hand: Vec::new(),
            graveyard: Vec::new(),
            extra: Vec::new(),
            banished: Vec::new(),
            monster_zones: vec![None; monster_zones],
            spell_trap_zones: vec![None; spell_trap_zones],
            field_zone: None,
            summons_used: 0,
            monsters_to_graveyard: 0,
            monsters_entered: 0,
            names_used: Vec::new(),
            names_locked: Vec::new(),
            skip_draw: false,
            skip_standby: false,
            skip_turn: false,
        }
    }

    pub fn monsters(&self) -> impl Iterator<Item = InstanceId> + '_ {
        self.monster_zones.iter().flatten().copied()
    }
}
