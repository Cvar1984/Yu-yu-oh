# M6 plan: neural policy and value, self-play training

Gate (handoff): **each promoted checkpoint beats the previous one.** A
candidate is promoted only if it beats the current best over enough games
for a 95% interval that excludes 50% (handoff, Testing: bot evaluation).

Status (2026-11-24): steps 1 to 4 done; step 5 started: the
network-guided search bot beats every reference bot with the owner's
deck (below). Owner (2026-11-24): burn is the library; the plan is
approved.

## Step 1: the library trial (done)

Owner (2026-11-16): training hardware is likely NVIDIA; pick the crate
with a trial. Owner (2026-11-24): run it in this environment. This cloud
container has 4 CPU cores, 15 GB of memory and no GPU, so the trial ran on
CPU; the CUDA half is ready to rerun on the owner's machine
(`--features cuda`, `tools/nn_spike/README.md`).

The same model in each library (`tools/nn_spike/`), shaped like M6's
network: up to 80 visible cards as feature vectors through a per-card net
and averaged, game-level numbers, a state MLP, one logit per legal action
scored from its own features (8 to 32 actions, the rest masked), and a
value head. It learns a synthetic task with a known answer, 300 Adam steps
on batches of 256, release build:

| | candle 0.11.1 (CPU) | burn 0.22.1 (`flex` CPU backend) |
| --- | --- | --- |
| Right action after training | 98.1% (from 12.5%) | 98.4% (from 7.6%) |
| Value loss after training | 0.067 (from 1.12) | 0.052 (from 1.01) |
| Training, samples/s (3 runs) | 984, 993, 1,032 | 3,199, 2,691, 2,857 |
| One decision, batch 1 (3 runs) | 501, 551, 564 µs | 73, 75, 76 µs |
| Cores used | about 1 | about 1 |
| Builds for the browser (wasm32) | yes, with `getrandom`'s `wasm_js` flag | yes, as is |
| Clean release build | 59 s, 159 crates | 89 s, 173 crates |
| CUDA | `--features cuda` (needs the CUDA toolkit) | `--features cuda` (CubeCL, needs the CUDA toolkit) |
| Training API | autograd and AdamW; the loop by hand | autograd, optimizers, `Module` derive, records |

tch-rs (libtorch) was not tried: its download host is blocked here, it
needs a 2 GB C++ library, it cannot build for the browser, and burn 0.22
itself deprecated its libtorch backend.

**Recommendation: burn.** It learns as well as candle and was 3 times
faster to train and 7 times faster per decision on this CPU, which matters
twice: self-play runs one game per core, and a bot decides many times a
game. It builds for the browser without extra flags, and its CUDA backend
is a feature flag. Its cost is a heavier build (30 s more from clean).
Its API moved in 0.22 (no backend type parameter; the old CPU backend is
deprecated for `flex`), so we pin the version. The CUDA speed has to be
measured on the owner's machine before long training runs.

One model lesson from the trial: summing the card vectors saturated the
value head's tanh (its loss stayed at 2.0); averaging them fixed it. M6
averages, or normalises, every pooled set.

## Steps 2 to 7

2. **Encoding** (`ygo-ai`, no new dependency). From a `PlayerView`
   only, so no hidden card ever reaches the network:
   - each visible card: kind, Spell/Trap subtype, Level, ATK and DEF
     (current and printed), attribute, Type, owner, controller, place,
     position, and its script's features: effect kinds and speeds, tags,
     hooks, once-per-turn; face-down cards the viewer cannot see get a
     "hidden" vector. No slot per card id, so a ban list change or a
     new script does not break a trained model (handoff);
   - the game: Life Points, turn, phase, who moves, counts per zone, the
     chain, the open window, the decision kind;
   - each legal action: its kind plus the features of the cards it
     names (the card, targets, costs, tributes, attack target).
   Tests: two games that look the same to the decider encode the same
   (`PlayerView::sample`, as `tests/view.rs` does); every action of
   random games encodes.
   **Done (2026-11-24):** `crates/ygo-ai/src/encode.rs`, `encode(&PlayerView)`:
   132 numbers per card (`CARD_F`), 66 for the game (`GLOBAL_F`) and 429
   per legal action (`ACTION_F`: its kind and details, then the card
   vectors of its subject, the mean of its targets and the mean of its
   costs or Tributes). Types, attributes and tags keep reserved slots
   (`Tag::ALL`, whose order a test pins), so a slot never changes meaning.
   The engine gained `Tag::ALL` and `PlayerView::shared` (public data).
   Tests (`crates/ygo-ai/tests/encode.rs`): 5,894 decisions of 30 random
   whole-pool games encode with the right sizes and finite numbers (up to
   115 cards and 52 legal actions), and at every decision of 15 more
   games a sample that looks the same to the decider encodes the same.
   Speed: 5.7 µs per decision (release, one core, 10,134 decisions of 50
   random games), next to about 75 µs for the trial network.
   **Additions (owner, 2026-11-24), after reviewing what the bot sees:**
   - its own Deck: the cards left, never their order
     (`PlayerView::own_deck`), as card vectors with the place "Deck";
   - memory (`ygo-engine/src/memory.rs`): a card it saw that is hidden
     again (bounced to the opponent's hand, turned face-down, shown by a
     reveal or a private look) stays known while it stays there; it is
     forgotten when the card moves, enters a Deck or changes identity
     (Magical Hats). The sampler keeps remembered cards, so search and
     lookahead bots know them too. Cost: 4.0% more instructions in random
     games, 3.0% for the Beatdown bot (callgrind; a first version cost
     57%, the bit-set one replaced it);
   - each lingering effect as a 305-number vector (`LINGER_F`): its kind
     (32 reserved slots), the player and card it names, its ATK/DEF change,
     turns left, and its source card;
   - current Types (`Observation::types`: DNA Surgery, D. Tribe);
   - the house rules: 25 numbers in the game vector.
   Card vectors are now 133 numbers (a "Deck" place), the game vector 91.
   Tests: sizes over 5,894 decisions (up to 122 cards, 52 actions and 5
   lingering effects), samples encode the same (lingering included), a
   D. Tribe Type change reaches the vector (and the test fails without
   it), memory tests in `ygo-cards/tests/memory.rs`. Encoding takes 13.4 µs
   a decision (release, Removal against Big Beatdown).
   Found on the way: the sampler redealt a card face-up in a Deck
   (Parasite Paracide); it is public now. Still left out on purpose: the
   meaning of a question's options (only their order).
