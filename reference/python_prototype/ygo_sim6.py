"""Simulator v6 (Nov 18 update): the owner's latest summon rule plus effects for newly owned cards.

Rules switched to what the owner described: only Normal/Tribute Summon (or Set) is limited to
1 per turn; Flip Summons and Special Summons do not use it.
New card effects (official text, simplified where noted):
  Jinzo #7, Leghul, Ooguchi: can attack directly.
  Fissure: destroy the face-up monster your opponent controls with the lowest ATK.
  Stim-Pack: equip +700 ATK, then -200 ATK each of your Standby Phases.
  The Bistro Butcher: when it inflicts battle damage, the opponent draws 2.
  Masked Sorcerer: when it inflicts battle damage, its controller draws 1.
  Jirai Gumo: toss a coin when it attacks; tails = its controller loses half their LP.
  Dark Zebra: approximated as unable to attack while it is your only monster.
  Spellbinding Circle (trap): on attack declaration, the attacker can't attack (stays bound).
Cards still not simulated are kept out of deck searches (see EXCLUDE in run_update.py).
"""
import ygo_sim2 as S
import ygo_sim4 as V4
from ygo_sim2 import C, ST

S.FLIP_COUNTS = False
S.SPECIAL_COUNTS = False
S.DIRECT_ATTACKERS |= {"Jinzo #7", "Leghul", "Ooguchi"}
S.VALUE.update({"Fissure": 75, "Stim-Pack": 58, "Spellbinding Circle": 66, "Jirai Gumo": 55,
                "The Bistro Butcher": 45, "Masked Sorcerer": 50, "Summoned Skull": 75})
if "Spellbinding Circle" not in S.TRAPS:
    S.TRAPS = S.TRAPS + ("Spellbinding Circle",)


class Duel6(V4.Duel4):
    def ctrl_step(self, pl, o):
        if "Fissure" in pl.hand and pl.st_plays < 5:
            ups = [m for m in o.mons if m.face_up]
            if ups:
                t = min(ups, key=lambda m: m.atk)
                if t.atk >= 1200 or len(o.mons) == 1:
                    if self.activate(pl, "Fissure"):
                        self.remove_mon(o, t)
                    pl.gy.append("Fissure")
                    return True
        return super().ctrl_step(pl, o)

    def ctrl_post(self, pl, o):
        if "Stim-Pack" in pl.hand and self.can_st(pl):
            att = [m for m in pl.mons if m.face_up and m.pos == "atk" and not m.temp]
            if att:
                m = max(att, key=lambda x: x.atk)
                if self.activate(pl, "Stim-Pack"):
                    m.stim = getattr(m, "stim", 0) + 700
                    m.bonus = getattr(m, "bonus", []) + [(0, 0)]  # marks it as equipped
                pl.gy.append("Stim-Pack")
        super().ctrl_post(pl, o)

    def play_turn(self, pl):
        # Stim-Pack decay at this player's Standby Phase (applied at turn start)
        for m in pl.mons:
            if getattr(m, "stim", 0) > 0:
                m.stim = max(0, m.stim - 200)
        super().play_turn(pl)

    def pay_attack_cost(self, pl, m):
        if m.name == "Dark Zebra" and len(pl.mons) == 1:
            return False
        if not super().pay_attack_cost(pl, m):
            return False
        if m.name == "Jirai Gumo" and self.rng.random() < 0.5:
            pl.lp -= pl.lp // 2
            self.log(f"{pl.name} Jirai Gumo coin: tails, LP {pl.lp}")
        return True

    def respond_attack(self, pl, m):
        o = self.opp(pl)
        sc = o.set_traps("Spellbinding Circle", self.turn)
        if sc and m.atk >= 1500:
            s = sc[0]
            o.st.remove(s)
            o.gy.append("Spellbinding Circle")  # simplification: bound monster stays bound after
            if self.trap_negated(o, "Spellbinding Circle"):
                return False
            m.paralyzed = True
            self.log(f"{o.name} Spellbinding Circle binds {m.name}")
            return True
        return False

    def on_battle_damage(self, pl, m, o):
        if m.name == "The Bistro Butcher":
            self.draw(o, 2)
        elif m.name == "Masked Sorcerer":
            self.draw(pl, 1)

    def remove_mon(self, ctrl, m, to="gy"):
        m.stim = 0
        super().remove_mon(ctrl, m, to)


_hook = S.STAT_HOOK


def stat_hook(m, idx):
    extra = _hook(m, idx)
    if idx == 2:
        extra += getattr(m, "stim", 0)
    return extra


S.STAT_HOOK = stat_hook
