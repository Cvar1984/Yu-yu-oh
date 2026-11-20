"""Duel simulator v2: generic control/beatdown AI + Exodia AI.

House rules: 1 monster enters the field per player per turn (any summon type),
tributes Lv5-6 = 1 / Lv7+ = 2, 5 monster + 5 S/T zones, max 5 S/T plays per
turn, hand limit 6 at End Phase, deck 40-60, max 3 copies, banlist ignored.
Card text = current official text. First player: no draw, no attack on turn 1.
"""
import random
from collections import Counter

PIECES = ["Exodia the Forbidden One", "Right Arm of the Forbidden One",
          "Left Arm of the Forbidden One", "Right Leg of the Forbidden One",
          "Left Leg of the Forbidden One"]

# name: (kind, level, atk, def)
C = {
    "Exodia the Forbidden One": ("M", 3, 1000, 1000),
    **{p: ("M", 1, 200, 300) for p in PIECES[1:]},
    "Sangan": ("M", 3, 1000, 600), "Witch of the Black Forest": ("M", 4, 1100, 1200),
    "Magician of Faith": ("M", 1, 300, 400), "Man-Eater Bug": ("M", 2, 450, 600),
    "Summoned Skull": ("M", 6, 2500, 1200), "7 Colored Fish": ("M", 4, 1800, 800),
    "Dark Elf": ("M", 4, 2000, 800), "Ryu-Kishin Powered": ("M", 4, 1600, 1200),
    "White Magical Hat": ("M", 3, 1000, 700), "Mystic Lamp": ("M", 1, 400, 300),
    "Blue-Eyes White Dragon": ("M", 8, 3000, 2500), "Giant Soldier of Stone": ("M", 3, 1300, 2000),
    "Pot of Greed": ("S",), "Upstart Goblin": ("S",), "Painful Choice": ("S",),
    "Raigeki": ("S",), "Dark Hole": ("S",), "Change of Heart": ("S",),
    "Snatch Steal": ("S",), "Monster Reborn": ("S",), "Heavy Storm": ("S",),
    "Mystical Space Typhoon": ("S",), "Giant Trunade": ("S",),
    "Swords of Revealing Light": ("S",), "Messenger of Peace": ("S",),
    "Confiscation": ("S",), "Delinquent Duo": ("S",), "The Forceful Sentry": ("S",),
    "Axe of Despair": ("S",), "Tribute to the Doomed": ("S",),
    "Mirror Force": ("T",), "Trap Hole": ("T",), "Magic Jammer": ("T",),
    "Solemn Judgment": ("T",), "Robbin' Goblin": ("T",), "Blank": ("S",),
}
DISRUPT = ("The Forceful Sentry", "Confiscation", "Delinquent Duo")
JAM_WORTHY = {"Raigeki", "Dark Hole", "Change of Heart", "Snatch Steal", "Monster Reborn",
              "Heavy Storm", "Swords of Revealing Light", "Confiscation", "Delinquent Duo",
              "Painful Choice", "Messenger of Peace", "The Forceful Sentry"}
SOLEMN_WORTHY = {"Raigeki", "Dark Hole", "Heavy Storm", "Snatch Steal", "Monster Reborn",
                 "Change of Heart", "Painful Choice"}
TRAPS = ("Seven Tools of the Bandit", "Mirror Force", "Trap Hole", "Magic Jammer", "Solemn Judgment", "Robbin' Goblin")

VALUE = {"Pot of Greed": 95, "Raigeki": 92, "Painful Choice": 85, "Snatch Steal": 90,
         "Change of Heart": 88, "Delinquent Duo": 87, "Confiscation": 84,
         "The Forceful Sentry": 84, "Heavy Storm": 83, "Monster Reborn": 86,
         "Mirror Force": 85, "Dark Hole": 80, "Swords of Revealing Light": 78,
         "Upstart Goblin": 60, "Mystical Space Typhoon": 72, "Solemn Judgment": 70,
         "Magic Jammer": 66, "Trap Hole": 68, "Summoned Skull": 75, "Witch of the Black Forest": 70,
         "Sangan": 69, "Magician of Faith": 64, "Man-Eater Bug": 72, "7 Colored Fish": 60,
         "Dark Elf": 58, "Blue-Eyes White Dragon": 40, "Axe of Despair": 55,
         "Tribute to the Doomed": 62, "Giant Trunade": 50, "Messenger of Peace": 55,
         "Robbin' Goblin": 50, "White Magical Hat": 45, "Mystic Lamp": 40,
         "Ryu-Kishin Powered": 50, "Giant Soldier of Stone": 45}
