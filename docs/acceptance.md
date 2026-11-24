# The prototype's acceptance cases

The handoff asks for the Python prototype's scenario tests (`reference/python_prototype/test_v8.py`, 40 cases, and `test_v9.py`, 37) to hold in the Rust engine. Each case below names the Rust test that covers it. Where the prototype's simplified rules differ from the modern rules, the Rust test follows the modern rules and the note says how. New tests for this port live in `crates/ygo-cards/tests/acceptance.rs` (cases no rulings test covered, and the two engine bugs the prototype found) and `crates/ygo-ai/tests/lookahead.rs` (the bot cases).

Result (2026-11-24): 76 of 77 covered and passing. One finding changed the engine: an attack now replays when the defender's number of monsters changes after the declaration (modern rule), not only when the target leaves. One bot case is open (see the end).

## The prototype's two engine bugs

- A flip effect or other effect that removes the attacker or the target before damage calculation ends that battle with no damage calculation: `an_attacker_removed_in_the_battle_step_deals_no_damage` (`ygo-cards:tests/acceptance.rs`). A face-down monster flipped by an attack battles first; its FLIP effect resolves after damage calculation (`penguin_soldier_attacked_face_down_battles_then_bounces_two`).
- A negated Spell goes to the Graveyard exactly once: `a_negated_spell_goes_to_the_graveyard_once` (`ygo-cards:tests/acceptance.rs`).

## Case by case

