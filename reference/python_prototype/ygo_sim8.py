"""Simulator v8 (Nov 19, second update): the 66 cards outside LOB/MRD/SRL that the owner's
"All" card list now shows (sets/extra66.json, YGOPRODeck text), on top of v7.

Every card is either simulated or, if it can never change a game here, listed in DEAD:
  Normal Monsters (22): stats only.
  Effect/Flip/Ritual monsters: Mysterious Puppeteer, The Wicked Worm Beast, Penguin Soldier,
      Trap Master, Black Luster Soldier (Ritual, needs Black Luster Ritual; can only be revived
      after it was properly Summoned).
  Spells: Graceful Charity, Card Destruction, De-Spell, Dian Keto, Fairy Meteor Crush (piercing),
      Invigoration, Sword of Dark Destruction, Last Will, Nobleman of Crossout (banish), Ookazi,
      Premature Burial (linked equip), Scapegoat (Set, used on the opponent's direct attack;
      needs 4 free zones), Soul Exchange (no Battle Phase that turn), Black Luster Ritual.
  Traps: Backup Soldier, Burst Breath, Castle Walls, D. Tribe, Disappear, Dust Tornado,
      Enchanted Javelin, Gift of The Mystical Elf, Jar of Greed, Just Desserts,
      Light of Intervention, Raigeki Break, Reinforcements, Reverse Trap, Riryoku Field,
      Shadow Spell (linked, stops the attack like Spellbinding Circle), Shift (attacks only),
      The Eye of Truth, Ultimate Offering, Waboku.
  DEAD (no possible effect in these games): Ancient Telescope and The Inexperienced Spy
      (information only), The Flute of Summoning Dragon (needs Lord of D.), White Dragon Ritual
      (needs Paladin of White Dragon), Gryphon Wing (needs Harpie's Feather Duster).
AI simplifications are noted next to each card below.
"""
import json
import ygo_sim2 as S
import ygo_sim3 as V3
import ygo_sim4 as V4
import ygo_sim7 as V7
from ygo_sim2 import C, Mon, ST, is_mon

# ---------------- card data ----------------
TYPE = {}
for f in ("Legend_of_Blue_Eyes_White_Dragon", "Metal_Raiders", "Spell_Ruler", "extra66"):
    for c in json.load(open(f"sets/{f}.json"))["data"]:
        n = c["name"]
        TYPE[n] = c["type"]
        if "Monster" in c["type"]:
            if n not in C:
                C[n] = ("M", c.get("level") or 0, c.get("atk") or 0, c.get("def") or 0)
            V3.ATTR[n], V3.RACE[n] = c.get("attribute"), c.get("race")
        elif n not in C:
            C[n] = ("T",) if "Trap" in c["type"] else ("S",)

TOKEN = "Sheep Token"
C[TOKEN] = ("M", 1, 0, 0)
V3.ATTR[TOKEN], V3.RACE[TOKEN], TYPE[TOKEN] = "EARTH", "Beast", "Token"
NORMAL = {n for n, t in TYPE.items() if t == "Normal Monster"}
FLIPS = {n for n, t in TYPE.items() if "Flip" in t}
FUSION = {n for n, t in TYPE.items() if "Fusion" in t}
S.NOMI |= {"Black Luster Soldier", "Manga Ryu-Ran", "Gate Guardian", "Great Moth", "Larvae Moth", "Wall Shadow",
           "Crab Turtle", "Hungry Burger", "Performance of Sword"}
RITUALS = {"Black Luster Ritual": ("Black Luster Soldier", 8)}
DEAD = {"Ancient Telescope", "The Inexperienced Spy", "The Flute of Summoning Dragon",
        "White Dragon Ritual", "Gryphon Wing"}
V3.BURN["Ookazi"] = 800
V4.HEAL["Dian Keto the Cure Master"] = 1000
ATTR_EQUIPS = {"Invigoration": ("EARTH", 400, -200), "Sword of Dark Destruction": ("DARK", 400, -200)}
ONE_TARGET = {"Change of Heart", "Snatch Steal", "Tribute to the Doomed", "Paralyzing Potion", "Stim-Pack",
              "Axe of Despair", "Horn of Light", "Ring of Magnetism", "Fairy Meteor Crush",
              *V3.EQUIPS, *ATTR_EQUIPS}  # Spells that target exactly 1 monster (Riryoku Field)
