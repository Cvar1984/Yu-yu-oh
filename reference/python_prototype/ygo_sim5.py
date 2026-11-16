"""Simulator v5: the user's "patient" strategy on top of the v4 stall AI.

Defend by default (v4 stall AI). Tribute Summon a big monster (Level 6+) only when the
player's rule allows it, then attack with it when the attack is safe.
  big_rule = "never"    : pure stall, big monsters stay in hand
  big_rule = "reckless" : summon whenever there are enough tributes
  big_rule = "solemn"   : only with a Set Solemn Judgment ready (and LP >= 3000 to pay it)
  big_rule = "safe"     : Solemn ready, OR the opponent has <= 1 card in hand and no Set cards
Change of Heart may supply a tribute (steal their monster + 1 own wall = 2 tributes).
Stats: how many big monsters were summoned and how many were gone before the owner's next turn.
"""
from collections import Counter
import ygo_sim4 as V4
from ygo_sim2 import C, Mon, is_mon

BIG_MIN_LEVEL = 6


class Duel5(V4.Duel4):
    def __init__(self, p1, p2, rng, **kw):
        super().__init__(p1, p2, rng, **kw)
        self.stats = Counter()
        self.watch = []  # (player, mon) big monsters summoned last turn

    def safe_to_go_big(self, pl, o):
        rule = getattr(pl, "big_rule", "never")
        solemn_ready = bool(pl.set_traps("Solemn Judgment", self.turn)) and pl.lp >= 3000
        if rule == "reckless":
            return True
        if rule == "solemn":
            return solemn_ready
        if rule == "safe":
            quiet = len(o.hand) <= 1 and not any(not s.face_up for s in o.st)
            return solemn_ready or quiet
        return False

    def try_big(self, pl, o):
        if pl.summoned:
            return
        bigs = [c for c in pl.hand if is_mon(c) and C[c][1] >= BIG_MIN_LEVEL]
        if not bigs or not self.safe_to_go_big(pl, o):
            return
        card = max(bigs, key=lambda c: C[c][2])
        need = 1 if C[card][1] <= 6 else 2
        own = sorted([m for m in pl.mons if not m.temp], key=lambda m: m.dfn)
        # Change of Heart as a tribute source
        if len(own) + len([m for m in pl.mons if m.temp]) < need and "Change of Heart" in pl.hand \
                and o.mons and pl.st_plays < 5 and len(own) >= need - 1:
            if self.activate(pl, "Change of Heart"):
                t = max(o.mons, key=lambda x: x.atk)
                o.mons.remove(t)
                t.temp = True
                pl.mons.append(t)
                self.log(f"{pl.name} borrows {t.name} as a tribute")
            pl.gy.append("Change of Heart")
        fodder = sorted(pl.mons, key=lambda m: (not m.temp, m.dfn))
        if len(fodder) < need:
            return
        for m in fodder[:need]:
            if m.temp:
                pl.mons.remove(m)
                m.owner.gy.append(m.name)
            else:
                self.remove_mon(pl, m)
        pl.hand.remove(card)
        mon = Mon(card, pl, True, "atk")
        pl.mons.append(mon)
        pl.summoned = True
        self.stats["big_summoned"] += 1
        self.log(f"{pl.name} tribute summons {card}")
        if self.respond_summon(pl, mon, "normal"):
            self.stats["big_lost"] += 1
            return
        self.watch.append((pl, mon))

    def play_turn(self, pl):
        if pl.role == "stall":
            # did last turn's big monster survive the opponent's turn?
            for owner, mon in [w for w in self.watch if w[0] is pl]:
                if mon not in pl.mons:
                    self.stats["big_lost"] += 1
                self.watch.remove((owner, mon))
            self.turn += 1
            o = self.opp(pl)
            pl.summoned, pl.st_plays = False, 0
            pl.searched, o.searched = set(), set()
            if self.turn > 1:
                self.draw(pl)
            for s in o.st:
                if s.name == "Snatch Steal":
                    pl.lp += 1000
            self.try_big(pl, o)        # go big first if the rule allows, else defend
            self.stall_main(pl, o)
            if self.turn > 1:
                self.battle(pl)        # only face-up Attack Position monsters attack (the big ones)
            self.end_phase(pl)
            return
        super().play_turn(pl)


def _end_phase(self, pl):
    if pl.role == "stall" and getattr(pl, "big_rule", "never") != "never":
        dead = ("Pot of Greed", "Tremendous Fire", "Mountain", "Karate Man")
        while len(pl.hand) > 6:
            c = next((x for x in pl.hand if x in dead), None) or min(
                pl.hand, key=lambda x: (C[x][3] + (5000 if C[x][1] >= BIG_MIN_LEVEL else 0)) if is_mon(x) else 4000)
            self.discard(pl, c)
    V4.Duel4.end_phase(self, pl)


Duel5.end_phase = _end_phase


def _respond_spell(self, caster, card):
    """Solemn Judgment also guards a face-up big monster against single-target removal."""
    o = self.opp(caster)
    big_up = any(C[m.name][1] >= BIG_MIN_LEVEL and m.face_up for m in o.mons)
    if big_up and card in ("Tribute to the Doomed", "Change of Heart", "Paralyzing Potion") and o.lp >= 3000:
        ss = o.set_traps("Solemn Judgment", self.turn)
        if ss:
            self.destroy_st(o, ss[0])
            o.lp -= o.lp // 2
            self.log(f"{o.name} Solemn Judgment protects its big monster from {card}")
            return True
    return V4.Duel4.respond_spell(self, caster, card)


Duel5.respond_spell = _respond_spell
