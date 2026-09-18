#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -x 'build/Anywhere.app/Contents/MacOS/Anywhere' ]]; then
  scripts/build-macos.sh
fi
# Launch through the bundle so Accessibility TCC matches the app, not the inner binary.
open -n 'build/Anywhere.app'
