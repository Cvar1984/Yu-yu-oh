"""Simulator v4: adds a defend-only "stall" AI (wins by the opponent decking out) and the
stall cards it needs, on top of v3. Card text used (official):
  Chorus of Sanctuary (Field): Defense Position monsters gain 500 DEF.
  Horn of Light (Equip): +800 DEF.
  Ring of Magnetism (Equip, own monster): -500 ATK/DEF; opponent's monsters can only attack it.
  Gravekeeper's Servant (Continuous): opponent sends top card of Deck to GY to declare an attack.
  Toll (Continuous): each player pays 500 LP to declare an attack.
  Block Attack: 1 face-up ATK monster the opponent controls -> face-up Defense Position.
  Eatgaboon / House of Adhesive Tape (Trap): destroy a monster the opponent Normal/Flip Summons
      with ATK <= 500 / DEF <= 500.
  Eternal Rest: destroy all monsters equipped with an Equip Card.
  Red Medicine (+500 LP), Goblin's Secret Remedy (+600 LP).
"""
import ygo_sim2 as S
import ygo_sim3 as V3
from ygo_sim2 import C, ST, Mon, is_mon

V3.FIELDS["Chorus of Sanctuary"] = []
V3.EQUIPS["Horn of Light"] = (None, 0, 800)
HEAL = {"Red Medicine": 500, "Goblin's Secret Remedy": 600}
WALL_LV5 = ("Labyrinth Wall", "Illusionist Faceless Mage", "Queen Bird")
STALL_ALLOW_POT = False

_v3_hook = V3.stat_hook


def stat_hook(m, idx):
    extra = _v3_hook(m, idx)
    d = m.owner.duel
    if idx == 3 and d.field and d.field.name == "Chorus of Sanctuary" and m.pos == "def":
        extra += 500
    return extra


S.STAT_HOOK = stat_hook


