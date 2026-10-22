#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
mkdir -p build/plugin-sdk
(
  cd packages/plugin-sdk
  npm pack --pack-destination "$ROOT/build/plugin-sdk"
)
SDK_ARCHIVE="$ROOT/build/plugin-sdk/anywhere-plugin-sdk-0.1.0.tgz"
# Build outside the repository: source-relative SDK imports cannot accidentally work.
STARTER_WORK="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-plugin-starter.XXXXXX")"
trap 'rm -rf "$STARTER_WORK"' EXIT
mkdir "$STARTER_WORK/sdk"
tar -xzf "$SDK_ARCHIVE" -C "$STARTER_WORK/sdk" --strip-components=1
cmp "$ROOT/LICENSE" "$STARTER_WORK/sdk/LICENSE"
node "$STARTER_WORK/sdk/tools/create.mjs" init "$STARTER_WORK/my-tool"
cmp "$ROOT/LICENSE" "$STARTER_WORK/my-tool/vendor/plugin-sdk/LICENSE"
# Refuse to overwrite an existing project, including its user changes.
if node "$STARTER_WORK/sdk/tools/create.mjs" init "$STARTER_WORK/my-tool"; then
  echo 'Starter unexpectedly overwrote an existing directory.' >&2
  exit 1
fi
(
  cd "$STARTER_WORK/my-tool"
  npm install --registry=https://registry.npmjs.org --ignore-scripts --no-audit --no-fund
  cmp "$ROOT/LICENSE" node_modules/@anywhere/plugin-sdk/LICENSE
  npm run build
  npm run pack
)
cargo run -p anywhere-plugin --example inspect_web_package --locked -- "$STARTER_WORK/my-tool/output/my-tool.awplugin"
cp "$STARTER_WORK/my-tool/output/my-tool.awplugin" "$ROOT/build/plugin-sdk/text-transform.awplugin"
printf 'Standalone starter built and validated outside the checkout.\n'
