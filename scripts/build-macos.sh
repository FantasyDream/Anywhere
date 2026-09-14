#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
source scripts/env.sh
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'This MVP builds on macOS only.' >&2
  exit 1
fi
ARCH="$(uname -m)"
cargo build --locked --release -p launcher-bridge-flutter
(
  cd apps/flutter
  flutter pub get
  flutter assemble --output="$ROOT/build/flutter" \
    -dTargetPlatform=darwin -dDarwinArchs="$ARCH" -dBuildMode=release \
    -dTargetFile=lib/main.dart release_macos_bundle_flutter_assets
)
APP="$ROOT/build/Context Launcher.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Frameworks" "$APP/Contents/Resources/plugins"
cp apps/flutter/macos/Runner/Info.plist "$APP/Contents/Info.plist"
# ditto preserves framework symlinks required by dyld and code signing.
ditto build/flutter/FlutterMacOS.framework "$APP/Contents/Frameworks/FlutterMacOS.framework"
ditto build/flutter/App.framework "$APP/Contents/Frameworks/App.framework"
cp target/release/liblauncher_bridge_flutter.dylib "$APP/Contents/Frameworks/"
cp plugins/examples/*.toml "$APP/Contents/Resources/plugins/"
xcrun swiftc -O -target "$ARCH-apple-macos12.0" \
  -module-cache-path "$ROOT/.tools/swift-cache" \
  -F "$APP/Contents/Frameworks" -framework FlutterMacOS -framework App -framework AppKit -framework Carbon \
  -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
  apps/flutter/macos/Runner/Main.swift -o "$APP/Contents/MacOS/ContextLauncher"
for framework in FlutterMacOS App; do
  codesign --force --sign - "$APP/Contents/Frameworks/$framework.framework"
done
codesign --force --sign - "$APP/Contents/Frameworks/liblauncher_bridge_flutter.dylib"
codesign --force --sign - --entitlements apps/flutter/macos/Runner/Release.entitlements "$APP"
codesign --verify --deep --strict "$APP"
printf 'Built %s\n' "$APP"
