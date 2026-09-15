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
# Keyed by repo path: clang modules embed absolute paths, so a cache made under a
# different checkout path is unusable. Renaming the repository just builds a fresh one.
SWIFT_MODULE_CACHE="${SWIFT_MODULE_CACHE_PATH:-$(dirname "$ROOT/.tools")/$(basename "$ROOT")-swift-cache}"
mkdir -p "$SWIFT_MODULE_CACHE"
cargo build --locked --release -p anywhere-bridge-flutter
(
  cd apps/flutter
  flutter pub get
  flutter assemble --output="$ROOT/build/flutter" \
    -dTargetPlatform=darwin -dDarwinArchs="$ARCH" -dBuildMode=release \
    -dTargetFile=lib/main.dart release_macos_bundle_flutter_assets
)
APP="$ROOT/build/Anywhere.app"
# Rebuild from scratch: the bundle is assembled by copying, so a file removed from the
# sources would otherwise linger and keep shipping (a deleted plugin did exactly that).
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Frameworks" "$APP/Contents/Resources"
# No bundled plugins: the app must not install a web search the user did not ask for.
cp apps/flutter/macos/Runner/Info.plist "$APP/Contents/Info.plist"
cp apps/flutter/macos/Runner/Assets/Anywhere.icns "$APP/Contents/Resources/Anywhere.icns"
# ditto preserves framework symlinks required by dyld and code signing.
ditto build/flutter/FlutterMacOS.framework "$APP/Contents/Frameworks/FlutterMacOS.framework"
ditto build/flutter/App.framework "$APP/Contents/Frameworks/App.framework"
cp target/release/libanywhere_bridge_flutter.dylib "$APP/Contents/Frameworks/"
xcrun swiftc -O -target "$ARCH-apple-macos12.0" \
  -module-cache-path "$SWIFT_MODULE_CACHE" \
  -F "$APP/Contents/Frameworks" -framework FlutterMacOS -framework App -framework AppKit -framework Carbon \
  -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
  apps/flutter/macos/Runner/Main.swift -o "$APP/Contents/MacOS/Anywhere"
for framework in FlutterMacOS App; do
  codesign --force --sign - "$APP/Contents/Frameworks/$framework.framework"
done
# The cargo-built dylib records its build path as its install name, which would make
# the bundle depend on `target/release/deps` still existing, and break as soon as the
# app is moved. Give it the same relative form Flutter's own frameworks use, then
# re-sign because editing the load command invalidates the signature.
install_name_tool -id "@rpath/libanywhere_bridge_flutter.dylib" \
  "$APP/Contents/Frameworks/libanywhere_bridge_flutter.dylib"
codesign --force --sign - "$APP/Contents/Frameworks/libanywhere_bridge_flutter.dylib"
codesign --force --sign - --entitlements apps/flutter/macos/Runner/Release.entitlements "$APP"
codesign --verify --deep --strict "$APP"
# The Dart entry point loads this by name; a rename that misses one of the two sides
# otherwise fails only at runtime, with an unhelpful dlopen error.
# Re-register with LaunchServices. macOS caches an app's icon and metadata by bundle
# path and identifier, so a freshly built or renamed bundle otherwise shows a stale
# icon (or the "no entry" placeholder) until the cache happens to expire.
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$LSREGISTER" ]]; then
  "$LSREGISTER" -f "$APP"
fi

expected_library="$(sed -n "s/.*Frameworks\/\(lib[a-z_]*\.dylib\)'.*/\1/p" apps/flutter/lib/main.dart)"
if [[ -z "$expected_library" || ! -f "$APP/Contents/Frameworks/$expected_library" ]]; then
  echo "packaging error: the Dart host loads '${expected_library:-unknown}', which is not in the bundle" >&2
  ls "$APP/Contents/Frameworks" >&2
  exit 1
fi
printf 'Built %s (host library %s)\n' "$APP" "$expected_library"
