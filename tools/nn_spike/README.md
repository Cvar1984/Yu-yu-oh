# Neural-net library trial (M6 step 1)

The same small model in two libraries, to choose the one M6 trains with.
It has the shape M6's network needs (handoff, "Learned policy and value"):

- **State**: up to 80 visible cards, each a 48-number feature vector, go
  through a per-card net and are summed (cards not present are masked
  out); 32 game-level numbers join them, and an MLP gives a 256-number
  state.
- **Policy**: each legal action has its own 64-number feature vector and
  is scored against the state; 8 to 32 legal actions per decision, the
  rest masked.
- **Value**: one number in -1..1, the expected result.

It learns a synthetic task with a known answer (the right action is the
one with the largest projection on a fixed random vector; the value is the
sign of one on the game-level numbers), so a library that trains wrongly
shows it.

```sh
cd tools/nn_spike/candle && cargo run --release           # CPU
cd tools/nn_spike/burn && cargo run --release             # CPU (flex)
# On an NVIDIA machine with the CUDA toolkit:
cd tools/nn_spike/candle && cargo run --release --features cuda
cd tools/nn_spike/burn && cargo run --release --features cuda
# The model alone, for the browser (candle also needs
# RUSTFLAGS='--cfg getrandom_backend="wasm_js"'):
cargo build --release --lib --target wasm32-unknown-unknown
```

Each prints the loss and accuracy before and after 300 Adam steps on
batches of 256, the training speed in samples/s, and the time of one
decision's forward pass (batch 1, the way a bot plays). The projects
stay outside the workspace (`[workspace]` of their own) so the trial adds
no dependency to it. Results: `docs/m6-plan.md`.
