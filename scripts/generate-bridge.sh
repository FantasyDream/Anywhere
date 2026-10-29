#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
python3 scripts/check-flutter.py
bridge_version="$(python3 scripts/check-versions.py --bridge-runtime-version)"
if [[ "$(flutter_rust_bridge_codegen --version)" != "flutter_rust_bridge_codegen $bridge_version" ]]; then
  echo "Install flutter_rust_bridge_codegen $bridge_version or put the matching isolated tool on PATH." >&2
  exit 1
fi
cd apps/flutter
flutter pub get
flutter_rust_bridge_codegen generate
