#!/usr/bin/env bash
# The CI checks, locally. Exits non-zero on the first failure.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets --quiet
cargo test --workspace --quiet
echo "check: ok"
