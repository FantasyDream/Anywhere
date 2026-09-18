#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
python3 scripts/check-flutter.py
cd apps/flutter
flutter pub get
flutter_rust_bridge_codegen generate
