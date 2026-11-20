"""Scenario tests for the v9 cards. Run: python3 test_v9.py"""
import random
import ygo_sim2 as S
import ygo_sim9 as V9
from ygo_sim2 import Player, Mon, ST, C

OX, SKULL = "Battle Ox", "Summoned Skull"
results = []


def duel(h1=(), h2=(), d1=None, d2=None, seed=1):
    rng = random.Random(seed)
    a = Player("A", "ctrl", d1 or [OX] * 30, rng)
    b = Player("B", "ctrl", d2 or [OX] * 30, rng)
    a.extra = b.extra = []
    d = V9.Duel9(a, b, rng, log=True)
    d.turn = 6
    S.CURRENT_TURN[0] = 6
    a.hand, b.hand = list(h1), list(h2)
    return d, a, b


def setc(p, name, turn=5, up=False):
    s = ST(name, up, turn)
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


@test("Toon World: pay 1000, then Toon Summoned Skull (1 tribute), no attack this turn")
def _():
    d, a, b = duel(h1=["Toon World", "Toon Summoned Skull"])
    mon(a, "Dark Assailant")
    while d.ctrl_step(a, b):
        pass
    assert a.lp == 7000 and a.active("Toon World") and [m.name for m in a.mons] == ["Toon Summoned Skull"], (a.lp, a.mons)
    d.battle(a)
    assert b.lp == 8000

@test("Toon attacks directly over a monster, pays 500")
def _():
    d, a, b = duel()
    a.st.append(ST("Toon World", True, 3))
    mon(a, "Toon Summoned Skull")
    mon(b, "Labyrinth Wall", pos="def")
    d.battle(a)
    assert b.lp == 5500 and a.lp == 7500, (b.lp, a.lp)

@test("Toons die when Toon World is destroyed")
def _():
    d, a, b = duel()
    s = ST("Toon World", True, 3); a.st.append(s)
    mon(a, "Toon Mermaid")
    d.destroy_st(a, s)
    assert not a.mons and "Toon Mermaid" in a.gy

@test("Gate Guardian: tribute Sanga, Kazejin, Suijin")
def _():
    d, a, b = duel(h1=["Gate Guardian"])
    for p in V9.GG_PIECES:
        mon(a, p)
    d.ctrl_step(a, b)
    assert [m.name for m in a.mons] == ["Gate Guardian"] and a.mons[0].atk == 3750

@test("Kazejin zeroes an attacker once; attackers avoid it")
def _():
    d, a, b = duel()
    m = mon(a, "Blue-Eyes White Dragon")
    k = mon(b, "Kazejin")
    assert not d.attack_ok(m, k)
    assert d.calc_atk(m, k) == 0 and d.calc_atk(m, k) == 3000

@test("Mystic Tomato summons a DARK monster from the Deck")
def _():
    d, a, b = duel(d2=[OX] * 5 + ["Sangan"])
    mon(a, SKULL)
    mon(b, "Mystic Tomato")
    d.battle(a)
    assert [m.name for m in b.mons] == ["Sangan"] and "Sangan" not in b.deck, b.mons

@test("Nimble Momonga: +1000 and copies face-down")
def _():
    d, a, b = duel(d2=[OX] * 5 + ["Nimble Momonga"] * 2)
    mon(a, SKULL)
    mon(b, "Nimble Momonga")
    d.battle(a)
    assert b.lp == 8000 - 1500 + 1000 and len(b.mons) == 2 and not b.mons[0].face_up, (b.lp, b.mons)

@test("Cyber Jar destroys all, both reveal 5")
def _():
    d, a, b = duel()
    a.deck, b.deck = [OX, OX, "Raigeki", OX, OX, OX], ["Pot of Greed"] * 5 + [OX]
    j = mon(a, "Cyber Jar", up=False, pos="def")
    mon(b, SKULL)
    d.flip(a, j)
    assert len(a.mons) == 4 and "Raigeki" in a.hand and not b.mons and b.hand.count("Pot of Greed") == 5

@test("Time Wizard: one side's monsters destroyed")
def _():
    d, a, b = duel(h1=["Time Wizard"])
    mon(b, SKULL); mon(b, "Blue-Eyes White Dragon")
    d.ctrl_summon(a, b)
    d.ctrl_step(a, b)
    assert not b.mons or not a.mons, (a.mons, b.mons)

@test("Catapult Turtle launches a borrowed monster")
def _():
    d, a, b = duel()
    mon(a, "Catapult Turtle")
    t = Mon(SKULL, b, True, "atk"); t.temp = True; a.mons.append(t)
    d.main2(a)
    assert b.lp == 8000 - 1250 and SKULL in b.gy, (b.lp, b.gy)

@test("Cannon Soldier for lethal")
def _():
    d, a, b = duel()
    mon(a, "Cannon Soldier"); mon(a, OX)
    b.lp = 500
    try:
        d.main2(a)
        assert False, "no win"
    except S.Win:
        pass