EXO_VALUE = {"Pot of Greed": 90, "Painful Choice": 85, "Upstart Goblin": 70,
             "Swords of Revealing Light": 60, "Messenger of Peace": 55, "Magician of Faith": 45,
             "Sangan": 42, "Witch of the Black Forest": 41}


STAT_HOOK = None  # optional fn(mon, 2|3) -> stat modifier (fields/equips), set by extensions
FLIP_COUNTS = True      # does a Flip Summon use the turn's Normal Summon? (official: no)
SPECIAL_COUNTS = True   # does a Special Summon use it? (official: no)
DIRECT_ATTACKERS = {"Mystic Lamp"}
WALLS_SET_ONLY = False  # low-ATK/high-DEF monsters are only Set, never summoned to attack
CURRENT_TURN = [0]
NOMI = {"Harpie Lady Sisters", "Relinquished"}  # cannot be Normal Summoned/Set (extended by v8)


def can_tribute(m):
    """Tokens such as Sheep Tokens cannot be Tributed for a Tribute Summon."""
    return not getattr(m, "no_tribute", False)


SET_PRIORITY = ["Man-Eater Bug", "Magician of Faith", "Witch of the Black Forest", "Sangan", "Giant Soldier of Stone"]


def is_wall(c):
    """Low-ATK, high-DEF monster meant to be Set (e.g. Prevent Rat 500/2000, Aqua Madoor 1200/2000)."""
    return C[c][0] == "M" and C[c][3] >= 1500 and C[c][3] - C[c][2] >= 500


def is_mon(c):
    return C[c][0] == "M"


class Mon:
    def __init__(self, name, owner, face_up=True, pos="atk"):
        self.name, self.owner, self.face_up, self.pos = name, owner, face_up, pos
        self.axe = 0
        self.born = CURRENT_TURN[0]
        self.temp = False      # Change of Heart
        self.snatched = None   # ST object of Snatch Steal

    @property
    def atk(self):
        extra = STAT_HOOK(self, 2) if STAT_HOOK else 0
        return max(0, C[self.name][2] + 1000 * self.axe + extra)

    @property
    def dfn(self):
        extra = STAT_HOOK(self, 3) if STAT_HOOK else 0
        return max(0, C[self.name][3] + extra)


class ST:
    def __init__(self, name, face_up, turn):
        self.name, self.face_up, self.set_turn, self.counter = name, face_up, turn, 0
        self.target = None


class Player:
    def __init__(self, name, role, decklist, rng):
        self.name, self.role = name, role
        self.deck = list(decklist)
        rng.shuffle(self.deck)
        self.hand, self.gy, self.mons, self.st = [], [], [], []
        self.lp = 8000
        self.summoned, self.st_plays, self.searched = False, 0, set()

    def value(self, card):
        if self.role == "exodia":
            if card in PIECES:
                return 100 if self.hand.count(card) <= 1 else 0
            return EXO_VALUE.get(card, 10)
        return VALUE.get(card, 10)

    def missing(self):
        return [p for p in PIECES if p not in self.hand]

    def active(self, name):
        return [s for s in self.st if s.name == name and s.face_up]

    def set_traps(self, name, turn):
        return [s for s in self.st if s.name == name and not s.face_up and s.set_turn < turn]

    def board_value(self):
        return sum(m.atk + 300 for m in self.mons if not m.temp)


class Win(Exception):
    def __init__(self, winner, reason):
        self.winner, self.reason = winner, reason


