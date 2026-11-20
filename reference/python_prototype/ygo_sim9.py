"""Simulator v9: the remaining cards the in-game enemies use (all 440 cards in the game), on top of v8.

Added (each has a scenario test in test_v9.py):
  Toons: Toon World (pay 1000), Toon Mermaid, Toon Summoned Skull (1 Tribute), Manga Ryu-Ran and
      Blue-Eyes Toon Dragon (2 Tributes). Special Summoned from hand while you control Toon World,
      cannot attack the turn they are Summoned, pay 500 LP per attack, attack directly, destroyed
      when Toon World leaves. (Simplified: a Toon facing an opponent's Toon still attacks directly.)
  Gate Guardian (Tribute Sanga, Kazejin, Suijin); Sanga/Kazejin/Suijin zero an attacker's ATK once.
  Rituals: Black Illusion Ritual -> Relinquished (absorbs 1 opponent monster, takes its ATK/DEF,
      the absorbed monster dies instead of it in battle), Commencement Dance, Hamburger Recipe,
      Turtle Oath. Senju of the Thousand Hands / Sonic Bird search them.
  Special Summons from Deck when destroyed by battle: Mystic Tomato, Giant Rat, Mother Grizzly,
      Shining Angel, UFO Turtle, Flying Kamakiri #1, Nimble Momonga. Elegant Egotist -> Harpie Lady Sisters.
  Removal / tempo: Cyber Jar, Time Wizard, Catapult Turtle, Cannon Soldier, Armed Ninja,
      Princess of Tsurugi, Invader of the Throne, Two-Pronged Attack, Remove Trap, Stop Defense,
      Final Destiny, Share the Pain, Horn of Heaven, Dragon Capture Jar, Gravedigger Ghoul, Soul Release.
  Boosts: Black Pendant (500 burn when it leaves), Horn of the Unicorn (returns to the Deck top),
      Megamorph (on your monster when behind on LP, on theirs when ahead), Rush Recklessly,
      Snake Fang, The Reliable Guardian, Vile Germs, Germ Infection, Sogen, Luminous Spark, Final Flame,
      Kuriboh, Muka Muka, Maha Vailo, Shadow Ghoul, Lava/Swamp Battleguard, Little Chimera,
      Milus Radiant, Hoshiningen, Witch's Apprentice, Bladefly, Insect Soldiers of the Sky,
      Flash Assailant, Castle of Dark Illusions + Pumpking, Queen's Double, Rainbow Flower.
Still not simulated (rarely played): see NOT_SIMULATED.
"""
import ygo_sim2 as S
import ygo_sim3 as V3
import ygo_sim8 as V8
from ygo_sim2 import C, Mon, ST, is_mon

TOONS = {"Toon Mermaid": 0, "Toon Summoned Skull": 1, "Manga Ryu-Ran": 2, "Blue-Eyes Toon Dragon": 2}
GG_PIECES = ("Sanga of the Thunder", "Kazejin", "Suijin")
SEARCHERS = {"Mystic Tomato": "DARK", "Giant Rat": "EARTH", "Mother Grizzly": "WATER", "Shining Angel": "LIGHT",
             "UFO Turtle": "FIRE", "Flying Kamakiri #1": "WIND"}
ATTR_BOOST = {"Star Boy": ("WATER", "FIRE"), "Little Chimera": ("FIRE", "WATER"), "Milus Radiant": ("EARTH", "WIND"),
              "Hoshiningen": ("LIGHT", "DARK"), "Witch's Apprentice": ("DARK", "LIGHT"), "Bladefly": ("WIND", "EARTH")}
NOT_SIMULATED = ["Shield & Sword", "Curse of Fiend", "Chain Energy", "The Cheerful Coffin", "Tailor of the Fickle",
                 "Fairy's Hand Mirror", "Darkness Approaches", "Magical Labyrinth", "Ameba", "Banisher of the Light",
                 "Big Eye", "Blast Juggler", "Boar Soldier", "Ceremonial Bell", "Cocoon of Evolution", "Crass Clown",
                 "Dragon Piper", "Dream Clown", "Electric Lizard", "Electric Snake", "Giant Germ", "Griggle",
                 "Hiro's Shadow Scout", "Jigen Bakudan", "Kotodama", "Minar", "Mushroom Man #2", "Penguin Knight",
                 "Spear Cretin", "Steel Scorpion", "Tainted Wisdom", "The Immortal of Thunder",
                 "The Little Swordsman of Aile", "The Unhappy Maiden", "Weather Report", "Yado Karu"]

