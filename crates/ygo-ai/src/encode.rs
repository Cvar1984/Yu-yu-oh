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
//!   then the card vectors of its subject, its targets and its costs.
//!
//! Each block has a fixed width: Types, attributes and tags have reserved
//! slots, so a slot's meaning never shifts.

use ygo_data::CardKind;
use ygo_engine::{
    Action, BattleStep, CardStats, DecisionKind, EffectKind, InstanceId, Observation, Phase, Place,
    PlayerView, Position, Shared, SpellTrapType, Tag, VisibleCard, WindowKind,
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
    + 9 // place
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
    + 6; // chain, its top link the viewer's, question, revealed, summon pending, lingering

/// Numbers per legal action.
pub const ACTION_F: usize = 14 // kind
    + 1 // face-down
    + 5 // position
    + 1 // effect number
    + OPTION_SLOTS
    + 4 // targets, costs, tributes, a direct attack
    + 3 * CARD_F; // the card, its targets (mean), its costs or tributes (mean)

/// One decision as numbers. `cards` holds `cards.len() / CARD_F` card
/// vectors; `actions` one vector per legal action, in `view.actions()`
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct Encoded {
    pub cards: Vec<f32>,
    pub globals: Vec<f32>,
    pub actions: Vec<f32>,
}

impl Encoded {
    pub fn card_count(&self) -> usize {
        self.cards.len() / CARD_F
    }

    pub fn action_count(&self) -> usize {
        self.actions.len() / ACTION_F
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
    let mut globals = Vec::with_capacity(GLOBAL_F);
    enc.globals(&mut globals);
    debug_assert_eq!(globals.len(), GLOBAL_F);
    let mut actions = Vec::with_capacity(view.actions().len() * ACTION_F);
    for a in view.actions() {
        let start = actions.len();
        enc.action(a, &mut actions);
        debug_assert_eq!(actions.len() - start, ACTION_F, "{a:?}");
    }
    Encoded {
        cards,
        globals,
        actions,
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
    fn stats(
        &self,
        card: ygo_data::CardId,
    ) -> Option<(&CardStats, Option<&ygo_engine::CardScript>)> {
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
                None,
                Place::Chain,
                Some(link.controller == viewer),
                Position::FaceUp,
                out,
            );
        }
    }

    fn card(&self, c: &VisibleCard, place: Place, mine: Option<bool>, out: &mut Vec<f32>) {
        match c {
            VisibleCard::Known {
                card,
                position,
                stats,
                ..
            } => self.known(*card, *stats, place, mine, *position, out),
            VisibleCard::Hidden { position, .. } => {
                let start = out.len();
                flag(out, false);
                flag(out, true);
                out.resize(start + CARD_F - 9 - 2 - 5, 0.0);
                self.where_and_how(place, mine, *position, out);
                debug_assert_eq!(out.len() - start, CARD_F);
            }
        }
    }

    fn known(
        &self,
        card: ygo_data::CardId,
        current: Option<(i32, i32)>,
        place: Place,
        mine: Option<bool>,
        position: Position,
        out: &mut Vec<f32>,
    ) {
        let start = out.len();
        let Some((s, script)) = self.stats(card) else {
            // Not in the pool: cannot happen in a game; encode as hidden.
            flag(out, false);
            flag(out, true);
            out.resize(start + CARD_F - 9 - 2 - 5, 0.0);
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
        let race = monster.then(|| {
            RACES
                .iter()
                .position(|r| *r == s.race)
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

    /// Place, side and position: the last 16 numbers of every card.
    fn where_and_how(
        &self,
        place: Place,
        mine: Option<bool>,
        position: Position,
        out: &mut Vec<f32>,
    ) {
        let p = match place {
            Place::Hand => 0,
            Place::Monster(_) => 1,
            Place::SpellTrap(_) => 2,
            Place::Field => 3,
            Place::Graveyard => 4,
            Place::Banished => 5,
            Place::Extra => 6,
            Place::Revealed => 7,
            Place::Chain => 8,
        };
        one_hot(out, 9, Some(p));
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
                None,
                Place::Chain,
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