class Duel:
    def __init__(self, p1, p2, rng, log=False, max_turns=80):
        self.p, self.rng, self.log_on, self.max_turns = [p1, p2], rng, log, max_turns
        self.turn, self.lines = 0, []

    # ---------------- basics ----------------
    def log(self, msg):
        if self.log_on:
            self.lines.append(f"T{self.turn:02d} {msg}")

    def opp(self, pl):
        return self.p[1] if pl is self.p[0] else self.p[0]

    def check_exodia(self, pl):
        if pl.role == "exodia" and not pl.missing():
            raise Win(pl, "Exodia assembled")

    def add_hand(self, pl, card):
        pl.hand.append(card)
        self.check_exodia(pl)

    def draw(self, pl, n=1):
        for _ in range(n):
            if not pl.deck:
                raise Win(self.opp(pl), "deck-out")
            self.add_hand(pl, pl.deck.pop())

    def damage(self, pl, amount):
        if amount <= 0:
            return
        pl.lp -= amount
        self.log(f"{pl.name} takes {amount} (LP {pl.lp})")
        if pl.lp <= 0:
            raise Win(self.opp(pl), "LP 0")

    def discard(self, pl, card):
        pl.hand.remove(card)
        pl.gy.append(card)

    def worst(self, pl, exclude=None):
        cands = [c for c in pl.hand if c != exclude] or pl.hand
        return min(cands, key=pl.value)

    def can_st(self, pl):
        return pl.st_plays < 5 and len(pl.st) < 5

    # ---------------- removal primitives ----------------
    def destroy_st(self, owner, s):
        if s not in owner.st:
            return
        owner.st.remove(s)
        owner.gy.append(s.name)
        self.log(f"{owner.name}'s {s.name} destroyed")
        if s.name == "Snatch Steal" and s.target:
            m, ctrl = s.target, self.opp(owner)  # owner of the equip controls the monster
            if m in owner.mons:
                owner.mons.remove(m)
                if len(m.owner.mons) < 5:
                    m.snatched = None
                    m.owner.mons.append(m)
                else:
                    m.owner.gy.append(m.name)

    def remove_mon(self, ctrl, m, to="gy"):
        """Monster leaves the field (destroyed/tributed). Goes to its owner's GY."""
        if m not in ctrl.mons:
            return
        ctrl.mons.remove(m)
        m.owner.gy.append(m.name)
        self.log(f"{m.name} ({ctrl.name}) -> GY")
        if m.snatched:
            for side in self.p:
                if m.snatched in side.st:
                    side.st.remove(m.snatched)
                    side.gy.append("Snatch Steal")
        if m.name in ("Sangan", "Witch of the Black Forest") and m.name not in m.owner.searched:
            self.search(m.owner, m.name)

    def search(self, own, src):
        own.searched.add(src)
        ok = (lambda c: is_mon(c) and C[c][2] <= 1500) if src == "Sangan" else \
             (lambda c: is_mon(c) and C[c][3] <= 1500)
        cands = [c for c in own.deck if ok(c)]
        if own.role == "exodia":
            pref = [c for c in own.missing() if c in cands]
            cands = pref or cands
        if not cands:
            return
        pick = max(cands, key=own.value) if own.role != "exodia" else cands[0]
        own.deck.remove(pick)
        self.rng.shuffle(own.deck)
        self.log(f"{own.name} searches {pick}")
        self.add_hand(own, pick)

    def flip(self, ctrl, m):
        m.face_up = True
        if m.name == "Magician of Faith":
            spells = [c for c in ctrl.gy if C[c][0] == "S" and c != "Blank"]
            if spells:
                pick = max(spells, key=ctrl.value)
                ctrl.gy.remove(pick)
                self.log(f"{ctrl.name} Magician of Faith returns {pick}")
                self.add_hand(ctrl, pick)
        elif m.name == "Man-Eater Bug":
            o = self.opp(ctrl)
            if o.mons:
                t = max(o.mons, key=lambda x: x.atk)
                self.log(f"Man-Eater Bug destroys {t.name}")
                self.remove_mon(o, t)
            elif len(ctrl.mons) > 1:
                pass  # must target a monster; may hit itself - ignore (simplify: none)

    # ---------------- responses ----------------
    def respond_spell(self, caster, card):
        """Opponent may negate with Magic Jammer / Solemn Judgment. True if negated."""
        o = self.opp(caster)
        if card in JAM_WORTHY and o.hand:
            js = o.set_traps("Magic Jammer", self.turn)
            if js:
                self.destroy_st(o, js[0])
                self.discard(o, self.worst(o))
                if self.trap_negated(o, "Magic Jammer"):
                    return False
                self.log(f"{o.name} Magic Jammer negates {card}")
                return True
        if card in SOLEMN_WORTHY and o.lp >= 3000:
            ss = o.set_traps("Solemn Judgment", self.turn)
            if ss:
                self.destroy_st(o, ss[0])
                o.lp -= o.lp // 2
                if self.trap_negated(o, "Solemn Judgment"):
                    return False
                self.log(f"{o.name} Solemn Judgment negates {card} (LP {o.lp})")
                return True
        return False

    def respond_summon(self, summoner, m, kind):
        """Trap Hole (Normal/Flip, ATK>=1000) or Solemn on big summons. True if destroyed."""
        o = self.opp(summoner)
        if kind in ("normal", "flip") and m.atk >= 1000:
            th = o.set_traps("Trap Hole", self.turn)
            if th:
                self.destroy_st(o, th[0])
                if self.trap_negated(o, "Trap Hole"):
                    return False
                self.log(f"{o.name} Trap Hole on {m.name}")
                self.remove_mon(summoner, m)
                return True
        if m.atk >= 2400 and o.lp >= 3000:
            ss = o.set_traps("Solemn Judgment", self.turn)
            if ss:
                self.destroy_st(o, ss[0])
                o.lp -= o.lp // 2
                if self.trap_negated(o, "Solemn Judgment"):
                    return False
                self.log(f"{o.name} Solemn Judgment negates {m.name}")
                self.remove_mon(summoner, m)
                return True
        return False

    def activate(self, pl, card):
        pl.hand.remove(card)
        pl.st_plays += 1
        self.log(f"{pl.name} activates {card}")
        return not self.respond_spell(pl, card)  # callers put the card in the GY

    # ---------------- shared spell effects ----------------
    def painful_choice(self, pl):
        o = self.opp(pl)
        if pl.role == "exodia":
            miss = pl.missing()
            cnt = Counter(pl.deck)
            # Only show missing pieces when the pick is guaranteed AND every
            # missing name keeps >=1 copy in the Deck; otherwise show 5 dead
            # cards (pure Deck thinning) so no missing copy is ever discarded.
            avail = {m: max(0, cnt[m] - 1) for m in miss}
            sel = []
            if sum(avail.values()) >= 5:
                while len(sel) < 5:
                    for m in miss:
                        if avail[m] and len(sel) < 5:
                            sel.append(m)
                            avail[m] -= 1
            else:
                rest = sorted([c for c in pl.deck if c not in miss],
                              key=lambda c: 0 if c in PIECES else pl.value(c))
                sel = rest[:5]
            if not sel:
                return
            give = min(sel, key=lambda c: 1000 if c in miss else pl.value(c))
        else:
            ranked = sorted(pl.deck, key=pl.value, reverse=True)
            sel = ranked[:5]
            give = min(sel, key=pl.value)
        if not sel:
            return
        for c in sel:
            pl.deck.remove(c)
        sel.remove(give)
        pl.gy.extend(sel)
        self.log(f"Painful Choice: {o.name} gives {give}")
        self.add_hand(pl, give)

    def strip(self, caster, kind):
        o = self.opp(caster)
        if not o.hand:
            return
        def tgt():
            return max(o.hand, key=lambda c: (o.value(c), -o.deck.count(c)))
        if kind == "The Forceful Sentry":
            t = tgt()
            o.hand.remove(t)
            o.deck.append(t)
            self.rng.shuffle(o.deck)
            self.log(f"Sentry shuffles {o.name}'s {t}")
        else:
            caster.lp -= 1000
            if kind == "Confiscation":
                t = tgt()
                self.discard(o, t)
                self.log(f"Confiscation: {o.name} loses {t}")
            else:
                t = self.rng.choice(o.hand)
                self.discard(o, t)
                self.log(f"Duo random: {o.name} loses {t}")
                if o.hand:
                    t2 = self.worst(o)
                    self.discard(o, t2)
                    self.log(f"Duo choice: {o.name} loses {t2}")

    # ---------------- Exodia AI ----------------
    def exodia_main(self, pl):
        o = self.opp(pl)
        progress = True
        while progress:
            progress = False
            if not pl.summoned and any(C[c][0] == "S" for c in pl.gy):
                for m in pl.mons:
                    if m.name == "Magician of Faith" and not m.face_up:
                        pl.summoned, m.pos = True, "def"
                        self.log(f"{pl.name} flips Magician of Faith")
                        self.flip(pl, m)
                        progress = True
                        break
            for card in ("Pot of Greed", "Painful Choice", "Upstart Goblin"):
                if card in pl.hand and pl.st_plays < 5 and len(pl.deck) > 6:
                    if self.activate(pl, card):
                        if card == "Pot of Greed":
                            self.draw(pl, 2)
                        elif card == "Upstart Goblin":
                            o.lp += 1000
                            self.draw(pl)
                        else:
                            self.painful_choice(pl)
                    pl.gy.append(card)
                    progress = True
                    break
            if progress:
                continue
            for card in DISRUPT:
                if card in pl.hand and o.hand and pl.st_plays < 5 and \
                        (card == "The Forceful Sentry" or pl.lp > 2000):
                    if self.activate(pl, card):
                        self.strip(pl, card)
                    pl.gy.append(card)
                    progress = True
                    break
            if progress:
                continue
            for card in ("Swords of Revealing Light", "Messenger of Peace"):
                if card in pl.hand and not pl.active(card) and self.can_st(pl):
                    if self.activate(pl, card):
                        self.place_swords_like(pl, card)
                    else:
                        pl.gy.append(card)
                    progress = True
                    break
            if progress:
                continue
            if not pl.summoned and len(pl.mons) < 5:
                for name in ("Magician of Faith", "Sangan", "Witch of the Black Forest"):
                    if name in pl.hand:
                        pl.hand.remove(name)
                        pl.mons.append(Mon(name, pl, False, "def"))
                        pl.summoned = True
                        self.log(f"{pl.name} sets a monster")
                        progress = True
                        break

    def place_swords_like(self, pl, card):
        s = ST(card, True, self.turn)
        s.counter = 3
        pl.st.append(s)
        if card.startswith("Swords"):
            for m in self.opp(pl).mons:
                m.face_up = True

    # ---------------- Generic control AI ----------------
    def ctrl_main(self, pl):
        o = self.opp(pl)
        for _ in range(40):
            if not self.ctrl_step(pl, o):
                break
        self.ctrl_summon(pl, o)
        self.ctrl_post(pl, o)

    def best_opp(self, o):
        return max(o.mons, key=lambda m: m.atk) if o.mons else None

    def ctrl_step(self, pl, o):
        h = pl.hand
        def use(card):
            return card in h and pl.st_plays < 5
        # 1. card draw
        if use("Pot of Greed") and len(pl.deck) > 2:
            if self.activate(pl, "Pot of Greed"):
                self.draw(pl, 2)
            pl.gy.append("Pot of Greed")
            return True
        if use("Painful Choice") and len(pl.deck) > 10:
            if self.activate(pl, "Painful Choice"):
                self.painful_choice(pl)
            pl.gy.append("Painful Choice")
            return True
        if use("Upstart Goblin") and len(pl.deck) > 12:
            if self.activate(pl, "Upstart Goblin"):
                o.lp += 1000
                self.draw(pl)
            pl.gy.append("Upstart Goblin")
            return True
        # 2. hand disruption
        for card in DISRUPT:
            if use(card) and o.hand and (card == "The Forceful Sentry" or pl.lp > 2500):
                if card == "Delinquent Duo" and len(o.hand) < 2 and o.role != "exodia":
                    continue
                if self.activate(pl, card):
                    self.strip(pl, card)
                pl.gy.append(card)
                return True
        # 3. spell/trap removal
        opp_st = o.st
        stall_up = any(x.face_up and x.name in ("Swords of Revealing Light", "Messenger of Peace") for x in opp_st)
        if use("Heavy Storm") and ((len(opp_st) >= 2 and len(opp_st) > len(pl.st)) or
                                   (stall_up and "Mystical Space Typhoon" not in h)):
            if self.activate(pl, "Heavy Storm"):
                for side in (o, pl):
                    for s in list(side.st):
                        self.destroy_st(side, s)
            pl.gy.append("Heavy Storm")
            return True
        if use("Mystical Space Typhoon") and opp_st:
            prio = [s for s in opp_st if s.face_up and s.name in
                    ("Swords of Revealing Light", "Snatch Steal", "Messenger of Peace", "Robbin' Goblin")]
            downs = [s for s in opp_st if not s.face_up]
            attackers = [m for m in pl.mons if m.pos == "atk"]
            tgt = prio[0] if prio else (self.rng.choice(downs) if downs and attackers else None)
            if tgt:
                if self.activate(pl, "Mystical Space Typhoon"):
                    self.destroy_st(o, tgt)
                pl.gy.append("Mystical Space Typhoon")
                return True
        # 4. monster removal / theft
        if o.mons:
            ob = self.best_opp(o)
            mine = max((m.atk for m in pl.mons), default=0)
            if use("Raigeki") and (len(o.mons) >= 2 or ob.atk >= mine or not ob.face_up):
                if self.activate(pl, "Raigeki"):
                    for m in list(o.mons):
                        self.remove_mon(o, m)
                pl.gy.append("Raigeki")
                return True
            if use("Snatch Steal") and self.can_st(pl) and len(pl.mons) < 5 and ob.atk >= 1500:
                if self.activate(pl, "Snatch Steal"):
                    s = ST("Snatch Steal", True, self.turn)
                    s.target, ob.snatched = ob, s
                    pl.st.append(s)
                    o.mons.remove(ob)
                    ob.face_up, ob.pos = True, "atk"
                    pl.mons.append(ob)
                    self.log(f"{pl.name} takes {ob.name}")
                else:
                    pl.gy.append("Snatch Steal")
                return True
            if use("Change of Heart") and len(pl.mons) < 5 and (
                    ob.atk >= 1500 or (not pl.summoned and self.tribute_target(pl))):
                if self.activate(pl, "Change of Heart"):
                    o.mons.remove(ob)
                    ob.temp, ob.face_up, ob.pos = True, True, "atk"
                    pl.mons.append(ob)
                    self.log(f"{pl.name} borrows {ob.name}")
                pl.gy.append("Change of Heart")
                return True
            if use("Dark Hole") and o.board_value() > pl.board_value() + 800:
                if self.activate(pl, "Dark Hole"):
                    for side in (o, pl):
                        for m in list(side.mons):
                            self.remove_mon(side, m)
                pl.gy.append("Dark Hole")
                return True
            if use("Tribute to the Doomed") and len(h) >= 2 and ob.atk >= 1500:
                pl.hand.remove("Tribute to the Doomed")
                pl.st_plays += 1
                self.discard(pl, self.worst(pl))
                if not self.respond_spell(pl, "Tribute to the Doomed"):
                    self.remove_mon(o, ob)
                pl.gy.append("Tribute to the Doomed")
                return True
        # 5. flip a set utility monster
        if not (FLIP_COUNTS and pl.summoned):
            for m in pl.mons:
                if not m.face_up and m.born < self.turn and ((m.name == "Man-Eater Bug" and o.mons) or
                                      (m.name == "Magician of Faith" and
                                       any(C[c][0] == "S" for c in pl.gy))):
                    pl.summoned = pl.summoned or FLIP_COUNTS
                    m.pos = "atk" if m.name == "Man-Eater Bug" else "def"
                    self.log(f"{pl.name} flip summons {m.name}")
                    if not self.respond_summon(pl, m, "flip"):
                        self.flip(pl, m)
                    return True
        return False

    def trap_negated(self, trap_owner, trap):
        """The trap owner's opponent may chain Seven Tools of the Bandit (pay 1000 LP) to negate it."""
        o = self.opp(trap_owner)
        st = o.set_traps("Seven Tools of the Bandit", self.turn)
        if st and o.lp > 1000:
            self.destroy_st(o, st[0])
            o.lp -= 1000
            self.log(f"{o.name} Seven Tools of the Bandit negates {trap}")
            return True
        return False

    def respond_attack(self, pl, m):
        """Extensions: defender's responses to an attack declaration (besides Mirror Force). True = attack stopped."""
        return False

    def on_battle_damage(self, pl, m, o):
        """Extensions: effects when monster m of pl inflicts battle damage to o."""

    def pay_attack_cost(self, pl, m):
        """Extensions add costs to declare an attack (Gravekeeper's Servant, Toll). False = no attack."""
        return True

    def can_revive(self, side, card):
        """Extensions: may this monster be Special Summoned from side's GY (proper-summon rule)."""
        return True

    def battle_destroy(self, ctrl, m):
        """Extensions: destruction by battle (Waboku can prevent it)."""
        self.remove_mon(ctrl, m)

    def battle_damage(self, pl, amount):
        """Extensions: battle damage to pl (Waboku can prevent it). Returns damage dealt."""
        self.damage(pl, amount)
        return amount

    def attack_ok(self, m, x):
        """Extensions: would attacker m pick face-up target x (e.g. avoid Sanga/Kazejin/Suijin)."""
        return True

    def calc_atk(self, m, tgt):
        """Extensions: attacker's ATK during damage calculation."""
        return m.atk

    def pierce(self, m, tgt):
        """Extensions: piercing damage when m destroys Defense Position tgt (Fairy Meteor Crush)."""
        return 0

    def eff_atk(self, card):
        """ATK a card would have on the field (extensions add field bonuses)."""
        return C[card][2]

    def tribute_target(self, pl):
        hi = [c for c in pl.hand if is_mon(c) and C[c][1] >= 5 and c not in NOMI]
        return max(hi, key=lambda c: C[c][2]) if hi else None

    def ctrl_summon(self, pl, o):
        if pl.summoned or len(pl.mons) >= 5:
            return
        ob = self.best_opp(o)
        ob_atk = ob.atk if ob and ob.face_up else (1500 if ob else 0)
        # Monster Reborn best monster from either GY
        gys = [(c, side) for side in (pl, o) for c in side.gy if is_mon(c) and c not in PIECES and self.can_revive(side, c)]
        hand_lv4 = [c for c in pl.hand if is_mon(c) and C[c][1] <= 4 and c not in NOMI]
        best_hand = max((self.eff_atk(c) for c in hand_lv4), default=0)
        if "Monster Reborn" in pl.hand and pl.st_plays < 5 and gys:
            c, side = max(gys, key=lambda x: C[x[0]][2])
            if C[c][2] >= max(1800, best_hand + 300):
                if self.activate(pl, "Monster Reborn"):
                    side.gy.remove(c)
                    m = Mon(c, side, True, "atk")
                    pl.mons.append(m)
                    pl.summoned = pl.summoned or SPECIAL_COUNTS
                    self.log(f"{pl.name} revives {c}")
                    self.respond_summon(pl, m, "special")
                pl.gy.append("Monster Reborn")
                if pl.summoned or len(pl.mons) >= 5:
                    return
        # Tribute summon
        t = self.tribute_target(pl)
        if t:
            need = 1 if C[t][1] <= 6 else 2
            fodder = sorted([m for m in pl.mons if can_tribute(m)], key=lambda m: (not m.temp, m.atk))
            if len(fodder) >= need and C[t][2] > sum(m.atk for m in fodder[:need] if not m.temp):
                for m in fodder[:need]:
                    if m.temp:  # borrowed monster goes to its owner's GY
                        pl.mons.remove(m)
                        m.owner.gy.append(m.name)
                        self.log(f"{pl.name} tributes borrowed {m.name}")
                    else:
                        self.remove_mon(pl, m)
                pl.hand.remove(t)
                m = Mon(t, pl, True, "atk")
                pl.mons.append(m)
                pl.summoned = True
                self.log(f"{pl.name} tribute summons {t}")
                self.respond_summon(pl, m, "normal")
                return
        # Normal summon / set
        msgr = bool(o.active("Messenger of Peace") or pl.active("Messenger of Peace"))
        beaters = [c for c in hand_lv4 if c not in ("Man-Eater Bug", "Magician of Faith")
                   and c not in PIECES and not (WALLS_SET_ONLY and (c in SET_PRIORITY or is_wall(c)))]
        if msgr:
            small = [c for c in hand_lv4 if C[c][2] < 1500 and C[c][2] >= 1000 and c not in PIECES]
            if small and (not ob or ob_atk < max(C[c][2] for c in small)):
                c = max(small, key=lambda x: C[x][2])
                pl.hand.remove(c)
                m = Mon(c, pl, True, "atk")
                pl.mons.append(m)
                pl.summoned = True
                self.log(f"{pl.name} summons {c} under Messenger")
                self.respond_summon(pl, m, "normal")
                return
        if beaters:
            c = max(beaters, key=self.eff_atk)
            if self.eff_atk(c) > ob_atk or not ob:
                pl.hand.remove(c)
                m = Mon(c, pl, True, "atk")
                pl.mons.append(m)
                pl.summoned = True
                self.log(f"{pl.name} summons {c}")
                self.respond_summon(pl, m, "normal")
                return
        for name in SET_PRIORITY:
            if name in pl.hand:
                pl.hand.remove(name)
                pl.mons.append(Mon(name, pl, False, "def"))
                pl.summoned = True
                self.log(f"{pl.name} sets a monster")
                return
        rest = beaters or [c for c in hand_lv4 if c not in PIECES]
        if rest:
            c = max(rest, key=lambda x: C[x][3])
            pl.hand.remove(c)
            pl.mons.append(Mon(c, pl, False, "def"))
            pl.summoned = True
            self.log(f"{pl.name} sets a monster")

    def ctrl_post(self, pl, o):
        ob = self.best_opp(o)
        mine = max((m.atk for m in pl.mons if m.pos == "atk"), default=0)
        if "Swords of Revealing Light" in pl.hand and not pl.active("Swords of Revealing Light") \
                and self.can_st(pl) and ob and ob.atk >= mine:
            if self.activate(pl, "Swords of Revealing Light"):
                self.place_swords_like(pl, "Swords of Revealing Light")
            else:
                pl.gy.append("Swords of Revealing Light")
        if "Messenger of Peace" in pl.hand and not pl.active("Messenger of Peace") and self.can_st(pl):
            if self.activate(pl, "Messenger of Peace"):
                self.place_swords_like(pl, "Messenger of Peace")
            else:
                pl.gy.append("Messenger of Peace")
        if "Axe of Despair" in pl.hand and self.can_st(pl) and not (o.active("Messenger of Peace") or pl.active("Messenger of Peace")):
            att = [m for m in pl.mons if m.pos == "atk" and m.face_up]
            if att:
                m = max(att, key=lambda x: x.atk)
                pl.hand.remove("Axe of Despair")
                pl.st_plays += 1
                m.axe += 1
                pl.gy.append("Axe of Despair")  # simplification
        for t in TRAPS:
            while t in pl.hand and self.can_st(pl):
                pl.hand.remove(t)
                pl.st_plays += 1
                pl.st.append(ST(t, False, self.turn))
        for s in pl.st:
            if s.name == "Robbin' Goblin" and not s.face_up and s.set_turn < self.turn:
                s.face_up = True

    # ---------------- battle ----------------
    def battle(self, pl):
        o = self.opp(pl)
        if o.active("Swords of Revealing Light"):
            return
        msgr = bool(pl.active("Messenger of Peace") or o.active("Messenger of Peace"))
        robbin = bool(pl.active("Robbin' Goblin"))
        order = sorted([m for m in pl.mons if m.pos == "atk" and m.face_up], key=lambda x: -x.atk)
        for m in order:
            if m not in pl.mons or (msgr and m.atk >= 1500) or getattr(m, "paralyzed", False):
                continue
            targets = [x for x in o.mons]
            forced = [x for x in targets if getattr(x, "magnet", False)]  # Ring of Magnetism
            if forced:
                targets = forced
            if (m.name not in DIRECT_ATTACKERS or forced) and targets:
                beatable = [x for x in targets if x.face_up and self.attack_ok(m, x) and
                            (x.atk if x.pos == "atk" else x.dfn) < m.atk]
                hidden = [x for x in targets if not x.face_up]
                if beatable:
                    tgt = max(beatable, key=lambda x: x.atk)
                elif hidden and m.atk >= 1300:
                    tgt = hidden[0]
                else:
                    continue
            else:
                tgt = None
            if not self.pay_attack_cost(pl, m):
                continue
            if m.name == "Dark Elf":
                if pl.lp <= 1000:
                    continue
                pl.lp -= 1000
            self.cur_target, self.redirect_to = tgt, None
            if self.respond_attack(pl, m):
                continue
            if self.redirect_to is not None:
                tgt = self.redirect_to if self.redirect_to in o.mons else tgt
            if m not in pl.mons or (tgt is not None and tgt not in o.mons):
                continue
            # Mirror Force on attack declaration
            mf = o.set_traps("Mirror Force", self.turn)
            if mf and not (self.destroy_st(o, mf[0]) or self.trap_negated(o, "Mirror Force")):
                self.log(f"{o.name} Mirror Force")
                for x in [x for x in pl.mons if x.pos == "atk"]:
                    self.remove_mon(pl, x)
                return
            dmg = 0
            if tgt is None and o.mons and m.name not in DIRECT_ATTACKERS:
                continue  # a monster appeared (e.g. Sheep Tokens): no direct attack
            if tgt is None:
                dmg = m.atk
                self.log(f"{m.name} attacks directly")
            else:
                was_down = not tgt.face_up
                if was_down:
                    tgt.face_up = True
                val = tgt.atk if tgt.pos == "atk" else tgt.dfn
                self.log(f"{m.name} ({m.atk}) attacks {tgt.name}")
                if was_down:
                    self.flip(o, tgt)
                    if m not in pl.mons or tgt not in o.mons:
                        continue  # the flip effect removed a monster: no battle
                    val = tgt.atk if tgt.pos == "atk" else tgt.dfn
                a = self.calc_atk(m, tgt)
                if a > val:
                    dmg = a - val if tgt.pos == "atk" else self.pierce(m, tgt)
                    self.battle_destroy(o, tgt)
                elif a < val:
                    self.battle_damage(pl, val - a)
                    if tgt.pos == "atk":
                        self.battle_destroy(pl, m)
                else:
                    if tgt.pos == "atk":
                        self.battle_destroy(pl, m)
                        self.battle_destroy(o, tgt)
            if dmg > 0 and self.battle_damage(o, dmg) > 0:
                self.on_battle_damage(pl, m, o)
                for trig in (robbin, m.name == "White Magical Hat"):
                    if trig and o.hand:
                        t = self.rng.choice(o.hand)
                        self.discard(o, t)
                        self.log(f"{o.name} random-discards {t}")

    # ---------------- turn ----------------
    def end_phase(self, pl):
        o = self.opp(pl)
        for m in list(pl.mons):
            if m.temp:
                pl.mons.remove(m)
                m.temp = False
                if len(o.mons) < 5:
                    o.mons.append(m)
                else:
                    o.gy.append(m.name)
        for s in list(o.st):
            if s.name == "Swords of Revealing Light":
                s.counter -= 1
                if s.counter <= 0:
                    self.destroy_st(o, s)
        while len(pl.hand) > 6:
            self.discard(pl, self.worst(pl))

    def play_turn(self, pl):
        self.turn += 1
        CURRENT_TURN[0] = self.turn
        o = self.opp(pl)
        pl.summoned, pl.st_plays = False, 0
        pl.searched, o.searched = set(), set()
        if self.turn > 1:
            self.draw(pl)
        for s in list(pl.st):
            if s.name == "Messenger of Peace" and s.face_up:
                if pl.lp > 100:
                    pl.lp -= 100
                else:
                    self.destroy_st(pl, s)
        for s in o.st:
            if s.name == "Snatch Steal":
                pl.lp += 1000
        if pl.role == "exodia":
            self.exodia_main(pl)
        elif pl.role == "ctrl":
            self.ctrl_main(pl)
            if self.turn > 1:
                self.battle(pl)
        self.end_phase(pl)

    def run(self):
        try:
            for pl in self.p:
                self.draw(pl, 5)
            while self.turn < self.max_turns:
                self.play_turn(self.p[self.turn % 2])
        except Win as w:
            return w.winner.name, w.reason, self.turn
        return None, "turn limit", self.turn


def build(spec):
    deck = []
    for name, n in spec.items():
        assert 1 <= n <= 3, name
        assert name in C, name
        deck += [name] * n
    assert 40 <= len(deck) <= 60, (len(deck),)
    return deck
