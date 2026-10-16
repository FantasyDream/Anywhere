#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || exit 0
host_test_directory="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-host-test.XXXXXX")"
trap 'rm -rf "$host_test_directory"' EXIT
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/RuntimePolicy.swift apps/flutter/macos/Runner/SettingsStore.swift \
  apps/flutter/macos/Runner/HotKeyMap.swift \
  apps/flutter/macos/Runner/HotKeyService.swift \
  apps/flutter/macos/Runner/ShortcutRecorder.swift \
  apps/flutter/macos/Runner/tests/HostServicesTests.swift \
  -o "$host_test_directory/host-test"
"$host_test_directory/host-test"
# Search scope authorization is separate from query-time filesystem traversal.
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/FileSearchAccess.swift \
  apps/flutter/macos/Runner/RuntimePolicy.swift apps/flutter/macos/Runner/SettingsStore.swift \
  apps/flutter/macos/Runner/HotKeyMap.swift \
  apps/flutter/macos/Runner/tests/FileSearchAccessTests.swift \
  -o "$host_test_directory/file-access-test"
"$host_test_directory/file-access-test"
# Native material ownership and resizing must survive the shared-engine window move.
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/WindowMaterial.swift \
  apps/flutter/macos/Runner/tests/WindowMaterialTests.swift \
  -o "$host_test_directory/material-test"
"$host_test_directory/material-test"
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/WindowInputFocus.swift \
  apps/flutter/macos/Runner/tests/WindowInputFocusTests.swift \
  -o "$host_test_directory/input-focus-test"
"$host_test_directory/input-focus-test"
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/NativeEditMenu.swift \
  apps/flutter/macos/Runner/tests/NativeEditMenuTests.swift \
  -o "$host_test_directory/edit-menu-test"
"$host_test_directory/edit-menu-test"