@test("Armed Ninja destroys a face-up Spell")
def _():
    d, a, b = duel()
    n = mon(a, "Armed Ninja", up=False, pos="def")
    b.st.append(ST("Toon World", True, 3))
    d.flip(a, n)
    assert not b.st

@test("Princess of Tsurugi burns 500 per S/T")
def _():
    d, a, b = duel()
    p = mon(a, "Princess of Tsurugi", up=False, pos="def")
    setc(b, "Waboku"); setc(b, "Mirror Force")
    d.flip(a, p)
    assert b.lp == 7000

@test("Invader of the Throne swaps control")
def _():
    d, a, b = duel()
    mon(a, "Invader of the Throne", up=False, pos="def")
    mon(b, SKULL)
    d.ctrl_step(a, b)
    assert a.mons[0].name == SKULL and b.mons[0].name == "Invader of the Throne"

@test("Relinquished ritual + absorb + battle shield")
def _():
    d, a, b = duel(h1=["Black Illusion Ritual", "Relinquished", "Dark Assailant"])
    mon(b, SKULL)
    d.ctrl_step(a, b); d.ctrl_step(a, b)
    r = a.mons[0]
    assert r.name == "Relinquished" and r.atk == 2500 and not b.mons, (a.mons, b.mons)
    d.battle_destroy(a, r)
    assert r in a.mons and r.atk == 0 and SKULL in b.gy

@test("Senju searches a Ritual Monster")
def _():
    d, a, b = duel(d1=[OX] * 5 + ["Crab Turtle"])
    m = mon(a, "Senju of the Thousand Hands")
    d.respond_summon(a, m, "normal")
    assert "Crab Turtle" in a.hand

@test("Elegant Egotist -> Harpie Lady Sisters")
def _():
    d, a, b = duel(h1=["Elegant Egotist"], d1=[OX] * 5 + ["Harpie Lady Sisters"])
    mon(a, "Harpie Lady")
    d.ctrl_step(a, b)
    assert any(m.name == "Harpie Lady Sisters" for m in a.mons)

@test("Black Pendant +500, 500 burn when it leaves")
def _():
    d, a, b = duel(h1=["Black Pendant"])
    m = mon(a, OX)
    d.ctrl_post(a, b)
    assert m.atk == 2200
    d.remove_mon(a, m)
    assert b.lp == 7500

@test("Horn of the Unicorn returns to the Deck top")
def _():
    d, a, b = duel(h1=["Horn of the Unicorn"])
    m = mon(a, OX)
    d.ctrl_post(a, b)
    assert m.atk == 2400
    d.remove_mon(a, m)
    assert a.deck[0] == "Horn of the Unicorn" and "Horn of the Unicorn" not in a.gy

@test("Megamorph doubles when behind, halves theirs when ahead")
def _():
    d, a, b = duel(h1=["Megamorph"])
    m = mon(a, OX); a.lp = 4000
    d.ctrl_post(a, b)
    assert m.atk == 3400, m.atk
    d2, a2, b2 = duel(h1=["Megamorph"])
    t = mon(b2, SKULL); b2.lp = 4000
    d2.ctrl_post(a2, b2)
    assert t.atk == 1250, t.atk

@test("Rush Recklessly lets a weaker monster win")
def _():
    d, a, b = duel(h1=["Rush Recklessly"])
    mon(a, OX)
    t = mon(b, "Judge Man")
    d.battle(a)
    assert t not in b.mons and b.lp == 8000 - 200

@test("Snake Fang lowers DEF for the attack")
def _():
    d, a, b = duel()
    mon(a, OX)
    t = mon(b, "Toon Alligator", pos="def")
    setc(a, "Snake Fang")
    d.battle(a)
    assert t not in b.mons

@test("The Reliable Guardian +700 DEF")
def _():
    d, a, b = duel()
    mon(a, "Judge Man")
    t = mon(b, "Toon Alligator", pos="def")
    setc(b, "The Reliable Guardian")
    d.battle(a)
    assert t in b.mons and a.lp == 8000 - 100

@test("Dragon Capture Jar stops a Dragon")
def _():
    d, a, b = duel()
    m = mon(a, "Blue-Eyes White Dragon")
    setc(b, "Dragon Capture Jar")
    d.battle(a)
    assert b.lp == 8000 and m.pos == "def"

@test("Horn of Heaven negates a big summon")
def _():
    d, a, b = duel()
    mon(b, "Dark Assailant")
    setc(b, "Horn of Heaven")
    m = mon(a, "Blue-Eyes White Dragon")
    assert d.respond_summon(a, m, "normal") and not a.mons and not b.mons

@test("Two-Pronged Attack")
def _():
    d, a, b = duel()
    for _ in range(2):
        t = mon(a, V9.V8.TOKEN, pos="def"); t.token = True
    mon(b, SKULL)
    setc(a, "Two-Pronged Attack")
    d.ctrl_step(a, b)
    assert not b.mons and not a.mons