| File | Case | Rust tests | Note |
| --- | --- | --- | --- |
| v8 | Normal Monster stats | `normal_monster_stats_match_the_prototype` (acceptance.rs) |  |
| v8 | Mysterious Puppeteer: +500 per Normal Summon | `mysterious_puppeteer_gains_500_per_normal_or_flip_summon_by_either_player` (extra.rs) |  |
| v8 | Wicked Worm Beast returns at own End Phase | `the_wicked_worm_beast_returns_to_the_hand_in_its_controllers_end_phase` (extra.rs) |  |
| v8 | Penguin Soldier flipped by attack bounces 2, no battle | `penguin_soldier_attacked_face_down_battles_then_bounces_two` (acceptance.rs)<br>`penguin_soldier_may_return_up_to_two_monsters_or_nothing` (extra.rs) | Modern: the battle happens first (damage calculation, Penguin destroyed), then the FLIP effect. |
| v8 | Trap Master destroys a Trap, leaves a Spell | `trap_master_destroys_a_set_trap_but_leaves_a_set_spell` (extra.rs) |  |
| v8 | Black Luster Ritual: Level 4+4 tributes | `black_luster_soldier_is_ritual_summoned_and_revived_only_after_that` (acceptance.rs) |  |
| v8 | Black Luster Soldier revive only after proper Summon | `black_luster_soldier_is_ritual_summoned_and_revived_only_after_that` (acceptance.rs)<br>`only_a_properly_summoned_monster_can_be_revived` (modern_rules.rs) |  |
| v8 | Graceful Charity: draw 3 discard 2 | `graceful_charity_draws_three_then_discards_two` (extra.rs) |  |
| v8 | Card Destruction: both refresh hands | `card_destruction_discards_both_hands_and_draws_as_many` (extra.rs) |  |
| v8 | De-Spell destroys a face-up Swords | `de_spell_destroys_a_spell_and_reveals_a_set_trap_without_destroying_it` (extra.rs) |  |
| v8 | Dian Keto +1000, Ookazi 800 | `dian_keto_and_ookazi` (extra.rs) |  |
| v8 | Fairy Meteor Crush pierces | `fairy_meteor_crush_inflicts_piercing_damage` (extra.rs) |  |
| v8 | Invigoration EARTH +400, Sword of Dark Destruction DARK +400 | `invigoration_and_sword_of_dark_destruction_fit_their_attribute_only` (extra.rs) |  |
| v8 | Last Will after losing a monster | `last_will_summons_from_the_deck_after_a_monster_of_yours_went_to_the_graveyard` (extra.rs) |  |
| v8 | Nobleman banishes a face-down Flip monster and its copies | `nobleman_of_crossout_banishes_a_flip_monster_and_every_copy_in_both_decks` (extra.rs) |  |
| v8 | Premature Burial: pay 800, linked equip | `premature_burial_revives_in_attack_position_and_takes_its_monster_down_with_it` (extra.rs) |  |
| v8 | Scapegoat on a direct attack | `scapegoat_turns_a_direct_attack_into_a_replay` (acceptance.rs) | Modern: the Tokens change the defender's monster count, so the attack replays. The engine lacked this rule (it replayed only when the target left) and now has it. |
| v8 | Sheep Tokens cannot be Tributed for a Tribute Summon | `scapegoat_summons_four_sheep_and_locks_other_summons` (extra_2.rs) |  |
| v8 | Soul Exchange: tribute their monster, no Battle Phase | `soul_exchange_tributes_the_opponents_monster_and_ends_the_battle_phase` (extra_2.rs) |  |
| v8 | Backup Soldier returns 3 Normal Monsters <= 1500 | `backup_soldier_returns_three_non_effect_monsters_of_1500_or_less` (extra.rs) |  |
| v8 | Burst Breath + D. Tribe with a non-Dragon | `d_tribe_makes_your_face_up_monsters_dragons_this_turn` (extra_2.rs)<br>`burst_breath_destroys_face_up_monsters_with_def_up_to_the_tributed_atk` (extra.rs) |  |
| v8 | Castle Walls saves a Defense Position monster | `castle_walls_and_reinforcements_last_until_the_end_of_the_turn` (extra.rs) |  |
| v8 | Disappear banishes a big monster from their GY | `disappear_banishes_a_card_from_the_opponents_graveyard` (extra.rs) |  |
| v8 | Dust Tornado destroys Swords, then Sets a card | `dust_tornado_destroys_then_may_set_a_card_from_the_hand` (extra.rs) |  |
| v8 | Enchanted Javelin gains attacker's ATK | `enchanted_javelin_gains_the_attackers_atk` (extra.rs) |  |
| v8 | End Phase traps: Jar of Greed, Just Desserts, Gift | `jar_of_greed_draws_one_and_just_desserts_burns_per_monster` (extra.rs)<br>`gift_of_the_mystical_elf_counts_every_monster_on_the_field` (extra.rs) |  |
| v8 | Light of Intervention: Sets become face-up | `light_of_intervention_turns_sets_into_face_up_defense_summons` (extra_2.rs) |  |
| v8 | Raigeki Break destroys the attacker | `raigeki_break_discards_then_destroys_any_card` (extra.rs)<br>`an_attacker_removed_in_the_battle_step_deals_no_damage` (acceptance.rs) |  |
| v8 | Reinforcements +500 in battle | `castle_walls_and_reinforcements_last_until_the_end_of_the_turn` (extra.rs) |  |
| v8 | Reverse Trap flips an equip bonus | `reverse_trap_reverses_additions_until_the_end_phase` (extra_2.rs) |  |
| v8 | Riryoku Field negates Change of Heart | `riryoku_field_negates_a_spell_that_targets_one_monster` (extra_2.rs) | Rush Recklessly stands in for Change of Heart: the same single-target Spell check. |
| v8 | Shadow Spell stops the attack, -700, released when destroyed | `shadow_spell_and_spellbinding_circle_stop_the_attack_they_answer` (extra.rs) |  |
| v8 | Shift moves the attack to a wall | `shift_moves_an_attack_to_another_of_your_monsters` (extra_2.rs) |  |
| v8 | The Eye of Truth: +1000 if a Spell is in hand | `the_eye_of_truth_reveals_only_the_opponents_hand_and_pays_them_for_spells` (extra.rs) |  |
| v8 | Ultimate Offering: extra Normal Summon for 500 | `ultimate_offering_grants_an_extra_tribute_summon` (extra_2.rs) |  |
| v8 | Waboku: no battle damage, no battle destruction | `waboku_stops_battle_damage_and_battle_destruction_for_the_turn` (extra.rs) |  |
| v8 | Hane-Hane bounces a Fusion to the Extra Deck | `a_bounced_fusion_monster_goes_back_to_the_extra_deck` (modern_rules.rs) |  |
| v8 | Star Boy: WATER +500, FIRE -400 | `every_attribute_booster_raises_one_attribute_and_lowers_another` (tables.rs) | Table test over every attribute booster. |
| v8 | Reaper of the Cards destroys a Trap | `reaper_of_the_cards_destroys_a_trap_and_spares_a_set_spell` (flips.rs) |  |
| v8 | Dead cards stay unplayed | `cards_needing_a_card_outside_the_pool_are_never_offered` (extra.rs) | The prototype's five dead cards all have effects here; only Gryphon Wing (it needs Harpie's Feather Duster, outside the pool) can never be activated. |
| v9 | Toon World: pay 1000, then Toon Summoned Skull (1 tribute), no attack this turn | `toon_world_costs_1000_and_toons_cannot_be_normal_summoned` (toons.rs)<br>`bigger_toons_tribute_by_level` (toons.rs)<br>`a_toon_summoned_this_turn_cannot_attack` (toons.rs) |  |
| v9 | Toon attacks directly over a monster, pays 500 | `a_toon_waits_a_turn_pays_500_and_attacks_directly_over_monsters` (toons.rs) |  |
| v9 | Toons die when Toon World is destroyed | `toons_are_destroyed_when_toon_world_is_destroyed` (toons.rs) |  |
| v9 | Gate Guardian: tribute Sanga, Kazejin, Suijin | `gate_guardian_needs_its_three_parts` (monsters_b.rs)<br>`gate_guardian_is_summoned_by_procedure_and_solemn_judgment_can_negate_it` (modern_rules.rs) |  |
| v9 | Kazejin zeroes an attacker once; attackers avoid it | `sanga_kazejin_and_suijin_zero_the_attackers_atk_once` (monsters_b.rs) | "Attackers avoid it" is bot judgement, not a rule. |
| v9 | Mystic Tomato summons a DARK monster from the Deck | `every_recruiter_may_special_summon_its_attribute_when_destroyed_by_battle` (monsters_a.rs) | Table test over every recruiter. |
| v9 | Nimble Momonga: +1000 and copies face-down | `giant_germ_burns_and_nimble_momonga_heals_then_copies_may_follow` (monsters_a.rs) |  |
| v9 | Cyber Jar destroys all, both reveal 5 | `cyber_jar_summons_every_revealed_small_monster_and_hands_out_the_rest` (flips.rs) |  |
| v9 | Time Wizard: one side's monsters destroyed | `time_wizard_follows_its_coin` (monsters_b.rs) |  |
| v9 | Catapult Turtle launches a borrowed monster | `catapult_turtle_and_cannon_soldier_burn_by_tributing` (monsters_b.rs) |  |
| v9 | Cannon Soldier for lethal | `catapult_turtle_and_cannon_soldier_burn_by_tributing` (monsters_b.rs) |  |
| v9 | Armed Ninja destroys a face-up Spell | `armed_ninja_destroys_a_spell` (flips.rs) |  |
| v9 | Princess of Tsurugi burns 500 per S/T | `princess_of_tsurugi_burns_for_each_opposing_spell_and_trap` (flips.rs) |  |
| v9 | Invader of the Throne swaps control | `invader_of_the_throne_swaps_control_with_an_opposing_monster` (flips.rs) |  |
| v9 | Relinquished ritual + absorb + battle shield | `relinquished_absorbs_a_monster_and_fights_with_its_stats` (ritual.rs)<br>`relinquished_loses_its_absorbed_monster_when_it_leaves` (ritual.rs) |  |
| v9 | Senju searches a Ritual Monster | `senju_and_sonic_bird_search_on_summon` (monsters_a.rs) |  |
| v9 | Elegant Egotist -> Harpie Lady Sisters | `elegant_egotist_calls_harpie_lady_sisters_from_the_deck` (monsters_b.rs) |  |
| v9 | Black Pendant +500, 500 burn when it leaves | `black_pendant_burns_500_when_it_leaves` (remaining.rs) |  |
| v9 | Horn of the Unicorn returns to the Deck top | `horn_of_light_and_horn_of_the_unicorn_come_back_to_the_deck` (remaining.rs) |  |
| v9 | Megamorph doubles when behind, halves theirs when ahead | `megamorph_doubles_when_behind_and_halves_when_ahead` (remaining.rs) |  |
| v9 | Rush Recklessly lets a weaker monster win | `snake_fang_rush_recklessly_and_the_reliable_guardian_last_until_the_end_phase` (oneshot.rs) |  |
| v9 | Snake Fang lowers DEF for the attack | `snake_fang_rush_recklessly_and_the_reliable_guardian_last_until_the_end_phase` (oneshot.rs) |  |
| v9 | The Reliable Guardian +700 DEF | `snake_fang_rush_recklessly_and_the_reliable_guardian_last_until_the_end_phase` (oneshot.rs) |  |
| v9 | Dragon Capture Jar stops a Dragon | `dragon_capture_jar_locks_dragons_in_defense_until_dragon_piper_breaks_it` (flips.rs) |  |
| v9 | Horn of Heaven negates a big summon | `horn_of_heaven_tributes_to_negate_a_summon` (remaining.rs) |  |
| v9 | Two-Pronged Attack | `two_pronged_attack_takes_two_of_yours_for_one_of_theirs` (oneshot.rs) |  |
| v9 | Final Destiny clears the field | `final_destiny_discards_five_and_clears_the_field` (oneshot.rs) |  |
| v9 | Share the Pain | `share_the_pain_tributes_one_each_and_the_opponent_picks_theirs` (oneshot.rs) |  |
| v9 | Remove Trap, Stop Defense, Gravedigger Ghoul | `remove_trap_destroys_a_face_up_trap_only` (oneshot.rs)<br>`stop_defense_and_block_attack_change_the_opponents_positions` (oneshot.rs)<br>`gravedigger_ghoul_banishes_up_to_two_monsters_from_the_opponents_graveyard` (oneshot.rs) |  |
| v9 | Kuriboh stops battle damage | `kuriboh_prevents_the_battle_damage_of_one_attack` (monsters.rs) |  |
| v9 | Stat effects: Muka Muka, Maha Vailo, Shadow Ghoul, Battleguards, attribute boosters, Flash Assailant | `conditional_stats_follow_the_graveyard_the_hand_and_the_equips` (monsters_a.rs)<br>`shadow_ghoul_counts_monsters_in_the_graveyard` (monsters_a.rs)<br>`the_battleguards_gain_500_atk_for_each_other` (tables.rs)<br>`every_attribute_booster_raises_one_attribute_and_lowers_another` (tables.rs) |  |
| v9 | Insect Soldiers +1000 vs WIND | `insect_soldiers_gain_1000_only_while_attacking_a_wind_monster` (monsters_a.rs) |  |
| v9 | Castle of Dark Illusions grows Zombies | `castle_of_dark_illusions_powers_up_zombies_for_four_standby_phases` (flips.rs) |  |
| v9 | Sogen, Luminous Spark, Vile Germs, Final Flame, Germ Infection | `every_type_field_spell_changes_the_named_types` (tables.rs)<br>`every_attribute_field_spell_gives_500_atk_and_takes_400_def` (tables.rs)<br>`every_type_equip_boosts_only_its_type` (tables.rs)<br>`burn_and_heal_cards_change_life_points_by_their_numbers` (oneshot.rs)<br>`germ_infection_weakens_its_monster_each_of_its_controllers_standby_phases` (remaining.rs) |  |
| v9 | Discard plan: Graceful Charity discards Red-Eyes, Monster Reborn brings it back | none | **Open (bot):** neither the plain nor the lookahead bot discards Red-Eyes here; the plan pays off a decision later, beyond the lookahead's reach. |
| v9 | Discard plan off without a revive card | `keeps_red_eyes_without_a_revive_card` (lookahead.rs) | Both bots. |
| v9 | Queen's Double attacks directly | `direct_attackers_may_attack_over_monsters` (monsters_a.rs) |  |

## Open

- **Discard plan** (v9): with Graceful Charity, Red-Eyes Black Dragon and Monster Reborn in hand, the prototype's bot discards Red-Eyes to revive it. Our bots discard other cards: the lookahead bot plays on only to the next open decision, so a revive that pays off later is out of its sight. This is bot strategy, not a rule; the owner decides whether the heuristic bot gets a rule for it or M6's trained policy should learn it.
