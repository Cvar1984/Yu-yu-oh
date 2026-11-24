//! What the neural network sees (M6, `docs/m6-plan.md`): a decision turned
//! into numbers, from the decider's `PlayerView` only, so no hidden card
//! ever reaches it.
//!
//! - every card the viewer can see (or knows is there, face-down or in a
//!   hidden hand) is a `CARD_F` vector of its features, never its id, so a
//!   ban list change or a new script does not break a trained model;
//! - the game is a `GLOBAL_F` vector: Life Points, turn, phase, decision,
//!   window, zone counts, the chain;
//! - every legal action is an `ACTION_F` vector: its kind and details,
//!   then the card vectors of its subject, its targets and its costs;
//! - every lingering effect (Swords' lock, Change of Heart's return, a
//!   stat change until the End Phase) is a `LINGER_F` vector.
//!
//! The viewer's own Deck is among the cards, by contents only (place
//! "Deck", never its order); cards they remember show as known
//! (`ygo-engine/src/memory.rs`); a monster's Type is its current one; the
//! house rules are part of the game vector.
//!
//! Each block has a fixed width: Types, attributes and tags have reserved
//! slots, so a slot's meaning never shifts.

use ygo_data::{CardId, CardKind, HandLimitCheck};
use ygo_engine::{
    Action, BattleStep, CardStats, DecisionKind, EffectKind, InstanceId, Lingering, LingeringKind,
    Observation, Phase, Place, PlayerView, Position, Shared, SpellTrapType, Tag, VisibleCard,
    WindowKind,
};

/// The official monster Types; any other Type takes the last slot.
const RACES: [&str; 26] = [
    "Aqua",
    "Beast",
    "Beast-Warrior",
    "Creator-God",
    "Cyberse",
    "Dinosaur",
    "Divine-Beast",
    "Dragon",
    "Fairy",
    "Fiend",
    "Fish",
    "Illusion",
    "Insect",
    "Machine",
    "Plant",
    "Psychic",
    "Pyro",
    "Reptile",
    "Rock",
    "Sea Serpent",
    "Spellcaster",
    "Thunder",
    "Warrior",
    "Winged Beast",
    "Wyrm",
    "Zombie",
];
const ATTRIBUTES: [&str; 7] = ["DARK", "DIVINE", "EARTH", "FIRE", "LIGHT", "WATER", "WIND"];
/// Slots kept for tags (`Tag::ALL` grows at its end).
const TAG_SLOTS: usize = 32;
/// Answer options told apart (`Action::Answer`); later ones share the last.
const OPTION_SLOTS: usize = 8;
/// Slots kept for lingering effect kinds.
const LINGER_SLOTS: usize = 32;

/// Where a card is, for the encoding: the observation's places, plus the
/// viewer's own Deck.
#[derive(Clone, Copy)]
enum At {
    Seen(Place),
    Deck,
}

/// The place, side and position numbers at the end of every card vector.
const PLACE_F: usize = 10 + 2 + 5;

/// Numbers per card.
pub const CARD_F: usize = 2 // known, hidden
    + 6 // kind
    + 7 // Spell/Trap subtype
    + 5 // Level, printed ATK and DEF, current ATK and DEF
    + ATTRIBUTES.len() + 1
    + RACES.len() + 1
    + 4 // Flip, Token, Extra Deck, scripted
    + 7 // effects of each kind
    + 3 // fastest speed, once per turn, Damage Step
    + 15 // continuous hooks, targets, costs
    + TAG_SLOTS
    + 10 // place (with the viewer's Deck)
    + 2 // the viewer's, the opponent's
    + 5; // position

/// Numbers for the game.
pub const GLOBAL_F: usize = 2 // Life Points
    + 2 // turn, the viewer's turn
    + 9 // phase
    + 7 // decision kind
    + 9 // window kind (or none)
    + 1 // the viewer holds priority
    + 2 * 8 // zone counts, each side
    + 6 // chain, its top link the viewer's, question, revealed, summon pending, lingering
    + 25; // the house rules

