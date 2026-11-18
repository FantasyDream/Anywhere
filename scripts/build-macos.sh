#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
APP="$(python3 scripts/macos_app_path.py "${ANYWHERE_APP_OUTPUT:-$ROOT/build/Anywhere.app}")"
ANYWHERE_OUTPUT="$APP" \
  ANYWHERE_BUILD="${ANYWHERE_BUILD:-$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' apps/gpui/macos/Info.plist)}" \
  scripts/build-gpui.sh
# Re-register with LaunchServices. macOS caches an app's icon and metadata by
# bundle path and identifier, so a freshly built or renamed bundle otherwise
# shows stale metadata until the cache expires.
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ "$APP" == "$ROOT/build/Anywhere.app" && -x "$LSREGISTER" ]]; then
  "$LSREGISTER" -f "$APP"
fi
printf 'Built %s\n' "$APP"
