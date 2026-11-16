use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::DataError;

/// The house rules from `rules.toml`. The engine reads these once at game
/// start and checks every limit in one place; nothing here is hard-coded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub deck: DeckRules,
    pub field: FieldRules,
    pub hand: HandRules,
    pub life: LifeRules,
    pub turn: TurnRules,
    pub summon: SummonRules,
    pub cards: CardRules,
    pub win: WinRules,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckRules {
    pub main_min: usize,
    pub main_max: usize,
    pub extra_max: usize,
    pub default_copy_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRules {
    pub monster_zones: usize,
    pub spell_trap_zones: usize,
    pub field_spells_on_field: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandLimitCheck {
    /// Official: discard down to the limit in your own End Phase.
    EndPhase,
    /// Owner's game: after your Draw Phase, send cards above the limit to
    /// the Graveyard. The hand is uncapped for the rest of the turn.
    AfterDraw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandRules {
    pub opening_size: usize,
    pub max_size: usize,
    pub hand_limit_check: HandLimitCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifeRules {
    pub starting_lp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnRules {
    pub first_player_skips_draw: bool,
    pub first_player_can_attack: bool,
    /// Whether a Trap may be activated in the same turn it was Set.
    pub trap_activation_turn_set: bool,
    /// Whether a Spell may target a monster that was Set this turn.
    pub spell_target_monster_set_this_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummonRules {
    pub normal_or_tribute_per_turn: usize,
    pub tributes_level_5_6: usize,
    pub tributes_level_7_plus: usize,
    pub summon_in_main_phase_2: bool,
    pub flip_summon_counts_toward_limit: bool,
    pub special_summon_counts_toward_limit: bool,
    /// 0 means no cap.
    pub max_monsters_entering_per_turn: usize,
    pub fusion_summons: bool,
    pub ritual_summons: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextVersion {
    Modern,
    Original2002,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardRules {
    pub text_version: TextVersion,
    /// Path to `cards.json`, relative to the data root.
    pub cards: PathBuf,
    /// Path to `banlist.json`, relative to the data root.
    pub banlist: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WinRules {
    pub deck_out_loses: bool,
    pub exodia: bool,
}

impl Rules {
    pub fn load(path: &Path) -> Result<Self, DataError> {
        let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let rules: Rules = toml::from_str(&text).map_err(|source| DataError::Toml {
            path: path.to_path_buf(),
            source,
        })?;
        rules.validate()?;
        Ok(rules)
    }

    /// Rejects values that can never describe a playable game.
    pub fn validate(&self) -> Result<(), DataError> {
        let invalid = |msg: String| Err(DataError::InvalidRules(msg));
        if self.deck.main_min == 0 || self.deck.main_min > self.deck.main_max {
            return invalid(format!(
                "deck.main_min ({}) must be at least 1 and at most deck.main_max ({})",
                self.deck.main_min, self.deck.main_max
            ));
        }
        if self.deck.default_copy_limit == 0 {
            return invalid("deck.default_copy_limit must be at least 1".into());
        }
        if self.field.monster_zones == 0 || self.field.spell_trap_zones == 0 {
            return invalid("field zones must be at least 1".into());
        }
        if self.hand.opening_size > self.deck.main_min {
            return invalid(format!(
                "hand.opening_size ({}) must not exceed deck.main_min ({})",
                self.hand.opening_size, self.deck.main_min
            ));
        }
        if self.life.starting_lp == 0 {
            return invalid("life.starting_lp must be above 0".into());
        }
        if self.summon.tributes_level_7_plus < self.summon.tributes_level_5_6 {
            return invalid(
                "summon.tributes_level_7_plus must not be below summon.tributes_level_5_6".into(),
            );
        }
        Ok(())
    }
}
