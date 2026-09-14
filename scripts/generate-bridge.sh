#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
if [[ -x "$PWD/.tools/frb/flutter_rust_bridge_codegen" ]]; then
  export PATH="$PWD/.tools/frb:$PATH"
fi
cd apps/flutter
flutter pub get
flutter_rust_bridge_codegen generate