STOPPERS = ("Swords of Revealing Light", "Gravekeeper's Servant", "Messenger of Peace", "Toll",
            "Snatch Steal", "Premature Burial", "Shadow Spell", "Ultimate Offering", "Light of Intervention")
ATTACK_TRAPS = ("Raigeki Break", "Shadow Spell", "Scapegoat", "Shift", "Castle Walls", "Reinforcements",
                "Waboku", "Reverse Trap", "Enchanted Javelin")
OWN_TURN_TRAPS = ("Dust Tornado", "Raigeki Break", "Shadow Spell", "Backup Soldier", "Burst Breath",
                  "Disappear", "Light of Intervention", "The Eye of Truth", "Ultimate Offering")
END_TRAPS = ("Jar of Greed", "Just Desserts", "Gift of The Mystical Elf")
SET_CARDS = ATTACK_TRAPS + OWN_TURN_TRAPS + END_TRAPS + ("Riryoku Field", "D. Tribe")
S.TRAPS = S.TRAPS + tuple(t for t in SET_CARDS if t not in S.TRAPS)
for n in ("Penguin Soldier", "Trap Master", "Reaper of the Cards"):
    if n not in S.SET_PRIORITY:
        S.SET_PRIORITY.append(n)
S.JAM_WORTHY |= {"Graceful Charity", "Premature Burial", "Black Luster Ritual", "Nobleman of Crossout",
                 "Soul Exchange", "Card Destruction"}
S.SOLEMN_WORTHY |= {"Premature Burial", "Black Luster Ritual"}
S.VALUE.update({
    "Graceful Charity": 90, "Premature Burial": 82, "Scapegoat": 70, "Penguin Soldier": 72,
    "Raigeki Break": 70, "Shadow Spell": 66, "Dust Tornado": 65, "Black Luster Soldier": 65,
    "Black Luster Ritual": 60, "Nobleman of Crossout": 60, "Waboku": 60, "Riryoku Field": 55,
    "Jar of Greed": 55, "Card Destruction": 50, "Reinforcements": 50, "Just Desserts": 50,
    "Ultimate Offering": 45, "Castle Walls": 45, "Shift": 45, "Enchanted Javelin": 45,
    "Soul Exchange": 45, "Last Will": 45, "Trap Master": 45, "Fairy Meteor Crush": 45,
    "Invigoration": 45, "Sword of Dark Destruction": 45, "De-Spell": 40, "Backup Soldier": 40,
    "Burst Breath": 40, "Ookazi": 40, "Reverse Trap": 35, "Mysterious Puppeteer": 35,
    "Dian Keto the Cure Master": 30, "Gift of The Mystical Elf": 30, "Light of Intervention": 30,
    "The Wicked Worm Beast": 30, "Disappear": 25, "D. Tribe": 20, "The Eye of Truth": 10,
    "La Jinn the Mystical Genie of the Lamp": 58, "Judge Man": 60, "Swordstalker": 58,
    "Battle Ox": 52, "Rude Kaiser": 50, **{d: 5 for d in DEAD}})

# ---------------- stats ----------------
_hook = S.STAT_HOOK


def stat_hook(m, idx):
    extra = _hook(m, idx)
    t = S.CURRENT_TURN[0]
    extra += sum(x[idx - 1] for x in getattr(m, "tmp", ()) if x[0] == t)  # Castle Walls, Reinforcements
    if idx == 2 and getattr(m, "shadow", None) is not None:
        extra -= 700
    d = getattr(m.owner, "duel", None)
    if idx == 2 and d is not None:  # Star Boy: WATER +500, FIRE -400 while face-up
        stars = sum(1 for p in d.p for x in p.mons if x.name == "Star Boy" and x.face_up)
        if stars:
            a = V3.ATTR.get(m.name)
            extra += stars * (500 if a == "WATER" else -400 if a == "FIRE" else 0)
    if d is not None and getattr(d, "reverse_turn", -1) == t:  # Reverse Trap: modifiers change sign
        extra = -extra - (2000 * m.axe if idx == 2 else 0)
    return extra


S.STAT_HOOK = stat_hook

_mon_init = Mon.__init__


def _mon_init_light(self, name, owner, face_up=True, pos="atk"):
    """Light of Intervention: a monster that would be Set is placed face-up in Defense Position."""
    d = getattr(owner, "duel", None)
    if not face_up and pos == "def" and d is not None and any(
            p.active("Light of Intervention") for p in d.p):
        face_up = True
    _mon_init(self, name, owner, face_up, pos)


