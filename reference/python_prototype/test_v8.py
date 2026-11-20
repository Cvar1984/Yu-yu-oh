"""Scenario tests for every simulated v8 card. Run: python3 test_v8.py"""
import random
import ygo_sim2 as S
import ygo_sim8 as V8
from ygo_sim2 import Player, Mon, ST, C

OX, JINN, SKULL = "Battle Ox", "La Jinn the Mystical Genie of the Lamp", "Summoned Skull"
results = []


def duel(h1=(), h2=(), d1=None, d2=None, seed=1):
    rng = random.Random(seed)
    a = Player("A", "ctrl", d1 or [OX] * 30, rng)
    b = Player("B", "ctrl", d2 or [OX] * 30, rng)
    a.extra = b.extra = []
    d = V8.Duel8(a, b, rng, log=True)
    d.turn = 6
    S.CURRENT_TURN[0] = 6
    a.hand, b.hand = list(h1), list(h2)
    return d, a, b


def setc(p, name, turn=5):
    s = ST(name, False, turn)
    p.st.append(s)
    return s


def mon(p, name, up=True, pos="atk"):
    m = Mon(name, p, up, pos)
    m.born = 1
    p.mons.append(m)
    return m


def test(name):
    def wrap(fn):
        try:
            fn()
            results.append((name, "ok", ""))
        except AssertionError as e:
            results.append((name, "FAIL", str(e)))
        return fn
    return wrap


@test("Normal Monster stats")
def _():
    assert C[JINN] == ("M", 4, 1800, 1000) and C["Judge Man"] == ("M", 6, 2200, 1500), (C[JINN], C["Judge Man"])

@test("Mysterious Puppeteer: +500 per Normal Summon")
def _():
    d, a, b = duel()
    mon(b, "Mysterious Puppeteer")
    m = mon(a, OX)
    d.respond_summon(a, m, "normal")
    assert b.lp == 8500, b.lp

@test("Wicked Worm Beast returns at own End Phase")
def _():
    d, a, b = duel()
    mon(a, "The Wicked Worm Beast")
    d.end_phase(a)
    assert "The Wicked Worm Beast" in a.hand and not a.mons

@test("Penguin Soldier flipped by attack bounces 2, no battle")
def _():
    d, a, b = duel()
    mon(a, OX); mon(a, JINN)
    mon(b, "Penguin Soldier", up=False, pos="def")
    d.battle(a)
    assert sorted(a.hand) == sorted([OX, JINN]) and len(b.mons) == 1 and b.lp == 8000, (a.hand, b.mons, b.lp)

@test("Trap Master destroys a Trap, leaves a Spell")
def _():
    d, a, b = duel()
    tm = mon(a, "Trap Master", up=False, pos="def")
    setc(b, "Waboku")
    d.flip(a, tm)
    assert not b.st and "Waboku" in b.gy
    d2, a2, b2 = duel()
    tm2 = mon(a2, "Trap Master", up=False, pos="def")
    setc(b2, "Scapegoat")
    d2.flip(a2, tm2)
    assert len(b2.st) == 1

@test("Black Luster Ritual: Level 4+4 tributes")
def _():
    d, a, b = duel(h1=["Black Luster Soldier", "Black Luster Ritual", OX, OX])
    assert d.ctrl_step(a, b)
    assert [m.name for m in a.mons] == ["Black Luster Soldier"] and a.gy.count(OX) == 2, (a.mons, a.gy)

@test("Black Luster Soldier revive only after proper Summon")
def _():
    d, a, b = duel()
    a.gy.append("Black Luster Soldier")
    assert not d.can_revive(a, "Black Luster Soldier")
    m = mon(a, "Black Luster Soldier")
    d.remove_mon(a, m)
    assert d.can_revive(a, "Black Luster Soldier")

@test("Graceful Charity: draw 3 discard 2")
def _():
    d, a, b = duel(h1=["Graceful Charity", "Raigeki"])
    d.ctrl_step(a, b)
    assert len(a.hand) == 2 and len(a.gy) == 3 and "Raigeki" in a.hand, (a.hand, a.gy)

@test("Card Destruction: both refresh hands")
def _():
    d, a, b = duel(h1=["Card Destruction", "The Eye of Truth", "Gryphon Wing", "D. Tribe"], h2=["Raigeki", "Pot of Greed", OX])
    d.ctrl_step(a, b)
    assert len(a.hand) == 3 and len(b.hand) == 3 and "Raigeki" in b.gy and "Gryphon Wing" in a.gy, (a.hand, b.hand)

@test("De-Spell destroys a face-up Swords")
def _():
    d, a, b = duel(h1=["De-Spell"])
    s = ST("Swords of Revealing Light", True, 5); b.st.append(s)
    d.ctrl_step(a, b)
    assert not b.st and "Swords of Revealing Light" in b.gy

@test("Dian Keto +1000, Ookazi 800")
def _():
    d, a, b = duel(h1=["Dian Keto the Cure Master", "Ookazi"])
    d.ctrl_post(a, b)
    assert a.lp == 9000 and b.lp == 7200, (a.lp, b.lp)

