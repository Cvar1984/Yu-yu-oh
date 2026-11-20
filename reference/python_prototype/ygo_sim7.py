"""Simulator v7 (Nov 19 update): newly owned cards and the decks' own cards.

Adds on top of v6 (official text, simplified where noted):
  Polymerization: fuse named materials from hand and/or field into a Fusion Monster from the Extra Deck.
  Thunder Dragon: discard it to add up to 2 "Thunder Dragon" from the Deck to the hand.
  Mask of Darkness: FLIP: add 1 Trap from your GY to your hand.
  Barrel Dragon: once per turn, toss 3 coins; with 2+ heads destroy 1 opponent monster (50%).
  Seven Tools of the Bandit: handled in v2's trap_negated hook (pay 1000 LP, negate a Trap).
  Control AI also plays the stall cards: Gravekeeper's Servant, Toll, Chorus of Sanctuary,
  Horn of Light and Ring of Magnetism on its best defender. Walls are Set, never sent to attack.
Not simulated (behave as blanks): Fake Trap, Suijin's and Castle of Dark Illusions' effects.
"""
import json
import ygo_sim2 as S
import ygo_sim3 as V3
import ygo_sim6 as V6
from ygo_sim2 import C, Mon, ST, is_mon

S.WALLS_SET_ONLY = True
if "Mask of Darkness" not in S.SET_PRIORITY:
    S.SET_PRIORITY.append("Mask of Darkness")
FUSIONS = json.load(open("coll3/fusions.json"))
S.VALUE.update({"Polymerization": 70, "Thunder Dragon": 65, "Mask of Darkness": 60, "Barrel Dragon": 70,
                "Seven Tools of the Bandit": 64, "Gravekeeper's Servant": 60, "Chorus of Sanctuary": 55,
                "Ring of Magnetism": 50, "Horn of Light": 45, "Monster Reborn": 86, "Fake Trap": 15})


