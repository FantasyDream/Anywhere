#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
source scripts/env.sh

if [[ "$(uname -s)" != Darwin ]]; then
  echo 'Anywhere builds on macOS only.' >&2
  exit 1
fi

APP="$(python3 scripts/macos_app_path.py \
  "${ANYWHERE_OUTPUT:-$ROOT/build/Anywhere.app}")"
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
BUILD="${ANYWHERE_BUILD:-1}"
SIGN_IDENTITY="${ANYWHERE_CODESIGN_IDENTITY:-Anywhere Local Code Signing}"

python3 scripts/generate-design-tokens.py --check
python3 scripts/generate-gpui-localizations.py --check
python3 -m unittest discover -s scripts/tests -p test_gpui_localizations.py
plutil -lint apps/gpui/macos/Info.plist apps/gpui/macos/Release.entitlements

cargo build --locked --release -p anywhere-gpui

rm -rf "$APP"
mkdir -p \
  "$APP/Contents/MacOS" \
  "$APP/Contents/Resources/plugins" \
  "$APP/Contents/Resources/Licenses"
cp target/release/anywhere-gpui "$APP/Contents/MacOS/Anywhere"
cp apps/gpui/macos/Info.plist "$APP/Contents/Info.plist"
cp resources/icon/Anywhere.icns "$APP/Contents/Resources/Anywhere.icns"
ditto resources/native-localizations "$APP/Contents/Resources"
cp LICENSE "$APP/Contents/Resources/Licenses/Anywhere.txt"

/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" \
  "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD" \
  "$APP/Contents/Info.plist"

if [[ "$SIGN_IDENTITY" == "-" ]] ||
  ! security find-identity -p codesigning | grep -Fq "$SIGN_IDENTITY"; then
  echo "Code signing identity '$SIGN_IDENTITY' was not found." >&2
  echo 'Run scripts/setup-local-signing.sh or set ANYWHERE_CODESIGN_IDENTITY.' >&2
  exit 1
fi

codesign --force --sign "$SIGN_IDENTITY" \
  --entitlements apps/gpui/macos/Release.entitlements "$APP"
codesign --verify --deep --strict "$APP"
plutil -lint "$APP/Contents/Info.plist"

printf 'Built %s (%s+%s)\n' "$APP" "$VERSION" "$BUILD"