Mon.__init__ = _mon_init_light


class Duel8(V7.Duel7):
    def __init__(self, p1, p2, rng, **kw):
        super().__init__(p1, p2, rng, **kw)
        for p in (p1, p2):
            p.proper = {}        # NOMI monsters that were properly Summoned (may be revived)
            p.lost_turn = -1     # turn a monster on this player's side went to their GY (Last Will)
            p.no_bp = -1         # Soul Exchange turn
            p.waboku = -1
            p.eye_turn = -1
        self.reverse_turn = -1
        self.cur_target = self.redirect_to = None

    # ---------------- leaving the field ----------------
    def _detach(self, m):
        """Break links between a leaving monster and Premature Burial / Shadow Spell."""
        for attr in ("pb", "shadow"):
            s = getattr(m, attr, None)
            setattr(m, attr, None)
            if s is not None:
                for side in self.p:
                    if s in side.st:
                        side.st.remove(s)
                        side.gy.append(s.name)
                        self.log(f"{side.name}'s {s.name} leaves with {m.name}")

    def remove_mon(self, ctrl, m, to="gy"):
        if m not in ctrl.mons:
            return
        self._detach(m)
        token = getattr(m, "token", False)
        if m.name in S.NOMI:
            m.owner.proper[m.name] = m.owner.proper.get(m.name, 0) + 1
        if m.owner is ctrl and not token:
            ctrl.lost_turn = self.turn
        super().remove_mon(ctrl, m, to)
        if token and TOKEN in m.owner.gy:
            m.owner.gy.remove(TOKEN)  # tokens vanish

    def _off_field(self, ctrl, m):
        ctrl.mons.remove(m)
        self._detach(m)
        if m.snatched:
            for side in self.p:
                if m.snatched in side.st:
                    side.st.remove(m.snatched)
                    side.gy.append("Snatch Steal")
            m.snatched = None

    def bounce(self, ctrl, m):
        """Return a monster to its owner's hand (Fusion -> Extra Deck, token -> gone)."""
        if m not in ctrl.mons:
            return
        self._off_field(ctrl, m)
        self.log(f"{m.name} returns to {m.owner.name}'s hand")
        if getattr(m, "token", False):
            return
        if m.name in FUSION:
            m.owner.extra = getattr(m.owner, "extra", []) + [m.name]
        else:
            self.add_hand(m.owner, m.name)

    def banish_mon(self, ctrl, m):
        if m not in ctrl.mons:
            return
        self._off_field(ctrl, m)
        self.log(f"{m.name} ({ctrl.name}) banished")

    def destroy_st(self, owner, s):
        if s not in owner.st:
            return
        tgt = s.target
        super().destroy_st(owner, s)
        if s.name == "Shadow Spell" and tgt is not None:
            tgt.shadow, tgt.paralyzed = None, False
        elif s.name == "Premature Burial" and tgt is not None:
            tgt.pb = None
            for side in self.p:
                if tgt in side.mons:
                    self.remove_mon(side, tgt)

    def can_revive(self, side, card):
        return card not in S.NOMI or side.proper.get(card, 0) > 0

    # ---------------- flips and summons ----------------
    def flip(self, ctrl, m):
        o = self.opp(ctrl)
        if m.name == "Hane-Hane":
            m.face_up = True
            if o.mons:
                self.bounce(o, max(o.mons, key=lambda x: x.atk))
            return
        if m.name == "Penguin Soldier":
            m.face_up = True
            # up to 2 opponent monsters; face-down ones count as an unknown 1500
            for t in sorted(o.mons, key=lambda x: -(x.atk if x.face_up else 1500))[:2]:
                self.bounce(o, t)
            return
        if m.name in ("Trap Master", "Reaper of the Cards"):
            m.face_up = True
            ups = [s for s in o.st if s.face_up and C[s.name][0] == "T"]
            downs = [s for s in o.st if not s.face_up]
            pick = ups[0] if ups else (self.rng.choice(downs) if downs else None)
            if pick is not None:
                if C[pick.name][0] == "T":
                    self.log(f"Trap Master destroys {pick.name}")
                    self.destroy_st(o, pick)
                else:
                    self.log(f"Trap Master reveals {pick.name} (a Spell, stays)")
            return
        super().flip(ctrl, m)

    def respond_summon(self, summoner, m, kind):
        gone = super().respond_summon(summoner, m, kind)
        if not gone and kind in ("normal", "flip"):
            for side in self.p:  # Mysterious Puppeteer
                n = sum(1 for x in side.mons if x.name == "Mysterious Puppeteer" and x.face_up)
                if n:
                    side.lp += 500 * n
                    self.log(f"{side.name} Mysterious Puppeteer +{500 * n} LP")
        return gone

    def ritual(self, pl):
        for spell, (mon, lv) in RITUALS.items():
            if spell not in pl.hand or mon not in pl.hand or pl.st_plays >= 5:
                continue
            # cheapest tributes from hand or field whose Levels total >= lv
            cands = [("h", c, S.VALUE.get(c, 10)) for c in pl.hand if is_mon(c) and c != mon] + \
                    [("f", m, -10 if m.temp or getattr(m, "token", False) else (m.atk + 300) / 30)
                     for m in pl.mons]
            cands.sort(key=lambda x: (x[2], -C[x[1] if x[0] == "h" else x[1].name][1]))
            use, tot = [], 0
            for x in cands:
                if tot >= lv:
                    break
                use.append(x)
                tot += C[x[1] if x[0] == "h" else x[1].name][1]
            field_used = sum(1 for x in use if x[0] == "f")
            if tot < lv or len(use) > 3 or len(pl.mons) - field_used >= 5:
                continue
            if self.activate(pl, spell):
                for kind, x, _ in use:
                    if kind == "h":
                        self.discard(pl, x)
                    elif x.temp:
                        pl.mons.remove(x)
                        x.owner.gy.append(x.name)
                    else:
                        self.remove_mon(pl, x)
                pl.hand.remove(mon)
                nm = Mon(mon, pl, True, "atk")
                pl.mons.append(nm)
                self.log(f"{pl.name} Ritual Summons {mon}")
                self.respond_summon(pl, nm, "special")
            pl.gy.append(spell)
            return True
        return False

    def ctrl_summon(self, pl, o):
        self.soul_exchange(pl, o)
        V7.Duel7.ctrl_summon(self, pl, o)
        # Ultimate Offering: pay 500 LP per extra Normal Summon/Set (AI: at most 2, LP > 2500)
        for _ in range(2):
            if not (pl.active("Ultimate Offering") and pl.summoned and pl.lp > 2500 and len(pl.mons) < 5):
                break
            if not any(is_mon(c) and c not in S.NOMI and C[c][1] <= 4 for c in pl.hand):
                break
            pl.summoned = False
            V7.Duel7.ctrl_summon(self, pl, o)
            if not pl.summoned:
                pl.summoned = True
                break
            pl.lp -= 500
            self.log(f"{pl.name} pays 500 for Ultimate Offering (LP {pl.lp})")

    def soul_exchange(self, pl, o):
        """Tribute the opponent's best monster for a Tribute Summon; no Battle Phase this turn."""
        if pl.summoned or "Soul Exchange" not in pl.hand or pl.st_plays >= 5 or not o.mons:
            return
        t = self.tribute_target(pl)
        if not t:
            return
        need = 1 if C[t][1] <= 6 else 2
        own = sorted([m for m in pl.mons if S.can_tribute(m) and not m.temp], key=lambda m: m.atk)
        ob = max(o.mons, key=lambda x: x.atk if x.face_up else 1500)
        if len(pl.mons) - (need - 1) >= 5:
            return  # no free zone after the Tributes
        if len(own) + 1 < need or not (len(own) < need or ob.atk >= 1800):
            return
        if self.activate(pl, "Soul Exchange"):
            pl.no_bp = self.turn
            self.log(f"{pl.name} tributes {o.name}'s {ob.name} (Soul Exchange)")
            self.remove_mon(o, ob)
            for m in own[:need - 1]:
                self.remove_mon(pl, m)
            pl.hand.remove(t)
            nm = Mon(t, pl, True, "atk")
            pl.mons.append(nm)
            pl.summoned = True
            self.log(f"{pl.name} tribute summons {t}")
            self.respond_summon(pl, nm, "normal")
        pl.gy.append("Soul Exchange")

    # ---------------- turn structure ----------------
    def standby(self, pl):
        o = self.opp(pl)
        if pl.eye_turn != self.turn and o.active("The Eye of Truth") and any(C[c][0] == "S" for c in pl.hand):
            pl.eye_turn = self.turn
            pl.lp += 1000
            self.log(f"{pl.name} gains 1000 (The Eye of Truth)")

    def ctrl_main(self, pl):
        self.standby(pl)
        super().ctrl_main(pl)

    def stall_main(self, pl, o):
        self.standby(pl)
        while self.own_turn_traps(pl, o):
            pass
        super().stall_main(pl, o)
        for t in SET_CARDS:
            while t in pl.hand and self.can_st(pl):
                pl.hand.remove(t)
                pl.st_plays += 1
                pl.st.append(ST(t, False, self.turn))

    def battle(self, pl):
        if pl.no_bp != self.turn:
            super().battle(pl)
        self.main2(pl)

    def main2(self, pl):
        self.last_will(pl)

    def end_phase(self, pl):
        o = self.opp(pl)
        for m in list(pl.mons):
            if m.name == "The Wicked Worm Beast" and m.face_up:
                self.bounce(pl, m)
        self.end_traps(o, pl)
        super().end_phase(pl)

    # ---------------- own-turn Spells ----------------
    def ctrl_step(self, pl, o):
        if self.own_turn_traps(pl, o):
            return True
        h = pl.hand

        def use(c):
            return c in h and pl.st_plays < 5

        if use("Graceful Charity") and len(pl.deck) > 3:
            if self.activate(pl, "Graceful Charity"):
                self.draw(pl, 3)
                for _ in range(min(2, len(pl.hand))):
                    self.discard(pl, self.worst(pl))
            pl.gy.append("Graceful Charity")
            return True
        if use("Card Destruction"):
            rest = [c for c in h if c != "Card Destruction"]
            n = len(rest)
            if n >= 2 and len(pl.deck) > n + 3 and len(o.hand) >= 2 and \
                    sum(pl.value(c) for c in rest) / n < 40:
                if self.activate(pl, "Card Destruction"):
                    no = len(o.hand)
                    for side, k in ((pl, len(pl.hand)), (o, no)):
                        side.gy.extend(side.hand)
                        side.hand.clear()
                    self.log(f"Card Destruction: {pl.name} redraws {n}, {o.name} redraws {no}")
                    self.draw(pl, n)
                    self.draw(o, no)
                pl.gy.append("Card Destruction")
                return True
        downs = [m for m in o.mons if not m.face_up]
        if use("Nobleman of Crossout") and downs:
            t = max(downs, key=lambda m: m.born)
            if self.activate(pl, "Nobleman of Crossout"):
                self.banish_mon(o, t)
                if t.name in FLIPS:
                    for side in self.p:
                        k = side.deck.count(t.name)
                        side.deck[:] = [c for c in side.deck if c != t.name]
                        if k:
                            self.log(f"Nobleman banishes {k} {t.name} from {side.name}'s Deck")
            pl.gy.append("Nobleman of Crossout")
            return True
        if use("De-Spell"):
            tgt = next((s for name in STOPPERS for s in o.st if s.face_up and s.name == name
                        and C[name][0] == "S"), None)
            if tgt is not None:
                if self.activate(pl, "De-Spell"):
                    self.destroy_st(o, tgt)
                pl.gy.append("De-Spell")
                return True
        if use("Premature Burial") and self.can_st(pl) and len(pl.mons) < 5 and pl.lp > 1800:
            gy = [c for c in pl.gy if is_mon(c) and c not in S.PIECES and self.can_revive(pl, c)]
            best_hand = max((self.eff_atk(c) for c in h if is_mon(c) and C[c][1] <= 4), default=0)
            if gy:
                c = max(gy, key=lambda x: C[x][2])
                if C[c][2] >= max(1700, best_hand + 300):
                    pl.lp -= 800  # cost
                    pl.hand.remove("Premature Burial")
                    pl.st_plays += 1
                    self.log(f"{pl.name} activates Premature Burial")
                    if self.respond_spell(pl, "Premature Burial"):
                        pl.gy.append("Premature Burial")
                    else:
                        pl.gy.remove(c)
                        m = Mon(c, pl, True, "atk")
                        s = ST("Premature Burial", True, self.turn)
                        s.target, m.pb = m, s
                        pl.mons.append(m)
                        pl.st.append(s)
                        self.log(f"{pl.name} revives {c} (Premature Burial)")
                        self.respond_summon(pl, m, "special")
                    return True
        if self.ritual(pl):
            return True
        # flip utility monsters (Flip Summon = face-up Attack Position)
        for m in pl.mons:
            if m.face_up or m.born >= self.turn:
                continue
            go = (m.name == "Penguin Soldier" and (len(o.mons) >= 2 or self.best_opp(o) and self.best_opp(o).atk >= 1800)) \
                or (m.name in ("Trap Master", "Reaper of the Cards") and any(not s.face_up or C[s.name][0] == "T" for s in o.st))
            if go:
                m.pos = "atk"
                self.log(f"{pl.name} flip summons {m.name}")
                if not self.respond_summon(pl, m, "flip"):
                    self.flip(pl, m)
                return True
        return super().ctrl_step(pl, o)

    def ctrl_post(self, pl, o):
        h = pl.hand
        msgr = bool(o.active("Messenger of Peace") or pl.active("Messenger of Peace"))
        att = [m for m in pl.mons if m.face_up and m.pos == "atk" and not m.temp]
        for eq in [c for c in h if c in ATTR_EQUIPS]:
            attr, a, d = ATTR_EQUIPS[eq]
            cands = [m for m in att if V3.ATTR.get(m.name) == attr]
            if cands and self.can_st(pl) and not msgr:
                m = max(cands, key=lambda x: x.atk)
                if self.activate(pl, eq):
                    m.bonus = getattr(m, "bonus", []) + [(a, d)]
                pl.gy.append(eq)
        if "Fairy Meteor Crush" in h and self.can_st(pl) and not msgr:
            cands = [m for m in att if not getattr(m, "pierce", False)]
            if cands:
                m = max(cands, key=lambda x: x.atk)
                if self.activate(pl, "Fairy Meteor Crush"):
                    m.pierce = True
                    m.bonus = getattr(m, "bonus", []) + [(0, 0)]
                pl.gy.append("Fairy Meteor Crush")
        while "Dian Keto the Cure Master" in h and pl.st_plays < 5:
            if self.activate(pl, "Dian Keto the Cure Master"):
                pl.lp += 1000
            pl.gy.append("Dian Keto the Cure Master")
        self.last_will(pl)
        # Scapegoat is Set and used on the opponent's turn
        while "Scapegoat" in h and self.can_st(pl):
            h.remove("Scapegoat")
            pl.st_plays += 1
            pl.st.append(ST("Scapegoat", False, self.turn))
        super().ctrl_post(pl, o)

    def last_will(self, pl):
        if "Last Will" not in pl.hand or pl.st_plays >= 5 or pl.lost_turn != self.turn or len(pl.mons) >= 5:
            return
        cands = [c for c in pl.deck if is_mon(c) and C[c][2] <= 1500 and c not in S.NOMI and c not in S.PIECES]
        if not cands:
            return
        if self.activate(pl, "Last Will"):
            c = max(cands, key=lambda x: (pl.value(x), C[x][2]))
            pl.deck.remove(c)
            self.rng.shuffle(pl.deck)
            m = Mon(c, pl, True, "atk" if C[c][2] >= 1300 else "def")
            pl.mons.append(m)
            self.log(f"{pl.name} Last Will summons {c}")
            self.respond_summon(pl, m, "special")
        pl.gy.append("Last Will")

    # ---------------- Traps ----------------
    def fire(self, owner, name):
        """Activate one of owner's Set cards (Set before this turn). False if none or negated."""
        ts = owner.set_traps(name, self.turn)
        if not ts:
            return False
        s = ts[0]
        if C[name][0] == "T" and self.trap_negated(owner, name):
            self.destroy_st(owner, s)
            return False
        owner.st.remove(s)
        owner.gy.append(name)
        self.log(f"{owner.name} activates {name}")
        return True

    def own_turn_traps(self, pl, o):
        """Own-turn Trap activations. Returns True if something was done."""
        has = lambda n: bool(pl.set_traps(n, self.turn))
        if has("Dust Tornado") and o.st:
            prio = [s for name in STOPPERS for s in o.st if s.face_up and s.name == name]
            downs = [s for s in o.st if not s.face_up]
            tgt = prio[0] if prio else (self.rng.choice(downs) if downs and any(m.pos == "atk" for m in pl.mons) else None)
            if tgt is not None and self.fire(pl, "Dust Tornado"):
                self.destroy_st(o, tgt)
                sets = [c for c in pl.hand if c in S.TRAPS]
                if sets and len(pl.st) < 5:
                    c = max(sets, key=pl.value)
                    pl.hand.remove(c)
                    pl.st.append(ST(c, False, self.turn))
                return True
        if has("Raigeki Break") and pl.hand:
            prio = [s for name in STOPPERS for s in o.st if s.face_up and s.name == name]
            ob = self.best_opp(o)
            mine = max((m.atk for m in pl.mons), default=0)
            tgt_m = ob if ob and ob.face_up and ob.atk >= max(1900, mine + 100) else None
            if (prio or tgt_m) and self.fire(pl, "Raigeki Break"):
                self.discard(pl, self.worst(pl))
                if prio:
                    self.destroy_st(o, prio[0])
                else:
                    self.remove_mon(o, tgt_m)
                return True
        if has("Shadow Spell"):
            ups = [m for m in o.mons if m.face_up and not getattr(m, "paralyzed", False) and m.atk >= 1600]
            if ups and self.can_shadow(pl):
                t = max(ups, key=lambda x: x.atk)
                self.shadow(pl, t)
                return True
        if has("Backup Soldier"):
            gm = [c for c in pl.gy if is_mon(c)]
            back = sorted([c for c in gm if c in NORMAL and C[c][2] <= 1500], key=lambda c: -C[c][2])
            if len(gm) >= 5 and back and (len(back) >= 2 or len(pl.hand) <= 1) and self.fire(pl, "Backup Soldier"):
                for c in back[:3]:
                    pl.gy.remove(c)
                    self.add_hand(pl, c)
                self.log(f"Backup Soldier returns {back[:3]}")
                return True
        if has("Burst Breath") and self.burst_breath(pl, o):
            return True
        if has("Disappear"):
            gm = [c for c in o.gy if is_mon(c) and C[c][2] >= 2000]
            if gm and self.fire(pl, "Disappear"):
                c = max(gm, key=lambda x: C[x][2])
                o.gy.remove(c)
                self.log(f"Disappear banishes {c} from {o.name}'s GY")
                return True
        for cont in ("Ultimate Offering", "The Eye of Truth", "Light of Intervention"):
            if has(cont) and not (cont == "Light of Intervention" and any(c in FLIPS for c in pl.hand)):
                s = pl.set_traps(cont, self.turn)[0]
                if self.trap_negated(pl, cont):
                    self.destroy_st(pl, s)
                else:
                    s.face_up = True
                    self.log(f"{pl.name} activates {cont}")
                return True
        return False

    def can_shadow(self, pl):
        return bool(pl.set_traps("Shadow Spell", self.turn))

    def shadow(self, owner, t):
        """Shadow Spell stays on the field linked to t: -700 ATK, cannot attack."""
        s = owner.set_traps("Shadow Spell", self.turn)[0]
        if self.trap_negated(owner, "Shadow Spell"):
            self.destroy_st(owner, s)
            return False
        s.face_up, s.target = True, t
        t.shadow, t.paralyzed = s, True
        self.log(f"{owner.name} Shadow Spell on {t.name}")
        return True

    def burst_breath(self, pl, o):
        """Tribute 1 Dragon (D. Tribe makes all yours Dragons): destroy face-up monsters with DEF <= its ATK."""
        tribe = bool(pl.set_traps("D. Tribe", self.turn))
        best = None
        for t in [m for m in pl.mons if m.face_up]:
            if V3.RACE.get(t.name) != "Dragon" and not tribe:
                continue
            hit = [(s, m) for s in (o, pl) for m in s.mons if m.face_up and m is not t and m.dfn <= t.atk]
            score = sum(m.atk + 300 for s, m in hit if s is o) - sum(m.atk + 300 for s, m in hit if s is pl) - t.atk - 300
            if best is None or score > best[0]:
                best = (score, t, hit, V3.RACE.get(t.name) != "Dragon")
        if best is None or best[0] < 1000:
            return False
        score, t, hit, need_tribe = best
        if need_tribe and not self.fire(pl, "D. Tribe"):
            return False
        if not self.fire(pl, "Burst Breath"):
            return False
        self.remove_mon(pl, t)
        for s, m in hit:
            self.remove_mon(s, m)
        self.log(f"Burst Breath (tribute {t.name}) destroys {[m.name for _, m in hit]}")
        return True

    def end_traps(self, o, pl):
        """o activates Traps during pl's End Phase."""
        if o.set_traps("Jar of Greed", self.turn) and len(o.deck) > 3 and self.fire(o, "Jar of Greed"):
            self.draw(o)
        n = len(pl.mons)
        if n and (n >= 2 or 500 * n >= pl.lp) and self.fire(o, "Just Desserts"):
            self.damage(pl, 500 * n)
        tot = len(pl.mons) + len(o.mons)
        if tot >= 3 and self.fire(o, "Gift of The Mystical Elf"):
            o.lp += 300 * tot

    def respond_spell(self, caster, card):
        o = self.opp(caster)
        if card in ONE_TARGET and o.set_traps("Riryoku Field", self.turn) and \
                card in ("Change of Heart", "Snatch Steal", "Tribute to the Doomed", "Paralyzing Potion",
                         "Stim-Pack", "Axe of Despair", "Malevolent Nuzzler") and self.fire(o, "Riryoku Field"):
            self.log(f"{o.name} Riryoku Field negates {card}")
            return True
        return super().respond_spell(caster, card)

    def respond_attack(self, pl, m):
        o = self.opp(pl)
        tgt = self.cur_target
        if m.atk >= 1600 and o.hand and (tgt is None or self._wins(m, tgt)) and self.fire(o, "Raigeki Break"):
            self.discard(o, self.worst(o))
            self.remove_mon(pl, m)
            return True
        if super().respond_attack(pl, m):  # Spellbinding Circle
            return True
        if m.atk >= 1500 and self.can_shadow(o) and self.shadow(o, m):
            return True  # like Spellbinding Circle, the attack stops
        if tgt is None and m.atk >= 1000 and len(o.mons) <= 1 and self.fire(o, "Scapegoat"):
            for _ in range(4):
                tk = Mon(TOKEN, o, True, "def")
                tk.token = tk.no_tribute = True
                o.mons.append(tk)
            self.log(f"{o.name} Scapegoat: 4 Sheep Tokens; {m.name} attacks a token instead")
            self.battle_destroy(o, next(x for x in o.mons if getattr(x, "token", False)))
            return True
        if tgt is not None and self._wins(m, tgt):
            safe = [x for x in o.mons if x is not tgt and not self._wins(m, x)]
            if safe and self.fire(o, "Shift"):
                self.redirect_to = max(safe, key=lambda x: x.dfn if x.pos == "def" else x.atk)
                self.log(f"{o.name} Shift: attack moves to {self.redirect_to.name}")
                tgt = self.redirect_to
        if tgt is not None and self._wins(m, tgt) and tgt.face_up:
            if tgt.pos == "def" and m.atk <= tgt.dfn + 500 and self.fire(o, "Castle Walls"):
                tgt.tmp = getattr(tgt, "tmp", []) + [(self.turn, 0, 500)]
            elif tgt.pos == "atk" and m.atk < tgt.atk + 500 and self.fire(o, "Reinforcements"):
                tgt.tmp = getattr(tgt, "tmp", []) + [(self.turn, 500, 0)]
        mod = m.atk - C[m.name][2]
        rev = max(0, C[m.name][2] - mod)  # attacker ATK after Reverse Trap
        flips = tgt is not None and self._wins(m, tgt) and rev < (tgt.dfn if tgt.pos == "def" else tgt.atk)
        if mod >= 300 and (flips or tgt is None) and self.fire(o, "Reverse Trap"):
            self.reverse_turn = self.turn
        big_hit = tgt is None and (m.atk >= 2000 or m.atk >= o.lp)
        valued = tgt is not None and self._wins(m, tgt) and (tgt.atk >= 1500 or C[tgt.name][1] >= 5)
        if o.waboku != self.turn and (big_hit or valued) and self.fire(o, "Waboku"):
            o.waboku = self.turn
        if m.atk >= 1500 and self.fire(o, "Enchanted Javelin"):
            o.lp += m.atk
        return False

    @staticmethod
    def _wins(m, tgt):
        val = tgt.dfn if tgt.pos == "def" else tgt.atk
        return m.atk > val or (m.atk == val and tgt.pos == "atk")

    def battle_destroy(self, ctrl, m):
        if ctrl.waboku == self.turn:
            return
        super().battle_destroy(ctrl, m)

    def battle_damage(self, pl, amount):
        if pl.waboku == self.turn:
            self.log(f"{pl.name} takes no battle damage (Waboku)")
            return 0
        return super().battle_damage(pl, amount)

    def pierce(self, m, tgt):
        return max(0, m.atk - tgt.dfn) if getattr(m, "pierce", False) else 0