@test("Fairy Meteor Crush pierces")
def _():
    d, a, b = duel(h1=["Fairy Meteor Crush"])
    mon(a, JINN)
    mon(b, "Dark Assailant", pos="def")   # 1200 DEF
    d.ctrl_post(a, b)
    d.battle(a)
    assert b.lp == 7400, b.lp

@test("Invigoration EARTH +400, Sword of Dark Destruction DARK +400")
def _():
    d, a, b = duel(h1=["Invigoration", "Sword of Dark Destruction"])
    ox, jinn = mon(a, OX), mon(a, JINN)
    d.ctrl_post(a, b)
    assert ox.atk == 2100 and jinn.atk == 2200 and ox.dfn == 800, (ox.atk, jinn.atk, ox.dfn)

@test("Last Will after losing a monster")
def _():
    d, a, b = duel(h1=["Last Will"], d1=[OX] * 10 + ["Sangan"])
    m = mon(a, OX)
    d.remove_mon(a, m)
    d.last_will(a)
    assert [x.name for x in a.mons] == ["Sangan"] and "Sangan" not in a.deck

@test("Nobleman banishes a face-down Flip monster and its copies")
def _():
    d, a, b = duel(h1=["Nobleman of Crossout"], d1=[OX] * 10 + ["Penguin Soldier"], d2=[OX] * 10 + ["Penguin Soldier"] * 2)
    mon(b, "Penguin Soldier", up=False, pos="def")
    d.ctrl_step(a, b)
    assert not b.mons and "Penguin Soldier" not in b.gy + b.deck + a.deck

@test("Premature Burial: pay 800, linked equip")
def _():
    d, a, b = duel(h1=["Premature Burial"])
    a.gy.append(SKULL)
    d.ctrl_step(a, b)
    assert a.lp == 7200 and a.mons[0].name == SKULL and a.st[0].name == "Premature Burial"
    d.destroy_st(a, a.st[0])
    assert not a.mons and SKULL in a.gy

@test("Scapegoat on a direct attack")
def _():
    d, a, b = duel()
    mon(a, OX); mon(a, JINN)
    setc(b, "Scapegoat")
    d.battle(a)
    toks = [m for m in b.mons if m.name == V8.TOKEN]
    assert b.lp == 8000 and len(toks) == 2 and V8.TOKEN not in b.gy, (b.lp, len(toks), b.gy)

@test("Sheep Tokens cannot be Tributed for a Tribute Summon")
def _():
    d, a, b = duel(h1=[SKULL])
    for _ in range(4):
        t = mon(a, V8.TOKEN, pos="def"); t.token = t.no_tribute = True
    d.ctrl_summon(a, b)
    assert all(m.name == V8.TOKEN for m in a.mons) and SKULL in a.hand

@test("Soul Exchange: tribute their monster, no Battle Phase")
def _():
    d, a, b = duel(h1=["Soul Exchange", SKULL])
    mon(b, JINN)
    d.ctrl_summon(a, b)
    d.battle(a)
    assert a.mons[0].name == SKULL and JINN in b.gy and b.lp == 8000

@test("Backup Soldier returns 3 Normal Monsters <= 1500")
def _():
    d, a, b = duel()
    a.gy += ["Dark Assailant", "D. Human", "Ryu-Kishin", "Sangan", SKULL]
    setc(a, "Backup Soldier")
    d.own_turn_traps(a, b)
    assert sorted(a.hand) == sorted(["Dark Assailant", "D. Human", "Ryu-Kishin"]), a.hand

@test("Burst Breath + D. Tribe with a non-Dragon")
def _():
    d, a, b = duel()
    mon(a, JINN)
    for n in ("Dark Assailant", "D. Human", "Master & Expert"):
        mon(b, n)
    setc(a, "D. Tribe"); setc(a, "Burst Breath")
    d.own_turn_traps(a, b)
    assert not b.mons and not a.mons, (b.mons, a.mons)

@test("Castle Walls saves a Defense Position monster")
def _():
    d, a, b = duel()
    mon(a, OX)
    t = mon(b, "Toon Alligator", pos="def")   # 1600 DEF
    setc(b, "Castle Walls")
    d.battle(a)
    assert t in b.mons and a.lp == 8000 - (2100 - 1700), a.lp

@test("Disappear banishes a big monster from their GY")
def _():
    d, a, b = duel()
    b.gy.append(SKULL)
    setc(a, "Disappear")
    d.own_turn_traps(a, b)
    assert SKULL not in b.gy

@test("Dust Tornado destroys Swords, then Sets a card")
def _():
    d, a, b = duel(h1=["Waboku"])
    b.st.append(ST("Swords of Revealing Light", True, 5))
    setc(a, "Dust Tornado")
    d.own_turn_traps(a, b)
    assert not b.st and [s.name for s in a.st] == ["Waboku"]

