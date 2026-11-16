# Yu-yu-oh

A Yu-Gi-Oh! rules engine, self-improving bot and deck builder for the first
three sets (Legend of Blue Eyes, Metal Raiders, Spell Ruler), written in Rust.

- `docs/handoff.md`: the full specification and milestone plan.
- `CLAUDE.md`: decisions, layout, conventions and open questions.
- `config/rules.toml`: house rules. `data/banlist.json`: copy limits.

```sh
cargo test --workspace
cargo run -p ygo-cli -- check-data
cargo run -p ygo-cli -- check-deck data/decks/<deck>.json --collection data/collection.json
```

Card names, text and art belong to Konami. Card images are not part of this
repository.
