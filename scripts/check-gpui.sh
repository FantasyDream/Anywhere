#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh

python3 scripts/generate-design-tokens.py --check
python3 scripts/generate-gpui-localizations.py --check
python3 -m unittest discover -s scripts/tests -p test_design_tokens.py
python3 -m unittest discover -s scripts/tests -p test_gpui_localizations.py
plutil -lint apps/gpui/macos/Info.plist apps/gpui/macos/Release.entitlements
cargo fmt --all -- --check
cargo test -p anywhere-gpui --locked
cargo clippy -p anywhere-gpui --all-targets --locked -- -D warnings
