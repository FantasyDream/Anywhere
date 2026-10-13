#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
python3 scripts/check-flutter.py
python3 scripts/generate-keymap.py --check
python3 scripts/generate-file-types.py --check
python3 scripts/generate-runtime-policies.py --check
scripts/check-macos-menu.sh
scripts/check-macos-host.sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 scripts/check-architecture.py
(
  cd apps/flutter
  # The SDK was checked above; keep this gate independent of SDK freshness fetches.
  flutter --no-version-check analyze
  flutter --no-version-check test
)
