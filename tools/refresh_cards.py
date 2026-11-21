"""Refresh data/cards.json and images from YGOPRODeck (stdlib only).
Usage: python3 tools/refresh_cards.py [bundle_root]
Keeps the schema of data/cards.json. Does not touch banlist.json (the game's limits are maintained by hand)."""
import json, os, re, sys, time, urllib.parse, urllib.request
ROOT = sys.argv[1] if len(sys.argv) > 1 else "."
API = "https://db.ygoprodeck.com/api/v7/cardinfo.php?cardset="
SETS = {"Legend of Blue Eyes White Dragon": "LOB", "Metal Raiders": "MRD", "Spell Ruler": "SRL",
        "Pharaoh's Servant": "PSV"}
IMG_HOST = "https://images.ygoprodeck.com/"

def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "ygo-rust-ai/0.1"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()

cards = {}
for set_name, code in SETS.items():
    data = json.loads(get(API + urllib.parse.quote(set_name)))["data"]
    for c in data:
        img = c["card_images"][0]
        e = cards.setdefault(c["id"], {
            "id": c["id"], "name": c["name"], "type": c["type"], "frame_type": c.get("frameType"),
            "text": c.get("desc", ""), "level": c.get("level"), "atk": c.get("atk"), "def": c.get("def"),
            "race": c.get("race"), "attribute": c.get("attribute"), "printings": [],
            "images": {"full": f"images/full/{c['id']}.jpg", "small": f"images/small/{c['id']}.jpg",
                       "url_full": img["image_url"], "url_small": img["image_url_small"]}})
        for s in c.get("card_sets", []):
            if s["set_name"] == set_name and re.fullmatch(code + r"-(EN)?\d{3}", s["set_code"]):
                num = int(s["set_code"][-3:])
                if not any(p["set"] == code and p["number"] == num for p in e["printings"]):
                    e["printings"].append({"set": code, "number": num, "code": s["set_code"], "rarity": s.get("set_rarity")})
    time.sleep(0.2)  # be polite to the API (documented limit: 20 requests/second)
for e in cards.values():
    e["in_game_index"] = min(e["printings"], key=lambda p: (list(SETS.values()).index(p["set"]), p["number"]))["set"]
out = sorted(cards.values(), key=lambda e: min((list(SETS.values()).index(p["set"]), p["number"]) for p in e["printings"]))
# EXTRA: cards the game lists outside the set tabs (names in data/extra_cards.txt), fetched by name
extra_path = f"{ROOT}/data/extra_cards.txt"
if os.path.exists(extra_path):
    names = [l.strip() for l in open(extra_path) if l.strip()]
    data = json.loads(get("https://db.ygoprodeck.com/api/v7/cardinfo.php?name=" + urllib.parse.quote("|".join(names))))["data"]
    for c in data:
        img = c["card_images"][0]
        pr = []
        for s in c.get("card_sets", []):
            m = re.fullmatch(r"([A-Z0-9]+)-(?:EN|E)?(\d{3})", s["set_code"])
            if m and not any(p["code"] == s["set_code"] for p in pr):
                pr.append({"set": m.group(1), "number": int(m.group(2)), "code": s["set_code"], "rarity": s.get("set_rarity"), "set_name": s["set_name"]})
        out.append({"id": c["id"], "name": c["name"], "type": c["type"], "frame_type": c.get("frameType"), "text": c.get("desc", ""),
                    "level": c.get("level"), "atk": c.get("atk"), "def": c.get("def"), "race": c.get("race"), "attribute": c.get("attribute"),
                    "printings": pr, "in_game_index": "EXTRA",
                    "images": {"full": f"images/full/{c['id']}.jpg", "small": f"images/small/{c['id']}.jpg",
                               "url_full": img["image_url"], "url_small": img["image_url_small"]}})
os.makedirs(f"{ROOT}/data", exist_ok=True)
json.dump({"schema_version": 2, "source": "YGOPRODeck API v7, fetched " + time.strftime("%Y-%m-%d"),
           "sets": list(SETS.values()) + ["EXTRA"], "count": len(out), "cards": out},
          open(f"{ROOT}/data/cards.json", "w"), indent=1, ensure_ascii=False)
for kind in ("full", "small"):
    os.makedirs(f"{ROOT}/images/{kind}", exist_ok=True)
for e in out:
    for kind in ("full", "small"):
        path, url = f"{ROOT}/{e['images'][kind]}", e["images"]["url_" + kind]
        if not url.startswith(IMG_HOST):
            raise SystemExit(f"unexpected image host: {url}")
        if not os.path.exists(path):
            open(path, "wb").write(get(url))
            time.sleep(0.1)
print(len(out), "cards written")