S.NOMI |= set(TOONS) | {"Gate Guardian", "Relinquished", "Performance of Sword", "Hungry Burger", "Crab Turtle",
                       "Harpie Lady Sisters", "Wall Shadow"}
S.DIRECT_ATTACKERS |= set(TOONS) | {"Queen's Double", "Rainbow Flower"}
V8.RITUALS.update({"Black Illusion Ritual": ("Relinquished", 1), "Commencement Dance": ("Performance of Sword", 6),
                   "Hamburger Recipe": ("Hungry Burger", 6), "Turtle Oath": ("Crab Turtle", 8)})
RITUAL_MONS = {m for m, _ in V8.RITUALS.values()}
V3.FIELDS["Sogen"] = [(lambda n: V3.RACE.get(n) in ("Warrior", "Beast-Warrior"), 200, 200)]
V3.FIELDS["Luminous Spark"] = [(lambda n: V3.ATTR.get(n) == "LIGHT", 500, -400)]
V3.EQUIPS["Vile Germs"] = ("Plant", 300, 300)
V3.BURN["Final Flame"] = 600
EXTRA_EQUIPS = {"Black Pendant": (500, 0), "Horn of the Unicorn": (700, 700)}
TRAPS9 = ("Dragon Capture Jar", "Horn of Heaven", "Two-Pronged Attack", "Snake Fang", "The Reliable Guardian")
S.TRAPS = S.TRAPS + tuple(t for t in TRAPS9 if t not in S.TRAPS)
V8.SET_CARDS = V8.SET_CARDS + TRAPS9
V8.STOPPERS = V8.STOPPERS + ("Toon World", "Dragon Capture Jar")
for n in ("Cyber Jar", "Armed Ninja", "Princess of Tsurugi", "Invader of the Throne", "Castle of Dark Illusions"):
    if n not in S.SET_PRIORITY:
        S.SET_PRIORITY.append(n)
S.JAM_WORTHY |= {"Toon World", "Final Destiny", "Black Illusion Ritual", "Turtle Oath", "Megamorph"}
S.SOLEMN_WORTHY |= {"Toon World", "Final Destiny"}
S.VALUE.update({
    "Toon World": 70, "Blue-Eyes Toon Dragon": 75, "Toon Summoned Skull": 72, "Manga Ryu-Ran": 65, "Toon Mermaid": 60,
    "Gate Guardian": 60, "Cyber Jar": 80, "Time Wizard": 55, "Catapult Turtle": 55, "Cannon Soldier": 45,
    "Megamorph": 65, "Black Pendant": 55, "Horn of the Unicorn": 60, "Rush Recklessly": 50, "Final Destiny": 40,
    "Two-Pronged Attack": 50, "Horn of Heaven": 60, "Dragon Capture Jar": 40, "Remove Trap": 35, "Stop Defense": 35,
    "Gravedigger Ghoul": 35, "Soul Release": 35, "Share the Pain": 40, "Mystic Tomato": 55, "Giant Rat": 50,
    "Mother Grizzly": 50, "Shining Angel": 50, "UFO Turtle": 50, "Flying Kamakiri #1": 50, "Nimble Momonga": 45,
    "Kuriboh": 50, "Relinquished": 65, "Black Illusion Ritual": 60, "Senju of the Thousand Hands": 55,
    "Sonic Bird": 50, "Elegant Egotist": 45, "Harpie Lady Sisters": 55, "Armed Ninja": 40, "Princess of Tsurugi": 40,
    "Invader of the Throne": 50, "Snake Fang": 30, "The Reliable Guardian": 35, "Germ Infection": 35})


def fup(side, name):
    return any(x.name == name and x.face_up for x in side.mons)


# ---------------- stats ----------------
_hook8 = S.STAT_HOOK


