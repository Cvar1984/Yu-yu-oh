#!/usr/bin/env bash
# Builds the browser game into web/play/dist: the page, the engine compiled
# to WebAssembly, and the data files it loads. Serve that folder over HTTP
# (python3 -m http.server -d web/play/dist) or publish it as one artifact.
#
# Card images are Konami-owned and never committed. The page loads them from
# YGOPRODeck while you play (owner, 2026-11-27), or from local copies: when
# the git-ignored images/ folder exists (tools/refresh_cards.py fetches it),
# its images are copied in and used first; --images <dir> takes them from
# another folder. --no-images builds a page with no card art at all (use it
# for anything you publish).
set -euo pipefail
cd "$(dirname "$0")/.."
images=images
while [ $# -gt 0 ]; do
  case "$1" in
    --no-images) images=""; shift ;;
    --images) images="$2"; shift 2 ;;
    *) echo "usage: $0 [--no-images | --images <dir>]" >&2; exit 2 ;;
  esac
done

out=web/play/dist
cargo build -p ygo-wasm --target wasm32-unknown-unknown --profile web

rm -rf "$out"
mkdir -p "$out/data/decks"
cp web/play/index.html "$out/index.html"
cp target/wasm32-unknown-unknown/web/ygo_wasm.wasm "$out/ygo.wasm"
cp config/rules.toml "$out/data/"
cp data/banlist.json data/tokens.json "$out/data/"
# The trained network the bots play with.
cp data/networks/best.bpk "$out/data/network.bpk"
# The engine needs no images or print records; the page ships neither, so it
# carries no links to card art. One card per line.
python3 -I - data/cards.json "$out/data/cards.json" <<'PY'
import json, sys
src, dst = sys.argv[1], sys.argv[2]
d = json.load(open(src, encoding="utf-8"))
blank = {"full": "", "small": "", "url_full": "", "url_small": ""}
cards = [dict(c, printings=[], images=blank) for c in d["cards"]]
head = {k: v for k, v in d.items() if k != "cards"}
with open(dst, "w", encoding="utf-8") as f:
    f.write(json.dumps(head, ensure_ascii=False)[:-1] + ', "cards": [\n')
    f.write(",\n".join(json.dumps(c, ensure_ascii=False) for c in cards))
    f.write("\n]}\n")
PY

decks=()
for f in data/decks/*.json; do
  cp "$f" "$out/data/decks/"
  decks+=("\"$(basename "$f")\"")
done
(IFS=,; echo "[${decks[*]}]") > "$out/data/decks.json"

# Card images: the page loads each card's image from YGOPRODeck (the URLs
# in data/cards.json), or from local copies when there are some; the index
# tells it which. --no-images writes an empty index: an art-free page.
mkdir -p "$out/images"
if [ -z "$images" ]; then
  echo '{"small":[],"full":[],"online":{}}' > "$out/images/index.json"
else
  python3 -I - "$images" "$out/images" data/cards.json <<'PY'
import json, shutil, sys
from pathlib import Path
src, dst, cards = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
index = {}
for size in ("small", "full"):
    ids = sorted(int(p.stem) for p in (src / size).glob("*.jpg") if p.stem.isdigit()) if (src / size).is_dir() else []
    if ids:
        (dst / size).mkdir(parents=True, exist_ok=True)
        for i in ids:
            shutil.copyfile(src / size / f"{i}.jpg", dst / size / f"{i}.jpg")
    index[size] = ids
index["online"] = {c["id"]: [c["images"]["url_small"], c["images"]["url_full"]]
                   for c in json.loads(cards.read_text(encoding="utf-8"))["cards"]
                   if c["images"].get("url_small")}
(dst / "index.json").write_text(json.dumps(index))
print(f"card images: {len(index['small'])} small and {len(index['full'])} full local copies, {len(index['online'])} online")
PY
fi

du -sh "$out"
