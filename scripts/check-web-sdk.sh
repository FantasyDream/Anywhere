#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
node -e 'if (Number(process.versions.node.split(".")[0]) < 22) throw new Error("Web SDK checks require Node.js 22 or newer")'
npm --prefix packages/plugin-sdk ci --registry=https://registry.npmjs.org --ignore-scripts --no-audit --no-fund
npm --prefix packages/plugin-sdk run check
npm --prefix packages/plugin-sdk run build
cargo build -p anywhere-core --example web_sdk_conformance --locked
sdk_test_target="$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
sdk_test_suffix="$(node -p 'process.platform === "win32" ? ".exe" : ""')"
ANYWHERE_WEB_BRIDGE_TEST_BIN="$sdk_test_target/debug/examples/web_sdk_conformance$sdk_test_suffix" npm --prefix packages/plugin-sdk test
python3 -m unittest discover -s packages/plugin-sdk/test -p '*_test.py'
