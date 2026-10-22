#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || exit 0
host_test_directory="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-host-test.XXXXXX")"
trap 'rm -rf "$host_test_directory"' EXIT
# Check the real release engine cache without starting a Flutter engine or GPU.
flutter_framework="$(python3 - <<'PY'
from pathlib import Path
import shutil
sdk = Path(shutil.which('flutter')).resolve().parents[1]
print(sdk / 'bin/cache/artifacts/engine/darwin-x64-release/FlutterMacOS.xcframework/macos-arm64_x86_64/FlutterMacOS.framework/FlutterMacOS')
PY
)"
xcrun clang -fobjc-arc -Wall -Wextra -Werror -mmacosx-version-min=12.0 \
  -framework Foundation -framework CoreGraphics \
  apps/flutter/macos/Runner/NativeFlutterSurfaceGuard.m \
  apps/flutter/macos/Runner/tests/NativeFlutterSurfaceGuardTests.m \
  -o "$host_test_directory/flutter-surface-test"
"$host_test_directory/flutter-surface-test" "$flutter_framework"
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/RuntimePolicy.swift apps/flutter/macos/Runner/SettingsStore.swift \
  apps/flutter/macos/Runner/HotKeyMap.swift \
  apps/flutter/macos/Runner/HotKeyService.swift \
  apps/flutter/macos/Runner/ShortcutRecorder.swift \
  apps/flutter/macos/Runner/tests/HostServicesTests.swift \
  -o "$host_test_directory/host-test"
"$host_test_directory/host-test"
# Search scope authorization is separate from query-time filesystem traversal.
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/FileSearchAccess.swift \
  apps/flutter/macos/Runner/RuntimePolicy.swift apps/flutter/macos/Runner/SettingsStore.swift \
  apps/flutter/macos/Runner/HotKeyMap.swift \
  apps/flutter/macos/Runner/tests/FileSearchAccessTests.swift \
  -o "$host_test_directory/file-access-test"
"$host_test_directory/file-access-test"
# Native material ownership and resizing must survive the shared-engine window move.
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/WindowMaterial.swift \
  apps/flutter/macos/Runner/tests/WindowMaterialTests.swift \
  -o "$host_test_directory/material-test"
"$host_test_directory/material-test"
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/WindowInputFocus.swift \
  apps/flutter/macos/Runner/tests/WindowInputFocusTests.swift \
  -o "$host_test_directory/input-focus-test"
"$host_test_directory/input-focus-test"
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/WindowMaterial.swift \
  apps/flutter/macos/Runner/WindowInputFocus.swift \
  apps/flutter/macos/Runner/LauncherWindowCoordinator.swift \
  apps/flutter/macos/Runner/tests/LauncherWindowCoordinatorTests.swift \
  -o "$host_test_directory/launcher-windows-test"
"$host_test_directory/launcher-windows-test"
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/NativeEditMenu.swift \
  apps/flutter/macos/Runner/tests/NativeEditMenuTests.swift \
  -o "$host_test_directory/edit-menu-test"
"$host_test_directory/edit-menu-test"

# Web canvas compatibility stays in the native adapter, with an opaque fallback.
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/NativeWebEditing.swift \
  apps/flutter/macos/Runner/NativeWebAppearance.swift \
  apps/flutter/macos/Runner/tests/NativeWebEditingTests.swift \
  -o "$host_test_directory/web-editing-test"
"$host_test_directory/web-editing-test"

# Shelf placement uses visible work areas, including nonzero/negative displays.
xcrun swiftc apps/flutter/macos/Runner/NativeLocalization.swift apps/flutter/macos/Runner/NativeText.g.swift -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/ToolWindowPlacement.swift \
  apps/flutter/macos/Runner/tests/ToolWindowPlacementTests.swift \
  -o "$host_test_directory/tool-placement-test"
"$host_test_directory/tool-placement-test"
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/ApplicationFocusReturner.swift \
  apps/flutter/macos/Runner/tests/ApplicationFocusReturnerTests.swift \
  -o "$host_test_directory/application-return-test"
"$host_test_directory/application-return-test"
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  apps/flutter/macos/Runner/NativeLocalization.swift \
  apps/flutter/macos/Runner/NativeText.g.swift \
  apps/flutter/macos/Runner/tests/NativeLocalizationTests.swift \
  -o "$host_test_directory/localization-test"
"$host_test_directory/localization-test" "$PWD/resources/native-localizations"
