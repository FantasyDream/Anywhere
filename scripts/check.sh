#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
python3 scripts/check-flutter.py
scripts/check-macos-menu.sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 scripts/check-architecture.py
(
  cd apps/flutter
  flutter analyze
  flutter test
)
