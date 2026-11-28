#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
SDK_ARCHIVE="${ANYWHERE_SDK_ARCHIVE:-}"
if [[ -z "$SDK_ARCHIVE" ]]; then SDK_ARCHIVE="$(scripts/package-plugin-sdk.sh)"; fi
if command -v cygpath >/dev/null 2>&1; then
  SDK_ARCHIVE="$(cygpath -u "$SDK_ARCHIVE")"
fi
# Build outside the repository so source-relative imports cannot succeed accidentally.
STARTER_WORK="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-plugin-starter.XXXXXX")"
trap 'rm -rf "$STARTER_WORK"' EXIT
mkdir "$STARTER_WORK/sdk"
tar -xzf "$SDK_ARCHIVE" -C "$STARTER_WORK/sdk" --strip-components=1
python3 scripts/check-docs.py --package "$STARTER_WORK/sdk"
cmp "$ROOT/LICENSE" "$STARTER_WORK/sdk/LICENSE"
CLI="$STARTER_WORK/sdk/tools/anywhere-plugin.mjs"

for variant in web ts background hybrid; do
  project="$STARTER_WORK/my-$variant"
  if [[ "$variant" == web ]]; then
    node "$CLI" init "$project"
  else
    node "$CLI" init "$project" "--$variant"
  fi
  cmp "$ROOT/LICENSE" "$project/vendor/plugin-sdk/LICENSE"
  cmp "$STARTER_WORK/sdk/tools/skill/SKILL.md" "$project/.agents/skills/anywhere-plugin-dev/SKILL.md"
  if [[ -e "$project/vendor/plugin-sdk/templates" ]]; then
    echo 'Vendored SDK unexpectedly contains starter templates.' >&2
    exit 1
  fi
  (
    cd "$project"
    npm install --registry=https://registry.npmjs.org --ignore-scripts --no-audit --no-fund
    node "$CLI" validate
    if node "$CLI" validate --built; then
      echo 'Built validation unexpectedly accepted a missing dist.' >&2
      exit 1
    fi
    if [[ "$variant" != web ]]; then npm run check; fi
    if [[ "$variant" == background ]]; then npm run dev; else npm run build; fi
    node "$CLI" test
    node "$CLI" validate --built
    npm run pack
    node "$CLI" pack --output output/repacked.awplugin
    cmp "output/my-$variant.awplugin" output/repacked.awplugin
  )
  package="$project/output/my-$variant.awplugin"
  cargo run -p anywhere-plugin --example inspect_web_package --locked -- "$package"
  if [[ "$(uname -s)" == Darwin ]]; then
    mkdir "$STARTER_WORK/host-$variant"
    ANYWHERE_DATA_DIR="$STARTER_WORK/host-$variant" cargo run -p anywhere-core --example web_sdk_conformance --locked -- --import "$package"
  fi
  if [[ "$variant" != background ]]; then
    node "$ROOT/packages/plugin-sdk/test/dev-server.mjs" "$project"
  fi
done

if node "$CLI" init "$STARTER_WORK/my-web"; then
  echo 'Starter unexpectedly overwrote an existing directory.' >&2
  exit 1
fi
cp "$STARTER_WORK/my-web/output/my-web.awplugin" "$ROOT/build/plugin-sdk/text-transform.awplugin"
cargo run -p anywhere-core --example web_sdk_conformance --locked -- --background "$STARTER_WORK/my-background/output/my-background.awplugin"
