#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -x 'build/Context Launcher.app/Contents/MacOS/ContextLauncher' ]]; then
  scripts/build-macos.sh
fi
# The native host saves Rust and window timing logs to Application Support/ContextLauncher.
exec 'build/Context Launcher.app/Contents/MacOS/ContextLauncher'
