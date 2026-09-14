#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
cargo fmt --all --check
cargo test --workspace --locked
python3 scripts/check-architecture.py
(
  cd apps/flutter
  flutter analyze
  flutter test
)