class Duel4(V3.Duel3):
    def __init__(self, p1, p2, rng, **kw):
        kw.setdefault("max_turns", 200)
        super().__init__(p1, p2, rng, **kw)

    # ---------- shared rules ----------
    def pay_attack_cost(self, pl, m):
        o = self.opp(pl)
        for s in o.st + pl.st:
            if s.name == "Toll" and s.face_up:
                if pl.lp <= 500:
                    return False
                pl.lp -= 500
        for s in o.st:
            if s.name == "Gravekeeper's Servant" and s.face_up:
                if not pl.deck:
                    return False
                pl.gy.append(pl.deck.pop())
                self.log(f"{pl.name} mills 1 to attack (Gravekeeper's Servant)")
        return True

    def respond_summon(self, summoner, m, kind):
        if super().respond_summon(summoner, m, kind):
            return True
        o = self.opp(summoner)
        if kind in ("normal", "flip"):
            for trap, ok in (("Eatgaboon", m.atk <= 500), ("House of Adhesive Tape", m.dfn <= 500)):
                ts = o.set_traps(trap, self.turn)
                if ok and ts:
                    self.destroy_st(o, ts[0])
                    if self.trap_negated(o, trap):
                        return False
                    self.log(f"{o.name} {trap} destroys {m.name}")
                    self.remove_mon(summoner, m)
                    return True
        return False

    def remove_mon(self, ctrl, m, to="gy"):
        m.magnet = False
        super().remove_mon(ctrl, m, to)

    def ctrl_main(self, pl):
        # Block Attack wears off: the owner switches its beaters back to Attack Position
        for m in pl.mons:
            if m.face_up and m.pos == "def" and C[m.name][2] >= 1400:
                m.pos = "atk"
        super().ctrl_main(pl)

    def play_turn(self, pl):
        if pl.role == "stall":
            self.turn += 1
            S.CURRENT_TURN[0] = self.turn
            o = self.opp(pl)
            pl.summoned, pl.st_plays = False, 0
            pl.searched, o.searched = set(), set()
            if self.turn > 1:
                self.draw(pl)
            for s in o.st:
                if s.name == "Snatch Steal":
                    pl.lp += 1000
            self.stall_main(pl, o)
            self.end_phase(pl)
            return
        super().play_turn(pl)

    # ---------- stall AI ----------
    def best_def(self, pl):
        return max((m.dfn for m in pl.mons), default=0)

    def stall_main(self, pl, o):
        h = pl.hand
        def can(card):
            return card in h and pl.st_plays < 5
        def cast(card):
            ok = self.activate(pl, card)
            return ok
        threat = [m for m in o.mons if m.pos == "atk" and not getattr(m, "paralyzed", False)]
        mydef = self.best_def(pl)
        # Field: Chorus of Sanctuary if it is not already up
        if can("Chorus of Sanctuary") and not (self.field and self.field.name == "Chorus of Sanctuary"):
            if cast("Chorus of Sanctuary"):
                if self.field:
                    self.field.owner_p.gy.append(self.field.name)
                self.field = ST("Chorus of Sanctuary", True, self.turn)
                self.field.owner_p = pl
            else:
                pl.gy.append("Chorus of Sanctuary")
        # Continuous stall cards
        for card in ("Gravekeeper's Servant", "Toll"):
            if can(card) and self.can_st(pl) and not pl.active(card):
                if cast(card):
                    pl.st.append(ST(card, True, self.turn))
                else:
                    pl.gy.append(card)
        if can("Swords of Revealing Light") and self.can_st(pl) and threat and \
                not pl.active("Swords of Revealing Light"):
            if cast("Swords of Revealing Light"):
                self.place_swords_like(pl, "Swords of Revealing Light")
            else:
                pl.gy.append("Swords of Revealing Light")
        # Removal, only against monsters that can break the wall line (or when there is no wall)
        def breakers():
            return [m for m in o.mons if m.pos == "atk" and not getattr(m, "paralyzed", False)
                    and (m.atk > self.best_def(pl) or not pl.mons)]
        if can("Raigeki") and (len(breakers()) >= 1 and len(o.mons) >= 2 or len(breakers()) >= 2):
            if cast("Raigeki"):
                for m in list(o.mons):
                    self.remove_mon(o, m)
            pl.gy.append("Raigeki")
        for card in ("Paralyzing Potion", "Block Attack", "Tribute to the Doomed"):
            while can(card) and breakers():
                t = max(breakers(), key=lambda x: x.atk)
                if card == "Paralyzing Potion" and V3.RACE.get(t.name) == "Machine":
                    break
                if card == "Tribute to the Doomed":
                    if len(h) < 2:
                        break
                    h.remove(card); pl.st_plays += 1
                    self.discard(pl, self.worst(pl))
                    if not self.respond_spell(pl, card):
                        self.remove_mon(o, t)
                    pl.gy.append(card)
                    continue
                if cast(card):
                    if card == "Paralyzing Potion":
                        t.paralyzed = True
                    else:
                        t.pos = "def"
                pl.gy.append(card)
        if can("Eternal Rest") and any(getattr(m, "bonus", None) or m.axe for m in o.mons) and \
                not any(getattr(m, "bonus", None) for m in pl.mons):
            if cast("Eternal Rest"):
                for side in (o, pl):
                    for m in [m for m in side.mons if getattr(m, "bonus", None) or m.axe]:
                        self.remove_mon(side, m)
            pl.gy.append("Eternal Rest")
        # Change of Heart: borrow their best monster as tribute for a Level 5 wall
        lv5 = [c for c in h if c in WALL_LV5]
        if can("Change of Heart") and lv5 and o.mons and not pl.summoned and len(pl.mons) < 5:
            if cast("Change of Heart"):
                t = max(o.mons, key=lambda x: x.atk)
                o.mons.remove(t); t.temp = True; pl.mons.append(t)
            pl.gy.append("Change of Heart")
        # Summon: Level 5 wall over the weakest monster, else the highest-DEF Level 4 or lower, face-down
        if not pl.summoned and len(pl.mons) < 5 + (1 if lv5 else 0):
            lv5 = [c for c in h if c in WALL_LV5]
            if lv5 and any(S.can_tribute(m) for m in pl.mons):
                fod = min([m for m in pl.mons if S.can_tribute(m)] or pl.mons, key=lambda m: (not m.temp, C[m.name][1] >= 6, m.dfn))  # never tribute own big monster
                card = max(lv5, key=lambda c: C[c][3])
                if fod.temp or (C[fod.name][1] < 6 and C[card][3] > fod.dfn + 300):
                    if fod.temp:
                        pl.mons.remove(fod); fod.owner.gy.append(fod.name)
                    else:
                        self.remove_mon(pl, fod)
                    h.remove(card)
                    pl.mons.append(Mon(card, pl, False, "def"))
                    pl.summoned = True
                    self.log(f"{pl.name} tributes for {card} (set)")
            if not pl.summoned and len(pl.mons) < 5:
                walls = [c for c in h if is_mon(c) and C[c][1] <= 4 and c not in S.NOMI]
                if walls:
                    card = max(walls, key=lambda c: C[c][3])
                    h.remove(card)
                    pl.mons.append(Mon(card, pl, False, "def"))
                    pl.summoned = True
                    self.log(f"{pl.name} sets {card}")
        # give a borrowed monster back at End Phase (handled by end_phase via temp)
        # Equips for defense
        for card, attr in (("Horn of Light", None), ("Ring of Magnetism", "magnet")):
            while can(card) and pl.mons:
                if attr and any(getattr(m, "magnet", False) for m in pl.mons):
                    break
                m = max([x for x in pl.mons if not x.temp] or pl.mons, key=lambda x: x.dfn)
                if cast(card):
                    if card == "Horn of Light":
                        m.bonus = getattr(m, "bonus", []) + [(0, 800)]
                    else:
                        m.bonus = getattr(m, "bonus", []) + [(-500, -500)]
                        m.magnet = True
                pl.gy.append(card)
        for card, gain in HEAL.items():
            while can(card):
                if cast(card):
                    pl.lp += gain
                pl.gy.append(card)
        if STALL_ALLOW_POT and can("Pot of Greed") and len(pl.deck) > len(o.deck) + 4:
            if cast("Pot of Greed"):
                self.draw(pl, 2)
            pl.gy.append("Pot of Greed")
        # Set traps
        for t in ("Solemn Judgment", "Magic Jammer", "Seven Tools of the Bandit", "Spellbinding Circle", "Eatgaboon", "House of Adhesive Tape", "Mirror Force", "Trap Hole"):
            while t in h and self.can_st(pl):
                h.remove(t); pl.st_plays += 1
                pl.st.append(ST(t, False, self.turn))

    def end_phase(self, pl):
        # stall player's discards: keep walls and defensive cards, drop dead cards first
        if pl.role == "stall":
            dead = ("Pot of Greed", "Tremendous Fire", "Mountain", "Sword of Deep-Seated", "Malevolent Nuzzler",
                    "Red-Eyes Black Dragon", "Flame Cerebrus", "Karate Man")
            while len(pl.hand) > 6:
                c = next((x for x in pl.hand if x in dead), None) or min(
                    pl.hand, key=lambda x: C[x][3] if is_mon(x) else 5000)
                self.discard(pl, c)
        super().end_phase(pl)
