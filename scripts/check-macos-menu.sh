#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == "Darwin" ]] || exit 0
MENU_TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-menu-test.XXXXXX")"
trap 'rm -rf "$MENU_TEST_DIR"' EXIT
xcrun clang -fobjc-arc -Wall -Wextra -Werror \
  crates/anywhere-platform/native/tests/macos_menu_test.m \
  -framework AppKit -framework ApplicationServices -o "$MENU_TEST_DIR/menu-test"
"$MENU_TEST_DIR/menu-test"
xcrun clang -fobjc-arc -Wall -Wextra -Werror -Wno-deprecated-declarations \
  crates/anywhere-platform/native/tests/macos_clipboard_test.m \
  -framework AppKit -o "$MENU_TEST_DIR/clipboard-test"
"$MENU_TEST_DIR/clipboard-test"
