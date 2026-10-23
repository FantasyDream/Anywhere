#!/bin/bash
# Install isolated development tools; no changes to system packages or Git hooks.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 -m venv .tools/code-style/python
.tools/code-style/python/bin/python -m pip install --disable-pip-version-check \
  --index-url https://pypi.org/simple -r tools/code-style/requirements.txt
npm --prefix tools/code-style ci --registry=https://registry.npmjs.org \
  --ignore-scripts --no-audit --no-fund

if [[ "$(uname -s)" == Darwin ]]; then
  # Same official formatter on local machines and CI, independent of Xcode's copy.
  swift_format_commit=65f9da9aad84adb7e2028eb32ca95164aa590e3b # swift-format 600.0.0
  swift_format_source="$PWD/.tools/code-style/swift-format"
  if [[ ! -d "$swift_format_source/.git" ]]; then
    git clone --depth 1 --branch 600.0.0 \
      https://github.com/swiftlang/swift-format.git "$swift_format_source"
  fi
  if [[ "$(git -C "$swift_format_source" rev-parse HEAD)" != "$swift_format_commit" ]]; then
    echo 'Unexpected swift-format source revision; remove its local tool cache and retry.' >&2
    exit 1
  fi
  cp tools/code-style/swift-package.resolved "$swift_format_source/Package.resolved"
  swift build --package-path "$swift_format_source" --configuration release --force-resolved-versions \
    --product swift-format --jobs 2
fi
