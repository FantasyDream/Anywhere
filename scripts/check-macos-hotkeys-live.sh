#!/bin/bash
# Opt-in: briefly registers Command+Option+Control+Shift Q/W/T/X/Y via real Carbon.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || exit 0
source scripts/env.sh
hotkey_test_directory="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-hotkeys-live.XXXXXX")"
trap 'rm -rf "$hotkey_test_directory"' EXIT
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/RuntimePolicy.swift apps/flutter/macos/Runner/SettingsStore.swift \
  apps/flutter/macos/Runner/HotKeyMap.swift \
  apps/flutter/macos/Runner/HotKeyService.swift \
  apps/flutter/macos/Runner/tests/HotKeyIntegrationTests.swift \
  -o "$hotkey_test_directory/hotkey-test"
"$hotkey_test_directory/hotkey-test"
