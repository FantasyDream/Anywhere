#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || exit 0
host_test_directory="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-host-test.XXXXXX")"
trap 'rm -rf "$host_test_directory"' EXIT
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/SettingsStore.swift \
  apps/flutter/macos/Runner/HotKeyMap.swift \
  apps/flutter/macos/Runner/HotKeyService.swift \
  apps/flutter/macos/Runner/tests/HostServicesTests.swift \
  -o "$host_test_directory/host-test"
"$host_test_directory/host-test"