def stat_hook(m, idx):
    extra = _hook8(m, idx)
    d = getattr(m.owner, "duel", None)
    if d is None:
        return extra
    ctrl = next((p for p in d.p if m in p.mons), m.owner)
    a = V3.ATTR.get(m.name)
    if idx == 2:
        for p in d.p:
            for x in p.mons:
                if x.face_up and x.name in ATTR_BOOST and x.name != "Star Boy":  # Star Boy is applied in v8
                    up, down = ATTR_BOOST[x.name]
                    extra += 500 if a == up else -400 if a == down else 0
        if getattr(m, "mega", None) is not None and m.face_up:
            me, opp = m.mega, d.opp(m.mega)
            base = C[m.name][2]
            extra += base if me.lp < opp.lp else -(base // 2) if me.lp > opp.lp else 0
        extra -= getattr(m, "germ", 0)
        if m.name == "Maha Vailo":
            extra += 500 * (len(getattr(m, "bonus", [])) + m.axe)
        if m.name == "Shadow Ghoul":
            extra += 100 * sum(1 for c in m.owner.gy if is_mon(c))
        if m.name == "Lava Battleguard":
            extra += 500 * sum(1 for x in ctrl.mons if x.name == "Swamp Battleguard" and x.face_up)
        if m.name == "Swamp Battleguard":
            extra += 500 * sum(1 for x in ctrl.mons if x.name == "Lava Battleguard" and x.face_up)
    if m.name == "Muka Muka":
        extra += 300 * len(ctrl.hand)
    if m.name == "Flash Assailant":
        extra -= 400 * len(ctrl.hand)
    if m.name == "Relinquished" and getattr(m, "absorbed", None):
        extra += C[m.absorbed][idx]
    castle = sum(getattr(x, "castle_lvl", 0) for p in d.p for x in p.mons if x.name == "Castle of Dark Illusions" and x.face_up)
    if castle:
        if V3.RACE.get(m.name) == "Zombie":
            extra += 200 * castle
        if m.name == "Pumpking the King of Ghosts":
            extra += 100 * castle
    return extra


S.STAT_HOOK = stat_hook


REVIVE = ("Monster Reborn", "Premature Burial")
DISCARD_BIG = "hand"   # discard plan: "off", "hand" (a revive card is in hand) or "deck" (one is still in the Deck)


class Duel9(V8.Duel8):
    def __init__(self, p1, p2, rng, **kw):
        super().__init__(p1, p2, rng, **kw)
        self.attacker = None

    def worst(self, pl, exclude=None):
        """Discard plan: with a revive card available, discard the strongest big monster we cannot Tribute
        Summon right now (Monster Reborn / Premature Burial bring it back for free)."""
        if pl.role == "ctrl" and DISCARD_BIG != "off":
            cands = [c for c in pl.hand if c != exclude] or pl.hand
            rev = any(r in pl.hand and r != exclude for r in REVIVE) or \
                (DISCARD_BIG == "deck" and any(r in pl.deck for r in REVIVE))
            fod = sum(1 for m in pl.mons if S.can_tribute(m))
            bigs = [c for c in cands if is_mon(c) and c not in S.NOMI and C[c][1] >= 5 and C[c][2] >= 2000
                    and fod < (1 if C[c][1] <= 6 else 2)]
            if rev and bigs:
                return max(bigs, key=lambda c: C[c][2])
        cands = [c for c in pl.hand if c != exclude] or pl.hand
        if pl.role == "exodia":
            return super().worst(pl, exclude)
        return min(cands, key=lambda c: (pl.value(c), C[c][2] if is_mon(c) else 0))  # ties: keep the stronger monster

    # ---------------- leaving the field ----------------
    def _detach(self, m):
        super()._detach(m)
        for p in getattr(m, "pendant", []):        # Black Pendant: 500 to the equipper's opponent
            self.log("Black Pendant burns 500")
            self.damage(self.opp(p), 500)
        for p in getattr(m, "unicorn", []):        # Horn of the Unicorn: back on top of the Deck
            p.deck.insert(0, "Horn of the Unicorn")
            if "Horn of the Unicorn" in p.gy:
                p.gy.remove("Horn of the Unicorn")
        m.pendant, m.unicorn = [], []
        if getattr(m, "absorbed", None):           # Relinquished's monster goes to its owner's GY
            m.absorbed_owner.gy.append(m.absorbed)
            m.absorbed = None

    def destroy_st(self, owner, s):
        super().destroy_st(owner, s)
        if s.name == "Toon World" and not owner.active("Toon World"):
            for m in [x for x in owner.mons if x.name in TOONS]:
                self.log(f"{m.name} is destroyed with Toon World")
                self.remove_mon(owner, m)

    # ---------------- battle ----------------
    def battle(self, pl):
        self.attacker = pl
        o = self.opp(pl)
        self.capture_jar()
        self.pre_battle(pl, o)
        super().battle(pl)

    def capture_jar(self):
        if any(s.name == "Dragon Capture Jar" and s.face_up for p in self.p for s in p.st):
            for p in self.p:
                for m in p.mons:
                    if m.face_up and V3.RACE.get(m.name) == "Dragon":
                        m.pos = "def"

    def pre_battle(self, pl, o):
        """Rush Recklessly / Snake Fang so an attacker can beat a monster it otherwise can't."""
        att = [m for m in pl.mons if m.pos == "atk" and m.face_up and not getattr(m, "paralyzed", False)]
        ups = [x for x in o.mons if x.face_up]
        if "Rush Recklessly" in pl.hand and pl.st_plays < 5:
            pairs = [(x, m) for x in ups for m in att if x.pos == "atk" and m.atk <= x.atk < m.atk + 700]
            if pairs:
                x, m = max(pairs, key=lambda p: p[0].atk)
                if self.activate(pl, "Rush Recklessly"):
                    m.tmp = getattr(m, "tmp", []) + [(self.turn, 700, 0)]
                pl.gy.append("Rush Recklessly")
        if pl.set_traps("Snake Fang", self.turn):
            pairs = [(x, m) for x in ups for m in att if x.pos == "def" and x.dfn >= m.atk > x.dfn - 500]
            if pairs and self.fire(pl, "Snake Fang"):
                x, m = pairs[0]
                x.tmp = getattr(x, "tmp", []) + [(self.turn, 0, -500)]

    def attack_ok(self, m, x):
        return not (x.name in GG_PIECES and x.face_up and not getattr(x, "zeroed", False))

    def calc_atk(self, m, tgt):
        if tgt.name in GG_PIECES and tgt.face_up and not getattr(tgt, "zeroed", False):
            tgt.zeroed = True
            self.log(f"{tgt.name} makes {m.name}'s ATK 0")
            return 0
        if m.name == "Insect Soldiers of the Sky" and V3.ATTR.get(tgt.name) == "WIND":
            return m.atk + 1000
        return m.atk

    def pay_attack_cost(self, pl, m):
        if m.name in TOONS:
            if m.born >= self.turn or pl.lp <= 500:
                return False
        if not super().pay_attack_cost(pl, m):
            return False
        if m.name in TOONS:
            pl.lp -= 500
        return True

    def battle_damage(self, pl, amount):
        if pl is not self.attacker and amount >= 800 and "Kuriboh" in pl.hand and pl.waboku != self.turn:
            self.discard(pl, "Kuriboh")
            self.log(f"{pl.name} discards Kuriboh: no battle damage")
            return 0
        return super().battle_damage(pl, amount)

    def battle_destroy(self, ctrl, m):
        if m.name == "Relinquished" and getattr(m, "absorbed", None) and ctrl.waboku != self.turn:
            self.log(f"Relinquished loses {m.absorbed} instead")
            m.absorbed_owner.gy.append(m.absorbed)
            m.absorbed = None
            return
        super().battle_destroy(ctrl, m)
        if m in ctrl.mons or ctrl is not m.owner:
            return
        if m.name in SEARCHERS and len(ctrl.mons) < 5:
            cands = [c for c in ctrl.deck if is_mon(c) and V3.ATTR.get(c) == SEARCHERS[m.name]
                     and C[c][2] <= 1500 and c not in S.NOMI and c not in S.PIECES]
            if cands:
                c = max(cands, key=lambda x: (ctrl.value(x), C[x][2]))
                ctrl.deck.remove(c)
                self.rng.shuffle(ctrl.deck)
                nm = Mon(c, ctrl, True, "atk")
                ctrl.mons.append(nm)
                self.log(f"{m.name} summons {c} from the Deck")
                self.respond_summon(ctrl, nm, "special")
        if m.name == "Nimble Momonga":
            ctrl.lp += 1000
            while "Nimble Momonga" in ctrl.deck and len(ctrl.mons) < 5:
                ctrl.deck.remove("Nimble Momonga")
                ctrl.mons.append(Mon("Nimble Momonga", ctrl, False, "def"))
                self.log("Nimble Momonga summons a copy")

    def respond_attack(self, pl, m):
        o = self.opp(pl)
        tgt = self.cur_target
        if V3.RACE.get(m.name) == "Dragon" and o.set_traps("Dragon Capture Jar", self.turn):
            s = o.set_traps("Dragon Capture Jar", self.turn)[0]
            if self.trap_negated(o, "Dragon Capture Jar"):
                self.destroy_st(o, s)
            else:
                s.face_up = True
                self.log(f"{o.name} Dragon Capture Jar")
                self.capture_jar()
                return True
        if tgt is not None and tgt.face_up and tgt.pos == "def" and tgt.dfn < m.atk <= tgt.dfn + 700 \
                and self.fire(o, "The Reliable Guardian"):
            tgt.tmp = getattr(tgt, "tmp", []) + [(self.turn, 0, 700)]
        return super().respond_attack(pl, m)

    # ---------------- summons ----------------
    def respond_summon(self, summoner, m, kind):
        o = self.opp(summoner)
        if m.atk >= 1900 and o.set_traps("Horn of Heaven", self.turn):
            fod = [x for x in o.mons if x.atk + 500 < m.atk]
            if fod and self.fire(o, "Horn of Heaven"):
                self.remove_mon(o, min(fod, key=lambda x: x.atk))
                self.log(f"{o.name} Horn of Heaven negates {m.name}")
                self.remove_mon(summoner, m)
                return True
        gone = super().respond_summon(summoner, m, kind)
        if not gone and kind in ("normal", "flip"):
            if m.name == "Senju of the Thousand Hands":
                self.tutor(summoner, [c for c in summoner.deck if c in RITUAL_MONS])
            elif m.name == "Sonic Bird":
                self.tutor(summoner, [c for c in summoner.deck if c in V8.RITUALS])
        return gone

    def tutor(self, pl, cands):
        if not cands:
            return
        def ready(c):  # prefer the half of a ritual pair we are missing
            spells = {s for s, (mm, _) in V8.RITUALS.items() if mm in pl.hand}
            mons = {mm for s, (mm, _) in V8.RITUALS.items() if s in pl.hand}
            return c in spells or c in mons
        c = max(cands, key=lambda x: (ready(x), pl.value(x)))
        pl.deck.remove(c)
        self.rng.shuffle(pl.deck)
        self.log(f"{pl.name} adds {c} from the Deck")
        self.add_hand(pl, c)

    def ctrl_summon(self, pl, o):
        if not pl.summoned and "Time Wizard" in pl.hand and len(pl.mons) < 5 and not fup(pl, "Time Wizard"):
            mine = sum(x.atk + 300 for x in pl.mons)
            theirs = sum((x.atk if x.face_up else 1500) + 300 for x in o.mons)
            if theirs >= mine + 2500:
                pl.hand.remove("Time Wizard")
                m = Mon("Time Wizard", pl, True, "atk")
                pl.mons.append(m)
                pl.summoned = True
                self.log(f"{pl.name} summons Time Wizard")
                self.respond_summon(pl, m, "normal")
        super().ctrl_summon(pl, o)

    def special_from_hand(self, pl, name, tributes):
        for t in tributes:
            if t.temp:
                pl.mons.remove(t)
                t.owner.gy.append(t.name)
            else:
                self.remove_mon(pl, t)
        pl.hand.remove(name)
        m = Mon(name, pl, True, "atk")
        pl.mons.append(m)
        self.log(f"{pl.name} Special Summons {name}")
        self.respond_summon(pl, m, "special")

    # ---------------- own turn ----------------
    def standby(self, pl):
        super().standby(pl)
        for x in pl.mons:
            if x.name == "Castle of Dark Illusions" and x.face_up and getattr(x, "castle_turn", -1) != self.turn:
                x.castle_turn = self.turn
                x.castle_lvl = min(5, getattr(x, "castle_lvl", 1) + (1 if getattr(x, "castle_lvl", 0) else 0))
        for p in self.p:
            for x in p.mons:
                if getattr(x, "germ_on", False) and p is pl and x.face_up:
                    x.germ = getattr(x, "germ", 0) + 300

    def flip(self, ctrl, m):
        o = self.opp(ctrl)
        n = m.name
        if n == "Cyber Jar":
            m.face_up = True
            self.log("Cyber Jar destroys all monsters")
            for side in self.p:
                for x in list(side.mons):
                    self.remove_mon(side, x)
            for side in (ctrl, o):
                top = side.deck[:5]
                del side.deck[:5]
                for c in top:
                    if is_mon(c) and C[c][1] <= 4 and c not in S.NOMI and len(side.mons) < 5:
                        up = C[c][2] >= 1300
                        side.mons.append(Mon(c, side, up, "atk" if up else "def"))
                    else:
                        self.add_hand(side, c)
                self.log(f"Cyber Jar: {side.name} reveals {top}")
            return
        if n == "Armed Ninja":
            m.face_up = True
            ups = [s for s in o.st if s.face_up and C[s.name][0] == "S"]
            if ups:
                self.destroy_st(o, ups[0])
            elif self.field is not None and self.field.owner_p is o:
                o.gy.append(self.field.name)
                self.log(f"Armed Ninja destroys {self.field.name}")
                self.field = None
            else:
                downs = [s for s in o.st if not s.face_up]
                if downs:
                    s = self.rng.choice(downs)
                    if C[s.name][0] == "S":
                        self.destroy_st(o, s)
            return
        if n == "Princess of Tsurugi":
            m.face_up = True
            k = len(o.st) + (1 if self.field is not None and self.field.owner_p is o else 0)
            if k:
                self.log(f"Princess of Tsurugi: {500 * k} damage")
                self.damage(o, 500 * k)
            return
        if n == "Castle of Dark Illusions":
            m.face_up = True
            m.castle_lvl, m.castle_turn = 1, self.turn
            return
        super().flip(ctrl, m)

    def ctrl_step(self, pl, o):
        h = pl.hand

        def use(c):
            return c in h and pl.st_plays < 5

        # Toon World and Toons
        toons = [c for c in h if c in TOONS]
        fodder_n = sum(1 for x in pl.mons if S.can_tribute(x) and x.name not in TOONS)
        summonable = any(TOONS[t] <= fodder_n and len(pl.mons) - TOONS[t] < 5 for t in toons)
        if use("Toon World") and not pl.active("Toon World") and summonable and pl.lp > 1500 and self.can_st(pl):
            pl.lp -= 1000
            if self.activate(pl, "Toon World"):
                pl.st.append(ST("Toon World", True, self.turn))
                self.log(f"{pl.name} pays 1000 for Toon World")
            else:
                pl.gy.append("Toon World")
            return True
        if pl.active("Toon World"):
            for t in sorted(toons, key=lambda c: -C[c][2]):
                need = TOONS[t]
                fod = sorted([x for x in pl.mons if S.can_tribute(x) and x.name not in TOONS],
                             key=lambda x: (not x.temp, x.atk))[:need]
                if len(fod) < need or len(pl.mons) - need >= 5:
                    continue
                if C[t][2] > sum(x.atk for x in fod if not x.temp) + 300:
                    self.special_from_hand(pl, t, fod)
                    return True
        # Gate Guardian
        if "Gate Guardian" in h:
            parts = [next((x for x in pl.mons if x.name == p), None) for p in GG_PIECES]
            if all(parts):
                self.special_from_hand(pl, "Gate Guardian", parts)
                return True
        # Elegant Egotist -> Harpie Lady Sisters
        if use("Elegant Egotist") and fup(pl, "Harpie Lady") and len(pl.mons) < 5:
            src = pl.hand if "Harpie Lady Sisters" in pl.hand else pl.deck if "Harpie Lady Sisters" in pl.deck else None
            if src is not None:
                if self.activate(pl, "Elegant Egotist"):
                    src.remove("Harpie Lady Sisters")
                    m = Mon("Harpie Lady Sisters", pl, True, "atk")
                    pl.mons.append(m)
                    self.log(f"{pl.name} Elegant Egotist summons Harpie Lady Sisters")
                    self.respond_summon(pl, m, "special")
                pl.gy.append("Elegant Egotist")
                return True
        # Time Wizard (once per turn)
        tw = next((x for x in pl.mons if x.name == "Time Wizard" and x.face_up and getattr(x, "tw_turn", -1) != self.turn), None)
        if tw is not None:
            mine = sum(x.atk + 300 for x in pl.mons if x is not tw)
            theirs = sum((x.atk if x.face_up else 1500) + 300 for x in o.mons)
            if theirs > mine + 1000:
                tw.tw_turn = self.turn
                if self.rng.random() < 0.5:
                    self.log("Time Wizard: right call, their monsters are destroyed")
                    for x in list(o.mons):
                        self.remove_mon(o, x)
                else:
                    lost = [x for x in pl.mons]
                    dmg = sum(x.atk for x in lost if x.face_up) // 2
                    self.log("Time Wizard: wrong call")
                    for x in lost:
                        self.remove_mon(pl, x)
                    self.damage(pl, dmg)
                return True
        # Relinquished absorbs a monster (once per turn)
        rel = next((x for x in pl.mons if x.name == "Relinquished" and x.face_up and not getattr(x, "absorbed", None)
                    and getattr(x, "rel_turn", -1) != self.turn), None)
        if rel is not None and o.mons:
            t = max(o.mons, key=lambda x: C[x.name][2])
            rel.rel_turn = self.turn
            self._off_field(o, t)
            if not getattr(t, "token", False):
                rel.absorbed, rel.absorbed_owner = t.name, t.owner
            self.log(f"Relinquished absorbs {t.name}")
            return True
        # flip-summon utility monsters
        for m in pl.mons:
            if not m.face_up and m.born < self.turn and m.name == "Invader of the Throne":
                best = max(o.mons, key=lambda x: x.atk if x.face_up else 1500, default=None)
                if best is not None and (best.atk if best.face_up else 1500) >= 1600:
                    m.face_up, m.pos = True, "atk"
                    self.log(f"{pl.name} flip summons Invader of the Throne")
                    if not self.respond_summon(pl, m, "flip") and m in pl.mons and best in o.mons:
                        o.mons.remove(best)
                        pl.mons.remove(m)
                        best.face_up = True
                        pl.mons.append(best)
                        o.mons.append(m)
                        self.log(f"Invader of the Throne swaps for {best.name}")
                    return True
        # removal spells
        ob = self.best_opp(o)
        my_best = max((x.atk for x in pl.mons if x.pos == "atk" and x.face_up), default=0)
        if use("Final Destiny") and len(h) >= 6:
            theirs = sum(x.atk + 500 for x in o.mons) + 800 * len(o.st)
            mine = sum(x.atk + 500 for x in pl.mons) + 800 * len(pl.st)
            if theirs - mine >= 4000:
                h.remove("Final Destiny")
                for c in sorted(h, key=pl.value)[:5]:
                    self.discard(pl, c)
                pl.st_plays += 1
                if not self.respond_spell(pl, "Final Destiny"):
                    self.log(f"{pl.name} Final Destiny destroys everything")
                    for side in self.p:
                        for x in list(side.mons):
                            self.remove_mon(side, x)
                        for s in list(side.st):
                            self.destroy_st(side, s)
                    if self.field is not None:
                        self.field.owner_p.gy.append(self.field.name)
                        self.field = None
                pl.gy.append("Final Destiny")
                return True
        if use("Share the Pain") and len(o.mons) == 1 and ob is not None and ob.atk >= 1800:
            cheap = [x for x in pl.mons if x.temp or getattr(x, "token", False) or x.atk < 1000]
            if cheap:
                if self.activate(pl, "Share the Pain"):
                    self.remove_mon(pl, min(cheap, key=lambda x: x.atk))
                    self.remove_mon(o, o.mons[0])
                pl.gy.append("Share the Pain")
                return True
        if pl.set_traps("Two-Pronged Attack", self.turn) and ob is not None and ob.atk >= 2000:
            cheap = sorted([x for x in pl.mons if x.temp or getattr(x, "token", False) or x.atk <= 1000], key=lambda x: x.atk)
            if len(cheap) >= 2 and self.fire(pl, "Two-Pronged Attack"):
                for x in cheap[:2]:
                    self.remove_mon(pl, x)
                self.remove_mon(o, ob)
                return True
        if use("Remove Trap"):
            t = next((s for s in o.st if s.face_up and C[s.name][0] == "T"), None)
            if t is not None:
                if self.activate(pl, "Remove Trap"):
                    self.destroy_st(o, t)
                pl.gy.append("Remove Trap")
                return True
        if use("Stop Defense") and my_best:
            cands = [x for x in o.mons if x.pos == "def" and (x.atk if x.face_up else C[x.name][2]) < my_best
                     and (not x.face_up or x.dfn >= my_best)]
            if cands:
                x = cands[0]
                if self.activate(pl, "Stop Defense"):
                    was_down = not x.face_up
                    x.face_up, x.pos = True, "atk"
                    self.log(f"Stop Defense: {x.name} to Attack Position")
                    if was_down:
                        self.flip(o, x)
                pl.gy.append("Stop Defense")
                return True
        for card, k in (("Gravedigger Ghoul", 2), ("Soul Release", 5)):
            big = sorted([c for c in o.gy if is_mon(c) and C[c][2] >= 1800], key=lambda c: -C[c][2])
            if use(card) and big:
                if self.activate(pl, card):
                    for c in big[:k]:
                        o.gy.remove(c)
                    self.log(f"{card} banishes {big[:k]}")
                pl.gy.append(card)
                return True
        return super().ctrl_step(pl, o)

    def ctrl_post(self, pl, o):
        h = pl.hand
        msgr = bool(o.active("Messenger of Peace") or pl.active("Messenger of Peace"))
        att = [m for m in pl.mons if m.face_up and m.pos == "atk" and not m.temp]
        for eq, (a, d) in EXTRA_EQUIPS.items():
            while eq in h and att and self.can_st(pl) and not msgr:
                m = max(att, key=lambda x: x.atk)
                if self.activate(pl, eq):
                    m.bonus = getattr(m, "bonus", []) + [(a, d)]
                    key = "pendant" if eq == "Black Pendant" else "unicorn"
                    setattr(m, key, getattr(m, key, []) + [pl])
                pl.gy.append(eq)
        if "Megamorph" in h and self.can_st(pl) and not msgr and pl.lp != o.lp:
            ups = [x for x in o.mons if x.face_up and getattr(x, "mega", None) is None]
            tgt = max(att, key=lambda x: C[x.name][2], default=None) if pl.lp < o.lp else \
                max(ups, key=lambda x: C[x.name][2], default=None)
            if tgt is not None and getattr(tgt, "mega", None) is None:
                if self.activate(pl, "Megamorph"):
                    tgt.mega = pl
                    tgt.bonus = getattr(tgt, "bonus", []) + [(0, 0)]
                pl.gy.append("Megamorph")
        if "Germ Infection" in h and self.can_st(pl) and not msgr:
            ups = [x for x in o.mons if x.face_up and V3.RACE.get(x.name) != "Machine" and not getattr(x, "germ_on", False)]
            if ups:
                t = max(ups, key=lambda x: x.atk)
                if self.activate(pl, "Germ Infection"):
                    t.germ_on = True
                pl.gy.append("Germ Infection")
        super().ctrl_post(pl, o)

    def main2(self, pl):
        super().main2(pl)
        o = self.opp(pl)
        # Catapult Turtle (once per turn each) / Cannon Soldier: borrowed monsters, or for lethal
        for cat in [x for x in pl.mons if x.name == "Catapult Turtle" and x.face_up]:
            fod = [x for x in pl.mons if x is not cat]
            pick = next((x for x in fod if x.temp), None) or \
                next((x for x in fod if x.atk // 2 >= o.lp), None)
            if pick is not None and getattr(cat, "cat_turn", -1) != self.turn:
                cat.cat_turn = self.turn
                dmg = pick.atk // 2
                self.tribute_effect(pl, pick)
                self.log(f"Catapult Turtle launches {pick.name}: {dmg}")
                self.damage(o, dmg)
        if fup(pl, "Cannon Soldier"):
            fod = [x for x in pl.mons if x.name != "Cannon Soldier"]
            for x in [x for x in fod if x.temp] or (fod if 500 * len(fod) >= o.lp else []):
                self.tribute_effect(pl, x)
                self.log("Cannon Soldier: 500 damage")
                self.damage(o, 500)

    def tribute_effect(self, pl, x):
        if x.temp:
            pl.mons.remove(x)
            x.owner.gy.append(x.name)
        else:
            self.remove_mon(pl, x)