@test("Enchanted Javelin gains attacker's ATK")
def _():
    d, a, b = duel()
    mon(a, OX)
    mon(b, "Dark Assailant")
    setc(b, "Enchanted Javelin")
    d.battle(a)
    assert b.lp == 8000 + 1700 - 500, b.lp

@test("End Phase traps: Jar of Greed, Just Desserts, Gift")
def _():
    d, a, b = duel()
    mon(a, OX); mon(a, JINN); mon(b, OX)
    for t in ("Jar of Greed", "Just Desserts", "Gift of The Mystical Elf"):
        setc(b, t)
    d.end_traps(b, a)
    assert len(b.hand) == 1 and a.lp == 7000 and b.lp == 8900, (b.hand, a.lp, b.lp)

@test("Light of Intervention: Sets become face-up")
def _():
    d, a, b = duel()
    s = setc(a, "Light of Intervention"); s.face_up = True
    m = Mon(OX, b, False, "def")
    assert m.face_up

@test("Raigeki Break destroys the attacker")
def _():
    d, a, b = duel(h2=["Ookazi"])
    mon(a, JINN)
    setc(b, "Raigeki Break")
    d.battle(a)
    assert not a.mons and b.lp == 8000 and "Ookazi" in b.gy

@test("Reinforcements +500 in battle")
def _():
    d, a, b = duel()
    m = mon(a, JINN)
    mon(b, OX)
    setc(b, "Reinforcements")
    d.battle(a)
    assert m not in a.mons and a.lp == 8000 - 400, (a.mons, a.lp)

@test("Reverse Trap flips an equip bonus")
def _():
    d, a, b = duel(h1=["Sword of Dark Destruction"])
    m = mon(a, JINN)
    mon(b, OX)
    d.ctrl_post(a, b)
    setc(b, "Reverse Trap")
    d.battle(a)
    assert m not in a.mons, a.mons

@test("Riryoku Field negates Change of Heart")
def _():
    d, a, b = duel(h1=["Change of Heart"])
    mon(b, SKULL)
    setc(b, "Riryoku Field")
    d.ctrl_step(a, b)
    assert not a.mons and b.mons and "Riryoku Field" in b.gy

@test("Shadow Spell stops the attack, -700, released when destroyed")
def _():
    d, a, b = duel()
    m = mon(a, OX)
    setc(b, "Shadow Spell")
    d.battle(a)
    assert b.lp == 8000 and m.atk == 1000 and m.paralyzed
    d.destroy_st(b, b.st[0])
    assert m.atk == 1700 and not m.paralyzed

@test("Shift moves the attack to a wall")
def _():
    d, a, b = duel()
    mon(a, JINN)
    weak = mon(b, "Dark Assailant")
    mon(b, "Labyrinth Wall", pos="def")
    setc(b, "Shift")
    d.battle(a)
    assert weak in b.mons and b.lp == 8000 and a.lp == 8000 - 1200, (b.mons, a.lp)

@test("The Eye of Truth: +1000 if a Spell is in hand")
def _():
    d, a, b = duel(h1=["Raigeki"])
    s = setc(b, "The Eye of Truth"); s.face_up = True
    d.standby(a)
    assert a.lp == 9000

@test("Ultimate Offering: extra Normal Summon for 500")
def _():
    d, a, b = duel(h1=[OX, JINN])
    s = setc(a, "Ultimate Offering"); s.face_up = True
    d.ctrl_summon(a, b)
    assert len(a.mons) == 2 and a.lp == 7500, (a.mons, a.lp)

@test("Waboku: no battle damage, no battle destruction")
def _():
    d, a, b = duel()
    mon(a, SKULL)
    t = mon(b, OX)
    setc(b, "Waboku")
    d.battle(a)
    assert t in b.mons and b.lp == 8000

@test("Hane-Hane bounces a Fusion to the Extra Deck")
def _():
    d, a, b = duel()
    h = mon(a, "Hane-Hane", up=False, pos="def")
    mon(b, "Twin-Headed Thunder Dragon")
    d.flip(a, h)
    assert not b.mons and b.extra == ["Twin-Headed Thunder Dragon"]

@test("Star Boy: WATER +500, FIRE -400")
def _():
    d, a, b = duel()
    w, f = mon(a, "7 Colored Fish"), mon(b, "Flame Cerebrus")
    mon(a, "Star Boy")
    assert w.atk == 2300 and f.atk == 1700, (w.atk, f.atk)

@test("Reaper of the Cards destroys a Trap")
def _():
    d, a, b = duel()
    r = mon(a, "Reaper of the Cards", up=False, pos="def")
    setc(b, "Waboku")
    d.flip(a, r)
    assert not b.st

@test("Dead cards stay unplayed")
def _():
    d, a, b = duel(h1=list(V8.DEAD))
    while d.ctrl_step(a, b):
        pass
    d.ctrl_post(a, b)
    assert sorted(a.hand) == sorted(V8.DEAD), a.hand


if __name__ == "__main__":
    for n, r, msg in results:
        print(f"{r:4} {n}" + (f"  -> {msg}" if msg else ""))
    print(sum(r == "ok" for _, r, _ in results), "/", len(results), "passed")
