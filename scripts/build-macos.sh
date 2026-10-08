#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
source scripts/env.sh
python3 scripts/check-flutter.py
python3 scripts/generate-keymap.py --check
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'This MVP builds on macOS only.' >&2
  exit 1
fi
ARCH="$(uname -m)"
SIGN_IDENTITY="${ANYWHERE_CODESIGN_IDENTITY:-Anywhere Local Code Signing}"
if [[ "$SIGN_IDENTITY" == "-" ]]; then
  echo 'Ad hoc signing changes the app identity on every rebuild and loses Accessibility permission.' >&2
  echo 'Run scripts/setup-local-signing.sh or set ANYWHERE_CODESIGN_IDENTITY to a persistent signing identity.' >&2
  exit 1
fi
if ! security find-identity -p codesigning | grep -Fq "$SIGN_IDENTITY"; then
  echo "Code signing identity '$SIGN_IDENTITY' was not found." >&2
  echo 'Run scripts/setup-local-signing.sh once, or set ANYWHERE_CODESIGN_IDENTITY to an existing identity.' >&2
  exit 1
fi
# Keyed by repo path: clang modules embed absolute paths, so a cache made under a
# different checkout path is unusable. Renaming the repository just builds a fresh one.
SWIFT_MODULE_CACHE="${SWIFT_MODULE_CACHE_PATH:-$(dirname "$ROOT/.tools")/$(basename "$ROOT")-swift-cache}"
mkdir -p "$SWIFT_MODULE_CACHE"
export ANYWHERE_MENU_IN_HOST=1
cargo build --locked --release -p anywhere-bridge-flutter
(
  cd apps/flutter
  # check-flutter.py already verifies the pinned SDK. Packaging must not wait
  # for an unrelated Flutter SDK freshness fetch.
  flutter --no-version-check pub get
  flutter --no-version-check assemble --output="$ROOT/build/flutter" \
    -dTargetPlatform=darwin -dDarwinArchs="$ARCH" -dBuildMode=release \
    -dTargetFile=lib/main.dart release_macos_bundle_flutter_assets
)
APP="${ANYWHERE_APP_OUTPUT:-$ROOT/build/Anywhere.app}"
if [[ "$APP" != "$ROOT"/build/*.app ]]; then
  echo 'ANYWHERE_APP_OUTPUT must be an .app path inside build/.' >&2
  exit 1
fi
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
xcrun clang -c -fobjc-arc -O2 -mmacosx-version-min=12.0 \
  -o "$ROOT/build/macos_menu.o" \
  "$ROOT/crates/anywhere-platform/native/macos_menu.m"
xcrun swiftc -O -target "$ARCH-apple-macos12.0" \
  -module-cache-path "$SWIFT_MODULE_CACHE" \
  -F "$APP/Contents/Frameworks" -framework FlutterMacOS -framework App -framework AppKit -framework Carbon -framework ApplicationServices \
  -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
  -Xlinker -export_dynamic \
  -Xlinker -ld_classic \
  apps/flutter/macos/Runner/*.swift "$ROOT/build/macos_menu.o" -o "$APP/Contents/MacOS/Anywhere"
for framework in FlutterMacOS App; do
  codesign --force --sign "$SIGN_IDENTITY" "$APP/Contents/Frameworks/$framework.framework"
done
# The cargo-built dylib records its build path as its install name, which would make
# the bundle depend on `target/release/deps` still existing, and break as soon as the
# app is moved. Give it the same relative form Flutter's own frameworks use, then
# re-sign because editing the load command invalidates the signature.
install_name_tool -id "@rpath/libanywhere_bridge_flutter.dylib" \
  "$APP/Contents/Frameworks/libanywhere_bridge_flutter.dylib"
codesign --force --sign "$SIGN_IDENTITY" "$APP/Contents/Frameworks/libanywhere_bridge_flutter.dylib"
codesign --force --sign "$SIGN_IDENTITY" --entitlements apps/flutter/macos/Runner/Release.entitlements "$APP"
codesign --verify --deep --strict "$APP"
if codesign -dr - "$APP" 2>&1 | grep -q 'designated => cdhash'; then
  echo 'Packaging error: the app still has a version-specific ad hoc identity.' >&2
  exit 1
fi
# HostRuntime supplies the library path to each Flutter engine. Validate the
# native path contract rather than expecting Dart to hard-code the bundle layout.
# Re-register with LaunchServices. macOS caches an app's icon and metadata by bundle
# path and identifier, so a freshly built or renamed bundle otherwise shows a stale
# icon (or the "no entry" placeholder) until the cache happens to expire.
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ "$APP" == "$ROOT/build/Anywhere.app" && -x "$LSREGISTER" ]]; then
  "$LSREGISTER" -f "$APP"
fi

expected_library="$(sed -n 's/.*"Contents\/Frameworks\/\(lib[a-z_]*\.dylib\)".*/\1/p' apps/flutter/macos/Runner/HostRuntime.swift)"
if [[ -z "$expected_library" || ! -f "$APP/Contents/Frameworks/$expected_library" ]]; then
  echo "packaging error: HostRuntime supplies '${expected_library:-unknown}', which is not in the bundle" >&2
  ls "$APP/Contents/Frameworks" >&2
  exit 1
fi
printf 'Built %s (host library %s)\n' "$APP" "$expected_library"
