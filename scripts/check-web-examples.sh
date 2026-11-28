#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${ANYWHERE_SDK_READY:-0}" == 1 ]]; then
  python3 scripts/build-web-examples.py --sdk-ready
else
  python3 scripts/build-web-examples.py
fi
for package in build/web-examples/dev.anywhere.json-workspace.awplugin build/web-examples/dev.example.text-pad.awplugin build/web-examples/dev.example.json-summary.awplugin \
  build/web-examples/dev.example.issue-link.awplugin build/web-examples/dev.example.selected-csv.awplugin; do
  cargo run -p anywhere-plugin --example inspect_web_package --locked -- "$package"
done
if ! cmp -s build/web-examples/dev.anywhere.json-workspace.awplugin crates/anywhere-core/resources/builtin-web-plugins/dev.anywhere.json-workspace.awplugin; then
  echo "built-in dev.anywhere.json-workspace.awplugin is out of date; run python3 scripts/build-web-examples.py --sync-builtin" >&2
  exit 1
fi
cargo run -p anywhere-core --example web_sdk_conformance --locked -- --background build/web-examples/dev.example.json-summary.awplugin
node packages/plugin-sdk/test/quickjs_text.mjs
cargo run -p anywhere-core --example web_sdk_conformance --locked -- --json build/web-examples/dev.anywhere.json-workspace.awplugin
scripts/check-plugin-starter.sh
