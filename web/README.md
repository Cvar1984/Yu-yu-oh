# web

- `play/`: the standalone browser game. The Rust engine, card scripts and
  heuristic bots run in the page as WebAssembly (`crates/ygo-wasm`); the
  page's JavaScript only draws and sends JSON commands, so it never computes
  rules itself. Build with `scripts/build-web.sh`, which writes the page,
  `ygo.wasm` and the data files to `play/dist/` (git-ignored).
- Browser board for milestone M9: talks to `ygo-server` over a WebSocket and
  never computes rules itself. Not started.
