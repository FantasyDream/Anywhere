#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -x 'build/Anywhere.app/Contents/MacOS/Anywhere' ]]; then
  scripts/build-macos.sh
fi
# The native host saves Rust and window timing logs to Application Support/Anywhere.
exec 'build/Anywhere.app/Contents/MacOS/Anywhere'