/// Numbers per legal action.
pub const ACTION_F: usize = 14 // kind
    + 1 // face-down
    + 5 // position
    + 1 // effect number
    + OPTION_SLOTS
    + 4 // targets, costs, tributes, a direct attack
    + 3 * CARD_F; // the card, its targets (mean), its costs or tributes (mean)

/// Numbers per lingering effect.
pub const LINGER_F: usize = LINGER_SLOTS
    + 3 // the player it names: the viewer, the opponent, none
    + 2 // ATK and DEF change
    + 2 // turns left, a count (a zone, Standby Phases)
    + 2 * CARD_F; // its source, the card it names

/// One decision as numbers. `cards` holds `cards.len() / CARD_F` card
/// vectors; `actions` one vector per legal action, in `view.actions()`
/// order; `lingering` one per lingering effect.
#[derive(Debug, Clone, PartialEq)]
pub struct Encoded {
    pub cards: Vec<f32>,
    pub globals: Vec<f32>,
    pub actions: Vec<f32>,
    pub lingering: Vec<f32>,
}

impl Encoded {
    pub fn card_count(&self) -> usize {
        self.cards.len() / CARD_F
    }

    pub fn action_count(&self) -> usize {
        self.actions.len() / ACTION_F
    }

    pub fn lingering_count(&self) -> usize {
        self.lingering.len() / LINGER_F
    }
}

/// Encodes the decision `view` is about.
pub fn encode(view: &PlayerView) -> Encoded {
    let enc = Encoder {
        obs: view.observation(),
        shared: view.shared(),
    };
    let mut cards = Vec::new();
    enc.all_cards(&mut cards);
    for id in view.own_deck() {
        let start = cards.len();
        enc.known(
            id,
            None,
            None,
            At::Deck,
            Some(true),
            Position::FaceDown,
            &mut cards,
        );
        debug_assert_eq!(cards.len() - start, CARD_F);
    }
    let mut globals = Vec::with_capacity(GLOBAL_F);
    enc.globals(&mut globals);
    debug_assert_eq!(globals.len(), GLOBAL_F);
    let mut actions = Vec::with_capacity(view.actions().len() * ACTION_F);
    for a in view.actions() {
        let start = actions.len();
        enc.action(a, &mut actions);
        debug_assert_eq!(actions.len() - start, ACTION_F, "{a:?}");
    }
    let lingering_effects = &view.observation().lingering;
    let mut lingering = Vec::with_capacity(lingering_effects.len() * LINGER_F);
    for l in lingering_effects {
        let start = lingering.len();
        enc.lingering(l, &mut lingering);
        debug_assert_eq!(lingering.len() - start, LINGER_F, "{l:?}");
    }
    Encoded {
        cards,
        globals,
        actions,
        lingering,
    }
}

struct Encoder<'a> {
    obs: &'a Observation,
    shared: &'a Shared,
}

/// Pushes a one-hot block of `width` with `index` set (none if `None`).
fn one_hot(out: &mut Vec<f32>, width: usize, index: Option<usize>) {
    let start = out.len();
    out.resize(start + width, 0.0);
    if let Some(i) = index {
        out[start + i.min(width - 1)] = 1.0;
    }
}

fn flag(out: &mut Vec<f32>, on: bool) {
    out.push(if on { 1.0 } else { 0.0 });
}