@test("Final Destiny clears the field")
def _():
    d, a, b = duel(h1=["Final Destiny", OX, OX, OX, OX, OX])
    mon(b, SKULL); mon(b, "Blue-Eyes White Dragon"); setc(b, "Mirror Force")
    d.ctrl_step(a, b)
    assert not b.mons and not b.st and len(a.hand) == 0

@test("Share the Pain")
def _():
    d, a, b = duel(h1=["Share the Pain"])
    t = mon(a, V9.V8.TOKEN, pos="def"); t.token = True
    mon(b, SKULL)
    d.ctrl_step(a, b)
    assert not b.mons and not a.mons

@test("Remove Trap, Stop Defense, Gravedigger Ghoul")
def _():
    d, a, b = duel(h1=["Remove Trap"])
    b.st.append(ST("Dragon Capture Jar", True, 3))
    d.ctrl_step(a, b)
    assert not b.st
    d, a, b = duel(h1=["Stop Defense"])
    mon(a, OX)
    t = mon(b, "Labyrinth Wall", pos="def")
    d.ctrl_step(a, b)
    assert t.pos == "atk"
    d, a, b = duel(h1=["Gravedigger Ghoul"])
    b.gy += [SKULL, "Blue-Eyes White Dragon", "Dark Assailant"]
    d.ctrl_step(a, b)
    assert b.gy == ["Dark Assailant"], b.gy

@test("Kuriboh stops battle damage")
def _():
    d, a, b = duel(h2=["Kuriboh"])
    mon(a, SKULL)
    d.battle(a)
    assert b.lp == 8000 and "Kuriboh" in b.gy

@test("Stat effects: Muka Muka, Maha Vailo, Shadow Ghoul, Battleguards, attribute boosters, Flash Assailant")
def _():
    d, a, b = duel(h1=[OX, OX])
    mm = mon(a, "Muka Muka")
    assert mm.atk == 600 + 600
    sg = mon(a, "Shadow Ghoul"); a.gy += [OX, OX, OX]
    assert sg.atk == 1900
    lv, sw = mon(a, "Lava Battleguard"), mon(a, "Swamp Battleguard")
    assert lv.atk == 2050 and sw.atk == 2300
    fa = mon(a, "Flash Assailant")
    assert fa.atk == 1200
    d2, a2, b2 = duel()
    w = mon(a2, "Dark Assailant"); l = mon(b2, "Shining Angel"); mon(a2, "Witch's Apprentice")
    assert w.atk == 1700 and l.atk == 1000

@test("Insect Soldiers +1000 vs WIND")
def _():
    d, a, b = duel()
    m = mon(a, "Insect Soldiers of the Sky")
    t = mon(b, "Harpie Lady")
    assert d.calc_atk(m, t) == 2000

@test("Castle of Dark Illusions grows Zombies")
def _():
    d, a, b = duel()
    c = mon(a, "Castle of Dark Illusions", up=False, pos="def")
    z = mon(a, "Armored Zombie")
    d.flip(a, c)
    assert z.atk == 1700
    d.turn = 8; S.CURRENT_TURN[0] = 8
    d.standby(a)
    assert z.atk == 1900, z.atk

@test("Sogen, Luminous Spark, Vile Germs, Final Flame, Germ Infection")
def _():
    d, a, b = duel(h1=["Final Flame", "Germ Infection"])
    t = mon(b, SKULL)
    d.ctrl_post(a, b)
    assert b.lp == 7400
    d.turn = 7; S.CURRENT_TURN[0] = 7
    d.standby(b)
    assert t.atk == 2200, t.atk
    assert V9.V3.FIELDS["Sogen"][0][0]("Celtic Guardian") and V9.V3.FIELDS["Luminous Spark"][0][0]("Shining Angel")

@test("Discard plan: Graceful Charity discards Red-Eyes, Monster Reborn brings it back")
def _():
    d, a, b = duel(h1=["Graceful Charity", "Red-Eyes Black Dragon", "Monster Reborn", OX])
    a.deck = ["Dark Assailant"] * 3 + a.deck
    d.ctrl_step(a, b)
    assert "Red-Eyes Black Dragon" in a.gy, (a.hand, a.gy)
    d.ctrl_summon(a, b)
    assert any(m.name == "Red-Eyes Black Dragon" for m in a.mons), a.mons

@test("Discard plan off without a revive card")
def _():
    d, a, b = duel(h1=["Tribute to the Doomed", "Red-Eyes Black Dragon", "Dark Assailant"])
    mon(b, SKULL)
    d.ctrl_step(a, b)
    assert "Red-Eyes Black Dragon" in a.hand, a.gy

@test("Queen's Double attacks directly")
def _():
    d, a, b = duel()
    mon(a, "Queen's Double")
    mon(b, SKULL)
    d.battle(a)
    assert b.lp == 8000 - 350


if __name__ == "__main__":
    for n, r, msg in results:
        print(f"{r:4} {n}" + (f"  -> {msg}" if msg else ""))
    print(sum(r == "ok" for _, r, _ in results), "/", len(results), "passed")
