#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
python3 scripts/check-flutter.py
python3 scripts/generate-design-tokens.py --check
python3 -m unittest discover -s scripts/tests -p test_design_tokens.py
python3 scripts/generate-keymap.py --check
python3 scripts/generate-file-types.py --check
python3 scripts/generate-runtime-policies.py --check
cargo run -p anywhere-core --example generate_feature_catalog --locked -- --check
cargo run -p anywhere-protocol --features web-schema --example generate_web_plugin_schema --locked -- --check
cargo run -p anywhere-protocol --features web-schema --example generate_web_sdk_contract --locked -- --check
python3 scripts/check-docs.py
scripts/check-macos-menu.sh
scripts/check-macos-host.sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
scripts/check-web-sdk.sh
scripts/check-web-examples.sh
python3 scripts/check-architecture.py
(
  cd apps/flutter
  # The SDK was checked above; keep this gate independent of SDK freshness fetches.
  flutter --no-version-check analyze
  flutter --no-version-check test
)