3. **Network and neural bot** (`ygo-ai`, burn behind a cargo feature
   `neural`, so the browser build and the other crates stay as they are
   until they want it). `NeuralPlayer` implements `Player`: the policy
   picks among the legal actions, the value scores positions.
   Checkpoints are files in `checkpoints/` (git-ignored).
   **Done (2026-11-24):** `crates/ygo-ai/src/neural.rs` behind the
   `neural` feature (burn pinned at `=0.22.1`, CPU `flex` backend; the
   CLI turns the feature on through `ygo-play`, the browser build does
   not). `Net`: a per-card net (133→128→128) averaged over the cards in
   play and, apart, over the decider's own Deck; a lingering-effect net
   (305→64→64) averaged; state MLP (411→256→256); each action scored
   from its features plus the state (429→128, one logit); value head
   (256→1, tanh). `Inputs` pads a batch with masks. `NeuralPlayer`
   implements `Player` (most likely action, or sampled at a
   temperature); `evaluate` gives the logits and the value. Checkpoints:
   `Net::save` writes next to the file and renames it into place,
   `Net::load` reads it whole into memory (burn reads lazily otherwise,
   which a test caught), 1.26 MB; `checkpoints/` is git-ignored. Bot
   name `neural:<checkpoint>` in `ygo play` and `ygo sim`.
   Tests (`crates/ygo-ai/tests/neural.rs`, `crates/ygo-cli/tests/neural.rs`):
   an untrained bot plays whole games with legal actions; a padded batch
   gives what each decision alone gives (to 1e-4); checkpoints round-trip
   exactly and a missing one is a clear error; 30 Adam steps halve the
   loss on a fixed batch; `ygo sim` plays a saved network by name.
   Untrained (random weights, greedy) against the random bot: 26.0% ±6.1
   (200 games, Removal against Big Beatdown); 27 games/s with one neural
   side, about 0.3 ms a neural decision.
4. **Imitation start** (`ygo-train`). Record games of the lookahead and
   search bots on the six enemy decks; train the policy on their choices
   and the value on the results. Check: the imitating bot against the
   plain heuristic bot with `ygo sim`.
   **Done (2026-11-24, owner: "both" teachers):** `ygo-train`'s
   `imitation` module and `ygo imitate <record dirs> --out <checkpoint>`.
   Records stay small (the actions); each epoch replays them and encodes
   every decision of a teacher (a player whose label is not `random`, or
   `--teachers` prefixes) that had a choice, with the action it took and
   the result for it, mixing them in a pool of 2,048 before batches of
   128 (Adam, 1e-3). One record in ten is held out; epoch 0 is the
   untrained baseline.
   Data (`tools/imitation_data.sh`): 4,800 lookahead games (Beatdown
   against Control style; the six enemy mirrors, 500 each, and six cross
   pairs, 300 each) and 1,200 search games (search against
   search-with-Control-playouts, the six mirrors, 200 each), 66 MB.
   Training (4 epochs, 444,338 samples each, about 400 s an epoch on one
   core): held-out agreement with the teacher 32.9% untrained, then
   69.6%, 71.3%, 71.5%, 72.0%; value error 1.021, then 0.817 to 0.838.
   Strength (`tools/neural_eval.sh`, `checkpoints/imitation-1.bpk`, each
   enemy mirror, 500 games, seed 90000):

   | Deck | Network vs plain Beatdown | Network vs lookahead Beatdown | Lookahead vs plain (same seeds) |
   | --- | --- | --- | --- |
   | E1 Toon | 46.2% ±4.4 | 37.6% ±4.2 | 61.2% ±4.3 |
   | E2 Special Summon | 44.4% ±4.4 | 42.2% ±4.3 | 51.4% ±4.4 |
   | E3 Removal | 71.0% ±4.0 | 46.2% ±4.4 | 72.0% ±3.9 |
   | E4 Big Beatdown | 61.4% ±4.3 | 47.2% ±4.4 | 61.8% ±4.3 |
   | E5 DARK + Plasma Zone | 56.8% ±4.3 | 47.4% ±4.4 | 56.6% ±4.3 |
   | E6 Gate Guardian | 49.6% ±4.4 | 42.4% ±4.3 | 57.6% ±4.3 |
   | Pooled (3,000 games) | 54.9% ±1.8 | 43.8% ±1.8 | 60.1% ±1.8 |

   The network, one forward pass a decision and no search, beats the
   plain heuristic bot (its interval excludes 50%) and matches the
   lookahead bot's result on the Removal, Big Beatdown and DARK decks; it
   trails on Toon, Special Summon and Gate Guardian, the decks with summon
   procedures and combos. Agreement stops rising after the second epoch,
   so more of the same data adds little; self-play (step 5) is the next
   source of strength.
