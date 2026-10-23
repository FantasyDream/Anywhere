#!/bin/bash
# Package an explicitly selected, already-built app without rebuilding or replacing it.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
if [[ "$(uname -s)" != Darwin || $# -ne 1 ]]; then
  echo 'Usage: scripts/package-macos.sh build/Anywhere-dev-0.5.app (macOS only)' >&2
  exit 2
fi
APP="$(
  python3 - "$1" <<'PY'
import pathlib, plistlib, re, sys

root = pathlib.Path.cwd()
app = pathlib.Path(sys.argv[1]).resolve()
try:
    app.relative_to(root / 'build')
except ValueError:
    sys.exit('Select an app inside this checkout\u2019s build directory.')
if app.suffix != '.app' or not app.is_dir():
    sys.exit('The selected app does not exist.')
source = plistlib.loads((root / 'apps/flutter/macos/Runner/Info.plist').read_bytes())
bundle = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
version = re.search(r'^version:\s*(\S+)\s*$',
                    (root / 'apps/flutter/pubspec.yaml').read_text(), re.MULTILINE)
expected = f"{source['CFBundleShortVersionString']}+{source['CFBundleVersion']}"
if version is None or version.group(1) != expected:
    sys.exit('Flutter and native source versions differ; align them before packaging.')
for key in ['CFBundleIdentifier', 'CFBundleShortVersionString', 'CFBundleVersion', 'FLTEnableImpeller']:
    if bundle.get(key) != source.get(key):
        sys.exit(f'The selected app has a stale {key}; rebuild it before packaging.')
print(app)
PY
)"
codesign --verify --deep --strict "$APP"
if codesign -dr - "$APP" 2>&1 | grep -q 'designated => cdhash'; then
  echo 'Use the persistent signing identity before packaging.' >&2
  exit 1
fi
ARCH="$(lipo -archs "$APP/Contents/MacOS/Anywhere")"
if [[ "$ARCH" == *' '* ]]; then ARCH=universal; fi
NAME="$(basename "$APP" .app)-macos-$ARCH.zip"
PACKAGING_DIR="$(mktemp -d "$ROOT/build/.package-macos.XXXXXX")"
trap 'rm -rf "$PACKAGING_DIR"' EXIT
ditto -c -k --sequesterRsrc --keepParent "$APP" "$PACKAGING_DIR/$NAME"
python3 - "$APP" "$PACKAGING_DIR/$NAME" <<'PY'
import hashlib, pathlib, plistlib, sys, zipfile

app, archive = map(pathlib.Path, sys.argv[1:])
with zipfile.ZipFile(archive) as bundle:
    for file in ['Contents/Info.plist', 'Contents/MacOS/Anywhere',
                 'Contents/Frameworks/libanywhere_bridge_flutter.dylib']:
        original = hashlib.sha256((app / file).read_bytes()).digest()
        packaged = hashlib.sha256(bundle.read(f'{app.name}/{file}')).digest()
        if original != packaged:
            sys.exit(f'Archive validation failed: {file}')
info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
print(f"Verified {info['CFBundleShortVersionString']}+{info['CFBundleVersion']}")
PY
mv -f "$PACKAGING_DIR/$NAME" "$ROOT/build/$NAME"
(
  cd "$ROOT/build"
  shasum -a 256 "$NAME" >"$NAME.sha256"
)
printf 'App: %s\nZIP: %s\nChecksum: %s\n' "$APP" "$ROOT/build/$NAME" "$ROOT/build/$NAME.sha256"
