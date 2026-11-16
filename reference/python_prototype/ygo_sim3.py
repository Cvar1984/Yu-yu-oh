"""Simulator v3: extends v2 with every LOB/MRD/SRL monster's real stats, Field Spells,
type-based Equip Spells, Paralyzing Potion, Hane-Hane and burn spells.
Card data comes from the YGOPRODeck set JSON files in ./sets."""
import json
import ygo_sim2 as S
from ygo_sim2 import C, Mon, ST, is_mon

ATTR, RACE = {}, {}
for f in ("Legend_of_Blue_Eyes_White_Dragon", "Metal_Raiders", "Spell_Ruler"):
    for c in json.load(open(f"sets/{f}.json"))["data"]:
        n = c["name"]
        if "Monster" in c["type"]:
            if n not in C:
                C[n] = ("M", c.get("level", 0), c.get("atk", 0) or 0, c.get("def", 0) or 0)
            ATTR[n], RACE[n] = c.get("attribute"), c.get("race")
        elif n not in C:
            C[n] = ("T",) if "Trap" in c["type"] else ("S",)

# Field Spells: (match function, atk mod, def mod) list
FIELDS = {
    "Umiiruka": [(lambda n: ATTR[n] == "WATER", 500, -400)],
    "Mystic Plasma Zone": [(lambda n: ATTR[n] == "DARK", 500, -400)],
    "Gaia Power": [(lambda n: ATTR[n] == "EARTH", 500, -400)],
    "Molten Destruction": [(lambda n: ATTR[n] == "FIRE", 500, -400)],
    "Rising Air Current": [(lambda n: ATTR[n] == "WIND", 500, -400)],
    "Umi": [(lambda n: RACE[n] in ("Fish", "Sea Serpent", "Thunder", "Aqua"), 200, 200),
            (lambda n: RACE[n] in ("Machine", "Pyro"), -200, -200)],
    "Yami": [(lambda n: RACE[n] in ("Fiend", "Spellcaster"), 200, 200),
             (lambda n: RACE[n] == "Fairy", -200, -200)],
    "Forest": [(lambda n: RACE[n] in ("Insect", "Beast", "Plant", "Beast-Warrior"), 200, 200)],
    "Wasteland": [(lambda n: RACE[n] in ("Dinosaur", "Zombie", "Rock"), 200, 200)],
    "Mountain": [(lambda n: RACE[n] in ("Dragon", "Winged Beast", "Thunder"), 200, 200)],
}
# Equip Spells: name -> (allowed race or None, atk, def)
EQUIPS = {
    "Power of Kaishin": ("Aqua", 300, 300), "Dark Energy": ("Fiend", 300, 300),
    "Legendary Sword": ("Warrior", 300, 300), "Book of Secret Arts": ("Spellcaster", 300, 300),
    "Dragon Treasure": ("Dragon", 300, 300), "Beast Fangs": ("Beast", 300, 300),
    "Raise Body Heat": ("Dinosaur", 300, 300), "Silver Bow and Arrow": ("Fairy", 300, 300),
    "Electro-Whip": ("Thunder", 300, 300), "Follow Wind": ("Winged Beast", 300, 300),
    "Laser Cannon Armor": ("Insect", 300, 300), "Mystical Moon": ("Beast-Warrior", 300, 300),
    "Machine Conversion Factory": ("Machine", 300, 300), "Violet Crystal": ("Zombie", 300, 300),
    "Malevolent Nuzzler": (None, 700, 0), "Sword of Deep-Seated": (None, 500, 500),
}
BURN = {"Hinotama": 500, "Sparks": 200, "Tremendous Fire": 1000}
for name, v in {"Paralyzing Potion": 70, "Hane-Hane": 70, "Umiiruka": 74, "Mystic Plasma Zone": 70,
                "Gaia Power": 70, "Molten Destruction": 70, "Rising Air Current": 70, "Umi": 60,
                "Forest": 55, "Yami": 55, "Malevolent Nuzzler": 62, "Sword of Deep-Seated": 60,
                "Power of Kaishin": 50, "Tremendous Fire": 45, "Hinotama": 35, "Sparks": 20}.items():
    S.VALUE.setdefault(name, v)


def stat_hook(m, idx):
    d = m.owner.duel
    extra = sum(b[idx - 2] for b in getattr(m, "bonus", []))
    if d.field:
        for match, a, df in FIELDS[d.field.name]:
            if match(m.name):
                extra += a if idx == 2 else df
    return extra