impl Encoder<'_> {
    fn stats(&self, card: CardId) -> Option<(&CardStats, Option<&ygo_engine::CardScript>)> {
        let pool = &self.shared.pool;
        let index = pool.index_of(card)?;
        Some((pool.get(index), self.shared.script(index)))
    }

    /// Every card the viewer knows of: both sides' hands, zones,
    /// Graveyards, banished cards and Extra Decks, then the cards shown for
    /// a choice and the chain.
    fn all_cards(&self, out: &mut Vec<f32>) {
        let viewer = self.obs.viewer;
        for p in [viewer, viewer.opponent()] {
            let side = &self.obs.sides[p.index()];
            let mine = Some(p == viewer);
            let piles: [(&[VisibleCard], Place); 4] = [
                (&side.hand, Place::Hand),
                (&side.graveyard, Place::Graveyard),
                (&side.banished, Place::Banished),
                (&side.extra, Place::Extra),
            ];
            for (pile, place) in piles {
                for c in pile {
                    self.card(c, place, mine, out);
                }
            }
            for (i, c) in side.monster_zones.iter().enumerate() {
                if let Some(c) = c {
                    self.card(c, Place::Monster(i), mine, out);
                }
            }
            for (i, c) in side.spell_trap_zones.iter().enumerate() {
                if let Some(c) = c {
                    self.card(c, Place::SpellTrap(i), mine, out);
                }
            }
            if let Some(c) = &side.field_zone {
                self.card(c, Place::Field, mine, out);
            }
        }
        for c in &self.obs.revealed {
            self.card(c, Place::Revealed, None, out);
        }
        for link in &self.obs.chain {
            self.known(
                link.card,
                Some(link.instance),
                None,
                At::Seen(Place::Chain),
                Some(link.controller == viewer),
                Position::FaceUp,
                out,
            );
        }
    }

    fn card(&self, c: &VisibleCard, place: Place, mine: Option<bool>, out: &mut Vec<f32>) {
        match c {
            VisibleCard::Known {
                instance,
                card,
                position,
                stats,
            } => self.known(
                *card,
                Some(*instance),
                *stats,
                At::Seen(place),
                mine,
                *position,
                out,
            ),
            VisibleCard::Hidden { position, .. } => {
                let start = out.len();
                flag(out, false);
                flag(out, true);
                out.resize(start + CARD_F - PLACE_F, 0.0);
                self.where_and_how(At::Seen(place), mine, *position, out);
                debug_assert_eq!(out.len() - start, CARD_F);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn known(
        &self,
        card: CardId,
        instance: Option<InstanceId>,
        current: Option<(i32, i32)>,
        place: At,
        mine: Option<bool>,
        position: Position,
        out: &mut Vec<f32>,
    ) {
        let start = out.len();
        let Some((s, script)) = self.stats(card) else {
            // Not in the pool: cannot happen in a game; encode as hidden.
            flag(out, false);
            flag(out, true);
            out.resize(start + CARD_F - PLACE_F, 0.0);
            self.where_and_how(place, mine, position, out);
            return;
        };
        flag(out, true);
        flag(out, false);
        let kind = [
            CardKind::Normal,
            CardKind::Effect,
            CardKind::Fusion,
            CardKind::Ritual,
            CardKind::Spell,
            CardKind::Trap,
        ]
        .iter()
        .position(|k| *k == s.kind);
        one_hot(out, 6, kind);
        let subtype = [
            SpellTrapType::Normal,
            SpellTrapType::QuickPlay,
            SpellTrapType::Continuous,
            SpellTrapType::Equip,
            SpellTrapType::Field,
            SpellTrapType::Ritual,
            SpellTrapType::Counter,
        ]
        .iter()
        .position(|t| *t == s.subtype);
        one_hot(out, 7, subtype);
        let (atk, def) = current.unwrap_or((s.atk, s.def));
        out.extend([
            f32::from(s.level) / 12.0,
            s.atk as f32 / 5000.0,
            s.def as f32 / 5000.0,
            atk as f32 / 5000.0,
            def as f32 / 5000.0,
        ]);
        let monster = s.is_monster();
        let attribute = s.attribute.as_deref().map(|a| {
            ATTRIBUTES
                .iter()
                .position(|x| *x == a)
                .unwrap_or(ATTRIBUTES.len())
        });
        one_hot(out, ATTRIBUTES.len() + 1, attribute.filter(|_| monster));
        // The current Type, when an effect changed it (DNA Surgery).
        let changed = instance.and_then(|i| self.obs.types.iter().find(|t| t.0 == i));
        let race_now = changed.map_or(s.race.as_str(), |t| t.1.as_str());
        let race = monster.then(|| {
            RACES
                .iter()
                .position(|r| *r == race_now)
                .unwrap_or(RACES.len())
        });
        one_hot(out, RACES.len() + 1, race);
        flag(out, s.flip);
        flag(out, s.token);
        flag(out, s.is_extra_deck());
        flag(out, script.is_some());
        self.script_features(script, out);
        self.where_and_how(place, mine, position, out);
        debug_assert_eq!(out.len() - start, CARD_F);
    }

    /// What the card's script does, as counts and flags.
    fn script_features(&self, script: Option<&ygo_engine::CardScript>, out: &mut Vec<f32>) {
        let effects = script.map_or(&[][..], |s| &s.effects[..]);
        for kind in 0..7 {
            let n = effects
                .iter()
                .filter(|e| {
                    let k = match e.kind {
                        EffectKind::Ignition => 0,
                        EffectKind::Activate => 1,
                        EffectKind::Trigger { .. } => 2,
                        EffectKind::Flip => 3,
                        EffectKind::Quick => 4,
                        EffectKind::Continuous => 5,
                        EffectKind::Procedure => 6,
                    };
                    k == kind
                })
                .count();
            out.push(n.min(3) as f32 / 3.0);
        }
        let speed = effects.iter().map(|e| e.speed).max().unwrap_or(0);
        out.push(f32::from(speed) / 3.0);
        flag(out, effects.iter().any(|e| e.once_per_turn));
        flag(out, effects.iter().any(|e| e.damage_step));
        let any = |f: fn(&ygo_engine::Effect) -> bool| effects.iter().any(f);
        flag(out, any(|e| e.modify.is_some()));
        flag(out, any(|e| e.restrict.is_some()));
        flag(out, any(|e| e.on_attack.is_some()));
        flag(out, any(|e| e.attack_lock.is_some()));
        flag(out, any(|e| e.on_battle_damage.is_some()));
        flag(out, any(|e| e.replace_battle_destruction.is_some()));
        flag(out, any(|e| e.lock_position.is_some()));
        flag(out, any(|e| e.pierce.is_some()));
        flag(out, any(|e| e.negate.is_some()));
        flag(out, any(|e| e.forbid.is_some()));
        flag(out, any(|e| e.race.is_some()));
        flag(out, any(|e| e.spell_immune.is_some()));
        flag(out, any(|e| e.unattackable.is_some()));
        flag(out, any(|e| e.targets.is_some()));
        flag(out, any(|e| e.cost.is_some() || e.cost_choices.is_some()));
        let start = out.len();
        out.resize(start + TAG_SLOTS, 0.0);
        if let Some(script) = script {
            for (i, tag) in Tag::ALL.iter().enumerate().take(TAG_SLOTS) {
                if script.has(*tag) {
                    out[start + i] = 1.0;
                }
            }
        }
    }

    /// Place, side and position: the last `PLACE_F` numbers of every card.
    fn where_and_how(&self, place: At, mine: Option<bool>, position: Position, out: &mut Vec<f32>) {
        let p = match place {
            At::Seen(Place::Hand) => 0,
            At::Seen(Place::Monster(_)) => 1,
            At::Seen(Place::SpellTrap(_)) => 2,
            At::Seen(Place::Field) => 3,
            At::Seen(Place::Graveyard) => 4,
            At::Seen(Place::Banished) => 5,
            At::Seen(Place::Extra) => 6,
            At::Seen(Place::Revealed) => 7,
            At::Seen(Place::Chain) => 8,
            At::Deck => 9,
        };
        one_hot(out, 10, Some(p));
        flag(out, mine == Some(true));
        flag(out, mine == Some(false));
        let pos = match position {
            Position::FaceUpAttack => 0,
            Position::FaceUpDefense => 1,
            Position::FaceDownDefense => 2,
            Position::FaceDown => 3,
            Position::FaceUp => 4,
        };
        one_hot(out, 5, Some(pos));
    }

    fn globals(&self, out: &mut Vec<f32>) {
        let obs = self.obs;
        let viewer = obs.viewer;
        let (me, them) = (obs.me(), obs.opponent());
        out.push(me.lp as f32 / 8000.0);
        out.push(them.lp as f32 / 8000.0);
        out.push(obs.turn as f32 / 50.0);
        flag(out, obs.turn_player == viewer);
        let phase = match obs.phase {
            Phase::Draw => 0,
            Phase::Standby => 1,
            Phase::Main1 => 2,
            Phase::Battle(BattleStep::Start) => 3,
            Phase::Battle(BattleStep::Battle) => 4,
            Phase::Battle(BattleStep::Damage) => 5,
            Phase::Battle(BattleStep::End) => 6,
            Phase::Main2 => 7,
            Phase::End => 8,
        };
        one_hot(out, 9, Some(phase));
        let decision = obs.decision.map(|d| match d.kind {
            DecisionKind::HandLimit => 0,
            DecisionKind::MainPhase => 1,
            DecisionKind::BattlePhase => 2,
            DecisionKind::Choose => 3,
            DecisionKind::Respond => 4,
            DecisionKind::Triggers => 5,
            DecisionKind::Replay => 6,
        });
        one_hot(out, 7, decision);
        let window = match obs.window.map(|w| w.0) {
            None => 0,
            Some(WindowKind::Open) => 1,
            Some(WindowKind::SummonDeclared) => 2,
            Some(WindowKind::SummonSuccess) => 3,
            Some(WindowKind::AttackDeclared) => 4,
            Some(WindowKind::DamageCalculation) => 5,
            Some(WindowKind::AfterBattle) => 6,
            Some(WindowKind::Chain) => 7,
            Some(WindowKind::AfterChain) => 8,
        };
        one_hot(out, 9, Some(window));
        flag(out, obs.window.is_some_and(|w| w.1 == viewer));
        for side in [me, them] {
            let monsters = side.monster_zones.iter().flatten().count();
            let spells = side.spell_trap_zones.iter().flatten().count();
            out.extend([
                side.hand.len() as f32 / 10.0,
                side.deck_count as f32 / 40.0,
                side.extra_count as f32 / 15.0,
                side.graveyard.len() as f32 / 20.0,
                side.banished.len() as f32 / 20.0,
                monsters as f32 / 5.0,
                spells as f32 / 5.0,
                if side.field_zone.is_some() { 1.0 } else { 0.0 },
            ]);
        }
        out.push(obs.chain.len() as f32 / 5.0);
        flag(
            out,
            obs.chain.last().is_some_and(|l| l.controller == viewer),
        );
        out.push(obs.question.len() as f32 / OPTION_SLOTS as f32);
        out.push(obs.revealed.len() as f32 / 10.0);
        flag(out, obs.pending_summon.is_some());
        out.push(obs.lingering.len() as f32 / 10.0);
        self.rules(out);
    }

    /// The house rules (`rules.toml`), so one network can play under any.
    fn rules(&self, out: &mut Vec<f32>) {
        let r = &self.shared.rules;
        out.extend([
            r.deck.main_min as f32 / 60.0,
            r.deck.main_max as f32 / 60.0,
            r.deck.extra_max as f32 / 15.0,
            r.deck.default_copy_limit as f32 / 3.0,
            r.field.monster_zones as f32 / 5.0,
            r.field.spell_trap_zones as f32 / 5.0,
            r.field.field_spells_on_field as f32 / 2.0,
            r.hand.opening_size as f32 / 10.0,
            r.hand.max_size as f32 / 10.0,
            r.life.starting_lp as f32 / 8000.0,
            r.summon.normal_or_tribute_per_turn as f32 / 5.0,
            r.summon.tributes_level_5_6 as f32 / 3.0,
            r.summon.tributes_level_7_plus as f32 / 3.0,
            r.summon.max_monsters_entering_per_turn as f32 / 5.0,
        ]);
        flag(out, r.hand.hand_limit_check == HandLimitCheck::AfterDraw);
        for on in [
            r.turn.first_player_skips_draw,
            r.turn.second_player_skips_draw,
            r.turn.first_player_can_attack,
            r.turn.trap_activation_turn_set,
            r.turn.spell_target_monster_set_this_turn,
            r.summon.summon_in_main_phase_2,
            r.summon.flip_summon_counts_toward_limit,
            r.summon.special_summon_counts_toward_limit,
            r.summon.fusion_summons,
            r.summon.ritual_summons,
        ] {
            flag(out, on);
        }
    }

    /// One lingering effect: its kind, the player and card it names, its
    /// numbers, and its source. Exhaustive on purpose: a new kind needs a
    /// slot here.
    fn lingering(&self, l: &Lingering, out: &mut Vec<f32>) {
        use LingeringKind as K;
        // (slot, the card it names, the player it names, ATK, DEF, a count)
        let (slot, card, player, atk, def, count) = match l.kind {
            K::ReturnControl { card, to } => (0, Some(card), Some(to), 0, 0, 0),
            K::Stats { card, atk, def } => (1, Some(card), None, atk, def, 0),
            K::Destroy { card } => (2, Some(card), None, 0, 0, 0),
            K::CannotAttack { card } => (3, Some(card), None, 0, 0, 0),
            K::DamageCalcStats { card, atk, def } => (4, Some(card), None, atk, def, 0),
            K::Marked { card } => (5, Some(card), None, 0, 0, 0),
            K::BattleShield { player } => (6, None, Some(player), 0, 0, 0),
            K::PositionLocked { card } => (7, Some(card), None, 0, 0, 0),
            K::NoSpellsOrTraps => (8, None, None, 0, 0, 0),
            K::Negated { card } => (9, Some(card), None, 0, 0, 0),
            K::ZoneLocked { player, zone } => (10, None, Some(player), 0, 0, zone),
            K::ReturnToHand {
                card,
                player,
                standbys,
            } => (11, Some(card), Some(player), 0, 0, standbys),
            K::ReturnToField {
                card, player, zone, ..
            } => (12, Some(card), Some(player), 0, 0, zone),
            K::Borrowed { card, player } => (13, Some(card), Some(player), 0, 0, 0),
            K::HatMonster { card } => (14, Some(card), None, 0, 0, 0),
            K::Race { card } => (15, Some(card), None, 0, 0, 0),
            K::StatChangesReversed => (16, None, None, 0, 0, 0),
            K::MustTribute { player, card } => (17, Some(card), Some(player), 0, 0, 0),
            K::NoBattlePhase { player } => (18, None, Some(player), 0, 0, 0),
            K::NoSummons { player } => (19, None, Some(player), 0, 0, 0),
        };
        one_hot(out, LINGER_SLOTS, Some(slot));
        let viewer = self.obs.viewer;
        flag(out, player == Some(viewer));
        flag(out, player == Some(viewer.opponent()));
        flag(out, player.is_none());
        out.extend([
            atk as f32 / 5000.0,
            def as f32 / 5000.0,
            l.expires_turn.saturating_sub(self.obs.turn).min(8) as f32 / 4.0,
            f32::from(count) / 5.0,
        ]);
        self.instance(l.source, out);
        match card {
            Some(c) => self.instance(c, out),
            None => out.resize(out.len() + CARD_F, 0.0),
        }
    }

    /// The card vector of instance `id` as the viewer sees it; a hidden
    /// vector if they cannot see it.
    fn instance(&self, id: InstanceId, out: &mut Vec<f32>) {
        let viewer = self.obs.viewer;
        if let Some(f) = self.obs.find(id) {
            self.card(f.card, f.place, f.owner.map(|o| o == viewer), out);
        } else if let Some(link) = self.obs.chain.iter().find(|l| l.instance == id) {
            self.known(
                link.card,
                Some(id),
                None,
                At::Seen(Place::Chain),
                Some(link.controller == viewer),
                Position::FaceUp,
                out,
            );
        } else {
            self.card(
                &VisibleCard::Hidden {
                    instance: id,
                    position: Position::FaceDown,
                },
                Place::Revealed,
                None,
                out,
            );
        }
    }

    /// The mean card vector of `ids`; zeros when there are none.
    fn mean(&self, ids: &[InstanceId], out: &mut Vec<f32>) {
        let start = out.len();
        out.resize(start + CARD_F, 0.0);
        if ids.is_empty() {
            return;
        }
        let mut one = Vec::with_capacity(CARD_F);
        for &id in ids {
            one.clear();
            self.instance(id, &mut one);
            for (o, x) in out[start..].iter_mut().zip(&one) {
                *o += x;
            }
        }
        for o in &mut out[start..] {
            *o /= ids.len() as f32;
        }
    }

    fn action(&self, a: &Action, out: &mut Vec<f32>) {
        let kind = match a {
            Action::Discard { .. } => 0,
            Action::Summon { .. } => 1,
            Action::FlipSummon { .. } => 2,
            Action::ChangePosition { .. } => 3,
            Action::SetSpellTrap { .. } => 4,
            Action::Activate { .. } => 5,
            Action::ProcedureSummon { .. } => 6,
            Action::Choose { .. } => 7,
            Action::Answer { .. } => 8,
            Action::Pass => 9,
            Action::EnterBattlePhase => 10,
            Action::Attack { .. } => 11,
            Action::EndBattlePhase => 12,
            Action::EndTurn => 13,
        };
        one_hot(out, 14, Some(kind));
        let none: &[InstanceId] = &[];
        let (card, targets, costs, face_down, position, effect, option, direct) = match a {
            Action::Discard { card }
            | Action::FlipSummon { card }
            | Action::ChangePosition { card }
            | Action::SetSpellTrap { card }
            | Action::Choose { card } => (Some(*card), none, none, false, None, 0, None, false),
            Action::Summon {
                card,
                tributes,
                face_down,
            } => (
                Some(*card),
                none,
                &tributes[..],
                *face_down,
                None,
                0,
                None,
                false,
            ),
            Action::Activate {
                card,
                effect,
                targets,
                cost,
            } => (
                Some(*card),
                &targets[..],
                &cost[..],
                false,
                None,
                *effect,
                None,
                false,
            ),
            Action::ProcedureSummon {
                card,
                effect,
                cost,
                position,
            } => (
                Some(*card),
                none,
                &cost[..],
                false,
                Some(*position),
                *effect,
                None,
                false,
            ),
            Action::Answer { option } => (None, none, none, false, None, 0, Some(*option), false),
            Action::Attack { attacker, target } => (
                Some(*attacker),
                target.as_slice(),
                none,
                false,
                None,
                0,
                None,
                target.is_none(),
            ),
            Action::Pass | Action::EnterBattlePhase | Action::EndBattlePhase | Action::EndTurn => {
                (None, none, none, false, None, 0, None, false)
            }
        };
        flag(out, face_down);
        let pos = position.map(|p| match p {
            Position::FaceUpAttack => 0,
            Position::FaceUpDefense => 1,
            Position::FaceDownDefense => 2,
            Position::FaceDown => 3,
            Position::FaceUp => 4,
        });
        one_hot(out, 5, pos);
        out.push(f32::from(effect) / 8.0);
        one_hot(out, OPTION_SLOTS, option.map(usize::from));
        out.extend([
            targets.len() as f32 / 5.0,
            if matches!(a, Action::Summon { .. }) {
                0.0
            } else {
                costs.len() as f32 / 5.0
            },
            if matches!(a, Action::Summon { .. }) {
                costs.len() as f32 / 3.0
            } else {
                0.0
            },
        ]);
        flag(out, direct);
        match card {
            Some(c) => self.instance(c, out),
            None => out.resize(out.len() + CARD_F, 0.0),
        }
        self.mean(targets, out);
        self.mean(costs, out);
    }
}
