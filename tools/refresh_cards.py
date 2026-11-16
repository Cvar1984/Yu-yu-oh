"""Refresh data/cards.json and images from YGOPRODeck (stdlib only).
Usage: python3 tools/refresh_cards.py [bundle_root]
Keeps the schema of data/cards.json. Does not touch banlist.json (the game's limits are maintained by hand)."""
import json, os, re, sys, time, urllib.parse, urllib.request
ROOT = sys.argv[1] if len(sys.argv) > 1 else "."
API = "https://db.ygoprodeck.com/api/v7/cardinfo.php?cardset="
SETS = {"Legend of Blue Eyes White Dragon": "LOB", "Metal Raiders": "MRD", "Spell Ruler": "SRL"}
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
out = sorted(cards.values(), key=lambda e: min((list(SETS.values()).index(p["set"]), p["number"]) for p in e["printings"]))
os.makedirs(f"{ROOT}/data", exist_ok=True)
json.dump({"schema_version": 1, "source": "YGOPRODeck API v7, fetched " + time.strftime("%Y-%m-%d"),
           "sets": list(SETS.values()), "count": len(out), "cards": out},
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
