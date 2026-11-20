# Rulings and rule decisions used by the prototype (sim v9, 2026-11-20)

Owner's house rules (confirmed): one Normal or Tribute Summon/Set per turn; Flip Summons and Special Summons are free;
no summoning in Main Phase 2 (Spells/Traps allowed); hand limit 6 at End Phase; max 5 Spell/Trap plays per turn;
5 monster zones, 5 Spell/Trap zones, 1 Field Spell; Level 5-6 need 1 Tribute, Level 7+ need 2; the limited list in
data/banlist.json applies to every player (enemies too).

Card rulings the Rust engine must follow (each has a scenario test in reference/python_prototype/test_v8.py / test_v9.py):
- Spellbinding Circle and Shadow Spell activated on an attacking monster: the attack stops (Goat-format ruling for Spellbinding Circle; Shadow Spell has the same "cannot attack" clause).
- Scapegoat needs 4 free Monster Zones to activate (5 zones in this game: you may control at most 1 monster). Sheep Tokens cannot be Tributed for a Tribute Summon (they can for effects and Rituals); tokens never go to the GY or hand.
- A flip effect that removes the attacker or the target (Penguin Soldier, Hane-Hane, Man-Eater Bug, Cyber Jar) ends that battle: no damage calculation.
- "Cannot be Normal Summoned" monsters (Rituals, Toons, Gate Guardian, Harpie Lady Sisters, Relinquished...) can be revived only after they were properly Summoned once.
- Toons: Special Summon from hand while you control Toon World (Toon Mermaid 0, Toon Summoned Skull 1, Manga Ryu-Ran and Blue-Eyes Toon Dragon 2 Tributes); cannot attack the turn they are Summoned; pay 500 LP per attack; attack directly unless the opponent controls a Toon (then they must attack a Toon: NOT yet in the prototype); destroyed when Toon World leaves the field.
- Sanga / Kazejin / Suijin: once while face-up, during damage calculation when attacked on the opponent's turn, the attacker's ATK becomes 0. Gate Guardian: Special Summon by Tributing all three from your field.
- Relinquished: absorbs 1 opponent monster (its ATK/DEF become that monster's); if Relinquished would be destroyed by battle, the absorbed monster is destroyed instead.
- Premature Burial and Shadow Spell stay linked to their monster: either leaving destroys the other.
- Black Pendant: 500 damage to the opponent when it is sent to the GY. Horn of the Unicorn: returns to the top of the Deck instead. Megamorph: original ATK doubled while its controller has fewer LP than the opponent, halved while more.
- Mystic Tomato, Giant Rat, Mother Grizzly, Shining Angel, UFO Turtle, Flying Kamakiri #1: when destroyed by battle and sent to the GY, Special Summon an attribute-matched monster with 1500 or less ATK from the Deck.
- Nobleman of Crossout: banish a face-down monster; if it is a Flip monster, banish all copies from both Decks.
- Soul Exchange: you cannot conduct your Battle Phase that turn. Ultimate Offering: 500 LP per extra Normal Summon/Set.
- A negated Spell goes to the GY exactly once (an old prototype bug put it there twice).

Known prototype simplifications (do better in Rust): Shift redirects attacks only (not targeted effects); Reinforcements is used only on defence;
the AI uses perfect knowledge of face-down Spell/Traps for a few choices; 43 cards are not simulated (see effects_catalog.json).
