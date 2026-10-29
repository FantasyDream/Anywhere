#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || exit 0
source scripts/env.sh
preview_test_directory="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-preview-test.XXXXXX")"
trap 'rm -rf "$preview_test_directory"' EXIT
preview_test_app="$preview_test_directory/PreviewTests.app"
flutter_framework_directory="$(
  python3 - <<'PY'
from pathlib import Path
import shutil
sdk = Path(shutil.which('flutter')).resolve().parents[1]
print(sdk / 'bin/cache/artifacts/engine/darwin-x64-release/FlutterMacOS.xcframework/macos-arm64_x86_64')
PY
)"
mkdir -p "$preview_test_app/Contents/MacOS"
python3 - "$preview_test_app/Contents/Info.plist" <<'PY'
import plistlib
import sys
with open(sys.argv[1], 'wb') as output:
    plistlib.dump({
        'CFBundleIdentifier': 'dev.anywhere.preview-tests',
        'CFBundleExecutable': 'PreviewTests',
        'CFBundlePackageType': 'APPL',
        'NSHighResolutionCapable': True,
    }, output)
PY
xcrun swiftc -warnings-as-errors -target "$(uname -m)-apple-macos12.0" \
  -F "$flutter_framework_directory" -framework FlutterMacOS -framework QuickLookUI \
  -Xlinker -rpath -Xlinker "$flutter_framework_directory" \
  apps/flutter/macos/Runner/WindowInputFocus.swift \
  apps/flutter/macos/Runner/NativePreviewHost.swift \
  apps/flutter/macos/Runner/tests/NativePreviewHostTests.swift \
  -o "$preview_test_app/Contents/MacOS/PreviewTests"
open -n -W --stdout "$preview_test_directory/output.log" \
  --stderr "$preview_test_directory/errors.log" "$preview_test_app"
cat "$preview_test_directory/output.log" "$preview_test_directory/errors.log"
rg -q '^Native preview live: .* passed$' "$preview_test_directory/output.log"