class Duel7(V6.Duel6):
    # ---------- fusion ----------
    def try_fusion(self, pl):
        if "Polymerization" not in pl.hand or pl.st_plays >= 5 or len(pl.mons) >= 5:
            return False
        field = [m for m in pl.mons if not m.temp]
        best = None
        for f in set(getattr(pl, "extra", [])):
            mats = FUSIONS[f]
            hand, used_field, ok = list(pl.hand), [], True
            hand.remove("Polymerization")
            for mat in mats:
                if mat in hand:
                    hand.remove(mat)
                else:
                    m = next((x for x in field if x.name == mat and x not in used_field), None)
                    if m is None:
                        ok = False
                        break
                    used_field.append(m)
            if ok and (best is None or C[f][2] > C[best[0]][2]):
                best = (f, mats, used_field)
        if not best:
            return False
        f, mats, used_field = best
        if self.activate(pl, "Polymerization"):
            for mat in mats:
                hit = next((m for m in used_field if m.name == mat), None)
                if hit:
                    used_field.remove(hit)
                    self.remove_mon(pl, hit)
                else:
                    self.discard(pl, mat)
            pl.extra.remove(f)
            mon = Mon(f, pl, True, "atk")
            pl.mons.append(mon)
            pl.summoned = pl.summoned or S.SPECIAL_COUNTS
            self.log(f"{pl.name} Fusion Summons {f}")
            self.respond_summon(pl, mon, "special")
        pl.gy.append("Polymerization")
        return True

    def ctrl_step(self, pl, o):
        # Thunder Dragon: discard 1, add up to 2 from the Deck
        if "Thunder Dragon" in pl.hand and "Thunder Dragon" in pl.deck:
            self.discard(pl, "Thunder Dragon")
            for _ in range(2):
                if "Thunder Dragon" in pl.deck:
                    pl.deck.remove("Thunder Dragon")
                    self.add_hand(pl, "Thunder Dragon")
            self.rng.shuffle(pl.deck)
            self.log(f"{pl.name} Thunder Dragon searches")
            return True
        if self.try_fusion(pl):
            return True
        # Barrel Dragon, once per turn each
        for m in pl.mons:
            if m.name == "Barrel Dragon" and m.face_up and getattr(m, "used", -1) != self.turn and o.mons:
                m.used = self.turn
                if self.rng.random() < 0.5:
                    t = max(o.mons, key=lambda x: x.atk)
                    self.log(f"Barrel Dragon destroys {t.name}")
                    self.remove_mon(o, t)
                return True
        return super().ctrl_step(pl, o)

    def flip(self, ctrl, m):
        if m.name == "Mask of Darkness":
            m.face_up = True
            traps = [c for c in ctrl.gy if C[c][0] == "T" and c != "Fake Trap"]
            if traps:
                pick = max(traps, key=ctrl.value)
                ctrl.gy.remove(pick)
                self.add_hand(ctrl, pick)
                self.log(f"Mask of Darkness returns {pick}")
            return
        super().flip(ctrl, m)

    def ctrl_post(self, pl, o):
        h = pl.hand
        for card in ("Gravekeeper's Servant", "Toll"):
            if card in h and self.can_st(pl) and not pl.active(card) and o.mons:
                if self.activate(pl, card):
                    pl.st.append(ST(card, True, self.turn))
                else:
                    pl.gy.append(card)
        if "Chorus of Sanctuary" in h and pl.st_plays < 5 and not self.field and \
                not any(c in V3.FIELDS and c != "Chorus of Sanctuary" for c in h) and \
                any(m.pos == "def" for m in pl.mons):
            if self.activate(pl, "Chorus of Sanctuary"):
                self.field = ST("Chorus of Sanctuary", True, self.turn)
                self.field.owner_p = pl
            else:
                pl.gy.append("Chorus of Sanctuary")
        defenders = [m for m in pl.mons if m.pos == "def" and not m.temp]
        if defenders:
            wall = max(defenders, key=lambda x: x.dfn)
            if "Horn of Light" in h and self.can_st(pl):
                if self.activate(pl, "Horn of Light"):
                    wall.bonus = getattr(wall, "bonus", []) + [(0, 800)]
                pl.gy.append("Horn of Light")
            if "Ring of Magnetism" in h and self.can_st(pl) and o.mons and \
                    not any(getattr(m, "magnet", False) for m in pl.mons):
                if self.activate(pl, "Ring of Magnetism"):
                    wall.bonus = getattr(wall, "bonus", []) + [(-500, -500)]
                    wall.magnet = True
                pl.gy.append("Ring of Magnetism")
        # keep Horn of Light out of the base class's attack-equip logic
        held = [c for c in h if c == "Horn of Light"]
        for c in held:
            h.remove(c)
        super().ctrl_post(pl, o)
        h.extend(held)


def _ctrl_summon(self, pl, o):
    """Tribute-Set a Level 5-6 wall (e.g. Labyrinth Wall 0/3000) over a weaker monster when threatened."""
    if not pl.summoned:
        walls = [c for c in pl.hand if is_mon(c) and 5 <= C[c][1] <= 6 and S.is_wall(c)]
        threat = max((m.atk for m in o.mons), default=0)
        mine = [m for m in pl.mons if not m.temp and C[m.name][1] <= 4 and S.can_tribute(m)]
        if walls and mine:
            card = max(walls, key=lambda c: C[c][3])
            fod = min(mine, key=lambda m: m.dfn)
            if threat >= max(x.dfn for x in pl.mons) and fod.dfn <= C[card][3] - 800:
                self.remove_mon(pl, fod)
                pl.hand.remove(card)
                pl.mons.append(Mon(card, pl, False, "def"))
                pl.summoned = True
                self.log(f"{pl.name} tributes {fod.name} to Set {card}")
                return
    S.Duel.ctrl_summon(self, pl, o)


Duel7.ctrl_summon = _ctrl_summon
