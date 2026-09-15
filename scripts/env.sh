#!/bin/sh
# Source this file from the repository root when using project-local SDKs.
if [ -d "$PWD/.tools/cargo" ]; then
  export CARGO_HOME="$PWD/.tools/cargo"
  export RUSTUP_HOME="$PWD/.tools/rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
if [ -d "$PWD/.tools/flutter" ]; then
  export PATH="$PWD/.tools/flutter/bin:$PATH"
fi
export PUB_CACHE="${PUB_CACHE:-$PWD/.tools/pub-cache}"
# clang/swift module caches embed absolute paths, so they are keyed by the checkout
# path: moving or renaming the repository just builds a fresh cache instead of failing
# with "missing required module 'SwiftShims'".
_checkout="$(basename "$PWD")"
export CLANG_MODULE_CACHE_PATH="$PWD/.tools/$_checkout-clang-cache"
export SWIFT_MODULE_CACHE_PATH="$PWD/.tools/$_checkout-swift-cache"