S.STAT_HOOK = stat_hook
S.SET_PRIORITY.insert(0, "Hane-Hane")


class Duel3(S.Duel):
    def __init__(self, p1, p2, rng, **kw):
        super().__init__(p1, p2, rng, **kw)
        self.field = None  # one Field Spell on the whole field (house rule)
        p1.duel = p2.duel = self

    # --- field value for a player: sum of ATK gained by own minus opponent's monsters ---
    def field_gain(self, pl, fname):
        o = self.opp(pl)
        def gain(names):
            return sum(a for n in names for match, a, _ in FIELDS[fname] if match(n))
        mine = [m.name for m in pl.mons] + [c for c in pl.hand if is_mon(c)]
        theirs = [m.name for m in o.mons]
        return gain(mine) - gain(theirs)

    def flip(self, ctrl, m):
        if m.name == "Hane-Hane":
            m.face_up = True
            o = self.opp(ctrl)
            if o.mons:
                t = max(o.mons, key=lambda x: x.atk)
                o.mons.remove(t)
                t.owner.hand.append(t.name)
                self.log(f"Hane-Hane returns {t.name} to hand")
            return
        super().flip(ctrl, m)

    def ctrl_step(self, pl, o):
        h = pl.hand
        # Field Spell
        fields = [c for c in h if c in FIELDS]
        if fields and pl.st_plays < 5:
            best = max(fields, key=lambda f: self.field_gain(pl, f))
            cur = self.field_gain(pl, self.field.name) if self.field else 0
            if self.field_gain(pl, best) > max(cur, 0) and not (self.field and self.field.name == best):
                if self.activate(pl, best):
                    if self.field:
                        self.field.owner_p.gy.append(self.field.name)
                    self.field = ST(best, True, self.turn)
                    self.field.owner_p = pl
                    self.log(f"Field is now {best}")
                else:
                    pl.gy.append(best)
                return True
        # Paralyzing Potion on the opponent's best attacker
        if "Paralyzing Potion" in h and pl.st_plays < 5 and o.mons:
            t = max(o.mons, key=lambda x: x.atk)
            mine = max((m.atk for m in pl.mons), default=0)
            if RACE.get(t.name) != "Machine" and t.face_up and not getattr(t, "paralyzed", False) and t.atk >= max(mine, 1500):
                if self.activate(pl, "Paralyzing Potion"):
                    t.paralyzed = True
                pl.gy.append("Paralyzing Potion")
                return True
        if super().ctrl_step(pl, o):
            return True
        # flip Hane-Hane when it has a target
        if not pl.summoned and o.mons:
            for m in pl.mons:
                if m.name == "Hane-Hane" and not m.face_up:
                    pl.summoned, m.pos = True, "def"
                    self.log(f"{pl.name} flip summons Hane-Hane")
                    if not self.respond_summon(pl, m, "flip"):
                        self.flip(pl, m)
                    return True
        return False

    def ctrl_post(self, pl, o):
        msgr = bool(o.active("Messenger of Peace") or pl.active("Messenger of Peace"))
        for eq in [c for c in pl.hand if c in EQUIPS]:
            race, a, d = EQUIPS[eq]
            cands = [m for m in pl.mons if m.face_up and m.pos == "atk" and not m.temp
                     and (race is None or RACE.get(m.name) == race)]
            if cands and self.can_st(pl) and not msgr:
                m = max(cands, key=lambda x: x.atk)
                if self.activate(pl, eq):
                    m.bonus = getattr(m, "bonus", []) + [(a, d)]
                pl.gy.append(eq)  # simplification: equip tracked on the monster
        for b in [c for c in pl.hand if c in BURN]:
            if pl.st_plays < 5 and (b != "Tremendous Fire" or pl.lp > 1500):
                if self.activate(pl, b):
                    if b == "Tremendous Fire":
                        self.damage(pl, 500)
                    self.damage(o, BURN[b])
                pl.gy.append(b)
        super().ctrl_post(pl, o)

    def remove_mon(self, ctrl, m, to="gy"):
        m.bonus, m.paralyzed = [], False
        super().remove_mon(ctrl, m, to)


def _eff_atk(self, card):
    extra = 0
    if self.field:
        for match, a, _ in FIELDS[self.field.name]:
            if match(card):
                extra += a
    return C[card][2] + extra


Duel3.eff_atk = _eff_atk
