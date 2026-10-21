#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/build-web-examples.py
for package in build/web-examples/dev.anywhere.json-workspace.awplugin build/web-examples/dev.example.text-pad.awplugin; do
  cargo run -p anywhere-plugin --example inspect_web_package --locked -- "$package"
done
scripts/check-plugin-starter.sh
