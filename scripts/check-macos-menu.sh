#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh
[[ "$(uname -s)" == "Darwin" ]] || exit 0
MENU_TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/anywhere-menu-test.XXXXXX")"
trap 'rm -rf "$MENU_TEST_DIR"' EXIT
CLANG="$(xcrun -f clang)"
SDK_PATH="$(xcrun --show-sdk-path)"
COMMON_FLAGS=(-isysroot "$SDK_PATH" -fobjc-arc -Wall -Wextra -Werror)
pids=()

"$CLANG" "${COMMON_FLAGS[@]}" \
  crates/anywhere-platform/native/tests/macos_menu_test.m \
  -framework AppKit -framework ApplicationServices -o "$MENU_TEST_DIR/menu-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" -Wno-deprecated-declarations \
  crates/anywhere-platform/native/tests/macos_clipboard_test.m \
  -framework AppKit -o "$MENU_TEST_DIR/clipboard-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" -Wno-deprecated-declarations \
  -DANYWHERE_TEST_LEGACY_SDK \
  crates/anywhere-platform/native/tests/macos_clipboard_test.m \
  -framework AppKit -o "$MENU_TEST_DIR/clipboard-legacy-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" -Wno-deprecated-declarations \
  crates/anywhere-platform/native/tests/macos_identity_test.m \
  -framework AppKit -o "$MENU_TEST_DIR/identity-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" -Wno-deprecated-declarations \
  crates/anywhere-platform/native/tests/macos_automation_test.m \
  -framework AppKit -framework Carbon -o "$MENU_TEST_DIR/automation-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" \
  crates/anywhere-platform/native/tests/macos_resources_test.m \
  -framework AppKit -o "$MENU_TEST_DIR/resources-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" \
  crates/anywhere-platform/native/tests/macos_spotlight_test.m \
  -framework Foundation -framework CoreServices -o "$MENU_TEST_DIR/spotlight-test" &
pids+=("$!")
"$CLANG" "${COMMON_FLAGS[@]}" -Wno-deprecated-declarations \
  crates/anywhere-platform/native/tests/macos_application_launch_test.m \
  -framework AppKit -o "$MENU_TEST_DIR/application-launch-test" &
pids+=("$!")
for variant in modern legacy; do
  APPLICATION_NAME_FLAGS=("${COMMON_FLAGS[@]}" -Wno-deprecated-declarations)
  if [[ "$variant" == legacy ]]; then APPLICATION_NAME_FLAGS+=(-DANYWHERE_TEST_LEGACY_SDK); fi
  "$CLANG" "${APPLICATION_NAME_FLAGS[@]}" \
    crates/anywhere-platform/native/tests/macos_application_names_test.m \
    -framework AppKit -o "$MENU_TEST_DIR/application-names-$variant-test" &
  pids+=("$!")
done
for pid in "${pids[@]}"; do
  wait "$pid"
done

for binary in \
  menu-test \
  clipboard-test \
  clipboard-legacy-test \
  identity-test \
  automation-test \
  resources-test \
  spotlight-test \
  application-launch-test \
  application-names-modern-test \
  application-names-legacy-test; do
  "$MENU_TEST_DIR/$binary"
done