5. **Self-play** (`ygo train --config <file>`). The search bot guided by
   the network (policy as priors, value at the leaves, AlphaZero-style)
   plays itself; games go to a replay buffer; the network trains on the
   search's visit counts and the results; a candidate checkpoint is saved
   every N games.
   **Started (2026-11-24), owner's goal: "train the network bot against
   everyone until it's better than every bot with the same deck"**, the
   owner's screenshot deck (40 cards, save A; not yet in `data/decks`,
   whether to save it as `save_a_owner_current_40` is the owner's call)
   on both sides.
   - Network for this deck, `owner-1`: `imitation-1` trained 3 more
     epochs on 16,000 lookahead mirrors and 2,000 search games with the
     deck (held-out agreement 88.7%, value error 0.693). Alone (greedy) it
     beats random, the six plain styles (64.8% to 74.8%) and four
     lookahead styles, but not lookahead Beatdown (45.0%), lookahead Burn
     (48.5%) or search (47.0%, 200 games). Its value head is too weak to
     search with (`neural-lookahead:` lost to both).
   - **`neural-search:<checkpoint>`**: the search bot with the policy as
     its prior. `SearchPlayer::with_prior` (a `Prior` trait): the moves of
     the decision being made are picked by PUCT (average result plus
     `prior_weight` × prior × sqrt(visits) / (1 + the move's visits), a
     move not yet tried starting from the average so far); deeper moves
     keep UCB1, playouts stay heuristic. One forward pass a decision.
     `prior_weight` against the plain search bot (400 games, seed 8800):
     0.1 59.5%, 0.25 60.2%, 0.5 58.0%, 1 55.0%, 2 50.5%; default 0.25.
     Control, the same PUCT with a flat prior: 49.0% (0.25) and 47.0% (1),
     so the gain is the network's.
   - Ladder (`tools/neural_ladder.sh`, seed 7700, 400 games each, the
     network-guided search with `owner-1` as player A):

     | Opponent | Result |
     | --- | --- |
     | random | 99.5% ±0.7 |
     | heuristic Beatdown, Control, Stall | 71.5% ±4.4, 75.5% ±4.2, 77.5% ±4.1 |
     | heuristic Burn, Exodia, Patient | 73.5% ±4.3, 81.2% ±3.8, 78.5% ±4.0 |
     | lookahead Beatdown, Control, Stall | 63.5% ±4.7, 62.3% ±4.8, 64.2% ±4.7 |
     | lookahead Burn, Exodia, Patient | 64.5% ±4.7, 68.8% ±4.5, 65.8% ±4.7 |
     | search | 53.0% ±4.9; then 2,000 games (seed 400000): **57.8% ±2.2** |

     Every interval excludes 50%. Against search, all 2,800 games pooled:
     57.5%. Speed: 4 to 13 games/s on 4 cores (the plain search bot: 10).
   - Expert iteration, round 1: those 2,000 games plus 2,000 mirrors of the
     network-guided search, `owner-1` trained on its choices (3 epochs,
     learning rate 3e-4, 171,944 samples): agreement 81.8% to 82.3%;
     against search on the same seeds 56.9% ±2.2, no better than
     `owner-1`, so not promoted. Training on the move played gives little
     here; the next round should train on the search's visit counts.
6. **Promotion and the gate.** Each candidate plays the current best
   (2,000 games, each player first in half); promoted only if the 95%
   interval excludes 50%. A ladder report beside it: random, heuristic,
   lookahead and search bots, and the six enemy decks.
7. **Docs and gate report.** CLAUDE.md, this plan's progress, the numbers.

Where it runs: the pipeline is built and checked here at small scale on
CPU. Long training runs belong on the owner's NVIDIA machine.
