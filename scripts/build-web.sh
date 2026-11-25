#!/usr/bin/env bash
# Builds the browser game into web/play/dist: the page, the engine compiled
# to WebAssembly, and the data files it loads. Serve that folder over HTTP
# (python3 -m http.server -d web/play/dist) or publish it as one artifact.
set -euo pipefail
cd "$(dirname "$0")/.."

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

du -sh "$out"
